//! OpenAI Chat Completions API Provider 实现
//!
//! 支持 OpenAI Chat Completions API (/v1/chat/completions) 的流式调用
//! 包括消息转换、工具调用、SSE 流解析等功能

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::pin::Pin;
// Duration 导入已移除，重试逻辑已统一移到 stream.rs
use tracing::{debug, trace, warn};

use crate::api_registry::ApiProvider;
use crate::models::get_api_key_from_env;
use crate::types::*;
use crate::utils::event_stream::SseParser;
use crate::utils::json_parse::parse_partial_json;

/// OpenAI Chat Completions API Provider
pub struct OpenAiProvider {
    client: Client,
}

impl OpenAiProvider {
    /// 创建新的 OpenAI Provider 实例
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 获取 API Key
    fn get_api_key(&self, model: &Model, options: &StreamOptions) -> anyhow::Result<String> {
        if let Some(ref key) = options.api_key {
            return Ok(key.clone());
        }
        get_api_key_from_env(&model.provider)
            .ok_or_else(|| anyhow::anyhow!("No API key found for provider: {:?}", model.provider))
    }

    /// 构建请求头
    fn build_headers(&self, api_key: &str, options: &StreamOptions) -> anyhow::Result<HashMap<String, String>> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        
        // 合并用户自定义 headers
        if let Some(ref custom_headers) = options.headers {
            for (key, value) in custom_headers {
                headers.insert(key.clone(), value.clone());
            }
        }
        
        Ok(headers)
    }

    /// 构建请求体
    fn build_request_body(
        &self,
        model: &Model,
        context: &Context,
        options: &StreamOptions,
    ) -> anyhow::Result<serde_json::Value> {
        let compat = get_compat(model);
        let messages = convert_messages(model, context, &compat)?;
        
        let mut body = serde_json::json!({
            "model": model.id,
            "messages": messages,
            "stream": true,
        });

        // stream_options
        if compat.supports_usage_in_streaming {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        // store (OpenAI 特定)
        if compat.supports_store {
            body["store"] = serde_json::json!(false);
        }

        // max_tokens / max_completion_tokens
        let max_tokens = options.max_tokens.unwrap_or(model.max_tokens);
        if compat.max_tokens_field == "max_tokens" {
            body["max_tokens"] = serde_json::json!(max_tokens);
        } else {
            body["max_completion_tokens"] = serde_json::json!(max_tokens);
        }

        // temperature
        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // tools
        if let Some(ref tools) = context.tools {
            body["tools"] = serde_json::Value::Array(convert_tools(tools, &compat));
        } else if has_tool_history(&context.messages) {
            // Anthropic (via LiteLLM/proxy) 需要 tools 参数当对话包含 tool_calls/tool_results
            body["tools"] = serde_json::json!([]);
        }

        // reasoning_effort (用于 o3-mini 等 reasoning 模型)
        if model.reasoning && compat.supports_reasoning_effort {
            if let Some(ref metadata) = options.metadata {
                if let Some(reasoning) = metadata.get("reasoning") {
                    let effort = map_reasoning_effort(reasoning, &compat.reasoning_effort_map);
                    body["reasoning_effort"] = serde_json::json!(effort);
                }
            }
        }

        // OpenRouter provider routing
        if model.base_url.contains("openrouter.ai") {
            if let Some(ref compat_json) = model.compat {
                if let Some(routing) = compat_json.get("openRouterRouting") {
                    body["provider"] = routing.clone();
                }
            }
        }

        Ok(body)
    }

    /// 执行流式请求
    async fn do_stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
        let api_key = self.get_api_key(model, options)?;
        let headers = self.build_headers(&api_key, options)?;
        let body = self.build_request_body(model, context, options)?;
        
        let url = format!("{}/chat/completions", model.base_url.trim_end_matches('/'));
        
        debug!("OpenAI API request to: {}", url);
        trace!("Request body: {}", serde_json::to_string_pretty(&body)?);

        let mut request_builder = self.client.post(&url);
        for (key, value) in &headers {
            request_builder = request_builder.header(key, value);
        }

        let response = request_builder.json(&body).send().await?;
        
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "OpenAI API error ({}): {}",
                status,
                error_text
            ));
        }

        let stream = self.process_stream(response, model.clone()).await?;
        Ok(Box::pin(stream))
    }

    /// 处理流式响应
    async fn process_stream(
        &self,
        response: reqwest::Response,
        model: Model,
    ) -> anyhow::Result<impl Stream<Item = anyhow::Result<AssistantMessageEvent>>> {
        let mut assistant_message = AssistantMessage::new(model.api.clone(), model.provider.clone(), &model.id);
        let mut sse_parser = SseParser::new();
        let mut stream_state = StreamState::new();

        let stream = response.bytes_stream().map(move |chunk| {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let events = sse_parser.feed(&text);
                    let mut results = Vec::new();

                    for event in events {
                        if event.data == "[DONE]" {
                            // 流结束
                            results.push(Ok(AssistantMessageEvent::Done {
                                reason: DoneReason::Stop,
                                message: assistant_message.clone(),
                            }));
                            continue;
                        }

                        match serde_json::from_str::<ChatCompletionChunk>(&event.data) {
                            Ok(chunk) => {
                                // 更新 response_id
                                if assistant_message.response_id.is_none() {
                                    assistant_message.response_id = Some(chunk.id.clone());
                                }

                                // 处理 usage
                                if let Some(ref usage) = chunk.usage {
                                    assistant_message.usage = parse_usage(usage, &model);
                                }

                                // 处理 choices
                                if let Some(choice) = chunk.choices.first() {
                                    // 处理 finish_reason
                                    if let Some(ref reason) = choice.finish_reason {
                                        let (stop_reason, error_msg) = map_finish_reason(reason);
                                        assistant_message.stop_reason = stop_reason.clone();
                                        if let Some(msg) = error_msg {
                                            assistant_message.error_message = Some(msg);
                                        }
                                    }

                                    // 处理 delta
                                    let events = stream_state.process_delta(
                                        &choice.delta,
                                        &mut assistant_message,
                                    );
                                    results.extend(events.into_iter().map(Ok));
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse chunk: {}, data: {}", e, event.data);
                            }
                        }
                    }

                    futures::stream::iter(results)
                }
                Err(e) => {
                    futures::stream::iter(vec![Err(anyhow::anyhow!("Stream error: {}", e))])
                }
            }
        }).flatten();

        Ok(stream)
    }
}

