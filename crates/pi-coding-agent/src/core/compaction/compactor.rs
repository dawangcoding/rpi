//! 会话压缩核心逻辑
//!
//! 提供会话压缩的核心功能，包括检查是否需要压缩、执行压缩、应用压缩结果等。

use std::sync::Arc;
use pi_ai::token_counter::TokenCounter;
use pi_ai::types::*;
use pi_agent::types::AgentMessage;

use super::summary_prompt::{build_summary_prompt, SUMMARY_SYSTEM_PROMPT};
use crate::core::session_manager::CompactionRecord;

/// 压缩结果
pub struct CompactionResult {
    /// 摘要消息（作为 User 消息插入）
    pub summary_message: AgentMessage,
    /// 被移除的消息数量
    pub removed_count: usize,
    /// 原始 token 数
    pub original_tokens: usize,
    /// 压缩后 token 数（仅摘要消息）
    pub compacted_tokens: usize,
    /// 压缩记录
    pub record: CompactionRecord,
}

/// 会话压缩器
pub struct SessionCompactor {
    token_counter: Arc<dyn TokenCounter>,
    context_window_size: usize,
    compact_threshold: f64,
    preserve_recent_turns: usize,
}

impl SessionCompactor {
    /// 创建新的会话压缩器
    pub fn new(
        token_counter: Arc<dyn TokenCounter>,
        context_window_size: usize,
    ) -> Self {
        Self {
            token_counter,
            context_window_size,
            compact_threshold: 0.85, // 默认 85% 阈值
            preserve_recent_turns: 4, // 默认保留最近 4 轮对话
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(
        token_counter: Arc<dyn TokenCounter>,
        context_window_size: usize,
        compact_threshold: f64,
        preserve_recent_turns: usize,
    ) -> Self {
        Self {
            token_counter,
            context_window_size,
            compact_threshold: compact_threshold.clamp(0.0, 1.0),
            preserve_recent_turns,
        }
    }

    /// 检查是否需要压缩
    pub fn needs_compaction(&self, messages: &[AgentMessage]) -> bool {
        if messages.len() < 10 {
            // 消息太少，不需要压缩
            return false;
        }

        let llm_messages: Vec<Message> = messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(msg) => Some(msg.clone()),
            })
            .collect();

        let total_tokens = self.token_counter.count_messages(&llm_messages);
        let threshold = self.context_window_size as f64 * self.compact_threshold;

        total_tokens as f64 > threshold
    }

