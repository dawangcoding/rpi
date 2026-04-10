//! Azure OpenAI Chat Completions API Provider 实现
//!
//! 支持 Azure OpenAI Chat Completions API 的流式调用
//! 与 OpenAI 格式兼容，但使用不同的 endpoint 格式和认证方式

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;
use tracing::{debug, trace, warn};

use crate::api_registry::ApiProvider;
use crate::models::get_api_key_from_env;
use crate::types::*;
use crate::utils::event_stream::SseParser;
use crate::utils::json_parse::parse_partial_json;

/// Azure OpenAI API Provider
pub struct AzureOpenAiProvider {
    client: Client,
}

impl AzureOpenAiProvider {
    /// 创建新的 Azure OpenAI Provider 实例
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

    /// 构建 Azure OpenAI 特有的 URL
    ///
    /// 格式: {base_url}/openai/deployments/{deployment_name}/chat/completions?api-version=2024-12-01-preview
    fn build_url(&self, model: &Model) -> String {
        let base_url = model.base_url.trim_end_matches('/');
        let deployment_name = &model.id;
        let api_version = "2024-12-01-preview";
        
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            base_url, deployment_name, api_version
        )
    }

    /// 构建请求头
    ///
    /// Azure OpenAI 使用 `api-key` 请求头而不是 `Authorization: Bearer`
    fn build_headers(&self, api_key: &str, options: &StreamOptions) -> anyhow::Result<HashMap<String, String>> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("api-key".to_string(), api_key.to_string());
        
        // 合并用户自定义 headers
        if let Some(ref custom_headers) = options.headers {
            for (key, value) in custom_headers {
                headers.insert(key.clone(), value.clone());
            }
        }
        
        Ok(headers)
    }

    /// 构建请求体
    ///
    /// 与 OpenAI 兼容，但不传 `model` 字段（部署名在 URL 中），不支持 `store` 参数
    fn build_request_body(
        &self,
        model: &Model,
        context: &Context,
        options: &StreamOptions,
    ) -> anyhow::Result<serde_json::Value> {
        let compat = get_compat(model);
        let messages = convert_messages(model, context, &compat)?;
        
        let mut body = serde_json::json!({
            "messages": messages,
            "stream": true,
        });

        // stream_options
        if compat.supports_usage_in_streaming {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        // 注意：Azure OpenAI 不需要 `store` 参数

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
            body["tools"] = serde_json::json!([]);
        }

        // reasoning_effort (用于推理模型)
        if model.reasoning && compat.supports_reasoning_effort {
            if let Some(ref metadata) = options.metadata {
                if let Some(reasoning) = metadata.get("reasoning") {
                    let effort = map_reasoning_effort(reasoning, &compat.reasoning_effort_map);
                    body["reasoning_effort"] = serde_json::json!(effort);
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
        let url = self.build_url(model);
        let headers = self.build_headers(&api_key, options)?;
        let body = self.build_request_body(model, context, options)?;
        
        debug!("Azure OpenAI API request to: {}", url);
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
                "Azure OpenAI API error ({}): {}",
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
        let mut assistant_message = AssistantMessage::new(Api::AzureOpenAiResponses, model.provider.clone(), &model.id);
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
                            results.push(Ok(AssistantMessageEvent::Done {
                                reason: DoneReason::Stop,
                                message: assistant_message.clone(),
                            }));
                            continue;
                        }

                        match serde_json::from_str::<ChatCompletionChunk>(&event.data) {
                            Ok(chunk) => {
                                if assistant_message.response_id.is_none() {
                                    assistant_message.response_id = Some(chunk.id.clone());
                                }

                                if let Some(ref usage) = chunk.usage {
                                    assistant_message.usage = parse_usage(usage, &model);
                                }

                                if let Some(choice) = chunk.choices.first() {
                                    if let Some(ref reason) = choice.finish_reason {
                                        let (stop_reason, error_msg) = map_finish_reason(reason);
                                        assistant_message.stop_reason = stop_reason.clone();
                                        if let Some(msg) = error_msg {
                                            assistant_message.error_message = Some(msg);
                                        }
                                    }

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

impl Default for AzureOpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ApiProvider for AzureOpenAiProvider {
    fn api(&self) -> Api {
        Api::AzureOpenAiResponses
    }

    async fn stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.do_stream(context, model, options).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    let should_retry = is_retryable_error(&e);
                    last_error = Some(e);
                    
                    if should_retry && attempt < max_retries - 1 {
                        let delay = Duration::from_millis(500 * (attempt as u64 + 1));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    break;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error")))
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
        let index = tool_delta.index as usize;
        
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

        if let Some(ref id) = tool_delta.id {
            if !state.started {
                state.id = id.clone();
                state.started = true;
                
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

        if let Some(ref function) = tool_delta.function {
            if let Some(ref name) = function.name {
                state.name = name.clone();
                if let Some(ContentBlock::ToolCall(ref mut tc)) = assistant_message.content.get_mut(state.content_index) {
                    tc.name = name.clone();
                }
            }

            if let Some(ref args) = function.arguments {
                state.arguments_json.push_str(args);
                
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
}

// =============================================================================
// Azure OpenAI API 类型定义
// =============================================================================

/// Chat Completion Chunk (SSE 事件)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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

/// Azure OpenAI 兼容性设置
#[derive(Debug, Clone)]
struct AzureCompat {
    supports_developer_role: bool,
    supports_reasoning_effort: bool,
    reasoning_effort_map: HashMap<String, String>,
    supports_usage_in_streaming: bool,
    max_tokens_field: String,
    requires_tool_result_name: bool,
    supports_strict_mode: bool,
}

impl Default for AzureCompat {
    fn default() -> Self {
        Self {
            supports_developer_role: true,
            supports_reasoning_effort: true,
            reasoning_effort_map: HashMap::new(),
            supports_usage_in_streaming: true,
            max_tokens_field: "max_completion_tokens".to_string(),
            requires_tool_result_name: false,
            supports_strict_mode: true,
        }
    }
}

/// 获取模型的兼容性设置
fn get_compat(model: &Model) -> AzureCompat {
    let mut detected = AzureCompat::default();
    
    // 如果模型有自定义 compat 设置，合并它们
    if let Some(ref compat_json) = model.compat {
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

/// 转换消息为 Azure OpenAI 格式
fn convert_messages(
    model: &Model,
    context: &Context,
    compat: &AzureCompat,
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

    for msg in &context.messages {
        match msg {
            Message::User(user_msg) => {
                let openai_msg = convert_user_message(user_msg, model)?;
                messages.push(openai_msg);
            }
            Message::Assistant(assistant_msg) => {
                if let Some(openai_msg) = convert_assistant_message(assistant_msg, compat)? {
                    messages.push(openai_msg);
                }
            }
            Message::ToolResult(tool_result) => {
                let openai_msg = convert_tool_result_message(tool_result, compat)?;
                messages.push(openai_msg);
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
                        if model.input.contains(&InputModality::Image) {
                            content_parts.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{};base64,{}", image.mime_type, image.data),
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
    _compat: &AzureCompat,
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
                // 处理思考内容（如果需要）
                let _ = thinking;
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

    if content.is_empty() && tool_calls.is_empty() {
        return Ok(None);
    }

    let mut msg = serde_json::json!({
        "role": "assistant",
    });

    if content.is_empty() {
        msg["content"] = serde_json::Value::Null;
    } else {
        msg["content"] = serde_json::json!(content);
    }

    if !tool_calls.is_empty() {
        msg["tool_calls"] = serde_json::json!(tool_calls);
    }

    Ok(Some(msg))
}

/// 转换工具结果消息
fn convert_tool_result_message(
    tool_result: &ToolResultMessage,
    compat: &AzureCompat,
) -> anyhow::Result<serde_json::Value> {
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

    if compat.requires_tool_result_name {
        msg["name"] = serde_json::json!(tool_result.tool_name.clone());
    }

    Ok(msg)
}

// =============================================================================
// 工具转换
// =============================================================================

/// 转换工具定义为 OpenAI 格式
fn convert_tools(tools: &[Tool], compat: &AzureCompat) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            let mut function = serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            });

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

    let cache_read = if cache_write > 0 {
        reported_cached.saturating_sub(cache_write)
    } else {
        reported_cached
    };

    let input = prompt_tokens.saturating_sub(cache_read).saturating_sub(cache_write);
    let output = completion_tokens + reasoning_tokens;

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
fn is_retryable_error(error: &anyhow::Error) -> bool {
    let error_string = error.to_string().to_lowercase();
    
    // 429 Too Many Requests
    if error_string.contains("429") || error_string.contains("too many requests") {
        return true;
    }
    
    // 网络错误
    if error_string.contains("connection") 
        || error_string.contains("timeout")
        || error_string.contains("network")
        || error_string.contains("dns")
        || error_string.contains("reset") {
        return true;
    }
    
    false
}

/// 注册 Azure OpenAI Provider
pub fn register() {
    let provider = std::sync::Arc::new(AzureOpenAiProvider::new());
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

    #[test]
    fn test_build_url() {
        let provider = AzureOpenAiProvider::new();
        let mut model = sample_model(Api::AzureOpenAiResponses, Provider::AzureOpenAiResponses);
        model.base_url = "https://myresource.openai.azure.com".to_string();
        model.id = "gpt-4o".to_string();

        let url = provider.build_url(&model);
        assert!(url.contains("myresource.openai.azure.com"));
        assert!(url.contains("/openai/deployments/gpt-4o/"));
        assert!(url.contains("api-version=2024-12-01-preview"));
    }

    #[tokio::test]
    async fn test_stream_text_response() {
        let mut server = Server::new_async().await;
        let provider = AzureOpenAiProvider::new();

        let sse_body = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
"#;

        // Azure OpenAI endpoint 格式
        let mock = server.mock("POST", mockito::Matcher::Regex(r"/openai/deployments/.*/chat/completions".to_string()))
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::AzureOpenAiResponses, Provider::AzureOpenAiResponses);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Say hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        assert!(!events.is_empty());

        let text_starts: Vec<_> = events.iter().filter_map(|e| match e {
            AssistantMessageEvent::TextStart { partial, .. } => {
                partial.content.iter().find_map(|c| match c {
                    ContentBlock::Text(t) => Some(t.text.clone()),
                    _ => None,
                })
            }
            _ => None,
        }).collect();
        
        let text_deltas: Vec<_> = events.iter().filter_map(|e| match e {
            AssistantMessageEvent::TextDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        }).collect();

        assert!(!text_starts.is_empty(), "No TextStart events found");
        assert!(text_starts.iter().any(|d| d.contains("Hello")));
        assert!(!text_deltas.is_empty(), "No TextDelta events found");
        assert!(text_deltas.iter().any(|d| d.contains("world")));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_tool_call() {
        let mut server = Server::new_async().await;
        let provider = AzureOpenAiProvider::new();

        let sse_body = r#"data: {"id":"chatcmpl-456","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_abc123","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Paris\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-456","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", mockito::Matcher::Regex(r"/openai/deployments/.*/chat/completions".to_string()))
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::AzureOpenAiResponses, Provider::AzureOpenAiResponses);
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

        let tool_call_starts: Vec<_> = events.iter().filter(|e| matches!(e, AssistantMessageEvent::ToolCallStart { .. })).collect();
        assert!(!tool_call_starts.is_empty(), "Expected at least one ToolCallStart event");

        assert!(matches!(events.last().unwrap(), AssistantMessageEvent::Done { .. }), "Expected Done event at the end");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_error_handling() {
        let mut server = Server::new_async().await;
        let provider = AzureOpenAiProvider::new();

        let error_json = r#"{"error": {"message": "Rate limit exceeded", "type": "rate_limit_error", "code": "rate_limit"}}"#;

        let mock = server.mock("POST", mockito::Matcher::Regex(r"/openai/deployments/.*/chat/completions".to_string()))
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut model = sample_model(Api::AzureOpenAiResponses, Provider::AzureOpenAiResponses);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());
    }
}
