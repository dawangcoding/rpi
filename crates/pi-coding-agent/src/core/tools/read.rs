//! 文件读取工具
//!
//! 读取文件内容，支持文本文件和图片

use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs;
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;
use super::truncate::*;

/// 文件读取工具
pub struct ReadTool {
    cwd: PathBuf,
}

impl ReadTool {
    /// 创建新的 ReadTool
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
        
        // 规范化路径并检查路径遍历
        let canonical = absolute_path.canonicalize().map_err(|e| {
            anyhow::anyhow!("Failed to resolve path '{}': {}", path, e)
        })?;
        
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

    /// 检测文件是否为图片
    fn detect_image_mime_type(&self, path: &PathBuf) -> Option<&'static str> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "jpg" | "jpeg" => Some("image/jpeg"),
            "png" => Some("image/png"),
            "gif" => Some("image/gif"),
            "webp" => Some("image/webp"),
            "bmp" => Some("image/bmp"),
            "svg" => Some("image/svg+xml"),
            _ => None,
        }
    }
}

#[async_trait]
impl AgentTool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn label(&self) -> &str {
        "Read File"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Supports text files and images (jpg, png, gif, webp). Images are sent as attachments. For text files, output is truncated to 500 lines or 1MB (whichever is hit first). Use offset/limit for large files."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (relative or absolute)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["path"]
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
        
        let offset = params["offset"].as_u64().map(|o| o as usize);
        let limit = params["limit"].as_u64().map(|l| l as usize);

        let absolute_path = self.resolve_path(path)?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        // 检查文件是否存在且可读
        let metadata = fs::metadata(&absolute_path).await.map_err(|e| {
            anyhow::anyhow!("Cannot access file '{}': {}", path, e)
        })?;

        if !metadata.is_file() {
            return Err(anyhow::anyhow!("'{}' is not a file", path));
        }

        // 检查是否为图片
        if let Some(mime_type) = self.detect_image_mime_type(&absolute_path) {
            // 读取图片文件
            let image_data = fs::read(&absolute_path).await.map_err(|e| {
                anyhow::anyhow!("Failed to read image file '{}': {}", path, e)
            })?;

            use base64::Engine;
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&image_data);
            
            // 构建结果
            let content = vec![
                ContentBlock::Text(TextContent::new(format!("Read image file [{}]", mime_type))),
                ContentBlock::Image(ImageContent::new(base64_data, mime_type)),
            ];

            return Ok(AgentToolResult {
                content,
                details: serde_json::json!({
                    "path": path,
                    "mime_type": mime_type,
                    "size": image_data.len(),
                }),
            });
        }

        // 读取文本文件
        let file_content = fs::read_to_string(&absolute_path).await.map_err(|e| {
            anyhow::anyhow!("Failed to read file '{}': {}", path, e)
        })?;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        // 分割成行
        let all_lines: Vec<&str> = file_content.lines().collect();
        let total_lines = all_lines.len();

        // 应用 offset（转换为 0-indexed）
        let start_line = offset.map(|o| (o.saturating_sub(1)).min(total_lines)).unwrap_or(0);
        
        // 检查 offset 是否超出范围
        if start_line >= total_lines && total_lines > 0 {
            return Err(anyhow::anyhow!(
                "Offset {} is beyond end of file ({} lines total)",
                offset.unwrap_or(1),
                total_lines
            ));
        }

        let start_line_display = start_line + 1;

        // 应用 limit
        let end_line = if let Some(lim) = limit {
            (start_line + lim).min(total_lines)
        } else {
            total_lines
        };

        // 提取选定的内容
        let selected_lines: Vec<&str> = all_lines[start_line..end_line].to_vec();
        let selected_content = selected_lines.join("\n");
        let user_limited_lines = end_line - start_line;

        // 应用截断
        let (truncated_content, truncation_result) = truncate_output_head(&selected_content, DEFAULT_MAX_LINES, DEFAULT_MAX_BYTES);

        // 构建输出
        let output_text = if truncation_result.was_truncated {
            let end_line_display = start_line_display + truncation_result.kept_lines - 1;
            let next_offset = end_line_display + 1;
            format!(
                "{}\n\n[Showing lines {}-{} of {}. Use offset={} to continue.]",
                truncated_content, start_line_display, end_line_display, total_lines, next_offset
            )
        } else if limit.is_some() && start_line + user_limited_lines < total_lines {
            // 用户指定的 limit 提前结束，但文件还有更多内容
            let remaining = total_lines - (start_line + user_limited_lines);
            let next_offset = start_line + user_limited_lines + 1;
            format!(
                "{}\n\n[{} more lines in file. Use offset={} to continue.]",
                truncated_content, remaining, next_offset
            )
        } else {
            truncated_content
        };

        Ok(AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(output_text))],
            details: serde_json::json!({
                "path": path,
                "total_lines": total_lines,
                "truncation": if truncation_result.was_truncated {
                    serde_json::to_value(&truncation_result)?
                } else {
                    serde_json::Value::Null
                },
            }),
        })
    }
}
