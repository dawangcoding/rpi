//! Token 计数模块
//!
//! 提供文本和消息的 token 计数功能

use crate::types::{ContentBlock, Message, UserContent};
use std::sync::Arc;
use tiktoken_rs::CoreBPE;

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
        // 比率越小，估算的 token 数越多（更保守）
        let ratio = match model_family.to_lowercase().as_str() {
            "claude" | "anthropic" => 3.5,  // Claude 保持 3.5
            "gpt" | "openai" => 4.0,       // OpenAI 保持 4.0（实际使用 TiktokenCounter）
            "gemini" | "google" => 3.8,    // Gemini 从 4.0 微调至 3.8
            "mistral" => 3.8,              // Mistral 从 4.0 微调至 3.8
            _ => 4.0,                       // 默认保持 4.0
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

/// Tiktoken 精确 token 计数器（用于 OpenAI 模型）
pub struct TiktokenCounter {
    bpe: CoreBPE,
}

impl TiktokenCounter {
    /// 创建新的 TiktokenCounter
    /// 对于 gpt-4o 系列使用 o200k_base
    /// 对于 gpt-4/gpt-3.5 使用 cl100k_base
    pub fn new(model: &str) -> Option<Self> {
        let model_lower = model.to_lowercase();
        
        // 尝试使用 tiktoken_rs::get_bpe_from_model
        // 如果失败，根据模型名称手动选择 BPE
        let bpe = match tiktoken_rs::get_bpe_from_model(&model_lower) {
            Ok(bpe) => bpe,
            Err(_) => {
                // 手动映射常见模型
                if model_lower.contains("gpt-4o") || model_lower.starts_with("o1") || model_lower.starts_with("o3") || model_lower.starts_with("o4") {
                    tiktoken_rs::o200k_base().ok()?
                } else if model_lower.contains("gpt-4") || model_lower.contains("gpt-3.5") {
                    tiktoken_rs::cl100k_base().ok()?
                } else {
                    return None;
                }
            }
        };
        
        Some(Self { bpe })
    }

    /// 计算内容块的 token 数
    fn count_content_block(&self, block: &ContentBlock) -> usize {
        match block {
            ContentBlock::Text(t) => self.count_text(&t.text),
            ContentBlock::Thinking(t) => self.count_text(&t.thinking),
            ContentBlock::Image(_) => 1024, // 固定估算
            ContentBlock::ToolCall(tc) => {
                let name_tokens = self.count_text(&tc.name);
                let args_tokens = self.count_text(&serde_json::to_string(&tc.arguments).unwrap_or_default());
                name_tokens + args_tokens
            }
        }
    }
}

impl TokenCounter for TiktokenCounter {
    fn count_text(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }

    fn count_message(&self, message: &Message) -> usize {
        // 基础消息开销（格式开销）
        const MESSAGE_OVERHEAD: usize = 4;

        let content_tokens = match message {
            Message::User(user_msg) => match &user_msg.content {
                UserContent::Text(text) => self.count_text(text),
                UserContent::Blocks(blocks) => {
                    blocks.iter().map(|b| self.count_content_block(b)).sum()
                }
            },
            Message::Assistant(assistant_msg) => {
                assistant_msg.content.iter().map(|b| self.count_content_block(b)).sum()
            }
            Message::ToolResult(tool_result) => {
                // tool_result 包含 tool_call_id、tool_name 和 content
                let id_tokens = self.count_text(&tool_result.tool_call_id);
                let name_tokens = self.count_text(&tool_result.tool_name);
                let content_tokens: usize = tool_result.content.iter().map(|b| self.count_content_block(b)).sum();
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

/// 判断是否为 OpenAI 模型
fn is_openai_model(model: &str) -> bool {
    let model_lower = model.to_lowercase();
    model_lower.starts_with("gpt-") 
        || model_lower.starts_with("o1")
        || model_lower.starts_with("o3")
        || model_lower.starts_with("o4")
        || model_lower.contains("openai")
}

/// 创建 token 计数器工厂函数
/// 对于 OpenAI 模型使用精确的 TiktokenCounter，其他模型使用启发式计数器
pub fn create_token_counter(model: &str) -> Arc<dyn TokenCounter> {
    // 尝试为 OpenAI 模型创建精确计数器
    if is_openai_model(model) {
        if let Some(counter) = TiktokenCounter::new(model) {
            return Arc::new(counter);
        }
    }
    // 回退到模型特定的启发式计数器
    Arc::new(ModelTokenCounter::new(model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UserMessage;

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

    #[test]
    fn test_tiktoken_counter_creation() {
        let counter = TiktokenCounter::new("gpt-4o");
        assert!(counter.is_some());

        let counter = TiktokenCounter::new("gpt-4");
        assert!(counter.is_some());

        let counter = TiktokenCounter::new("gpt-3.5-turbo");
        assert!(counter.is_some());

        let counter = TiktokenCounter::new("o1-preview");
        assert!(counter.is_some());
    }

    #[test]
    fn test_tiktoken_count_text() {
        let counter = TiktokenCounter::new("gpt-4o").unwrap();
        let count = counter.count_text("Hello, world!");
        assert!(count > 0);
        assert!(count < 10); // 合理范围
    }

    #[test]
    fn test_create_token_counter_openai() {
        let counter = create_token_counter("gpt-4o");
        let count = counter.count_text("Hello, world!");
        assert!(count > 0);
    }

    #[test]
    fn test_create_token_counter_claude() {
        let counter = create_token_counter("claude-sonnet-4-20250514");
        let count = counter.count_text("Hello, world!");
        assert!(count > 0);
    }

    #[test]
    fn test_model_token_counter_updated_ratios() {
        let gemini = ModelTokenCounter::new("gemini");
        let default = EstimateTokenCounter::new();
        // Gemini 使用 3.8 而非 4.0，应产生更多 tokens（更保守的估算）
        let text = "a".repeat(100);
        assert!(gemini.count_text(&text) >= default.count_text(&text));
    }

    #[test]
    fn test_tiktoken_counter_message() {
        let counter = TiktokenCounter::new("gpt-4o").unwrap();
        let msg = Message::User(UserMessage::new("Hello, world!"));
        let count = counter.count_message(&msg);
        assert!(count > 4); // 至少要有 overhead
    }

    #[test]
    fn test_tiktoken_counter_messages() {
        let counter = TiktokenCounter::new("gpt-4o").unwrap();
        let messages = vec![
            Message::User(UserMessage::new("Hello")),
            Message::User(UserMessage::new("World")),
        ];
        let count = counter.count_messages(&messages);
        assert!(count > 0);
    }

    #[test]
    fn test_is_openai_model() {
        assert!(is_openai_model("gpt-4o"));
        assert!(is_openai_model("gpt-4"));
        assert!(is_openai_model("gpt-3.5-turbo"));
        assert!(is_openai_model("o1-preview"));
        assert!(is_openai_model("o3-mini"));
        assert!(is_openai_model("openai/gpt-4"));
        assert!(!is_openai_model("claude-sonnet"));
        assert!(!is_openai_model("gemini-pro"));
    }
}
