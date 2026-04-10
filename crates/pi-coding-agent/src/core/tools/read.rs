//! 文件读取工具
//!
//! 读取文件内容，支持文本文件和图片

use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs;
use tokio::io::AsyncBufReadExt;
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;
use super::truncate::*;
use super::truncate::format_size;

/// 大文件阈值 (1MB)
const LARGE_FILE_THRESHOLD: u64 = 1_048_576;

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
    fn detect_image_mime_type(&self, path: &std::path::Path) -> Option<&'static str> {
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

        // 文件大小预检
        let file_size = metadata.len();
        let is_large_file = file_size > LARGE_FILE_THRESHOLD;

        // 检查取消
        if cancel.is_cancelled() {
            return Err(anyhow::anyhow!("Operation aborted"));
        }

        let (all_lines, total_lines): (Vec<String>, usize);

        if is_large_file {
            // 大文件：使用 BufReader 分块读取
            let file = fs::File::open(&absolute_path).await?;
            let reader = tokio::io::BufReader::new(file);
            let mut lines = reader.lines();
            
            let max_lines_to_read = limit.unwrap_or(DEFAULT_MAX_LINES);
            let mut collected = Vec::with_capacity(max_lines_to_read.min(10000));
            // line_count 用于潜在的未来功能
            let _line_count = 0usize;
            
            // 如果有 offset，先跳过前面的行
            let skip_lines = offset.map(|o| o.saturating_sub(1)).unwrap_or(0);
            
            // 跳过 offset 之前的行
            for _ in 0..skip_lines {
                if lines.next_line().await?.is_none() {
                    break;
                }
            }
            
            // 读取需要的行
            while let Some(line) = lines.next_line().await? {
                let _ = _line_count;
                collected.push(line);
                
                if collected.len() >= max_lines_to_read {
                    break;
                }
                
                // 检查取消
                if cancel.is_cancelled() {
                    return Err(anyhow::anyhow!("Operation aborted"));
                }
            }
            
            // 统计总行数（继续读取但不保存）
            let mut remaining_count = 0usize;
            while lines.next_line().await?.is_some() {
                remaining_count += 1;
            }
            
            total_lines = skip_lines + collected.len() + remaining_count;
            all_lines = collected;
        } else {
            // 小文件：直接读取全部内容
            let file_content = fs::read_to_string(&absolute_path).await.map_err(|e| {
                anyhow::anyhow!("Failed to read file '{}': {}", path, e)
            })?;

            // 检查取消
            if cancel.is_cancelled() {
                return Err(anyhow::anyhow!("Operation aborted"));
            }

            // 分割成行
            let lines: Vec<&str> = file_content.lines().collect();
            total_lines = lines.len();
            all_lines = lines.into_iter().map(|s| s.to_string()).collect();
        }

        // 对于大文件，all_lines 已经包含了从 offset 开始的内容
        // 对于小文件，需要应用 offset 和 limit
        let (selected_content, user_limited_lines, start_line_display, end_line): (String, usize, usize, usize);
        
        if is_large_file {
            // 大文件：all_lines 已经是从 offset 开始的内容
            start_line_display = offset.unwrap_or(1);
            let selected: Vec<&str> = all_lines.iter().map(|s| s.as_str()).collect();
            selected_content = selected.join("\n");
            user_limited_lines = all_lines.len();
            end_line = start_line_display + user_limited_lines.saturating_sub(1);
            
            // 添加大文件提示
            if !selected_content.is_empty() {
                let _truncated_hint = format!(
                    "\n\n[Large file detected ({}). Showing lines {}-{} of {}. Use offset={} to continue.]",
                    format_size(file_size as usize),
                    start_line_display,
                    end_line,
                    total_lines,
                    end_line + 1
                );
                // 我们将在后面处理这个提示
            }
        } else {
            // 小文件：应用 offset（转换为 0-indexed）
            let start_line = offset.map(|o| (o.saturating_sub(1)).min(total_lines)).unwrap_or(0);
            
            // 检查 offset 是否超出范围
            if start_line >= total_lines && total_lines > 0 {
                return Err(anyhow::anyhow!(
                    "Offset {} is beyond end of file ({} lines total)",
                    offset.unwrap_or(1),
                    total_lines
                ));
            }

            start_line_display = start_line + 1;

            // 应用 limit
            let end_line_idx = if let Some(lim) = limit {
                (start_line + lim).min(total_lines)
            } else {
                total_lines
            };

            // 提取选定的内容
            let selected_lines: Vec<String> = all_lines[start_line..end_line_idx].to_vec();
            selected_content = selected_lines.join("\n");
            user_limited_lines = end_line_idx - start_line;
            end_line = end_line_idx;
        }

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
        } else if is_large_file && end_line < total_lines {
            // 大文件还有更多内容
            let remaining = total_lines - end_line;
            let next_offset = end_line + 1;
            format!(
                "{}\n\n[Large file ({}). {} more lines. Use offset={} to continue.]",
                truncated_content, format_size(file_size as usize), remaining, next_offset
            )
        } else if limit.is_some() && start_line_display + user_limited_lines - 1 < total_lines {
            // 用户指定的 limit 提前结束，但文件还有更多内容
            let remaining = total_lines - (start_line_display + user_limited_lines - 1);
            let next_offset = start_line_display + user_limited_lines;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap()}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证内容包含文件内容
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
        assert!(text.contains("line3"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "/nonexistent/file.txt"}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 验证错误处理
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Failed to resolve path") || 
                error.to_string().contains("Cannot access file"));
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "offset": 3}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        
        // 从第3行开始，应该包含 line3, line4, line5
        assert!(text.contains("line3"));
        assert!(text.contains("line4"));
        assert!(text.contains("line5"));
    }

    #[tokio::test]
    async fn test_read_with_limit() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "limit": 2}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        
        // 限制2行，应该包含 line1, line2
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
        // 应该提示还有更多内容
        assert!(text.contains("more lines") || text.contains("offset="));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5\nline6").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap(), "offset": 2, "limit": 2}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        
        // 从第2行开始，限制2行，应该包含 line2, line3
        assert!(text.contains("line2"));
        assert!(text.contains("line3"));
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.txt");
        std::fs::write(&file_path, "").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap()}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        
        // 空文件应该返回空内容或提示
        assert!(text.is_empty() || text.contains("0") || text.contains("empty"));
    }

    #[tokio::test]
    async fn test_read_relative_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("subdir/test.txt");
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(&file_path, "relative path test").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": "subdir/test.txt"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        
        assert!(text.contains("relative path test"));
    }

    #[tokio::test]
    async fn test_read_image_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.png");
        // 写入一个最小的 PNG 文件头
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        std::fs::write(&file_path, &png_header).unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap()}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 图片应该返回文本内容和图片块
        assert!(!result.content.is_empty());
        
        // 验证包含图片内容
        let has_image = result.content.iter().any(|c| matches!(c, ContentBlock::Image(_)));
        assert!(has_image, "Should contain Image content block");
    }

    #[tokio::test]
    async fn test_read_cancellation() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();
        
        let tool = ReadTool::new(dir.path().to_path_buf());
        let cancel = CancellationToken::new();
        cancel.cancel();
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"path": file_path.to_str().unwrap()}),
            cancel,
            None,
        ).await;
        
        // 应该返回取消错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("aborted"));
    }

    #[test]
    fn test_read_tool_name() {
        let tool = ReadTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "read");
    }

    #[test]
    fn test_read_tool_label() {
        let tool = ReadTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.label(), "Read File");
    }

    #[test]
    fn test_read_tool_parameters() {
        let tool = ReadTool::new(PathBuf::from("/tmp"));
        let params = tool.parameters();
        
        assert!(params.is_object());
        let obj = params.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));
        
        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("path"));
        assert!(properties.contains_key("offset"));
        assert!(properties.contains_key("limit"));
    }
}