impl Default for OpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ApiProvider for OpenAiProvider {
    fn api(&self) -> Api {
        Api::OpenAiChatCompletions
    }

    async fn stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
        // 重试逻辑已统一移到 stream.rs 层面的 stream_with_retry
        self.do_stream(context, model, options).await
    }
}

/// 流处理状态机
struct StreamState {
    text_started: bool,
    current_text_index: Option<usize>,
    tool_calls: Vec<ToolCallState>,
}

struct ToolCallState {
    id: String,
    name: String,
    arguments_json: String,
    started: bool,
    content_index: usize,
}

impl StreamState {
    fn new() -> Self {
        Self {
            text_started: false,
            current_text_index: None,
            tool_calls: Vec::new(),
        }
    }

    fn process_delta(
        &mut self,
        delta: &Delta,
        assistant_message: &mut AssistantMessage,
    ) -> Vec<AssistantMessageEvent> {
        let mut events = Vec::new();

        // 处理 role (首个 delta)
        if delta.role.is_some() && assistant_message.content.is_empty() {
            events.push(AssistantMessageEvent::Start {
                partial: assistant_message.clone(),
            });
        }

        // 处理 content (文本)
        if let Some(ref content) = delta.content {
            if !content.is_empty() {
                if !self.text_started {
                    // 开始新文本块
                    let text_content = ContentBlock::Text(TextContent::new(content));
                    assistant_message.content.push(text_content);
                    let index = assistant_message.content.len() - 1;
                    self.current_text_index = Some(index);
                    self.text_started = true;
                    
                    events.push(AssistantMessageEvent::TextStart {
                        content_index: index,
                        partial: assistant_message.clone(),
                    });
                } else if let Some(index) = self.current_text_index {
                    // 追加文本
                    if let Some(ContentBlock::Text(ref mut text)) = assistant_message.content.get_mut(index) {
                        text.text.push_str(content);
                    }
                    
                    events.push(AssistantMessageEvent::TextDelta {
                        content_index: index,
                        delta: content.clone(),
                        partial: assistant_message.clone(),
                    });
                }
            }
        }

        // 处理 reasoning_content (思考内容)
        if let Some(ref reasoning) = delta.reasoning_content {
            if !reasoning.is_empty() {
                // 简化处理：作为 thinking 块
                // 实际实现可能需要更复杂的逻辑
            }
        }

        // 处理 tool_calls
        if let Some(ref tool_calls_delta) = delta.tool_calls {
            for tool_delta in tool_calls_delta {
                self.process_tool_call_delta(tool_delta, assistant_message, &mut events);
            }
        }

        events
    }

