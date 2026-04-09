//! 初始消息构建
//!
//! 从 CLI 参数构建初始用户消息

use pi_agent::types::AgentMessage;
use pi_ai::types::*;
use std::path::Path;

/// 从 CLI 参数构建初始用户消息
pub fn build_initial_message(
    prompt: &str,
    files: &[String],
    cwd: &Path,
) -> anyhow::Result<AgentMessage> {
    let mut parts: Vec<ContentBlock> = vec![];

    // 添加文件内容
    for file_path in files {
        let path = if Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            cwd.join(file_path).to_string_lossy().to_string()
        };

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", path, e))?;

        parts.push(ContentBlock::Text(TextContent::new(format!(
            "<file path=\"{}\">\n{}\n</file>",
            path, content
        ))));
    }

    // 添加用户 prompt
    if !prompt.is_empty() {
        parts.push(ContentBlock::Text(TextContent::new(prompt.to_string())));
    }

    Ok(AgentMessage::Llm(Message::User(UserMessage::new(parts))))
}

/// 构建简单的文本初始消息
pub fn build_simple_message(prompt: &str) -> AgentMessage {
    AgentMessage::user(prompt)
}
