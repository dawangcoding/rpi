pub mod types;
pub mod agent;
pub mod agent_loop;
pub mod context_manager;

#[cfg(test)]
pub mod test_fixtures;

// 重导出核心类型
pub use types::{
    AgentContext, AgentEvent, AgentMessage, AgentState, AgentTool, AgentToolResult,
    AfterToolCallResult, BeforeToolCallResult, QueueMode, PendingMessageQueue,
    ToolCallContext, ToolExecutionMode,
};

pub use agent::{Agent, AgentOptions, default_convert_to_llm};
pub use agent_loop::{AgentLoopConfig, run_agent_loop, run_agent_loop_continue};
pub use context_manager::{ContextWindowManager, ContextUsage};
