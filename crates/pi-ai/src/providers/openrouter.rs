//! OpenRouter API Provider 实现
//!
//! 支持 OpenRouter API (/api/v1/chat/completions) 的流式调用
//! OpenRouter 提供统一的接口访问多个 LLM 提供商的模型

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

/// OpenRouter API Provider
pub struct OpenRouterProvider {
    client: Client,
}

impl OpenRouterProvider {
    /// 创建新的 OpenRouter Provider 实例
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
            .ok_or_else(|| anyhow::anyhow!("No API key found for provider: {:?}. Please set OPENROUTER_API_KEY environment variable.", model.provider))
    }

    /// 构建请求头
    fn build_headers(&self, api_key: &str, options: &StreamOptions) -> anyhow::Result<HashMap<String, String>> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        
        // OpenRouter 特有请求头
        headers.insert("HTTP-Referer".to_string(), "https://github.com/anthropics/claude-code".to_string());
        headers.insert("X-Title".to_string(), "pi-coding-agent".to_string());
        
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

        // OpenRouter 不支持 store 参数
        // 跳过 store 设置

        // max_completion_tokens (OpenRouter 使用 max_completion_tokens)
        let max_tokens = options.max_tokens.unwrap_or(model.max_tokens);
        body["max_completion_tokens"] = serde_json::json!(max_tokens);

        // temperature
        if let Some(temp) = options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // tools
        if let Some(ref tools) = context.tools {
            body["tools"] = serde_json::Value::Array(convert_tools(tools, &compat));
        } else if has_tool_history(&context.messages) {
            // 某些 provider 需要 tools 参数当对话包含 tool_calls/tool_results
            body["tools"] = serde_json::json!([]);
        }

        // reasoning_effort (用于支持推理的模型)
        if model.reasoning && compat.supports_reasoning_effort {
            if let Some(ref metadata) = options.metadata {
                if let Some(reasoning) = metadata.get("reasoning") {
                    let effort = map_reasoning_effort(reasoning, &compat.reasoning_effort_map);
                    body["reasoning_effort"] = serde_json::json!(effort);
                }
            }
        }

        // OpenRouter provider routing
        if let Some(ref compat_json) = model.compat {
            if let Some(routing) = compat_json.get("openRouterRouting") {
                body["provider"] = routing.clone();
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
        
        debug!("OpenRouter API request to: {}", url);
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
                "OpenRouter API error ({}): {}",
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
        let mut assistant_message = AssistantMessage::new(Api::Openrouter, model.provider.clone(), &model.id);
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

impl Default for OpenRouterProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ApiProvider for OpenRouterProvider {
    fn api(&self) -> Api {
        Api::Openrouter
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

    #[allow(dead_code)]
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
// OpenRouter API 类型定义
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

/// OpenRouter 兼容性设置
#[derive(Debug, Clone)]
struct OpenRouterCompat {
    supports_reasoning_effort: bool,
    reasoning_effort_map: HashMap<String, String>,
    supports_usage_in_streaming: bool,
}

impl Default for OpenRouterCompat {
    fn default() -> Self {
        Self {
            supports_reasoning_effort: true,
            reasoning_effort_map: HashMap::new(),
            supports_usage_in_streaming: true,
        }
    }
}

/// 获取模型的兼容性设置
fn get_compat(model: &Model) -> OpenRouterCompat {
    let mut compat = OpenRouterCompat::default();
    
    // 如果模型有自定义 compat 设置，合并它们
    if let Some(ref compat_json) = model.compat {
        if let Some(reasoning) = compat_json.get("supportsReasoningEffort").and_then(|v| v.as_bool()) {
            compat.supports_reasoning_effort = reasoning;
        }
        if let Some(usage) = compat_json.get("supportsUsageInStreaming").and_then(|v| v.as_bool()) {
            compat.supports_usage_in_streaming = usage;
        }
    }
    
    compat
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
    _compat: &OpenRouterCompat,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // System prompt - OpenRouter 不支持 developer 角色，使用 system
    if let Some(ref system_prompt) = context.system_prompt {
        messages.push(serde_json::json!({
            "role": "system",
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
                if let Some(openai_msg) = convert_assistant_message(assistant_msg)? {
                    messages.push(openai_msg);
                }
            }
            Message::ToolResult(tool_result) => {
                let openai_msg = convert_tool_result_message(tool_result)?;
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
                content.push_str(&thinking.thinking);
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

    Ok(serde_json::json!({
        "role": "tool",
        "content": content,
        "tool_call_id": tool_result.tool_call_id,
    }))
}

// =============================================================================
// 工具转换
// =============================================================================

/// 转换工具定义为 OpenAI 格式
fn convert_tools(tools: &[Tool], _compat: &OpenRouterCompat) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            let function = serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            });

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

/// 注册 OpenRouter Provider
pub fn register() {
    let provider = std::sync::Arc::new(OpenRouterProvider::new());
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
        let provider = OpenRouterProvider::new();

        // OpenRouter SSE 格式 - 与 OpenAI 兼容
        let sse_body = r#"data: {"id":"gen-123","object":"chat.completion.chunk","created":1677652288,"model":"openrouter/auto","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"gen-123","object":"chat.completion.chunk","created":1677652288,"model":"openrouter/auto","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"gen-123","object":"chat.completion.chunk","created":1677652288,"model":"openrouter/auto","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"gen-123","object":"chat.completion.chunk","created":1677652288,"model":"openrouter/auto","choices":[{"index":0,"delta":{}},"finish_reason":"stop"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::Openrouter, Provider::Openrouter);
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
        
        // 验证文本内容
        assert!(!text_starts.is_empty(), "No TextStart events found");
        assert!(text_starts.iter().any(|d| d.contains("Hello")), "No 'Hello' found in text_starts: {:?}", text_starts);
        assert!(!text_deltas.is_empty(), "No TextDelta events found");
        assert!(text_deltas.iter().any(|d| d.contains("world")), "No 'world' found in deltas: {:?}", text_deltas);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_tool_call() {
        let mut server = Server::new_async().await;
        let provider = OpenRouterProvider::new();

        // 工具调用测试
        let sse_body = r#"data: {"id":"gen-456","object":"chat.completion.chunk","created":1677652288,"model":"openrouter/auto","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_abc123","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Paris\"}"}}]},"finish_reason":null}]}

data: {"id":"gen-456","object":"chat.completion.chunk","created":1677652288,"model":"openrouter/auto","choices":[{"index":0,"delta":{}},"finish_reason":"tool_calls"}]}

data: [DONE]
"#;

        let mock = server.mock("POST", "/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::Openrouter, Provider::Openrouter);
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
    async fn test_error_handling() {
        let mut server = Server::new_async().await;
        let provider = OpenRouterProvider::new();

        // 测试 429 错误
        let error_json = r#"{"error": {"message": "Rate limit exceeded", "type": "rate_limit_error", "code": "rate_limit"}}"#;

        let _mock = server.mock("POST", "/chat/completions")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect_at_least(1)
            .create_async()
            .await;

        let mut model = sample_model(Api::Openrouter, Provider::Openrouter);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_build_headers_includes_openrouter_headers() {
        let provider = OpenRouterProvider::new();
        let options = StreamOptions::default();
        
        let headers = provider.build_headers("test-key", &options).unwrap();
        
        assert_eq!(headers.get("Authorization"), Some(&"Bearer test-key".to_string()));
        assert_eq!(headers.get("HTTP-Referer"), Some(&"https://github.com/anthropics/claude-code".to_string()));
        assert_eq!(headers.get("X-Title"), Some(&"pi-coding-agent".to_string()));
        assert_eq!(headers.get("Content-Type"), Some(&"application/json".to_string()));
    }

    #[tokio::test]
    async fn test_build_request_body_uses_max_completion_tokens() {
        let provider = OpenRouterProvider::new();
        
        let model = Model {
            id: "openrouter/auto".to_string(),
            name: "OpenRouter Auto".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: crate::types::ModelCost {
                input: 0.0,
                output: 0.0,
                cache_read: None,
                cache_write: None,
            },
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        };
        
        let context = Context::new(vec![Message::User(UserMessage::new("Hello"))]);
        let options = StreamOptions {
            max_tokens: Some(2048),
            ..Default::default()
        };
        
        let body = provider.build_request_body(&model, &context, &options).unwrap();
        
        // 验证使用 max_completion_tokens 而不是 max_tokens
        assert!(body.get("max_completion_tokens").is_some());
        assert_eq!(body["max_completion_tokens"], 2048);
        
        // 验证没有 store 参数
        assert!(body.get("store").is_none());
        
        // 验证 stream 为 true
        assert_eq!(body["stream"], true);
    }
}