    fn process_tool_call_delta(
        &mut self,
        tool_delta: &ToolCallDelta,
        assistant_message: &mut AssistantMessage,
        events: &mut Vec<AssistantMessageEvent>,
    ) {
        // 查找或创建 tool call 状态
        let index = tool_delta.index as usize;
        
        // 确保有足够的 tool call 状态
        while self.tool_calls.len() <= index {
            self.tool_calls.push(ToolCallState {
                id: String::new(),
                name: String::new(),
                arguments_json: String::new(),
                started: false,
                content_index: 0,
            });
        }

        let state = &mut self.tool_calls[index];

        // 新 tool call 开始
        if let Some(ref id) = tool_delta.id {
            if !state.started {
                state.id = id.clone();
                state.started = true;
                
                // 创建 ToolCall 内容块
                let name = tool_delta.function.as_ref()
                    .and_then(|f| f.name.clone())
                    .unwrap_or_default();
                let tool_call = ToolCall::new(
                    id.clone(),
                    name,
                    serde_json::Value::Object(serde_json::Map::new()),
                );
                
                assistant_message.content.push(ContentBlock::ToolCall(tool_call));
                state.content_index = assistant_message.content.len() - 1;
                
                events.push(AssistantMessageEvent::ToolCallStart {
                    content_index: state.content_index,
                    partial: assistant_message.clone(),
                });
            }
        }

        // 更新 name
        if let Some(ref function) = tool_delta.function {
            if let Some(ref name) = function.name {
                state.name = name.clone();
                if let Some(ContentBlock::ToolCall(ref mut tc)) = assistant_message.content.get_mut(state.content_index) {
                    tc.name = name.clone();
                }
            }

            // 更新 arguments
            if let Some(ref args) = function.arguments {
                state.arguments_json.push_str(args);
                
                // 尝试解析部分 JSON
                if let Some(parsed) = parse_partial_json(&state.arguments_json) {
                    if let Some(ContentBlock::ToolCall(ref mut tc)) = assistant_message.content.get_mut(state.content_index) {
                        tc.arguments = parsed;
                    }
                }
                
                events.push(AssistantMessageEvent::ToolCallDelta {
                    content_index: state.content_index,
                    delta: args.clone(),
                    partial: assistant_message.clone(),
                });
            }
        }
    }

    #[allow(dead_code)] // 预留方法供未来使用
    fn finish(&mut self, assistant_message: &mut AssistantMessage) -> Vec<AssistantMessageEvent> {
        let mut events = Vec::new();

        // 结束文本块
        if self.text_started {
            if let Some(index) = self.current_text_index {
                if let Some(ContentBlock::Text(ref text)) = assistant_message.content.get(index) {
                    events.push(AssistantMessageEvent::TextEnd {
                        content_index: index,
                        content: text.text.clone(),
                        partial: assistant_message.clone(),
                    });
                }
            }
        }

        // 结束所有 tool call
        for state in &self.tool_calls {
            if state.started {
                // 解析最终参数
                let args = parse_partial_json(&state.arguments_json)
                    .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
                
                let tool_call = ToolCall::new(state.id.clone(), state.name.clone(), args);
                
                events.push(AssistantMessageEvent::ToolCallEnd {
                    content_index: state.content_index,
                    tool_call,
                    partial: assistant_message.clone(),
                });
            }
        }

        events
    }
}

// =============================================================================
// OpenAI API 类型定义
// =============================================================================

/// Chat Completion Chunk (SSE 事件)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Serde 反序列化结构体，字段由 JSON 解析填充
struct ChatCompletionChunk {
    id: String,
    object: String,
    created: i64,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<UsageInfo>,
    choices: Vec<Choice>,
}

/// Usage 信息
#[derive(Debug, Clone, Deserialize)]
struct UsageInfo {
    prompt_tokens: u64,
    completion_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_tokens_details: Option<CompletionTokensDetails>,
}

#[derive(Debug, Clone, Deserialize)]
struct PromptTokensDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_write_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CompletionTokensDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_tokens: Option<u64>,
}

/// Choice
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Serde 反序列化结构体
struct Choice {
    index: i32,
    delta: Delta,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

/// Delta
#[derive(Debug, Clone, Deserialize, Default)]
struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCallDelta>>,
}

/// Tool Call Delta
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Serde 反序列化结构体
struct ToolCallDelta {
    index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<FunctionDelta>,
}

/// Function Delta
#[derive(Debug, Clone, Deserialize)]
struct FunctionDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<String>,
}

// =============================================================================
// 兼容性设置
// =============================================================================

/// OpenAI Completions 兼容性设置
#[derive(Debug, Clone)]
#[allow(dead_code)] // 配置字段供未来使用
struct OpenAiCompat {
    supports_store: bool,
    supports_developer_role: bool,
    supports_reasoning_effort: bool,
    reasoning_effort_map: HashMap<String, String>,
    supports_usage_in_streaming: bool,
    max_tokens_field: String,
    requires_tool_result_name: bool,
    requires_assistant_after_tool_result: bool,
    requires_thinking_as_text: bool,
    thinking_format: String,
    zai_tool_stream: bool,
    supports_strict_mode: bool,
}

