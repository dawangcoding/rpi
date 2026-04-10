//! 文件写入工具
//!
//! 创建或覆盖文件，自动创建父目录

use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs;
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;

/// 文件写入工具
pub struct WriteTool {
    cwd: PathBuf,
}

impl WriteTool {
    /// 创建新的 WriteTool
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
        
        // 对于写操作，文件可能还不存在，所以不能使用 canonicalize
        // 我们只需要确保路径在 cwd 下即可
        let canonical_cwd = self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone());
        
        // 尝试 canonicalize 路径（如果存在）或其存在的部分
        let canonical_path = if absolute_path.exists() {
            absolute_path.canonicalize()?
        } else {
            // 路径不存在，尝试 canonicalize 存在的部分
            let mut current = absolute_path.as_path();
            let mut to_canonicalize = Vec::new();
            
            // 向上查找直到找到一个存在的目录
            while !current.exists() {
                if let Some(file_name) = current.file_name() {
                    to_canonicalize.push(file_name.to_os_string());
                }
                current = current.parent()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path: no parent directory"))?;
            }
            
            // canonicalize 存在的部分
            let canonical_base = current.canonicalize()?;
            
            // 重新组装路径
            let mut result = canonical_base;
            for component in to_canonicalize.into_iter().rev() {
                result = result.join(component);
            }
            result
        };
        
        // 确保路径在 cwd 下
        if !canonical_path.starts_with(&canonical_cwd) {
            return Err(anyhow::anyhow!(
                "Path '{}' is outside the working directory",
                path
            ));
        }
        
        Ok(canonical_path)
    }

    /// 原子写入文件（写入临时文件后重命名）
    async fn atomic_write(&self, path: &PathBuf, content: &str) -> anyhow::Result<()> {
        // 创建父目录
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                anyhow::anyhow!("Failed to create parent directories for '{}': {}", path.display(), e)
            })?;
        }

        // 写入临时文件
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, content).await.map_err(|e| {
            anyhow::anyhow!("Failed to write temporary file '{}': {}", temp_path.display(), e)
        })?;

        // 重命名（原子操作）
        fs::rename(&temp_path, path).await.map_err(|e| {
            // 尝试清理临时文件
            drop(fs::remove_file(&temp_path));
            anyhow::anyhow!("Failed to rename temporary file to '{}': {}", path.display(), e)
        })?;

        Ok(())
    }
}

#[async_trait]
impl AgentTool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn label(&self) -> &str {
        "Write File"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories. Use only for new files or complete rewrites."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write (relative or absolute)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
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

        let path = params["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;
        
        let content = params["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' parameter"))?;

        let absolute_path = self.resolve_path(path)?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        // 原子写入文件
        self.atomic_write(&absolute_path, content).await?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        let content_len = content.len();

        Ok(AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(format!(
                "Successfully wrote {} bytes to {}",
                content_len, path
            )))],
            details: serde_json::json!({
                "path": path,
                "bytes_written": content_len,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("new_file.txt");
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": "hello world"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证文件存在且内容正确
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");
        
        // 验证返回结果
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("Successfully wrote"));
        assert!(text.contains("11 bytes"));
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("subdir/deep/file.txt");
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": "nested"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证父目录和文件都被创建
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "nested");
    }

    #[tokio::test]
    async fn test_write_overwrite_existing() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "old content").unwrap();
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": "new content"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证内容被覆盖
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "new content");
    }

    #[tokio::test]
    async fn test_write_empty_content() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.txt");
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": ""}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证空文件被创建
        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "");
    }

    #[tokio::test]
    async fn test_write_multiline_content() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("multiline.txt");
        let content = "line1\nline2\nline3\n";
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": content}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证多行内容
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);
    }

    #[tokio::test]
    async fn test_write_relative_path() {
        let dir = TempDir::new().unwrap();
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        tool.execute(
            "call_1",
            serde_json::json!({"path": "relative.txt", "content": "relative content"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证相对路径文件被创建
        assert!(dir.path().join("relative.txt").exists());
        assert_eq!(std::fs::read_to_string(dir.path().join("relative.txt")).unwrap(), "relative content");
    }

    #[tokio::test]
    async fn test_write_missing_path_parameter() {
        let dir = TempDir::new().unwrap();
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"content": "test"}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'path' parameter"));
    }

    #[tokio::test]
    async fn test_write_missing_content_parameter() {
        let dir = TempDir::new().unwrap();
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "test.txt"}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'content' parameter"));
    }

    #[tokio::test]
    async fn test_write_cancellation() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("cancelled.txt");
        
        let tool = WriteTool::new(dir.path().to_path_buf());
        let cancel = CancellationToken::new();
        cancel.cancel();
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "content": "test"}),
            cancel,
            None,
        ).await;
        
        // 应该返回取消错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("aborted"));
    }

    #[test]
    fn test_write_tool_name() {
        let tool = WriteTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "write");
    }

    #[test]
    fn test_write_tool_label() {
        let tool = WriteTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.label(), "Write File");
    }

    #[test]
    fn test_write_tool_parameters() {
        let tool = WriteTool::new(PathBuf::from("/tmp"));
        let params = tool.parameters();
        
        assert!(params.is_object());
        let obj = params.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));
        
        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("path"));
        assert!(properties.contains_key("content"));
        
        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("path")));
        assert!(required.contains(&serde_json::json!("content")));
    }
}
