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

    /// 构建 glob matcher
    fn build_glob_matcher(&self, pattern: &str) -> anyhow::Result<GlobMatcher> {
        let glob = Glob::new(pattern).map_err(|e| anyhow::anyhow!("Invalid glob pattern: {}", e))?;
        Ok(glob.compile_matcher())
    }

    /// 获取相对路径
    fn get_relative_path(&self, path: &std::path::Path, base: &std::path::Path) -> String {
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
                        let rel_path = self.get_relative_path(path, &absolute_path);
                        
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_find_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(dir.path().join("file2.txt"), "content2").unwrap();
        std::fs::write(dir.path().join("script.rs"), "code").unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*.txt", "path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证找到 txt 文件
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("file1.txt") || text.contains("file1"));
        assert!(text.contains("file2.txt") || text.contains("file2"));
        assert!(!text.contains("script.rs"));
        
        let details = result.details.as_object().unwrap();
        assert!(details["result_count"].as_u64().unwrap() >= 2);
    }

    #[tokio::test]
    async fn test_find_with_glob_star() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("test.rs"), "").unwrap();
        std::fs::write(dir.path().join("main.rs"), "").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "").unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*.rs", "path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证找到所有 rs 文件
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("test.rs") || text.contains("test"));
        assert!(text.contains("main.rs") || text.contains("main"));
        assert!(text.contains("lib.rs") || text.contains("lib"));
    }

    #[tokio::test]
    async fn test_find_recursive() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("root.txt"), "").unwrap();
        std::fs::write(dir.path().join("subdir/nested.txt"), "").unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "**/*.txt", "path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证递归查找
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("root.txt") || text.contains("root"));
        assert!(text.contains("nested.txt") || text.contains("nested"));
    }

    #[tokio::test]
    async fn test_find_directories() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::create_dir(dir.path().join("tests")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*", "path": ".", "type": "dir"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证只找到目录
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("src") || text.contains("src/"));
        assert!(text.contains("tests") || text.contains("tests/"));
        assert!(!text.contains("file.txt"));
    }

    #[tokio::test]
    async fn test_find_files_only() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*", "path": ".", "type": "file"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证只找到文件
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("file.txt") || text.contains("file"));
    }

    #[tokio::test]
    async fn test_find_no_match() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*.rs", "path": "."}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证无匹配
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("No files found"));
        
        let details = result.details.as_object().unwrap();
        assert_eq!(details["result_count"], 0);
    }

    #[tokio::test]
    async fn test_find_with_limit() {
        let dir = TempDir::new().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("file{}.txt", i)), "").unwrap();
        }
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*.txt", "path": ".", "limit": 5}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证限制
        let details = result.details.as_object().unwrap();
        assert!(details["result_count"].as_u64().unwrap() <= 5);
        assert_eq!(details["limit_reached"], true);
    }

    #[tokio::test]
    async fn test_find_missing_pattern() {
        let dir = TempDir::new().unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "."}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'pattern' parameter"));
    }

    #[tokio::test]
    async fn test_find_invalid_glob() {
        let dir = TempDir::new().unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "[invalid", "path": "."}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid glob pattern"));
    }

    #[tokio::test]
    async fn test_find_nonexistent_path() {
        let dir = TempDir::new().unwrap();
        
        let tool = FindTool::new(dir.path().to_path_buf());
        // 使用相对路径指向一个不存在的目录
        let result = tool.execute(
            "call_1",
            serde_json::json!({"pattern": "*", "path": "nonexistent_dir"}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // 错误消息可能是 "Path not found" 或 "No such file or directory" 或 "outside"
        assert!(err_msg.contains("Path not found") || err_msg.contains("No such file") || err_msg.contains("outside"));
    }

    #[test]
    fn test_find_tool_name() {
        let tool = FindTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "find");
    }

    #[test]
    fn test_find_tool_label() {
        let tool = FindTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.label(), "Find Files");
    }

    #[test]
    fn test_find_tool_parameters() {
        let tool = FindTool::new(PathBuf::from("/tmp"));
        let params = tool.parameters();
        
        assert!(params.is_object());
        let obj = params.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));
        
        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("pattern"));
        assert!(properties.contains_key("path"));
        assert!(properties.contains_key("type"));
        assert!(properties.contains_key("limit"));
    }
}
