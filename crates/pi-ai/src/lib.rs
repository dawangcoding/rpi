pub mod types;
pub mod api_registry;
pub mod stream;
pub mod models;
pub mod providers;
pub mod utils;
pub mod token_counter;

#[cfg(test)]
pub mod test_fixtures;

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

// 重导出 token 计数器
pub use token_counter::{TokenCounter, EstimateTokenCounter, ModelTokenCounter};

/// 初始化并注册所有内置 Provider
///
/// 在应用启动时调用此函数，将 Anthropic、OpenAI、Google 三个 Provider
/// 注册到全局 ApiRegistry 中。重复调用是安全的（会跳过已注册的情况）。
pub fn init_providers() {
    use std::sync::Arc;

    // 避免重复注册
    if has_api_provider(&Api::Anthropic) {
        return;
    }

    register_api_provider(Arc::new(providers::AnthropicProvider::new()));
    register_api_provider(Arc::new(providers::OpenAiProvider::new()));
    register_api_provider(Arc::new(providers::GoogleProvider::new()));
    register_api_provider(Arc::new(providers::MistralProvider::new()));
    register_api_provider(Arc::new(providers::BedrockProvider::new()));
    register_api_provider(Arc::new(providers::AzureOpenAiProvider::new()));
    register_api_provider(Arc::new(providers::XaiProvider::new()));
    register_api_provider(Arc::new(providers::OpenRouterProvider::new()));

    tracing::debug!("Registered 8 built-in providers: Anthropic, OpenAI(ChatCompletions), Google, Mistral, Bedrock, AzureOpenAI, XAI, OpenRouter");
}
