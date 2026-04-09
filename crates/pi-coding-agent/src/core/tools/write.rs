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
        
        // 规范化路径
        let canonical = absolute_path.canonicalize().unwrap_or(absolute_path);
        
        // 确保路径在 cwd 下（防止路径遍历攻击）
        let canonical_cwd = self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone());
        if !canonical.starts_with(&canonical_cwd) {
            return Err(anyhow::anyhow!(
                "Path '{}' is outside the working directory",
                path
            ));
        }
        
        Ok(canonical)
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
            let _ = fs::remove_file(&temp_path);
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
