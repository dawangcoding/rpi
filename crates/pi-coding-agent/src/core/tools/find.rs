//! 文件查找工具
//!
//! 使用 glob 模式查找文件

use std::path::PathBuf;
use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use ignore::WalkBuilder;
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;
use super::truncate::*;

/// Find 文件查找工具
pub struct FindTool {
    cwd: PathBuf,
}

impl FindTool {
    /// 创建新的 FindTool
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    /// 解析路径（相对路径相对于 cwd）
    fn resolve_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let path_buf = PathBuf::from(path);
        let absolute_path = if path_buf.is_absolute() {
            path_buf
        } else {
            self.cwd.join(path_buf)
        };
        
        // 规范化路径
        let canonical = absolute_path.canonicalize().unwrap_or(absolute_path);
        
        // 确保路径在 cwd 下
        let canonical_cwd = self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone());
        if !canonical.starts_with(&canonical_cwd) {
            return Err(anyhow::anyhow!(
                "Path '{}' is outside the working directory",
                path
            ));
        }
        
        Ok(canonical)
    }

    /// 构建 glob matcher
    fn build_glob_matcher(&self, pattern: &str) -> anyhow::Result<GlobMatcher> {
        let glob = Glob::new(pattern).map_err(|e| anyhow::anyhow!("Invalid glob pattern: {}", e))?;
        Ok(glob.compile_matcher())
    }

    /// 获取相对路径
    fn get_relative_path(&self, path: &PathBuf, base: &PathBuf) -> String {
        path.strip_prefix(base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.file_name().unwrap_or_default().to_string_lossy().to_string())
    }
}

#[async_trait]
impl AgentTool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn label(&self) -> &str {
        "Find Files"
    }

    fn description(&self) -> &str {
        "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to 1000 results or 1MB (whichever is hit first)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files, e.g. '*.rs', '**/*.toml', or 'src/**/*.rs'"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: current directory)"
                },
                "type": {
                    "type": "string",
                    "enum": ["file", "dir"],
                    "description": "Filter by type: 'file' or 'dir' (default: both)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 1000)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
        _on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
    ) -> anyhow::Result<AgentToolResult> {
        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        let pattern = params["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'pattern' parameter"))?;

        let search_path = params["path"].as_str().unwrap_or(".");
        let file_type = params["type"].as_str();
        let limit = params["limit"].as_u64().map(|l| l as usize).unwrap_or(1000);

        let absolute_path = self.resolve_path(search_path)?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        // 检查路径是否存在
        if !absolute_path.exists() {
            return Err(anyhow::anyhow!("Path not found: {}", search_path));
        }

        // 构建 glob matcher
        let matcher = self.build_glob_matcher(pattern)?;

        // 收集结果
        let mut results: Vec<String> = Vec::new();
        let mut limit_reached = false;

        // 使用 ignore 遍历
        let walker = WalkBuilder::new(&absolute_path)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker {
            // 检查取消
            if cancel.is_cancelled() {
                return Err(anyhow::anyhow!("Operation aborted"));
            }

            if results.len() >= limit {
                limit_reached = true;
                break;
            }

            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    
                    // 类型过滤
                    if let Some(ft) = file_type {
                        let is_dir = path.is_dir();
                        match ft {
                            "file" if is_dir => continue,
                            "dir" if !is_dir => continue,
                            _ => {}
                        }
                    }

                    // 获取文件名进行匹配
                    let file_name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    if matcher.is_match(&file_name) {
                        // 获取相对路径
                        let rel_path = self.get_relative_path(&path.to_path_buf(), &absolute_path);
                        
                        // 目录添加斜杠后缀
                        let display_path = if path.is_dir() && !rel_path.ends_with('/') {
                            format!("{}/", rel_path)
                        } else {
                            rel_path
                        };

                        results.push(display_path);
                    }
                }
                Err(_) => continue,
            }
        }

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        if results.is_empty() {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::Text(TextContent::new("No files found matching pattern".to_string()))],
                details: serde_json::json!({
                    "pattern": pattern,
                    "path": search_path,
                    "result_count": 0,
                }),
            });
        }

        // 排序结果
        results.sort();

        // 格式化输出
        let output = results.join("\n");

        // 应用截断（只限制字节数，不限制行数）
        let (truncated_output, truncation_result) = truncate_output_head(&output, usize::MAX, DEFAULT_MAX_BYTES);

        // 构建通知
        let mut notices = Vec::new();
        if limit_reached {
            notices.push(format!("{} results limit reached. Use limit={} for more, or refine pattern", limit, limit * 2));
        }
        if truncation_result.was_truncated {
            notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        }

        let final_output = if notices.is_empty() {
            truncated_output
        } else {
            format!("{}\n\n[{}]", truncated_output, notices.join(". "))
        };

        Ok(AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(final_output))],
            details: serde_json::json!({
                "pattern": pattern,
                "path": search_path,
                "result_count": results.len(),
                "limit_reached": limit_reached,
                "truncation": if truncation_result.was_truncated {
                    serde_json::to_value(&truncation_result)?
                } else {
                    serde_json::Value::Null
                },
            }),
        })
    }
}
