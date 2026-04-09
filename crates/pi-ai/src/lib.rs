pub mod types;
pub mod api_registry;
pub mod stream;
pub mod models;
pub mod providers;
pub mod utils;

// 重导出核心类型，方便直接使用
pub use types::{
    Api, AssistantMessage, AssistantMessageEvent, CacheRetention, ContentBlock, Context,
    DoneReason, ErrorReason, ImageContent, InputModality, Message, Model, ModelCost, Provider,
    SimpleStreamOptions, StopReason, StreamOptions, TextContent, ThinkingBudgets, ThinkingContent,
    ThinkingLevel, Tool, ToolCall, ToolResultMessage, Transport, Usage, UserContent, UserMessage,
};

// 重导出 API 注册表
pub use api_registry::{
    ApiProvider, ApiRegistry, 
    register_api_provider, get_api_provider, has_api_provider,
    get_all_api_providers, clear_api_providers, resolve_api_provider,
};

// 重导出流式 API
pub use stream::{
    stream, stream_simple, complete, complete_simple,
    stream_by_model_id, complete_by_model_id,
};

// 重导出模型相关函数
pub use models::{
    get_model, get_models, get_models_by_provider, get_models_by_api,
    calculate_cost, supports_xhigh, models_are_equal,
    get_api_key_from_env, get_api_key_env_var,
};

// 重导出工具模块
pub use utils::{
    event_stream::{SseEvent, SseParser, parse_sse_line, parse_json_stream_events},
    json_parse::{parse_partial_json, IncrementalJsonParser, StreamingJsonParser},
};
