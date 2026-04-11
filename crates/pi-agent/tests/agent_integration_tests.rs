//! Agent 集成测试
//!
//! 测试 Agent 模块的集成行为和公共 API

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::sync::CancellationToken;

use pi_agent::{
    Agent, AgentOptions, AgentMessage, AgentEvent, AgentTool, AgentToolResult,
    AgentLoopConfig,
    ToolExecutionMode, QueueMode, ToolCallContext, BeforeToolCallResult, AfterToolCallResult,
    AgentContext,
};
use pi_ai::types::*;

// ============== Agent 创建和配置测试 ==============

/// 测试 Agent 使用默认配置创建
#[test]
fn test_agent_creation_with_defaults() {
    let options = AgentOptions::default();
    let _agent = Agent::new(options);
    
    // Agent 应该成功创建
    // 由于字段是私有的，我们通过后续操作验证
}

/// 测试 Agent 创建带完整配置
#[test]
fn test_agent_creation_with_full_config() {
    let model = Model {
        id: "test-model".to_string(),
        name: "Test Model".to_string(),
        api: Api::Anthropic,
        provider: Provider::Anthropic,
        base_url: "https://test.com".to_string(),
        reasoning: false,
        input: vec![InputModality::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: None,
            cache_write: None,
        },
        context_window: 100000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    };
    
    let options = AgentOptions {
        model: Some(model),
        system_prompt: Some("You are a test assistant".to_string()),
        tools: vec![],
        thinking_level: ThinkingLevel::Medium,
        thinking_budgets: None,
        transport: None,
        tool_execution: ToolExecutionMode::Sequential,
        session_id: Some("test-session".to_string()),
        max_retry_delay_ms: Some(5000),
        convert_to_llm: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        steering_mode: QueueMode::All,
        follow_up_mode: QueueMode::OneAtATime,
    };
    
    let _agent = Agent::new(options);
}

/// 测试 Agent 状态快照
#[tokio::test]
async fn test_agent_state_snapshot() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    let state = agent.state().await;
    
    // 验证初始状态
    assert!(!state.is_streaming);
    assert!(state.streaming_message.is_none());
    assert!(state.pending_tool_calls.is_empty());
    assert!(state.error_message.is_none());
}

// ============== 消息队列管理测试 ==============

/// 测试 steering 队列操作
#[tokio::test]
async fn test_steering_queue_operations() {
    let options = AgentOptions {
        steering_mode: QueueMode::OneAtATime,
        ..Default::default()
    };
    let agent = Agent::new(options);
    
    // 初始状态应该没有队列消息
    assert!(!agent.has_queued_messages().await);
    
    // 添加 steering 消息
    agent.steer(AgentMessage::user("Steering message 1")).await;
    agent.steer(AgentMessage::user("Steering message 2")).await;
    
    // 验证队列有消息
    assert!(agent.has_queued_messages().await);
    
    // 清除 steering 队列
    agent.clear_steering_queue().await;
    
    // 清除后应该没有消息（假设 follow_up 队列也是空的）
    // 注意：follow_up_queue 可能仍然有消息
}

/// 测试 follow_up 队列操作
#[tokio::test]
async fn test_follow_up_queue_operations() {
    let options = AgentOptions {
        follow_up_mode: QueueMode::OneAtATime,
        ..Default::default()
    };
    let agent = Agent::new(options);
    
    // 添加 follow_up 消息
    agent.follow_up(AgentMessage::user("Follow up message")).await;
    
    // 验证队列有消息
    assert!(agent.has_queued_messages().await);
    
    // 清除 follow_up 队列
    agent.clear_follow_up_queue().await;
}

/// 测试清除所有队列
#[tokio::test]
async fn test_clear_all_queues() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    // 添加消息到两个队列
    agent.steer(AgentMessage::user("Steering")).await;
    agent.follow_up(AgentMessage::user("Follow up")).await;
    
    // 验证队列有消息
    assert!(agent.has_queued_messages().await);
    
    // 清除所有队列
    agent.clear_all_queues().await;
    
    // 验证队列为空
    assert!(!agent.has_queued_messages().await);
}

// ============== 事件订阅测试 ==============

/// 测试事件订阅和通知
#[tokio::test]
async fn test_event_subscription() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    let event_count = Arc::new(AtomicUsize::new(0));
    let event_count_clone = event_count.clone();
    
    let listener: Arc<dyn Fn(AgentEvent, CancellationToken) + Send + Sync> = 
        Arc::new(move |_event, _cancel| {
            event_count_clone.fetch_add(1, Ordering::SeqCst);
        });
    
    let _unsubscribe = agent.subscribe(listener);
    
    // 给订阅一点时间注册
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    // 验证订阅成功（通过后续操作间接验证）
    // 由于事件由内部触发，这里主要验证订阅接口可用
}

/// 测试取消订阅
#[tokio::test]
async fn test_event_unsubscribe() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    let listener: Arc<dyn Fn(AgentEvent, CancellationToken) + Send + Sync> = 
        Arc::new(|_event, _cancel| {});
    
    let unsubscribe = agent.subscribe(listener.clone());
    
    // 给订阅一点时间注册
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    // 取消订阅
    unsubscribe();
    
    // 给取消订阅一点时间生效
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    // 验证取消订阅成功（通过不 panic 来验证）
}

