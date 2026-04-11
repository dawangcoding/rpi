//! AI 类型定义模块
//!
//! 包含消息、请求、响应等核心类型的定义

use serde::{Deserialize, Serialize};

/// API 类型枚举
/// 
/// 支持的 LLM API 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Api {
    Anthropic,
    #[serde(rename = "anthropic-messages")]
    AnthropicMessages,
    #[serde(rename = "openai-chat-completions")]
    OpenAiChatCompletions,
    #[serde(rename = "openai-completions")]
    OpenAiCompletions,
    #[serde(rename = "openai-responses")]
    OpenAiResponses,
    #[serde(rename = "azure-openai-responses")]
    AzureOpenAiResponses,
    #[serde(rename = "openai-codex-responses")]
    OpenAiCodexResponses,
    Google,
    #[serde(rename = "google-generative-ai")]
    GoogleGenerativeAi,
    #[serde(rename = "google-vertex")]
    GoogleVertex,
    #[serde(rename = "google-gemini-cli")]
    GoogleGeminiCli,
    #[serde(rename = "google-antigravity")]
    GoogleAntigravity,
    #[serde(rename = "mistral-conversations")]
    MistralConversations,
    Mistral,
    #[serde(rename = "amazon-bedrock")]
    AmazonBedrock,
    #[serde(rename = "bedrock-converse-stream")]
    BedrockConverseStream,
    Xai,
    Groq,
    Cerebras,
    Openrouter,
    #[serde(rename = "vercel-ai-gateway")]
    VercelAiGateway,
    Zai,
    Minimax,
    #[serde(rename = "minimax-cn")]
    MinimaxCn,
    Huggingface,
    Opencode,
    #[serde(rename = "opencode-go")]
    OpencodeGo,
    #[serde(rename = "kimi-coding")]
    KimiCoding,
    DeepSeek,
    Qwen,
    #[serde(untagged)]
    Other(String),
}

/// Provider 类型枚举
/// 
/// LLM 服务提供商
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Anthropic,
    Openai,
    Google,
    #[serde(rename = "google-gemini-cli")]
    GoogleGeminiCli,
    #[serde(rename = "google-vertex")]
    GoogleVertex,
    #[serde(rename = "google-antigravity")]
    GoogleAntigravity,
    Mistral,
    #[serde(rename = "amazon-bedrock")]
    AmazonBedrock,
    #[serde(rename = "azure-openai-responses")]
    AzureOpenAiResponses,
    #[serde(rename = "openai-codex")]
    OpenAiCodex,
    #[serde(rename = "github-copilot")]
    GithubCopilot,
    Xai,
    Groq,
    Cerebras,
    Openrouter,
    #[serde(rename = "vercel-ai-gateway")]
    VercelAiGateway,
    Zai,
    Minimax,
    #[serde(rename = "minimax-cn")]
    MinimaxCn,
    Huggingface,
    Opencode,
    #[serde(rename = "opencode-go")]
    OpencodeGo,
    #[serde(rename = "kimi-coding")]
    KimiCoding,
    DeepSeek,
    Qwen,
    #[serde(untagged)]
    Other(String),
}

/// 停止原因枚举
/// 
/// 助手消息生成停止的原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    Stop,
    Length,
    ToolUse,
    Error,
    Aborted,
}

/// 思考级别枚举
/// 
/// 控制模型思考/推理的深度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
}

/// 文本内容块
/// 
/// 表示消息中的文本片段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
    #[serde(rename = "textSignature", skip_serializing_if = "Option::is_none")]
    pub text_signature: Option<String>,
}

impl TextContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: text.into(),
            text_signature: None,
        }
    }
}

/// 思考内容块
/// 
/// 表示模型的思考过程
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub thinking: String,
    #[serde(rename = "thinkingSignature", skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted: Option<bool>,
}

impl ThinkingContent {
    pub fn new(thinking: impl Into<String>) -> Self {
        Self {
            content_type: "thinking".to_string(),
            thinking: thinking.into(),
            thinking_signature: None,
            redacted: None,
        }
    }
}

