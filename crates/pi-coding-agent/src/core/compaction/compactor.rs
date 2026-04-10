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

impl CompactionResult {
    /// 计算节省的 token 百分比
    pub fn savings_percentage(&self) -> f64 {
        if self.original_tokens == 0 {
            return 0.0;
        }
        (1.0 - (self.compacted_tokens as f64 / self.original_tokens as f64)) * 100.0
    }
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
            .map(|m| match m {
                AgentMessage::Llm(msg) => msg.clone(),
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
            .map(|m| match m {
                AgentMessage::Llm(msg) => msg.clone(),
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
            .map(|m| match m {
                AgentMessage::Llm(msg) => msg.clone(),
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
    /// 策略：基于 token 预算动态确定保留数量，保留第一条消息（通常是系统消息）
    fn determine_compress_range(&self, messages: &[AgentMessage]) -> (usize, usize) {
        if messages.len() < 4 {
            return (0, 0); // 太少，不压缩
        }

        // 目标: 压缩后保留的 token 数约为 context_window 的 50%
        let target_retained_tokens = (self.context_window_size as f64 * 0.5) as usize;

        // 从最新消息向前累计 token，确定保留起始位置
        let mut retained_tokens = 0;
        let mut retain_start = messages.len();

        for i in (1..messages.len()).rev() { // 跳过 index 0（通常是系统消息）
            if let Some(msg) = self.get_llm_message(&messages[i]) {
                let msg_tokens = self.token_counter.count_message(msg);

                if retained_tokens + msg_tokens > target_retained_tokens {
                    break;
                }

                retained_tokens += msg_tokens;
                retain_start = i;
            }
        }

        // 确保至少保留最近 2 轮（4 条消息）
        let min_retain = messages.len().saturating_sub(4);
        if retain_start > min_retain {
            retain_start = min_retain;
        }

        // 确保至少保留 index 0（系统消息）
        // 压缩范围: [1, retain_start)
        let compress_start = 1; // 跳过第一条消息（系统提示）
        let compress_end = retain_start;

        if compress_end <= compress_start + 1 {
            return (0, 0); // 没有足够的消息可压缩
        }

        // 对齐到完整的 turn 边界，避免切割 ToolCall 和 ToolResult 对
        let compress_end = self.align_to_turn_boundary(messages, compress_end);

        if compress_end <= compress_start + 1 {
            return (0, 0);
        }

        (compress_start, compress_end)
    }

    /// 确保压缩边界不会切割 ToolCall 和 ToolResult 对
    /// 返回调整后的 end 索引，确保不会在 ToolCall 和 ToolResult 之间切割
    fn align_to_turn_boundary(&self, messages: &[AgentMessage], end: usize) -> usize {
        // 边界检查
        if end == 0 || end > messages.len() {
            return end;
        }

        let mut aligned_end = end;

        // 情况1: end 指向 ToolResult，需要向前移动，跳过整个 ToolCall + ToolResult 对
        // 这样压缩范围就不会包含不完整的工具调用对
        if self.is_tool_result(&messages[aligned_end - 1]) {
            // 向前跳过所有连续的 ToolResult
            while aligned_end > 1 && self.is_tool_result(&messages[aligned_end - 1]) {
                aligned_end -= 1;
            }
            // 现在 aligned_end 指向第一个 ToolResult 之前的位置
            // 需要再向前跳过包含 ToolCall 的 assistant 消息
            if aligned_end > 1 && self.has_tool_calls(&messages[aligned_end - 1]) {
                aligned_end -= 1;
            }
            return aligned_end;
        }

        // 情况2: end 指向包含 ToolCall 的 assistant 消息之后的位置
        // 需要向后移动，包含所有对应的 ToolResult
        if aligned_end > 0 && self.has_tool_calls(&messages[aligned_end - 1]) {
            // 向后包含所有连续的 ToolResult
            while aligned_end < messages.len() && self.is_tool_result(&messages[aligned_end]) {
                aligned_end += 1;
            }
        }

        aligned_end
    }

    /// 检查消息是否是 ToolResult 类型
    fn is_tool_result(&self, message: &AgentMessage) -> bool {
        matches!(message, AgentMessage::Llm(Message::ToolResult(_)))
    }

    /// 检查消息是否包含 ToolCall
    fn has_tool_calls(&self, message: &AgentMessage) -> bool {
        if let AgentMessage::Llm(Message::Assistant(assistant)) = message {
            assistant.content.iter().any(|block| matches!(block, ContentBlock::ToolCall(_)))
        } else {
            false
        }
    }

    /// 获取 AgentMessage 内部的 LLM Message 引用
    fn get_llm_message<'a>(&self, message: &'a AgentMessage) -> Option<&'a Message> {
        match message {
            AgentMessage::Llm(msg) => Some(msg),
        }
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
    use pi_ai::types::{AssistantMessage, ToolCall, ToolResultMessage, TextContent, Api, Provider};

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
        // 使用非常小的 context window 确保触发压缩逻辑
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 100); // 100 tokens window

        // 消息太少（少于 4 条），不压缩
        let messages = create_test_messages(3);
        let range = compactor.determine_compress_range(&messages);
        assert_eq!(range, (0, 0));

        // 足够消息，可以压缩 - 小窗口会触发压缩
        let messages = create_test_messages(20);
        let range = compactor.determine_compress_range(&messages);
        // 验证基本约束
        assert!(range.0 >= 1, "Should skip first message");
        assert!(range.1 <= messages.len() - 4, "Should preserve at least last 4 messages");
        assert!(range.1 > range.0 + 1, "Should have messages to compress");
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
        // 使用非常小的 context window 测试动态保留
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 100); // 很小的窗口

        // 创建足够长的消息
        let messages = create_test_messages(20);
        let range = compactor.determine_compress_range(&messages);

        // 应该压缩，且保留第一条消息
        assert!(range.0 >= 1);
        // 动态算法确保至少保留最后 4 条
        assert!(range.1 <= messages.len() - 4 || range.1 == messages.len());
    }

    #[test]
    fn test_compaction_preserves_recent_messages() {
        // 使用小窗口确保触发压缩
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 100);

        // 创建 20 条消息
        let messages = create_test_messages(20);
        let range = compactor.determine_compress_range(&messages);

        // 验证保留第一条消息
        assert!(range.0 >= 1);

        // 验证保留最近的消息（动态算法保证至少保留最后 4 条）
        assert!(range.1 <= messages.len() - 4 || range.1 == messages.len());

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

        // 验证最近的消息还在
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

    #[test]
    fn test_savings_percentage() {
        let summary_message = AgentMessage::user("Summary");
        let record = CompactionRecord {
            compacted_at: 0,
            removed_message_range: (1, 5),
            summary_tokens: 100,
            original_tokens: 1000,
        };
        let result = CompactionResult {
            summary_message,
            removed_count: 4,
            original_tokens: 1000,
            compacted_tokens: 100,
            record,
        };

        // 100/1000 = 10%, savings = 90%
        assert!((result.savings_percentage() - 90.0).abs() < 0.01);

        // 测试零 token 情况
        let summary_message = AgentMessage::user("Summary");
        let record = CompactionRecord {
            compacted_at: 0,
            removed_message_range: (1, 5),
            summary_tokens: 0,
            original_tokens: 0,
        };
        let result = CompactionResult {
            summary_message,
            removed_count: 4,
            original_tokens: 0,
            compacted_tokens: 0,
            record,
        };
        assert_eq!(result.savings_percentage(), 0.0);
    }

    #[test]
    fn test_align_to_turn_boundary() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        // 创建包含 ToolCall 和 ToolResult 的消息序列
        let mut messages = Vec::new();
        
        // 用户消息
        messages.push(AgentMessage::user("Hello"));
        // 包含 ToolCall 的助手消息
        let mut assistant_msg = AssistantMessage::new(Api::Anthropic, Provider::Anthropic, "claude-3");
        assistant_msg.content.push(ContentBlock::ToolCall(ToolCall::new(
            "call_1",
            "read",
            serde_json::json!({"file_path": "/test.rs"})
        )));
        messages.push(AgentMessage::Llm(Message::Assistant(assistant_msg)));
        
        // ToolResult 消息
        messages.push(AgentMessage::Llm(Message::ToolResult(ToolResultMessage::new(
            "call_1",
            "read",
            vec![ContentBlock::Text(TextContent::new("file content"))]
        ))));
        
        // 更多用户消息
        messages.push(AgentMessage::user("Continue"));
        messages.push(AgentMessage::user("Done"));

        // 测试边界对齐在 ToolResult 处
        // 如果 end 指向 ToolResult（index 2），应该向前移动到 ToolCall 之前（index 1）
        let aligned = compactor.align_to_turn_boundary(&messages, 3); // end=3 表示包含 index 0,1,2
        // 应该跳过整个 ToolCall + ToolResult 对，返回到 index 1 之前
        assert_eq!(aligned, 1); // 应该返回到 index 1（ToolCall 消息）之前

        // 测试边界对齐在 ToolCall 后
        // 如果 end 指向 assistant with ToolCall 之后（index 2），应该包含对应的 ToolResult
        let aligned = compactor.align_to_turn_boundary(&messages, 2); // end=2 表示包含 index 0,1
        // 应该包含对应的 ToolResult（index 2）
        assert_eq!(aligned, 3); // 应该包含到 ToolResult 结束
    }

    #[test]
    fn test_is_tool_result() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        // 用户消息不是 ToolResult
        let user_msg = AgentMessage::user("Hello");
        assert!(!compactor.is_tool_result(&user_msg));

        // ToolResult 消息
        let tool_result = AgentMessage::Llm(Message::ToolResult(ToolResultMessage::new(
            "call_1",
            "read",
            vec![ContentBlock::Text(TextContent::new("result"))]
        )));
        assert!(compactor.is_tool_result(&tool_result));
    }

    #[test]
    fn test_has_tool_calls() {
        let counter = Arc::new(EstimateTokenCounter::new());
        let compactor = SessionCompactor::new(counter, 10000);

        // 用户消息没有 ToolCall
        let user_msg = AgentMessage::user("Hello");
        assert!(!compactor.has_tool_calls(&user_msg));

        // 不包含 ToolCall 的助手消息
        let mut assistant_no_tools = AssistantMessage::new(Api::Anthropic, Provider::Anthropic, "claude-3");
        assistant_no_tools.content.push(ContentBlock::Text(TextContent::new("Hello")));
        assert!(!compactor.has_tool_calls(&AgentMessage::Llm(Message::Assistant(assistant_no_tools))));

        // 包含 ToolCall 的助手消息
        let mut assistant_with_tools = AssistantMessage::new(Api::Anthropic, Provider::Anthropic, "claude-3");
        assistant_with_tools.content.push(ContentBlock::ToolCall(ToolCall::new(
            "call_1",
            "read",
            serde_json::json!({})
        )));
        assert!(compactor.has_tool_calls(&AgentMessage::Llm(Message::Assistant(assistant_with_tools))));
    }
}