// ============== Agent 状态管理测试 ==============

/// 测试 Agent 重置
#[tokio::test]
async fn test_agent_reset() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    // 添加一些队列消息
    agent.steer(AgentMessage::user("Test")).await;
    
    // 重置
    agent.reset().await;
    
    // 验证状态被重置
    let state = agent.state().await;
    assert!(state.messages.is_empty());
    assert!(!state.is_streaming);
    assert!(state.streaming_message.is_none());
    assert!(state.pending_tool_calls.is_empty());
    assert!(state.error_message.is_none());
    assert!(!agent.has_queued_messages().await);
}

/// 测试 Agent 中止操作
#[tokio::test]
async fn test_agent_abort() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    // 中止没有活动的操作应该不会 panic
    agent.abort().await;
    
    // 验证 cancel token 被清除
    let token = agent.cancel_token().await;
    assert!(token.is_none());
}

/// 测试等待空闲
#[tokio::test]
async fn test_wait_for_idle() {
    let options = AgentOptions::default();
    let agent = Agent::new(options);
    
    // 当没有活动操作时，wait_for_idle 应该立即返回
    let timeout = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        agent.wait_for_idle()
    ).await;
    
    assert!(timeout.is_ok(), "wait_for_idle should return immediately when not streaming");
}

// ============== 工具相关测试 ==============

/// 测试工具注册
#[test]
fn test_tool_registration() {
    // 使用简单的 mock 工具测试注册
    use async_trait::async_trait;
    
    struct SimpleMockTool;
    
    #[async_trait]
    impl AgentTool for SimpleMockTool {
        fn name(&self) -> &str { "mock_tool" }
        fn label(&self) -> &str { "Mock Tool" }
        fn description(&self) -> &str { "A mock tool for testing" }
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
            Ok(AgentToolResult {
                content: vec![],
                details: serde_json::json!({}),
            })
        }
    }
    
    let tools: Vec<Arc<dyn AgentTool>> = vec![
        Arc::new(SimpleMockTool),
    ];
    
    let options = AgentOptions {
        tools,
        ..Default::default()
    };
    
    let _agent = Agent::new(options);
}

/// 测试工具名称唯一性
#[test]
fn test_tool_names_unique() {
    // 使用简单的 mock 工具测试名称唯一性
    let names = vec!["search", "calculator", "weather"];
    
    let mut unique_names = std::collections::HashSet::new();
    for name in &names {
        assert!(
            unique_names.insert(*name),
            "Tool name {} should be unique",
            name
        );
    }
}

/// 测试工具参数 schema
#[test]
fn test_tool_parameter_schemas() {
    use async_trait::async_trait;
    
    struct SchemaTestTool;
    
    #[async_trait]
    impl AgentTool for SchemaTestTool {
        fn name(&self) -> &str { "schema_test_tool" }
        fn label(&self) -> &str { "Schema Test Tool" }
        fn description(&self) -> &str { "A tool for testing schema" }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
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
                content: vec![],
                details: serde_json::json!({}),
            })
        }
    }
    
    let tool = SchemaTestTool;
    let schema = tool.parameters();
    
    // 验证 schema 是有效的 JSON 对象
    assert!(schema.is_object(), "Parameters should be a JSON object");
    
    // 验证有 type 字段
    assert!(schema.get("type").is_some(), "Parameters should have a 'type' field");
    
    // 验证有 properties 字段
    assert!(schema.get("properties").is_some(), "Parameters should have a 'properties' field");
}

// ============== AgentLoop 配置测试 ==============

/// 测试 AgentLoopConfig 创建
#[test]
fn test_agent_loop_config_creation() {
    let model = Model {
        id: "test-model".to_string(),
        name: "Test Model".to_string(),
        api: Api::Anthropic,
        provider: Provider::Anthropic,
        base_url: "https://test.com".to_string(),
        reasoning: false,
        input: vec![InputModality::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: None,
            cache_write: None,
        },
        context_window: 100000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    };
    
    let config = AgentLoopConfig {
        model,
        thinking_level: ThinkingLevel::Medium,
        thinking_budgets: None,
        temperature: Some(0.7),
        max_tokens: Some(4096),
        transport: None,
        cache_retention: None,
        session_id: Some("test-session".to_string()),
        max_retry_delay_ms: Some(5000),
        context_manager: None,
        convert_to_llm: Arc::new(pi_agent::default_convert_to_llm),
        transform_context: None,
        get_api_key: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: ToolExecutionMode::Parallel,
        before_tool_call: None,
        after_tool_call: None,
    };
    
    assert_eq!(config.thinking_level, ThinkingLevel::Medium);
    assert_eq!(config.temperature, Some(0.7));
    assert_eq!(config.max_tokens, Some(4096));
    assert_eq!(config.session_id, Some("test-session".to_string()));
}

// ============== AgentContext 测试 ==============

