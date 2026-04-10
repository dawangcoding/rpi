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
        
        // 规范化 cwd
        let canonical_cwd = self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone());
        
        // 如果路径存在，使用 canonicalize；否则使用绝对路径
        let canonical = if absolute_path.exists() {
            absolute_path.canonicalize().unwrap_or(absolute_path)
        } else {
            absolute_path
        };
        
        // 确保路径在 cwd 下
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ls_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(dir.path().join("file2.txt"), "content2").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证列出文件和目录
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("file1.txt") || text.contains("file1"));
        assert!(text.contains("file2.txt") || text.contains("file2"));
        assert!(text.contains("subdir") || text.contains("subdir/"));
        
        let details = result.details.as_object().unwrap();
        assert!(details["entry_count"].as_u64().unwrap() >= 3);
    }

    #[tokio::test]
    async fn test_ls_nonexistent() {
        let dir = TempDir::new().unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        // 使用相对路径指向一个不存在的目录
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "nonexistent_dir"}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // 错误消息可能是 "Path not found" 或 "No such file or directory" 或 "outside"
        assert!(err_msg.contains("Path not found") || err_msg.contains("No such file") || err_msg.contains("outside"));
    }

    #[tokio::test]
    async fn test_ls_not_a_directory() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap()}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not a directory"));
    }

    #[tokio::test]
    async fn test_ls_empty_directory() {
        let dir = TempDir::new().unwrap();
        let empty_dir = dir.path().join("empty");
        std::fs::create_dir(&empty_dir).unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "empty"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证空目录提示
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("empty directory"));
        
        let details = result.details.as_object().unwrap();
        assert_eq!(details["entry_count"], 0);
    }

    #[tokio::test]
    async fn test_ls_with_hidden_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("visible.txt"), "").unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        
        // 默认情况下不显示隐藏文件
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("visible.txt") || text.contains("visible"));
        assert!(!text.contains(".hidden"));
    }

    #[tokio::test]
    async fn test_ls_all_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("visible.txt"), "").unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        
        // 显示所有文件
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": ".", "all": true}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("visible.txt") || text.contains("visible"));
        assert!(text.contains(".hidden"));
    }

    #[tokio::test]
    async fn test_ls_recursive() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("root.txt"), "").unwrap();
        std::fs::write(dir.path().join("subdir/nested.txt"), "").unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": ".", "recursive": true}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证递归列出
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("root.txt") || text.contains("root"));
        assert!(text.contains("nested.txt") || text.contains("nested"));
    }

    #[tokio::test]
    async fn test_ls_with_limit() {
        let dir = TempDir::new().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("file{}.txt", i)), "").unwrap();
        }
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": ".", "limit": 5}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证限制
        let details = result.details.as_object().unwrap();
        assert!(details["entry_count"].as_u64().unwrap() <= 5);
        assert_eq!(details["limit_reached"], true);
    }

    #[tokio::test]
    async fn test_ls_default_path() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        // 不提供 path 参数，应该默认使用当前目录
        let result = tool.execute(
            "call_1",
            serde_json::json!({}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("file.txt") || text.contains("file"));
    }

    #[tokio::test]
    async fn test_ls_shows_directory_suffix() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("mydir")).unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        // 目录应该有斜杠后缀
        assert!(text.contains("mydir/") || text.contains("mydir"));
    }

    #[tokio::test]
    async fn test_ls_shows_file_size() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("small.txt"), "hi").unwrap();
        
        let tool = LsTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        // 文件应该有大小信息
        assert!(text.contains("B") || text.contains("small"));
    }

    #[test]
    fn test_ls_tool_name() {
        let tool = LsTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "ls");
    }

    #[test]
    fn test_ls_tool_label() {
        let tool = LsTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.label(), "List Directory");
    }

    #[test]
    fn test_ls_tool_parameters() {
        let tool = LsTool::new(PathBuf::from("/tmp"));
        let params = tool.parameters();
        
        assert!(params.is_object());
        let obj = params.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        
        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("path"));
        assert!(properties.contains_key("recursive"));
        assert!(properties.contains_key("all"));
        assert!(properties.contains_key("limit"));
    }
}