/// 图片内容块
/// 
/// 表示消息中的图片数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    /// base64 编码的图片数据
    pub data: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

impl ImageContent {
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            content_type: "image".to_string(),
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }
}

/// 工具调用
/// 
/// 表示助手请求调用的工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(rename = "type")]
    pub content_type: String,
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            content_type: "toolCall".to_string(),
            id: id.into(),
            name: name.into(),
            arguments,
            thought_signature: None,
        }
    }
}

/// 内容块枚举
/// 
/// 消息内容的组成部分
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text(TextContent),
    #[serde(rename = "thinking")]
    Thinking(ThinkingContent),
    #[serde(rename = "image")]
    Image(ImageContent),
    #[serde(rename = "toolCall")]
    ToolCall(ToolCall),
}

/// 用户内容
/// 
/// 可以是纯字符串或内容块数组
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl From<String> for UserContent {
    fn from(s: String) -> Self {
        UserContent::Text(s)
    }
}

impl From<&str> for UserContent {
    fn from(s: &str) -> Self {
        UserContent::Text(s.to_string())
    }
}

impl From<Vec<ContentBlock>> for UserContent {
    fn from(blocks: Vec<ContentBlock>) -> Self {
        UserContent::Blocks(blocks)
    }
}

/// Token 使用量
/// 
/// 记录 API 调用的 token 消耗统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    #[serde(rename = "inputTokens", alias = "input")]
    pub input_tokens: u64,
    #[serde(rename = "outputTokens", alias = "output")]
    pub output_tokens: u64,
    #[serde(rename = "cacheReadTokens", alias = "cacheRead", skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(rename = "cacheWriteTokens", alias = "cacheWrite", skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
}

/// 用户消息
/// 
/// 表示用户发送的消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    #[serde(skip)]
    pub role: String,
    pub content: UserContent,
    pub timestamp: i64,
}

impl UserMessage {
    pub fn new(content: impl Into<UserContent>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn with_timestamp(content: impl Into<UserContent>, timestamp: i64) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            timestamp,
        }
    }
}

/// 助手消息
/// 
/// 表示 AI 助手生成的回复消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    #[serde(skip)]
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub api: Api,
    pub provider: Provider,
    pub model: String,
    #[serde(rename = "responseId", skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    pub usage: Usage,
    #[serde(rename = "stopReason")]
    pub stop_reason: StopReason,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub timestamp: i64,
}

impl Default for AssistantMessage {
    fn default() -> Self {
        Self {
            role: "assistant".to_string(),
            content: Vec::new(),
            api: Api::Anthropic,
            provider: Provider::Anthropic,
            model: String::new(),
            response_id: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

impl AssistantMessage {
    pub fn new(api: Api, provider: Provider, model: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Vec::new(),
            api,
            provider,
            model: model.into(),
            response_id: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn with_content(mut self, content: Vec<ContentBlock>) -> Self {
        self.content = content;
        self
    }

    pub fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_stop_reason(mut self, stop_reason: StopReason) -> Self {
        self.stop_reason = stop_reason;
        self
    }

    pub fn with_error_message(mut self, error: impl Into<String>) -> Self {
        self.error_message = Some(error.into());
        self.stop_reason = StopReason::Error;
        self
    }
}

/// 工具结果消息
/// 
/// 表示工具执行后返回的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    #[serde(skip)]
    pub role: String,
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    #[serde(rename = "toolName")]
    pub tool_name: String,
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(rename = "isError")]
    pub is_error: bool,
    pub timestamp: i64,
}

impl ToolResultMessage {
    pub fn new(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: Vec<ContentBlock>,
    ) -> Self {
        Self {
            role: "toolResult".to_string(),
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            content,
            details: None,
            is_error: false,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_error(mut self, is_error: bool) -> Self {
        self.is_error = is_error;
        self
    }
}

/// 消息枚举
/// 
/// 对话中的消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "toolResult")]
    ToolResult(ToolResultMessage),
}

/// 工具定义
/// 
/// 描述可供模型调用的工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema 格式的参数定义
    pub parameters: serde_json::Value,
}

impl Tool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// 对话上下文
/// 
/// 包含系统提示词、消息历史和可用工具
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Context {
    #[serde(rename = "systemPrompt", skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

impl Context {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            system_prompt: None,
            messages,
            tools: None,
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }
}

/// 输入模态枚举
/// 
/// 模型支持的输入类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum InputModality {
    Text,
    Image,
}

/// 模型成本
/// 
/// 每百万 token 的定价信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    /// $/million tokens
    pub input: f64,
    /// $/million tokens
    pub output: f64,
    /// $/million tokens
    #[serde(rename = "cacheRead", skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    /// $/million tokens
    #[serde(rename = "cacheWrite", skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

/// 模型定义
/// 
/// 描述 LLM 模型的配置和元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub api: Api,
    pub provider: Provider,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub reasoning: bool,
    pub input: Vec<InputModality>,
    pub cost: ModelCost,
    #[serde(rename = "contextWindow")]
    pub context_window: u64,
    #[serde(rename = "maxTokens")]
    pub max_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<serde_json::Value>,
}