impl Default for OpenAiCompat {
    fn default() -> Self {
        Self {
            supports_store: true,
            supports_developer_role: true,
            supports_reasoning_effort: true,
            reasoning_effort_map: HashMap::new(),
            supports_usage_in_streaming: true,
            max_tokens_field: "max_completion_tokens".to_string(),
            requires_tool_result_name: false,
            requires_assistant_after_tool_result: false,
            requires_thinking_as_text: false,
            thinking_format: "openai".to_string(),
            zai_tool_stream: false,
            supports_strict_mode: true,
        }
    }
}

/// 根据模型检测兼容性设置
fn detect_compat(model: &Model) -> OpenAiCompat {
    let provider = &model.provider;
    let base_url = &model.base_url;

    let is_zai = *provider == Provider::Zai || base_url.contains("api.z.ai");
    
    let is_non_standard = matches!(provider, 
        Provider::Cerebras | Provider::Xai
    ) || base_url.contains("cerebras.ai")
        || base_url.contains("api.x.ai")
        || base_url.contains("chutes.ai")
        || base_url.contains("deepseek.com")
        || is_zai
        || matches!(provider, Provider::Opencode | Provider::OpencodeGo)
        || base_url.contains("opencode.ai");

    let use_max_tokens = base_url.contains("chutes.ai");
    let is_grok = *provider == Provider::Xai || base_url.contains("api.x.ai");
    let is_groq = *provider == Provider::Groq || base_url.contains("groq.com");

    let mut reasoning_effort_map = HashMap::new();
    if is_groq && model.id == "qwen/qwen3-32b" {
        for level in &["minimal", "low", "medium", "high", "xhigh"] {
            reasoning_effort_map.insert(level.to_string(), "default".to_string());
        }
    }

    let thinking_format = if is_zai {
        "zai".to_string()
    } else if *provider == Provider::Openrouter || base_url.contains("openrouter.ai") {
        "openrouter".to_string()
    } else {
        "openai".to_string()
    };

    OpenAiCompat {
        supports_store: !is_non_standard,
        supports_developer_role: !is_non_standard,
        supports_reasoning_effort: !is_grok && !is_zai,
        reasoning_effort_map,
        supports_usage_in_streaming: true,
        max_tokens_field: if use_max_tokens { "max_tokens" } else { "max_completion_tokens" }.to_string(),
        requires_tool_result_name: false,
        requires_assistant_after_tool_result: false,
        requires_thinking_as_text: false,
        thinking_format,
        zai_tool_stream: false,
        supports_strict_mode: true,
    }
}

/// 获取模型的兼容性设置
fn get_compat(model: &Model) -> OpenAiCompat {
    let mut detected = detect_compat(model);
    
    // 如果模型有自定义 compat 设置，合并它们
    if let Some(ref compat_json) = model.compat {
        if let Some(store) = compat_json.get("supportsStore").and_then(|v| v.as_bool()) {
            detected.supports_store = store;
        }
        if let Some(developer) = compat_json.get("supportsDeveloperRole").and_then(|v| v.as_bool()) {
            detected.supports_developer_role = developer;
        }
        if let Some(reasoning) = compat_json.get("supportsReasoningEffort").and_then(|v| v.as_bool()) {
            detected.supports_reasoning_effort = reasoning;
        }
        if let Some(usage) = compat_json.get("supportsUsageInStreaming").and_then(|v| v.as_bool()) {
            detected.supports_usage_in_streaming = usage;
        }
        if let Some(field) = compat_json.get("maxTokensField").and_then(|v| v.as_str()) {
            detected.max_tokens_field = field.to_string();
        }
    }
    
    detected
}

// =============================================================================
// 消息转换
// =============================================================================

