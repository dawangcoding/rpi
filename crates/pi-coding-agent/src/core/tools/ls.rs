//! 目录列表工具
//!
//! 列出目录内容

use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs;
use ignore::WalkBuilder;
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;
use super::truncate::*;

/// Ls 目录列表工具
pub struct LsTool {
    cwd: PathBuf,
}

/// 目录项
#[derive(Debug, Clone)]
struct DirEntry {
    name: String,
    is_dir: bool,
    size: Option<u64>,
}

impl LsTool {
    /// 创建新的 LsTool
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

    /// 格式化文件大小
    fn format_size(&self, size: u64) -> String {
        if size < 1024 {
            format!("{}B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1}KB", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1}GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

#[async_trait]
impl AgentTool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn label(&self) -> &str {
        "List Directory"
    }

    fn description(&self) -> &str {
        "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Output is truncated to 500 entries or 1MB (whichever is hit first)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to list (default: current directory)"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "List recursively (default: false)"
                },
                "all": {
                    "type": "boolean",
                    "description": "Include hidden files (default: false)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of entries to return (default: 500)"
                }
            }
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

        let path = params["path"].as_str().unwrap_or(".");
        let recursive = params["recursive"].as_bool().unwrap_or(false);
        let all = params["all"].as_bool().unwrap_or(false);
        let limit = params["limit"].as_u64().map(|l| l as usize).unwrap_or(500);

        let absolute_path = self.resolve_path(path)?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        // 检查路径是否存在
        if !absolute_path.exists() {
            return Err(anyhow::anyhow!("Path not found: {}", path));
        }

        // 检查是否为目录
        let metadata = fs::metadata(&absolute_path).await.map_err(|e| {
            anyhow::anyhow!("Cannot access path '{}': {}", path, e)
        })?;

        if !metadata.is_dir() {
            return Err(anyhow::anyhow!("Not a directory: {}", path));
        }

        let mut entries: Vec<DirEntry> = Vec::new();
        let mut limit_reached = false;

        if recursive {
            // 递归遍历
            let walker = WalkBuilder::new(&absolute_path)
                .hidden(!all)
                .git_ignore(false) // ls 通常不遵循 gitignore
                .max_depth(None)
                .build();

            for result in walker {
                // 检查取消
                if cancel.is_cancelled() {
                    return Err(anyhow::anyhow!("Operation aborted"));
                }

                if entries.len() >= limit {
                    limit_reached = true;
                    break;
                }

                match result {
                    Ok(entry) => {
                        let entry_path = entry.path();
                        
                        // 跳过根目录本身
                        if entry_path == absolute_path {
                            continue;
                        }

                        // 获取相对路径
                        let rel_path = entry_path.strip_prefix(&absolute_path)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| entry_path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default());

                        // 获取文件信息
                        let (is_dir, size) = if let Ok(meta) = entry.metadata() {
                            (meta.is_dir(), if meta.is_file() { Some(meta.len()) } else { None })
                        } else {
                            (entry_path.is_dir(), None)
                        };

                        entries.push(DirEntry {
                            name: rel_path,
                            is_dir,
                            size,
                        });
                    }
                    Err(_) => continue,
                }
            }
        } else {
            // 非递归，只读取直接子项
            let mut dir_entries = fs::read_dir(&absolute_path).await.map_err(|e| {
                anyhow::anyhow!("Cannot read directory '{}': {}", path, e)
            })?;

            while let Some(entry) = dir_entries.next_entry().await? {
                // 检查取消
                if cancel.is_cancelled() {
                    return Err(anyhow::anyhow!("Operation aborted"));
                }

                if entries.len() >= limit {
                    limit_reached = true;
                    break;
                }

                let file_name = entry.file_name().to_string_lossy().to_string();

                // 隐藏文件过滤
                if !all && file_name.starts_with('.') {
                    continue;
                }

                let metadata = entry.metadata().await.ok();
                let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = metadata.as_ref().filter(|m| m.is_file()).map(|m| m.len());

                entries.push(DirEntry {
                    name: file_name,
                    is_dir,
                    size,
                });
            }
        }

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        if entries.is_empty() {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::Text(TextContent::new("(empty directory)".to_string()))],
                details: serde_json::json!({
                    "path": path,
                    "entry_count": 0,
                }),
            });
        }

        // 按字母顺序排序（不区分大小写）
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // 格式化输出
        let mut output_lines = Vec::new();
        for entry in &entries {
            let suffix = if entry.is_dir { "/" } else { "" };
            let size_str = if let Some(size) = entry.size {
                format!(" ({})", self.format_size(size))
            } else {
                String::new()
            };
            output_lines.push(format!("{}{}{}", entry.name, suffix, size_str));
        }

        let output = output_lines.join("\n");

        // 应用截断（只限制字节数）
        let (truncated_output, truncation_result) = truncate_output_head(&output, usize::MAX, DEFAULT_MAX_BYTES);

        // 构建通知
        let mut notices = Vec::new();
        if limit_reached {
            notices.push(format!("{} entries limit reached. Use limit={} for more", limit, limit * 2));
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
                "path": path,
                "entry_count": entries.len(),
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