/// 传输方式枚举
/// 
/// API 数据传输协议
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Sse,
    Websocket,
    Auto,
}

/// 缓存保留策略枚举
/// 
/// 控制 prompt 缓存的保留时长
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CacheRetention {
    None,
    Short,
    Long,
}

/// 流选项
/// 
/// 配置流式 API 调用的参数
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    #[serde(rename = "cacheRetention", skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<CacheRetention>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "maxRetryDelayMs", skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(rename = "retryConfig", skip_serializing_if = "Option::is_none")]
    pub retry_config: Option<crate::retry::RetryConfig>,
}

/// 思考预算
/// 
/// 控制模型思考的 token 预算
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThinkingBudgets {
    #[serde(rename = "thinkingBudget", skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u64>,
    #[serde(rename = "planBudget", skip_serializing_if = "Option::is_none")]
    pub plan_budget: Option<u64>,
}

/// 简化流选项
/// 
/// 简化版的流式 API 配置参数
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimpleStreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    #[serde(rename = "cacheRetention", skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<CacheRetention>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "maxRetryDelayMs", skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ThinkingLevel>,
    #[serde(rename = "thinkingBudgets", skip_serializing_if = "Option::is_none")]
    pub thinking_budgets: Option<ThinkingBudgets>,
    #[serde(rename = "retryConfig", skip_serializing_if = "Option::is_none")]
    pub retry_config: Option<crate::retry::RetryConfig>,
}

/// 完成原因枚举
/// 
/// 消息生成正常完成的原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum DoneReason {
    Stop,
    Length,
    ToolUse,
}

/// 错误原因枚举
/// 
/// 消息生成异常终止的原因
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ErrorReason {
    Aborted,
    Error,
}

/// 助手消息事件枚举
/// 
/// 流式响应中的事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantMessageEvent {
    Start {
        partial: AssistantMessage,
    },
    TextStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        partial: AssistantMessage,
    },
    TextDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
        partial: AssistantMessage,
    },
    TextEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: String,
        partial: AssistantMessage,
    },
    ThinkingStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        partial: AssistantMessage,
    },
    ThinkingDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
        partial: AssistantMessage,
    },
    ThinkingEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        content: String,
        partial: AssistantMessage,
    },
    ToolCallStart {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        partial: AssistantMessage,
    },
    ToolCallDelta {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        delta: String,
        partial: AssistantMessage,
    },
    ToolCallEnd {
        #[serde(rename = "contentIndex")]
        content_index: usize,
        #[serde(rename = "toolCall")]
        tool_call: ToolCall,
        partial: AssistantMessage,
    },
    Done {
        reason: DoneReason,
        message: AssistantMessage,
    },
    Error {
        reason: ErrorReason,
        error: AssistantMessage,
    },
}