/// 测试 AgentContext 创建和快照
#[test]
fn test_agent_context_creation() {
    let context = AgentContext {
        system_prompt: "You are a helpful assistant".to_string(),
        messages: vec![AgentMessage::user("Hello")],
        tools: vec![],
    };
    
    assert_eq!(context.system_prompt, "You are a helpful assistant");
    assert_eq!(context.messages.len(), 1);
    assert!(context.tools.is_empty());
    
    // 测试快照
    let snapshot = context.snapshot();
    assert_eq!(snapshot.system_prompt, context.system_prompt);
    assert_eq!(snapshot.messages.len(), context.messages.len());
}

/// 测试 AgentContext 克隆
#[test]
fn test_agent_context_clone() {
    let context = AgentContext {
        system_prompt: "Test".to_string(),
        messages: vec![AgentMessage::user("Hello")],
        tools: vec![],
    };
    
    let cloned = context.clone();
    assert_eq!(cloned.system_prompt, context.system_prompt);
    assert_eq!(cloned.messages.len(), context.messages.len());
}

// ============== 消息类型测试 ==============

/// 测试 AgentMessage 创建
#[test]
fn test_agent_message_creation() {
    let user_msg = AgentMessage::user("Hello, world!");
    
    // 验证消息角色
    assert_eq!(user_msg.role(), "user");
    
    // 验证可以获取内部消息
    assert!(user_msg.as_message().is_some());
}

/// 测试 AgentMessage 带图片
#[test]
fn test_agent_message_with_images() {
    let images = vec![
        ImageContent::new("base64encodeddata", "image/png"),
    ];
    
    let msg = AgentMessage::user_with_images("Describe this image", images);
    assert_eq!(msg.role(), "user");
}

// ============== 工具执行模式测试 ==============

/// 测试工具执行模式枚举
#[test]
fn test_tool_execution_modes() {
    let sequential = ToolExecutionMode::Sequential;
    let parallel = ToolExecutionMode::Parallel;
    
    // 验证它们是不同的值
    assert_ne!(
        std::mem::discriminant(&sequential),
        std::mem::discriminant(&parallel)
    );
}

/// 测试队列模式枚举
#[test]
fn test_queue_modes() {
    let all = QueueMode::All;
    let one_at_a_time = QueueMode::OneAtATime;
    
    // 验证它们是不同的值
    assert_ne!(
        std::mem::discriminant(&all),
        std::mem::discriminant(&one_at_a_time)
    );
}

// ============== 回调钩子测试 ==============

/// 测试 before_tool_call 钩子配置
#[test]
fn test_before_tool_call_hook() {
    // 使用类型别名简化复杂类型
    type BeforeHookFn = Arc<
        dyn Fn(&ToolCallContext, CancellationToken) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<BeforeToolCallResult>> + Send>> + Send + Sync,
    >;
    
    let hook: BeforeHookFn = Arc::new(|_ctx, _cancel| {
        Box::pin(async move { None })
    });
    
    let options = AgentOptions {
        before_tool_call: Some(hook),
        ..Default::default()
    };
    
    let _agent = Agent::new(options);
}

/// 测试 after_tool_call 钩子配置
#[test]
fn test_after_tool_call_hook() {
    // 使用类型别名简化复杂类型
    type AfterHookFn = Arc<
        dyn Fn(&ToolCallContext, &AgentToolResult, bool, CancellationToken) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<AfterToolCallResult>> + Send>> + Send + Sync,
    >;
    
    let hook: AfterHookFn = Arc::new(|_ctx, _result, _is_error, _cancel| {
        Box::pin(async move { None })
    });
    
    let options = AgentOptions {
        after_tool_call: Some(hook),
        ..Default::default()
    };
    
    let _agent = Agent::new(options);
}

// ============== 边界条件测试 ==============

/// 测试空消息处理
#[test]
fn test_empty_messages_handling() {
    let context = AgentContext {
        system_prompt: "".to_string(),
        messages: vec![],
        tools: vec![],
    };
    
    assert!(context.messages.is_empty());
    
    // 转换空消息列表
    let llm_messages = pi_agent::default_convert_to_llm(&context.messages);
    assert!(llm_messages.is_empty());
}

/// 测试大量消息处理
#[test]
fn test_large_message_volume() {
    let mut messages = vec![];
    
    // 添加大量消息
    for i in 0..100 {
        messages.push(AgentMessage::user(&format!("Message {}", i)));
    }
    
    let context = AgentContext {
        system_prompt: "Test".to_string(),
        messages: messages.clone(),
        tools: vec![],
    };
    
    assert_eq!(context.messages.len(), 100);
    
    // 测试消息转换
    let llm_messages = pi_agent::default_convert_to_llm(&messages);
    assert_eq!(llm_messages.len(), 100);
}

/// 测试空工具列表
#[test]
fn test_empty_tool_list() {
    let context = AgentContext {
        system_prompt: "You are helpful".to_string(),
        messages: vec![AgentMessage::user("Hello")],
        tools: vec![],
    };
    
    assert!(context.tools.is_empty());
    assert_eq!(context.messages.len(), 1);
}