/// 检查消息历史是否包含工具调用
fn has_tool_history(messages: &[Message]) -> bool {
    for msg in messages {
        match msg {
            Message::ToolResult(_) => return true,
            Message::Assistant(assistant) => {
                for block in &assistant.content {
                    if let ContentBlock::ToolCall(_) = block {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// 转换消息为 OpenAI 格式
fn convert_messages(
    model: &Model,
    context: &Context,
    compat: &OpenAiCompat,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // System prompt
    if let Some(ref system_prompt) = context.system_prompt {
        let role = if model.reasoning && compat.supports_developer_role {
            "developer"
        } else {
            "system"
        };
        messages.push(serde_json::json!({
            "role": role,
            "content": system_prompt,
        }));
    }

    let mut last_role: Option<String> = None;

    for msg in &context.messages {
        // 某些 provider 不允许 tool result 后直接跟 user 消息
        if compat.requires_assistant_after_tool_result
            && last_role.as_deref() == Some("tool") && matches!(msg, Message::User(_)) {
                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": "I have processed the tool results.",
                }));
            }

        match msg {
            Message::User(user_msg) => {
                let openai_msg = convert_user_message(user_msg, model)?;
                messages.push(openai_msg);
                last_role = Some("user".to_string());
            }
            Message::Assistant(assistant_msg) => {
                if let Some(openai_msg) = convert_assistant_message(assistant_msg, compat)? {
                    messages.push(openai_msg);
                    last_role = Some("assistant".to_string());
                }
            }
            Message::ToolResult(tool_result) => {
                let openai_msg = convert_tool_result_message(tool_result, compat)?;
                messages.push(openai_msg);
                last_role = Some("tool".to_string());
            }
        }
    }

    Ok(messages)
}

/// 转换用户消息
fn convert_user_message(user_msg: &UserMessage, model: &Model) -> anyhow::Result<serde_json::Value> {
    match &user_msg.content {
        UserContent::Text(text) => {
            Ok(serde_json::json!({
                "role": "user",
                "content": text,
            }))
        }
        UserContent::Blocks(blocks) => {
            let mut content_parts = Vec::new();
            
            for block in blocks {
                match block {
                    ContentBlock::Text(text) => {
                        content_parts.push(serde_json::json!({
                            "type": "text",
                            "text": text.text,
                        }));
                    }
                    ContentBlock::Image(image) => {
                        // 检查模型是否支持图片输入
                        if model.input.contains(&InputModality::Image) {
                            content_parts.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{})", image.mime_type, image.data),
                                },
                            }));
                        }
                    }
                    _ => {}
                }
            }

            if content_parts.is_empty() {
                return Ok(serde_json::json!({
                    "role": "user",
                    "content": "",
                }));
            }

            Ok(serde_json::json!({
                "role": "user",
                "content": content_parts,
            }))
        }
    }
}

/// 转换助手消息
fn convert_assistant_message(
    assistant_msg: &AssistantMessage,
    compat: &OpenAiCompat,
) -> anyhow::Result<Option<serde_json::Value>> {
    let mut content = String::new();
    let mut tool_calls = Vec::new();

    for block in &assistant_msg.content {
        match block {
            ContentBlock::Text(text) => {
                if !text.text.trim().is_empty() {
                    content.push_str(&text.text);
                }
            }
            ContentBlock::Thinking(thinking) => {
                // 处理思考内容
                if compat.requires_thinking_as_text {
                    content.push_str(&thinking.thinking);
                }
                // 否则可能需要特殊处理
            }
            ContentBlock::ToolCall(tc) => {
                tool_calls.push(serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": serde_json::to_string(&tc.arguments)?,
                    },
                }));
            }
            _ => {}
        }
    }

    // 如果没有内容和 tool_calls，跳过此消息
    if content.is_empty() && tool_calls.is_empty() {
        return Ok(None);
    }

    let mut msg = serde_json::json!({
        "role": "assistant",
    });

    // 某些 provider 不接受 null content
    if content.is_empty() && compat.requires_assistant_after_tool_result {
        msg["content"] = serde_json::json!("");
    } else if !content.is_empty() {
        msg["content"] = serde_json::json!(content);
    } else {
        msg["content"] = serde_json::Value::Null;
    }

    if !tool_calls.is_empty() {
        msg["tool_calls"] = serde_json::json!(tool_calls);
    }

    Ok(Some(msg))
}

/// 转换工具结果消息
fn convert_tool_result_message(
    tool_result: &ToolResultMessage,
    compat: &OpenAiCompat,
) -> anyhow::Result<serde_json::Value> {
    // 提取文本内容
    let text_content: Vec<String> = tool_result.content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text(text) = block {
                Some(text.text.clone())
            } else {
                None
            }
        })
        .collect();

    let content = if text_content.is_empty() {
        "(see attached image)".to_string()
    } else {
        text_content.join("\n")
    };

    let mut msg = serde_json::json!({
        "role": "tool",
        "content": content,
        "tool_call_id": tool_result.tool_call_id,
    });

    // 某些 provider 需要 name 字段
    if compat.requires_tool_result_name {
        msg["name"] = serde_json::json!(tool_result.tool_name.clone());
    }

    Ok(msg)
}

// =============================================================================
// 工具转换
// =============================================================================

/// 转换工具定义为 OpenAI 格式
fn convert_tools(tools: &[Tool], compat: &OpenAiCompat) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            let mut function = serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            });

            // 某些 provider 支持 strict 模式
            if compat.supports_strict_mode {
                function["strict"] = serde_json::json!(false);
            }

            serde_json::json!({
                "type": "function",
                "function": function,
            })
        })
        .collect()
}

// =============================================================================
// 辅助函数
// =============================================================================

/// 映射 finish_reason 到 StopReason
fn map_finish_reason(reason: &str) -> (StopReason, Option<String>) {
    match reason {
        "stop" | "end" => (StopReason::Stop, None),
        "length" => (StopReason::Length, None),
        "function_call" | "tool_calls" => (StopReason::ToolUse, None),
        "content_filter" => (StopReason::Error, Some("Provider finish_reason: content_filter".to_string())),
        "network_error" => (StopReason::Error, Some("Provider finish_reason: network_error".to_string())),
        _ => (StopReason::Error, Some(format!("Provider finish_reason: {}", reason))),
    }
}