    /// 获取当前 token 使用情况
    pub fn estimate_usage(&self, messages: &[AgentMessage]) -> (usize, f64) {
        let llm_messages: Vec<Message> = messages
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(msg) => Some(msg.clone()),
            })
            .collect();

        let total_tokens = self.token_counter.count_messages(&llm_messages);
        let usage_percent = if self.context_window_size > 0 {
            (total_tokens as f64 / self.context_window_size as f64) * 100.0
        } else {
            0.0
        };

        (total_tokens, usage_percent)
    }

    /// 执行压缩：调用 LLM 生成摘要
    pub async fn compact(
        &self,
        messages: &[AgentMessage],
        model: &Model,
    ) -> anyhow::Result<CompactionResult> {
        if messages.len() < 5 {
            anyhow::bail!("Not enough messages to compact (minimum 5)");
        }

        // 1. 确定可压缩范围（保留第一条+最近N轮）
        let compress_range = self.determine_compress_range(messages);
        if compress_range.0 >= compress_range.1 {
            anyhow::bail!("No messages available for compression");
        }

        // 2. 提取可压缩消息
        let messages_to_compact = &messages[compress_range.0..compress_range.1];

        // 3. 计算原始 token 数
        let llm_messages: Vec<Message> = messages_to_compact
            .iter()
            .filter_map(|m| match m {
                AgentMessage::Llm(msg) => Some(msg.clone()),
            })
            .collect();
        let original_tokens = self.token_counter.count_messages(&llm_messages);

        // 4. 构造摘要提示词
        let summary_prompt = build_summary_prompt(messages_to_compact);

        // 5. 调用 LLM 生成摘要
        let summary_text = self.generate_summary(&summary_prompt, model).await?;

        // 6. 构造摘要消息（作为 User 消息，带有特殊前缀标识）
        let summary_content = format!(
            "[Conversation Summary - {} messages compacted]\n\n{}",
            messages_to_compact.len(),
            summary_text
        );
        let summary_message = AgentMessage::user(&summary_content);

        // 7. 计算摘要 token 数
        let summary_llm_msg = match &summary_message {
            AgentMessage::Llm(Message::User(msg)) => Message::User(msg.clone()),
            _ => unreachable!(),
        };
        let compacted_tokens = self.token_counter.count_message(&summary_llm_msg);

        // 8. 创建压缩记录
        let record = CompactionRecord {
            compacted_at: chrono::Utc::now().timestamp_millis(),
            removed_message_range: compress_range,
            summary_tokens: compacted_tokens,
            original_tokens,
        };

        Ok(CompactionResult {
            summary_message,
            removed_count: messages_to_compact.len(),
            original_tokens,
            compacted_tokens,
            record,
        })
    }

    /// 应用压缩结果到消息列表
    pub fn apply_compaction(
        &self,
        messages: &mut Vec<AgentMessage>,
        result: &CompactionResult,
    ) {
        let (start, end) = result.record.removed_message_range;

        // 确保范围有效
        if start >= end || end > messages.len() {
            return;
        }

        // 移除被压缩的消息，插入摘要消息
        messages.splice(start..end, std::iter::once(result.summary_message.clone()));
    }

    /// 确定可压缩的消息范围
    /// 策略：保留第一条消息（通常是系统消息）和最近 N 轮对话
    fn determine_compress_range(&self, messages: &[AgentMessage]) -> (usize, usize) {
        if messages.len() < 5 {
            return (0, 0); // 消息太少，不压缩
        }

        // 保留第一条消息
        let keep_first = 1;

        // 计算要保留的最近消息数（每轮对话通常包含 user + assistant + 可能的 tool results）
        // 这里我们简单保留最近 preserve_recent_turns * 2 条消息
        let keep_last = self.preserve_recent_turns * 2;

        if messages.len() <= keep_first + keep_last {
            return (0, 0); // 消息不够多，不压缩
        }

        let compress_start = keep_first;
        let compress_end = messages.len() - keep_last;

        if compress_start >= compress_end {
            return (0, 0);
        }

        (compress_start, compress_end)
    }

    /// 调用 LLM 生成摘要
    async fn generate_summary(
        &self,
        prompt: &str,
        model: &Model,
    ) -> anyhow::Result<String> {
        // 构建上下文
        let messages = vec![Message::User(UserMessage::new(prompt))];
        let context = Context {
            system_prompt: Some(SUMMARY_SYSTEM_PROMPT.to_string()),
            messages,
            tools: None,
        };

        // 构建选项
        let options = SimpleStreamOptions {
            temperature: Some(0.3), // 较低的温度以获得更确定的摘要
            max_tokens: Some(2048), // 限制摘要长度
            api_key: pi_ai::models::get_api_key_from_env(&model.provider),
            transport: None,
            cache_retention: None,
            session_id: None,
            headers: None,
            max_retry_delay_ms: None,
            metadata: None,
            reasoning: None,
            thinking_budgets: None,
        };

        // 调用 LLM
        let response = pi_ai::stream::complete_simple(&context, model, &options).await?;

        // 提取摘要文本
        let summary = response
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if summary.trim().is_empty() {
            anyhow::bail!("LLM returned empty summary");
        }

        Ok(summary)
    }

    /// 获取上下文窗口大小
    pub fn context_window_size(&self) -> usize {
        self.context_window_size
    }

    /// 获取压缩阈值
    pub fn compact_threshold(&self) -> f64 {
        self.compact_threshold
    }

    /// 获取保留的对话轮数
    pub fn preserve_recent_turns(&self) -> usize {
        self.preserve_recent_turns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_ai::EstimateTokenCounter;

    fn create_test_messages(count: usize) -> Vec<AgentMessage> {
        let mut messages = Vec::new();
        for i in 0..count {
            let content = format!("Message {}", i);
            messages.push(AgentMessage::user(&content));
        }
        messages
    }

    #[test]
    fn test_session_compactor_new() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        assert_eq!(compactor.context_window_size(), 10000);
        assert_eq!(compactor.compact_threshold(), 0.85);
        assert_eq!(compactor.preserve_recent_turns(), 4);
    }

    #[test]
    fn test_session_compactor_with_config() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::with_config(counter, 20000, 0.75, 6);

        assert_eq!(compactor.context_window_size(), 20000);
        assert_eq!(compactor.compact_threshold(), 0.75);
        assert_eq!(compactor.preserve_recent_turns(), 6);
    }

    #[test]
    fn test_determine_compress_range() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        // 消息太少，不压缩
        let messages = create_test_messages(5);
        let range = compactor.determine_compress_range(&messages);
        assert_eq!(range, (0, 0));

        // 足够消息，可以压缩
        let messages = create_test_messages(20);
        let range = compactor.determine_compress_range(&messages);
        // 保留第1条，保留最后 4*2=8 条，所以压缩范围是 1..12
        assert_eq!(range, (1, 12));
    }

    #[test]
    fn test_apply_compaction() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        let mut messages = create_test_messages(10);
        let original_len = messages.len();

        // 创建模拟的压缩结果
        let summary_message = AgentMessage::user("Summary");
        let record = CompactionRecord {
            compacted_at: 0,
            removed_message_range: (1, 5),
            summary_tokens: 10,
            original_tokens: 100,
        };
        let result = CompactionResult {
            summary_message,
            removed_count: 4,
            original_tokens: 100,
            compacted_tokens: 10,
            record,
        };

        compactor.apply_compaction(&mut messages, &result);

        // 原始 10 条，移除 4 条，添加 1 条摘要 = 7 条
        assert_eq!(messages.len(), original_len - 4 + 1);
        // 第1条应该是摘要
        match &messages[1] {
            AgentMessage::Llm(Message::User(user_msg)) => {
                match &user_msg.content {
                    pi_ai::types::UserContent::Text(text) => {
                        assert!(text.contains("Summary"));
                    }
                    _ => panic!("Expected text content"),
                }
            }
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_needs_compaction_below_threshold() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        // 少量消息，不需要压缩
        let messages = create_test_messages(5);
        assert!(!compactor.needs_compaction(&messages));

        // 消息数量够但 token 未达阈值
        let messages = create_test_messages(10);
        // EstimateTokenCounter 估算的 token 数通常不会达到 8500 (85% of 10000)
        assert!(!compactor.needs_compaction(&messages));
    }

    #[test]
    fn test_needs_compaction_above_threshold() {
        // 使用一个小的上下文窗口来测试阈值触发
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 100);

        // 创建大量消息，应该超过 85% 阈值 (85 tokens)
        let mut messages = Vec::new();
        for i in 0..50 {
            messages.push(AgentMessage::user(&format!("This is a longer message number {} with enough content to generate tokens", i)));
        }

        assert!(compactor.needs_compaction(&messages));
    }

    #[test]
    fn test_compaction_range_calculation() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::with_config(
            counter,
            10000,
            0.85,
            3, // 保留最近 3 轮
        );

        // 创建 15 条消息
        let messages = create_test_messages(15);
        let range = compactor.determine_compress_range(&messages);

        // 保留第1条，保留最后 3*2=6 条
        // 所以压缩范围应该是 1..9
        assert_eq!(range, (1, 9));
    }

    #[test]
    fn test_compaction_preserves_recent_messages() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        // 创建 20 条消息
        let messages = create_test_messages(20);
        let range = compactor.determine_compress_range(&messages);

        // 验证保留第一条消息
        assert!(range.0 >= 1);

        // 验证保留最近的消息 (最后 8 条)
        let preserved_end = messages.len() - range.1;
        assert_eq!(preserved_end, 8);

        // 应用压缩并验证最近消息被保留
        let mut messages_clone = messages.clone();
        let summary_message = AgentMessage::user("Summary");
        let record = CompactionRecord {
            compacted_at: 0,
            removed_message_range: range,
            summary_tokens: 10,
            original_tokens: 100,
        };
        let result = CompactionResult {
            summary_message,
            removed_count: range.1 - range.0,
            original_tokens: 100,
            compacted_tokens: 10,
            record,
        };

        compactor.apply_compaction(&mut messages_clone, &result);

        // 验证第一条消息还在
        match &messages_clone[0] {
            AgentMessage::Llm(Message::User(user_msg)) => {
                match &user_msg.content {
                    pi_ai::types::UserContent::Text(text) => {
                        assert!(text.contains("Message 0"));
                    }
                    _ => panic!("Expected text content"),
                }
            }
            _ => panic!("Expected user message"),
        }

        // 验证最近的消息还在（最后 8 条应该保留）
        let last_idx = messages_clone.len() - 1;
        match &messages_clone[last_idx] {
            AgentMessage::Llm(Message::User(user_msg)) => {
                match &user_msg.content {
                    pi_ai::types::UserContent::Text(text) => {
                        assert!(text.contains("Message 19"));
                    }
                    _ => panic!("Expected text content"),
                }
            }
            _ => panic!("Expected user message"),
        }
    }

    #[test]
    fn test_estimate_usage() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        let messages = create_test_messages(10);
        let (tokens, percent) = compactor.estimate_usage(&messages);

        assert!(tokens > 0);
        assert!(percent >= 0.0);
        assert!(percent < 100.0); // 少量消息应该远低于 100%
    }

    #[test]
    fn test_compact_threshold_clamping() {
        let counter = Arc::new(EstimateTokenCounter::new());
        
        // 测试阈值被限制在 0-1 范围内
        let compactor = SessionCompactor::with_config(
            counter.clone(),
            10000,
            1.5, // 超过 1
            4,
        );
        assert_eq!(compactor.compact_threshold(), 1.0);

        let compactor = SessionCompactor::with_config(
            counter,
            10000,
            -0.5, // 小于 0
            4,
        );
        assert_eq!(compactor.compact_threshold(), 0.0);
    }
}
