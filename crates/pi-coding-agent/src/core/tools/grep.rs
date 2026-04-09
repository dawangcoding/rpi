//! 全文搜索工具
//!
//! 使用正则表达式搜索文件内容

use std::path::PathBuf;
use async_trait::async_trait;
use regex::Regex;
use ignore::WalkBuilder;
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;
use super::truncate::*;

/// Grep 搜索工具
pub struct GrepTool {
    cwd: PathBuf,
}

/// 单个匹配结果
#[derive(Debug, Clone)]
struct Match {
    file_path: String,
    line_number: usize,
    content: String,
}

impl GrepTool {
    /// 创建新的 GrepTool
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

    /// 构建正则表达式
    fn build_regex(&self, pattern: &str, case_insensitive: bool) -> anyhow::Result<Regex> {
        let mut regex_builder = regex::RegexBuilder::new(pattern);
        regex_builder.case_insensitive(case_insensitive);
        regex_builder.build().map_err(|e| anyhow::anyhow!("Invalid regex pattern: {}", e))
    }

    /// 搜索单个文件
    async fn search_file(&self, path: &PathBuf, regex: &Regex, context_lines: usize) -> anyhow::Result<Vec<Match>> {
        let content = tokio::fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                // 添加上下文行
                let start = if context_lines > 0 {
                    i.saturating_sub(context_lines)
                } else {
                    i
                };
                let end = if context_lines > 0 {
                    (i + context_lines + 1).min(lines.len())
                } else {
                    i + 1
                };

                for j in start..end {
                    let is_match_line = j == i;
                    let prefix = if is_match_line { "> " } else { "  " };
                    matches.push(Match {
                        file_path: path.to_string_lossy().to_string(),
                        line_number: j + 1, // 1-indexed
                        content: format!("{}{}", prefix, lines[j]),
                    });
                }
            }
        }

        Ok(matches)
    }
}

#[async_trait]
impl AgentTool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn label(&self) -> &str {
        "Search Files"
    }

    fn description(&self) -> &str {
        "Search file contents for a pattern. Returns matching lines with file paths and line numbers. Respects .gitignore. Output is truncated to 100 matches or 1MB (whichever is hit first)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (regex)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search (default: current directory)"
                },
                "caseInsensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search (default: false)"
                },
                "context": {
                    "type": "integer",
                    "description": "Number of lines to show before and after each match (default: 0)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 100)"
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
        let case_insensitive = params["caseInsensitive"].as_bool().unwrap_or(false);
        let context_lines = params["context"].as_u64().map(|c| c as usize).unwrap_or(0);
        let limit = params["limit"].as_u64().map(|l| l as usize).unwrap_or(100);

        let absolute_path = self.resolve_path(search_path)?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        // 构建正则表达式
        let regex = self.build_regex(pattern, case_insensitive)?;

        // 检查路径类型
        let metadata = tokio::fs::metadata(&absolute_path).await.map_err(|e| {
            anyhow::anyhow!("Cannot access path '{}': {}", search_path, e)
        })?;

        let mut all_matches: Vec<Match> = Vec::new();

        if metadata.is_file() {
            // 搜索单个文件
            let matches = self.search_file(&absolute_path, &regex, context_lines).await?;
            all_matches.extend(matches);
        } else {
            // 遍历目录
            let walker = WalkBuilder::new(&absolute_path)
                .hidden(false)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .build();

            for result in walker {
                // 检查取消
                if cancel.is_cancelled() {
                    return Err(anyhow::anyhow!("Operation aborted"));
                }

                if all_matches.len() >= limit {
                    break;
                }

                match result {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_file() {
                            // 尝试读取并搜索文件
                            if let Ok(matches) = self.search_file(&path.to_path_buf(), &regex, context_lines).await {
                                all_matches.extend(matches);
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        if all_matches.is_empty() {
            return Ok(AgentToolResult {
                content: vec![ContentBlock::Text(TextContent::new("No matches found".to_string()))],
                details: serde_json::json!({
                    "pattern": pattern,
                    "path": search_path,
                    "match_count": 0,
                }),
            });
        }

        // 限制匹配数量
        let match_limit_reached = all_matches.len() > limit;
        let matches_to_show: Vec<_> = all_matches.into_iter().take(limit).collect();

        // 格式化输出
        let mut output_lines = Vec::new();
        let mut last_file: Option<String> = None;

        for m in &matches_to_show {
            // 文件分隔
            if last_file.as_ref() != Some(&m.file_path) {
                if !output_lines.is_empty() {
                    output_lines.push(String::new());
                }
                // 使用相对路径
                let rel_path = if m.file_path.starts_with(self.cwd.to_string_lossy().as_ref()) {
                    m.file_path[self.cwd.to_string_lossy().len() + 1..].to_string()
                } else {
                    m.file_path.clone()
                };
                output_lines.push(format!("{}:", rel_path));
                last_file = Some(m.file_path.clone());
            }

            // 截断长行
            let (truncated_line, _) = truncate_line(&m.content, 500);
            output_lines.push(format!("{}:{}", m.line_number, truncated_line));
        }

        let output = output_lines.join("\n");
        
        // 应用截断
        let (truncated_output, truncation_result) = truncate_output_head(&output, DEFAULT_MAX_LINES, DEFAULT_MAX_BYTES);

        // 构建通知
        let mut notices = Vec::new();
        if match_limit_reached {
            notices.push(format!("{} matches limit reached", limit));
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
                "match_count": matches_to_show.len(),
                "match_limit_reached": match_limit_reached,
                "truncation": if truncation_result.was_truncated {
                    serde_json::to_value(&truncation_result)?
                } else {
                    serde_json::Value::Null
                },
            }),
        })
    }
}
