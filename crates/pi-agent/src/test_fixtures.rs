//! Agent 测试辅助模块
//!
//! 提供测试用的 fixtures 和辅助函数

#[cfg(test)]
pub mod fixtures {
    use crate::types::*;
    use async_trait::async_trait;
    use pi_ai::types::*;
    use std::sync::Arc;

    /// 模拟工具 - 返回固定结果
    pub struct MockTool {
        pub name: String,
        pub result: String,
    }

    impl MockTool {
        /// 创建一个新的 MockTool
        pub fn new(name: impl Into<String>, result: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                result: result.into(),
            }
        }
    }

    #[async_trait]
    impl AgentTool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn label(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock tool for testing"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input parameter"
                    }
                },
                "required": ["input"]
            })
        }

        async fn execute(
            &self,
            _tool_call_id: &str,
            _params: serde_json::Value,
            _cancel: tokio_util::sync::CancellationToken,
            _on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
        ) -> anyhow::Result<AgentToolResult> {
            Ok(AgentToolResult {
                content: vec![ContentBlock::Text(TextContent::new(&self.result))],
                details: serde_json::json!({"mock": true}),
            })
        }
    }

    /// 模拟错误工具 - 返回错误结果
    pub struct MockErrorTool {
        pub name: String,
        pub error_message: String,
    }

    impl MockErrorTool {
        /// 创建一个新的 MockErrorTool
        pub fn new(name: impl Into<String>, error_message: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                error_message: error_message.into(),
            }
        }
    }

    #[async_trait]
    impl AgentTool for MockErrorTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn label(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock error tool for testing"
        }

        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }

        async fn execute(
            &self,
            _tool_call_id: &str,
            _params: serde_json::Value,
            _cancel: tokio_util::sync::CancellationToken,
            _on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
        ) -> anyhow::Result<AgentToolResult> {
            Err(anyhow::anyhow!("{}", self.error_message))
        }
    }

    /// 创建测试用的 AgentContext
    pub fn sample_agent_context() -> AgentContext {
        AgentContext {
            system_prompt: "You are a helpful assistant".to_string(),
            messages: vec![AgentMessage::user("Hello")],
            tools: vec![],
        }
    }

    /// 创建测试用的 AgentContext，带工具
    pub fn sample_agent_context_with_tools(tools: Vec<Arc<dyn AgentTool>>) -> AgentContext {
        AgentContext {
            system_prompt: "You are a helpful assistant".to_string(),
            messages: vec![AgentMessage::user("Hello")],
            tools,
        }
    }

    /// 创建测试用的消息列表
    pub fn sample_conversation() -> Vec<AgentMessage> {
        vec![
            AgentMessage::user("Hello, can you help me?"),
            AgentMessage::user("I need some assistance with a task."),
        ]
    }

    /// 创建包含工具调用的对话
    pub fn sample_conversation_with_tool_call() -> Vec<AgentMessage> {
        vec![
            AgentMessage::user("Search for something"),
            AgentMessage::Llm(Message::Assistant(AssistantMessage {
                role: "assistant".to_string(),
                content: vec![ContentBlock::ToolCall(ToolCall::new(
                    "call_123",
                    "search",
                    serde_json::json!({"query": "test"}),
                ))],
                api: Api::Anthropic,
                provider: Provider::Anthropic,
                model: "claude-3-sonnet".to_string(),
                response_id: Some("msg_123".to_string()),
                usage: Usage::default(),
                stop_reason: StopReason::ToolUse,
                error_message: None,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            })),
        ]
    }

    /// 创建测试用的 AgentState
    pub fn sample_agent_state() -> AgentState {
        let model = Model {
            id: "claude-3-sonnet-20240229".to_string(),
            name: "Claude 3 Sonnet".to_string(),
            api: Api::Anthropic,
            provider: Provider::Anthropic,
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            },
            context_window: 200000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        };

        AgentState::new(model)
    }

    /// 创建测试用的 AgentState，带工具
    pub fn sample_agent_state_with_tools(tools: Vec<Arc<dyn AgentTool>>) -> AgentState {
        let mut state = sample_agent_state();
        state.tools = tools;
        state
    }

    /// 创建 AgentToolResult
    pub fn sample_tool_result(content: &str) -> AgentToolResult {
        AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(content))],
            details: serde_json::json!({}),
        }
    }

    /// 创建错误 AgentToolResult
    pub fn sample_error_tool_result(error: &str) -> AgentToolResult {
        AgentToolResult::error(error)
    }

    /// 创建待处理消息队列
    pub fn sample_pending_queue(mode: QueueMode) -> PendingMessageQueue {
        PendingMessageQueue::new(mode)
    }

    /// 创建 ToolCallContext
    pub fn sample_tool_call_context() -> ToolCallContext {
        ToolCallContext {
            assistant_message: AssistantMessage::new(Api::Anthropic, Provider::Anthropic, "claude-3-sonnet"),
            tool_call: ToolCall::new("call_123", "test_tool", serde_json::json!({})),
            args: serde_json::json!({"input": "test"}),
        }
    }

    /// 创建 BeforeToolCallResult
    pub fn sample_before_tool_call_result_allow() -> BeforeToolCallResult {
        BeforeToolCallResult::default()
    }

    /// 创建 BeforeToolCallResult (blocked)
    pub fn sample_before_tool_call_result_block(reason: &str) -> BeforeToolCallResult {
        BeforeToolCallResult::blocked(reason)
    }

    /// 创建 AfterToolCallResult
    pub fn sample_after_tool_call_result(content: Vec<ContentBlock>) -> AfterToolCallResult {
        AfterToolCallResult {
            content: Some(content),
            details: None,
            is_error: Some(false),
        }
    }

    /// 创建 AgentEvent::AgentStart
    pub fn sample_event_agent_start() -> AgentEvent {
        AgentEvent::AgentStart
    }

    /// 创建 AgentEvent::TurnStart
    pub fn sample_event_turn_start() -> AgentEvent {
        AgentEvent::TurnStart
    }

    /// 创建 AgentEvent::MessageStart
    pub fn sample_event_message_start() -> AgentEvent {
        AgentEvent::MessageStart {
            message: AgentMessage::user("Test message"),
        }
    }

    /// 创建 AgentEvent::ToolExecutionStart
    pub fn sample_event_tool_execution_start(tool_name: &str) -> AgentEvent {
        AgentEvent::ToolExecutionStart {
            tool_call_id: "call_123".to_string(),
            tool_name: tool_name.to_string(),
            args: serde_json::json!({"input": "test"}),
        }
    }

    /// 创建一组常用的 mock 工具
    pub fn sample_mock_tools() -> Vec<Arc<dyn AgentTool>> {
        vec![
            Arc::new(MockTool::new("search", "Search result: found 5 items")),
            Arc::new(MockTool::new("calculator", "42")),
            Arc::new(MockTool::new("weather", "Sunny, 25°C")),
        ]
    }

    /// 创建 LLM 上下文（用于测试转换）
    pub fn sample_llm_context() -> Context {
        Context::new(vec![
            Message::User(UserMessage::new("Hello")),
            Message::Assistant(AssistantMessage::new(Api::Anthropic, Provider::Anthropic, "claude-3-sonnet")),
        ])
        .with_system_prompt("You are a helpful assistant")
    }

    /// 创建 LLM 工具定义
    pub fn sample_llm_tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "search",
                "Search for information",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }),
            ),
            Tool::new(
                "calculator",
                "Perform calculations",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {"type": "string"}
                    },
                    "required": ["expression"]
                }),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use crate::types::AgentTool;
    use crate::QueueMode;

    #[test]
    fn test_mock_tool() {
        let tool = MockTool::new("test", "result");
        assert_eq!(tool.name(), "test");
        assert_eq!(tool.label(), "test");
    }

    #[test]
    fn test_mock_error_tool() {
        let tool = MockErrorTool::new("error_tool", "Something went wrong");
        assert_eq!(tool.name(), "error_tool");
    }

    #[test]
    fn test_sample_agent_context() {
        let ctx = sample_agent_context();
        assert_eq!(ctx.system_prompt, "You are a helpful assistant");
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn test_sample_agent_state() {
        let state = sample_agent_state();
        // AgentState::new 创建时 system_prompt 为空，model 不为空
        assert!(!state.model.id.is_empty());
        assert_eq!(state.messages.len(), 0);
    }

    #[test]
    fn test_sample_conversation() {
        let conv = sample_conversation();
        assert_eq!(conv.len(), 2);
    }

    #[test]
    fn test_sample_pending_queue() {
        let queue = sample_pending_queue(QueueMode::OneAtATime);
        assert!(!queue.has_items());
    }

    #[test]
    fn test_sample_tool_result() {
        let result = sample_tool_result("Test result");
        assert!(!result.content.is_empty());
    }

    #[test]
    fn test_sample_mock_tools() {
        let tools = sample_mock_tools();
        assert_eq!(tools.len(), 3);
    }
}
