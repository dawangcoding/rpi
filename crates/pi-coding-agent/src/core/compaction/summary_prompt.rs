//! 摘要提示词模块
//!
//! 提供用于生成会话摘要的提示词模板

use pi_agent::types::AgentMessage;
use pi_ai::types::{ContentBlock, Message};

/// 摘要生成系统提示词
pub const SUMMARY_SYSTEM_PROMPT: &str = r#"You are a conversation summarizer. Your task is to create a concise but comprehensive summary of a conversation between a user and an AI coding assistant.

The summary should preserve:

1. **File Changes**: List all files that were created, modified, or deleted, with a brief description of changes.
2. **Key Decisions**: Important technical decisions made during the conversation.
3. **Current Working Directory**: The working directory context if mentioned.
4. **Active Files**: Files currently being worked on or recently edited.
5. **Error Context**: Any errors encountered and their resolution status.
6. **Conversation Summary**: A chronological summary of the discussion flow.
7. **Pending Items**: Any unfinished tasks or open questions.

Format the summary as structured text with clear section headers. Be concise but ensure no critical context is lost. Focus on information that would be needed to continue the conversation effectively."#;

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
                // 使用智能截断格式化工具结果
                let truncated = format_tool_result(&tool_result.tool_name, &content);
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
        .map(|block| match block {
            ContentBlock::Text(text) => text.text.clone(),
            ContentBlock::Thinking(thinking) => format!(
                "[Thinking: {}]",
                &thinking.thinking[..thinking.thinking.len().min(500)]
            ),
            ContentBlock::ToolCall(tool_call) => format!(
                "[Tool Call: {}({})]",
                tool_call.name,
                tool_call.arguments
            ),
            ContentBlock::Image(_) => "[Image]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 智能格式化工具结果，根据工具类型应用不同的截断策略
fn format_tool_result(tool_name: &str, content: &str) -> String {
    let max_len = 3000;

    // 对文件内容类工具，只保留摘要
    if tool_name.eq_ignore_ascii_case("read") || tool_name.eq_ignore_ascii_case("cat") {
        // 提取文件路径（通常在第一行或参数中）
        let first_line = content.lines().next().unwrap_or("");
        let line_count = content.lines().count();
        return format!("[File content: {} ({} lines)]", first_line, line_count);
    }

    // 对 bash 命令，保留命令本身和输出摘要
    if (tool_name.eq_ignore_ascii_case("bash") || tool_name.eq_ignore_ascii_case("shell"))
        && content.len() > max_len
    {
        let truncated = &content[..max_len];
        return format!("{}... [truncated, {} total chars]", truncated, content.len());
    }

    // 对 grep/find 等搜索工具，保留前 500 字符
    if (tool_name.eq_ignore_ascii_case("grep") || tool_name.eq_ignore_ascii_case("find"))
        && content.len() > 500
    {
        let truncated = &content[..500];
        return format!("{}... [truncated, {} total chars]", truncated, content.len());
    }

    // 默认截断
    if content.len() > max_len {
        format!("{}... [truncated]", &content[..max_len])
    } else {
        content.to_string()
    }
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

    #[test]
    fn test_format_tool_result() {
        // 测试 read 工具 - 应该只返回摘要
        let read_result = format_tool_result("read", "Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
        assert!(read_result.contains("File content:"));
        assert!(read_result.contains("5 lines"));
        assert!(!read_result.contains("Line 2")); // 不应该包含完整内容

        // 测试 bash 工具 - 长内容应该截断
        let long_content = "a".repeat(4000);
        let bash_result = format_tool_result("bash", &long_content);
        assert!(bash_result.contains("[truncated"));
        assert!(bash_result.contains("4000 total chars"));

        // 测试 bash 工具 - 短内容应该保留
        let short_content = "echo hello";
        let bash_result = format_tool_result("bash", short_content);
        assert_eq!(bash_result, short_content);

        // 测试 grep 工具 - 应该截断到 500 字符
        let grep_content = "x".repeat(1000);
        let grep_result = format_tool_result("grep", &grep_content);
        assert!(grep_result.contains("[truncated"));
        assert!(grep_result.contains("1000 total chars"));

        // 测试默认工具 - 应该截断到 3000 字符
        let other_content = "y".repeat(5000);
        let other_result = format_tool_result("other", &other_content);
        assert!(other_result.contains("[truncated]"));
    }
}
