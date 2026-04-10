//! 摘要提示词模块
//!
//! 提供用于生成会话摘要的提示词模板

use pi_agent::types::AgentMessage;
use pi_ai::types::{ContentBlock, Message};

/// 摘要生成系统提示词
pub const SUMMARY_SYSTEM_PROMPT: &str = r#"You are a conversation summarizer. Your task is to create a concise but comprehensive summary of the conversation history provided.

Preserve the following information:
1. File paths and their modifications (what was changed and why)
2. Key technical decisions made
3. Unfinished tasks or pending items
4. Important context that would be needed to continue the conversation

Output format:
## File Changes
- List each file that was modified with a brief description

## Key Decisions
- List important technical decisions

## Conversation Summary
- Brief summary of the discussion flow

## Pending Items
- Any unfinished tasks or items that need follow-up

Keep the summary concise but ensure all critical information is preserved."#;

/// 构建摘要提示词
pub fn build_summary_prompt(messages: &[AgentMessage]) -> String {
    let mut prompt = String::new();
    prompt.push_str("Please summarize the following conversation history:\n\n");
    prompt.push_str("---\n\n");

    for (idx, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::Llm(Message::User(user_msg)) => {
                prompt.push_str(&format!("[Message {}] User:\n", idx));
                let content = extract_message_content(user_msg.content.clone());
                prompt.push_str(&content);
                prompt.push_str("\n\n");
            }
            AgentMessage::Llm(Message::Assistant(assistant)) => {
                prompt.push_str(&format!("[Message {}] Assistant:\n", idx));
                let content = extract_content_blocks(&assistant.content);
                prompt.push_str(&content);
                prompt.push_str("\n\n");
            }
            AgentMessage::Llm(Message::ToolResult(tool_result)) => {
                prompt.push_str(&format!(
                    "[Message {}] Tool Result ({}):\n",
                    idx, tool_result.tool_name
                ));
                let content = extract_content_blocks(&tool_result.content);
                // 限制工具结果长度，避免摘要提示词过长
                let truncated = if content.len() > 2000 {
                    format!("{}... [truncated]", &content[..2000])
                } else {
                    content
                };
                prompt.push_str(&truncated);
                prompt.push_str("\n\n");
            }
        }
    }

    prompt.push_str("---\n\n");
    prompt.push_str("Please provide a structured summary following the format specified in the system prompt.");

    prompt
}

/// 从 UserContent 提取文本内容
fn extract_message_content(content: pi_ai::types::UserContent) -> String {
    match content {
        pi_ai::types::UserContent::Text(text) => text,
        pi_ai::types::UserContent::Blocks(blocks) => extract_content_blocks(&blocks),
    }
}

/// 从 ContentBlock 数组提取文本内容
fn extract_content_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            ContentBlock::Thinking(thinking) => Some(format!(
                "[Thinking: {}]",
                &thinking.thinking[..thinking.thinking.len().min(500)]
            )),
            ContentBlock::ToolCall(tool_call) => Some(format!(
                "[Tool Call: {}({})]",
                tool_call.name,
                tool_call.arguments
            )),
            ContentBlock::Image(_) => Some("[Image]".to_string()),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_ai::types::{UserMessage, ContentBlock, TextContent};

    #[test]
    fn test_build_summary_prompt_empty() {
        let messages: Vec<AgentMessage> = vec![];
        let prompt = build_summary_prompt(&messages);
        assert!(prompt.contains("Please summarize"));
        assert!(prompt.contains("---"));
    }

    #[test]
    fn test_build_summary_prompt_with_messages() {
        let messages = vec![
            AgentMessage::Llm(Message::User(UserMessage::new("Hello"))),
            AgentMessage::Llm(Message::User(UserMessage::new("World"))),
        ];
        let prompt = build_summary_prompt(&messages);
        assert!(prompt.contains("Hello"));
        assert!(prompt.contains("World"));
        assert!(prompt.contains("[Message 0]"));
        assert!(prompt.contains("[Message 1]"));
    }

    #[test]
    fn test_extract_content_blocks() {
        let blocks = vec![
            ContentBlock::Text(TextContent::new("Hello")),
            ContentBlock::Text(TextContent::new("World")),
        ];
        let content = extract_content_blocks(&blocks);
        assert!(content.contains("Hello"));
        assert!(content.contains("World"));
    }
}