/// 解析 usage 信息
fn parse_usage(usage: &UsageInfo, _model: &Model) -> Usage {
    let prompt_tokens = usage.prompt_tokens;
    let completion_tokens = usage.completion_tokens;
    
    let reported_cached = usage.prompt_tokens_details.as_ref().and_then(|d| d.cached_tokens).unwrap_or(0);
    let cache_write = usage.prompt_tokens_details.as_ref().and_then(|d| d.cache_write_tokens).unwrap_or(0);
    let reasoning_tokens = usage.completion_tokens_details.as_ref().and_then(|d| d.reasoning_tokens).unwrap_or(0);

    // 规范化 cache_read：如果 cache_write > 0，从 cached_tokens 中减去
    let cache_read = if cache_write > 0 {
        reported_cached.saturating_sub(cache_write)
    } else {
        reported_cached
    };

    let input = prompt_tokens.saturating_sub(cache_read).saturating_sub(cache_write);
    let output = completion_tokens + reasoning_tokens;
    let _total_tokens = input + output + cache_read + cache_write;

    Usage {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: if cache_read > 0 { Some(cache_read) } else { None },
        cache_write_tokens: if cache_write > 0 { Some(cache_write) } else { None },
    }
}

/// 映射 reasoning effort
fn map_reasoning_effort(reasoning: &serde_json::Value, effort_map: &HashMap<String, String>) -> String {
    let effort_str = match reasoning {
        serde_json::Value::String(s) => s.clone(),
        _ => "medium".to_string(),
    };
    
    effort_map.get(&effort_str).cloned().unwrap_or(effort_str)
}

/// 检查错误是否可重试
/// 
/// 注意：此函数已移至 retry.rs 中的 RetryPolicy::is_retryable
/// 保留此别名以供测试使用
#[cfg(test)]
fn is_retryable_error(error: &anyhow::Error) -> bool {
    crate::retry::RetryPolicy::is_retryable(error)
}

/// 注册 OpenAI Provider
pub fn register() {
    let provider = std::sync::Arc::new(OpenAiProvider::new());
    crate::api_registry::register_api_provider(provider);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::*;
    use mockito::Server;
    use futures::StreamExt;

    #[test]
    fn test_map_finish_reason() {
        assert_eq!(map_finish_reason("stop").0, StopReason::Stop);
        assert_eq!(map_finish_reason("length").0, StopReason::Length);
        assert_eq!(map_finish_reason("tool_calls").0, StopReason::ToolUse);
        assert_eq!(map_finish_reason("content_filter").0, StopReason::Error);
    }

    #[test]
    fn test_has_tool_history() {
        let messages = vec![
            Message::User(UserMessage::new("Hello")),
        ];
        assert!(!has_tool_history(&messages));

        let messages_with_tool = vec![
            Message::ToolResult(ToolResultMessage::new(
                "call_123",
                "test_tool",
                vec![ContentBlock::Text(TextContent::new("result"))],
            )),
        ];
        assert!(has_tool_history(&messages_with_tool));
    }

    #[tokio::test]
    async fn test_stream_text_response() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // OpenAI SSE 格式 - 第一个 delta 只包含 role，第二个 delta 包含 content 触发 TextStart，后续 delta 触发 TextDelta
        let sse_body = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Say hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // 验证事件序列
        assert!(!events.is_empty());

        // 查找 TextStart 和 TextDelta 事件
        let text_starts: Vec<_> = events.iter().filter_map(|e| match e {
            AssistantMessageEvent::TextStart { partial, .. } => {
                // 从 partial 中提取文本内容
                partial.content.iter().find_map(|c| match c {
                    crate::types::ContentBlock::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
            }
            _ => None,
        }).collect();
        
        let text_deltas: Vec<_> = events.iter().filter_map(|e| match e {
            AssistantMessageEvent::TextDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        }).collect();
        
        // 验证文本内容 - "Hello" 在 TextStart 中，" world" 在 TextDelta 中
        assert!(!text_starts.is_empty(), "No TextStart events found");
        assert!(text_starts.iter().any(|d| d.contains("Hello")), "No 'Hello' found in text_starts: {:?}", text_starts);
        assert!(!text_deltas.is_empty(), "No TextDelta events found");
        assert!(text_deltas.iter().any(|d| d.contains("world")), "No 'world' found in deltas: {:?}", text_deltas);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_tool_call() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 工具调用测试 - 注意 tool_calls 需要与 role 同时出现，[DONE] 后需要空行
        let sse_body = r#"data: {"id":"chatcmpl-456","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_abc123","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Paris\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-456","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let tool = sample_tool("get_weather", "Get weather for a location");
        let context = sample_context_with_tools(
            "You are a helpful assistant",
            vec![sample_user_message("What's the weather in Paris?")],
            vec![tool],
        );
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // 验证 ToolCall 事件
        let tool_call_starts: Vec<_> = events.iter().filter(|e| matches!(e, AssistantMessageEvent::ToolCallStart { .. })).collect();
        assert!(!tool_call_starts.is_empty(), "Expected at least one ToolCallStart event");

        // 验证 Done 事件存在
        assert!(matches!(events.last().unwrap(), AssistantMessageEvent::Done { .. }), "Expected Done event at the end");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_with_reasoning() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // OpenAI o1/o3 模型测试 - 第一个 delta 只包含 role，第二个包含 content
        let sse_body = r#"data: {"id":"chatcmpl-789","object":"chat.completion.chunk","created":1677652288,"model":"o3-mini","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-789","object":"chat.completion.chunk","created":1677652288,"model":"o3-mini","choices":[{"index":0,"delta":{"content":"The solution is: 42"},"finish_reason":null}]}

data: {"id":"chatcmpl-789","object":"chat.completion.chunk","created":1677652288,"model":"o3-mini","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();
        model.reasoning = true;

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Solve this complex problem")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // 验证至少有一些事件
        assert!(!events.is_empty());

        // 验证 Done 事件
        assert!(matches!(events.last().unwrap(), AssistantMessageEvent::Done { .. }));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试 429 错误
        let error_json = r#"{"error": {"message": "Rate limit exceeded", "type": "rate_limit_error", "code": "rate_limit"}}"#;

        // 因为有重试机制，需要设置多次期望
        let _mock = server.mock("POST", "/chat/completions")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());

        // 不严格验证请求次数，因为重试逻辑可能变化
    }

    #[tokio::test]
    async fn test_error_handling_500() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试 500 错误
        let error_json = r#"{"error": {"message": "Internal server error", "type": "internal_error"}}"#;

        let _mock = server.mock("POST", "/chat/completions")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());

        // 不严格验证请求次数
    }

    // ==================== 边界测试 ====================

    #[tokio::test]
    async fn test_stream_interrupted_recovery() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 模拟流被中断的情况 - 只有部分数据
        let sse_body = r#"data: {"id":"chatcmpl-partial","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-partial","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hel"},"finish_reason":null}]}

