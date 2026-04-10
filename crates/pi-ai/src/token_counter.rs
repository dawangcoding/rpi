//! Token 计数模块
//!
//! 提供文本和消息的 token 计数功能

use crate::types::{ContentBlock, Message};

/// Token 计数 trait
pub trait TokenCounter: Send + Sync {
    /// 计算文本的 token 数
    fn count_text(&self, text: &str) -> usize;
    /// 计算单条消息的 token 数
    fn count_message(&self, message: &Message) -> usize;
    /// 计算多条消息的总 token 数
    fn count_messages(&self, messages: &[Message]) -> usize;
}

/// 启发式 token 估算器（默认）
/// 使用字符数 / 4 的经验公式，对英文约 80% 准确
pub struct EstimateTokenCounter {
    chars_per_token: f64, // 默认 4.0
}

impl EstimateTokenCounter {
    pub fn new() -> Self {
        Self {
            chars_per_token: 4.0,
        }
    }

    pub fn with_ratio(chars_per_token: f64) -> Self {
        Self { chars_per_token }
    }
}

impl Default for EstimateTokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter for EstimateTokenCounter {
    fn count_text(&self, text: &str) -> usize {
        (text.chars().count() as f64 / self.chars_per_token).ceil() as usize
    }

    fn count_message(&self, message: &Message) -> usize {
        // 基础消息开销（格式开销）
        const MESSAGE_OVERHEAD: usize = 4;

        let content_tokens = match message {
            Message::User(user_msg) => match &user_msg.content {
                crate::types::UserContent::Text(text) => self.count_text(text),
                crate::types::UserContent::Blocks(blocks) => {
                    count_content_blocks(self, blocks)
                }
            },
            Message::Assistant(assistant_msg) => {
                count_content_blocks(self, &assistant_msg.content)
            }
            Message::ToolResult(tool_result) => {
                // tool_result 包含 tool_call_id、tool_name 和 content
                let id_tokens = self.count_text(&tool_result.tool_call_id);
                let name_tokens = self.count_text(&tool_result.tool_name);
                let content_tokens = count_content_blocks(self, &tool_result.content);
                id_tokens + name_tokens + content_tokens
            }
        };

        MESSAGE_OVERHEAD + content_tokens
    }

    fn count_messages(&self, messages: &[Message]) -> usize {
        if messages.is_empty() {
            return 0;
        }
        messages.iter().map(|m| self.count_message(m)).sum::<usize>() + 3 // 3 tokens for reply priming
    }
}

/// 计算内容块的 token 数
fn count_content_blocks(counter: &dyn TokenCounter, blocks: &[ContentBlock]) -> usize {
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text(text) => counter.count_text(&text.text),
            ContentBlock::Thinking(thinking) => counter.count_text(&thinking.thinking),
            ContentBlock::Image(_) => {
                // 图片通常占用较多 token，但这里使用估算值
                // 实际 token 数取决于图片尺寸和编码方式
                1024
            }
            ContentBlock::ToolCall(tool_call) => {
                // 工具调用包含 name 和 arguments
                let name_tokens = counter.count_text(&tool_call.name);
                let args_tokens = counter.count_text(&tool_call.arguments.to_string());
                name_tokens + args_tokens
            }
        })
        .sum()
}

/// 模型特定的 token 计数器
pub struct ModelTokenCounter {
    model_family: String,
    base_counter: EstimateTokenCounter,
}

impl ModelTokenCounter {
    pub fn new(model_family: &str) -> Self {
        // 不同模型家族使用不同的字符/token 比率
        let ratio = match model_family.to_lowercase().as_str() {
            "claude" | "anthropic" => 3.5,
            "gpt" | "openai" => 4.0,
            "gemini" | "google" => 4.0,
            "mistral" => 4.0,
            _ => 4.0,
        };
        Self {
            model_family: model_family.to_string(),
            base_counter: EstimateTokenCounter::with_ratio(ratio),
        }
    }

    pub fn model_family(&self) -> &str {
        &self.model_family
    }
}

impl TokenCounter for ModelTokenCounter {
    fn count_text(&self, text: &str) -> usize {
        self.base_counter.count_text(text)
    }

    fn count_message(&self, message: &Message) -> usize {
        self.base_counter.count_message(message)
    }

    fn count_messages(&self, messages: &[Message]) -> usize {
        self.base_counter.count_messages(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TextContent, UserContent, UserMessage};

    #[test]
    fn test_count_text() {
        let counter = EstimateTokenCounter::new();
        // "Hello World" 有 11 个字符，11/4 = 2.75，ceil = 3
        assert_eq!(counter.count_text("Hello World"), 3);
    }

    #[test]
    fn test_count_message_user() {
        let counter = EstimateTokenCounter::new();
        let msg = Message::User(UserMessage::new("Hello"));
        // "Hello" = 5 字符，5/4 = 1.25 -> 2 + 4 overhead = 6
        assert_eq!(counter.count_message(&msg), 6);
    }

    #[test]
    fn test_count_messages() {
        let counter = EstimateTokenCounter::new();
        let messages = vec![
            Message::User(UserMessage::new("Hello")),
            Message::User(UserMessage::new("World")),
        ];
        // 2 条消息，每条约 6 tokens，加上 3 tokens reply priming = 15
        let total = counter.count_messages(&messages);
        assert!(total >= 10);
    }

    #[test]
    fn test_model_token_counter() {
        let counter = ModelTokenCounter::new("claude");
        assert_eq!(counter.model_family(), "claude");

        let counter = ModelTokenCounter::new("gpt");
        assert_eq!(counter.model_family(), "gpt");
    }
}