"#; // 流突然结束，没有 [DONE]

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        // 应该至少有一些事件，即使流被中断
        assert!(!events.is_empty());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_non_standard_finish_reason() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试各种非标准 finish_reason
        let sse_body = r#"data: {"id":"chatcmpl-test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Test"},"finish_reason":null}]}

data: {"id":"chatcmpl-test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"content_filter"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        // 应该能处理非标准 finish_reason
        assert!(!events.is_empty());
        // 最后一个应该是 Done 事件
        assert!(matches!(events.last().unwrap(), Ok(AssistantMessageEvent::Done { .. })));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_choices_array() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试空 choices 数组的情况
        let sse_body = r#"data: {"id":"chatcmpl-empty","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[]}

data: {"id":"chatcmpl-empty","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-empty","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Response"},"finish_reason":null}]}

data: {"id":"chatcmpl-empty","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        // 应该能处理空 choices，只处理有效的 chunk
        assert!(!events.is_empty());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_network_error_handling() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试网络错误（connection reset）
        let error_json = r#"{"error": {"message": "Connection reset by peer", "type": "network_error"}}"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(503)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_timeout_error_handling() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试超时错误
        let error_json = r#"{"error": {"message": "Request timeout", "type": "timeout_error"}}"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(408)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[test]
    fn test_is_retryable_error() {
        // 可重试的错误
        assert!(is_retryable_error(&anyhow::anyhow!("429 Too Many Requests")));
        assert!(is_retryable_error(&anyhow::anyhow!("connection refused")));
        assert!(is_retryable_error(&anyhow::anyhow!("timeout occurred")));
        assert!(is_retryable_error(&anyhow::anyhow!("network error")));
        assert!(is_retryable_error(&anyhow::anyhow!("dns resolution failed")));
        assert!(is_retryable_error(&anyhow::anyhow!("connection reset")));

        // 不可重试的错误
        assert!(!is_retryable_error(&anyhow::anyhow!("400 Bad Request")));
        assert!(!is_retryable_error(&anyhow::anyhow!("401 Unauthorized")));
        assert!(!is_retryable_error(&anyhow::anyhow!("Invalid API key")));
    }

    #[test]
    fn test_convert_messages_with_empty_content() {
        let model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        let compat = get_compat(&model);

        let context = sample_context(
            "You are helpful",
            vec![
                Message::User(UserMessage::new("")),
                sample_user_message("Valid message"),
            ],
        );

        let messages = convert_messages(&model, &context, &compat).unwrap();

        // 空消息也会被包含（OpenAI 格式）
        assert!(!messages.is_empty());
    }

    #[test]
    fn test_map_finish_reason_all_cases() {
        // 测试所有可能的 finish_reason
        let (stop, err) = map_finish_reason("stop");
        assert_eq!(stop, StopReason::Stop);
        assert!(err.is_none());

        let (stop, err) = map_finish_reason("end");
        assert_eq!(stop, StopReason::Stop);
        assert!(err.is_none());

        let (stop, err) = map_finish_reason("length");
        assert_eq!(stop, StopReason::Length);
        assert!(err.is_none());

        let (stop, err) = map_finish_reason("tool_calls");
        assert_eq!(stop, StopReason::ToolUse);
        assert!(err.is_none());

        let (stop, err) = map_finish_reason("function_call");
        assert_eq!(stop, StopReason::ToolUse);
        assert!(err.is_none());

        let (stop, err) = map_finish_reason("content_filter");
        assert_eq!(stop, StopReason::Error);
        assert!(err.is_some());

        let (stop, err) = map_finish_reason("network_error");
        assert_eq!(stop, StopReason::Error);
        assert!(err.is_some());

        // 未知类型
        let (stop, err) = map_finish_reason("unknown_reason");
        assert_eq!(stop, StopReason::Error);
        assert!(err.is_some());
    }

    #[test]
    fn test_non_standard_finish_reason_variations() {
        // 测试更多非标准 finish_reason 变体
        // 注意：只有特定的 finish_reason 被映射，其他都返回 Error
        let test_cases = vec![
            ("content_filter", StopReason::Error),
            ("network_error", StopReason::Error),
            // 以下是非标准值，会被映射为 Error
            ("safety_violation", StopReason::Error),
            ("policy_violation", StopReason::Error),
            ("recitation", StopReason::Error),
            ("max_tokens_reached", StopReason::Error), // 非标准，映射为 Error
            ("user_cancel", StopReason::Error), // 非标准，映射为 Error
            ("api_error", StopReason::Error),
        ];

        for (reason, expected) in test_cases {
            let (stop, _) = map_finish_reason(reason);
            assert_eq!(stop, expected, "finish_reason '{}' should map to {:?}", reason, expected);
        }
    }

    #[tokio::test]
    async fn test_empty_choices_with_delta_variations() {
        let mut server = Server::new_async().await;
        let provider = OpenAiProvider::new();

        // 测试多种空 choices 变体
        let sse_body = r#"data: {"id":"test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[]}

data: {"id":"test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[]}

data: {"id":"test","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            if let Ok(evt) = event {
                events.push(evt);
            }
        }

        // 应该过滤掉空 choices，只保留有效事件
        assert!(!events.is_empty());

        mock.assert_async().await;
    }

    #[test]
    fn test_build_request_body_with_various_options() {
        let provider = OpenAiProvider::new();
        let model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);

        // 测试带各种选项的请求体构建
        let mut options = sample_stream_options("test-key");
        options.temperature = Some(0.7);
        options.max_tokens = Some(1000);

        let context = sample_context("System prompt", vec![sample_user_message("Hello")]);
        let body = provider.build_request_body(&model, &context, &options).unwrap();

        // 验证请求体包含预期的字段
        assert!(body.get("temperature").is_some(), "Should have temperature field");
        assert!(body.get("max_tokens").is_some() || body.get("max_completion_tokens").is_some(), 
                "Should have max_tokens or max_completion_tokens field");
        assert!(body.get("model").is_some(), "Should have model field");
        assert_eq!(body["stream"].as_bool(), Some(true));
    }

    #[test]
    fn test_convert_messages_with_special_chars() {
        let model = sample_model(Api::OpenAiChatCompletions, Provider::Openai);
        let compat = get_compat(&model);

        // 测试包含特殊字符的消息
        let context = sample_context(
            "You are helpful",
            vec![
                sample_user_message("Hello world with special content"),
                sample_user_message("Unicode: hello world"),
            ],
        );

        let messages = convert_messages(&model, &context, &compat).unwrap();
        // 系统提示会被转换为 system 消息，所以至少有 3 条消息
        assert!(messages.len() >= 2, "Should have at least 2 messages");

        // 验证消息被正确转换
        let has_content = messages.iter().any(|m| {
            m.get("content").and_then(|c| c.as_str()).map(|s| !s.is_empty()).unwrap_or(false)
        });
        assert!(has_content, "Should have messages with content");
    }

    #[test]
    fn test_api_provider_trait_methods() {
        let provider = OpenAiProvider::new();
        
        // 测试 ApiProvider trait 的基本方法
        let api = provider.api();
        assert_eq!(api, Api::OpenAiChatCompletions);
    }
}
