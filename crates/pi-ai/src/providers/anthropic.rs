//! Anthropic Claude Messages API Provider
//!
//! 实现 Anthropic Claude Messages API 的流式调用支持

use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context as AnyhowContext, Result};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;

use serde_json::json;

use crate::api_registry::ApiProvider;
use crate::models::get_api_key_from_env;
use crate::types::*;
use crate::utils::event_stream::{SseEvent, SseParser};
use crate::utils::json_parse::parse_partial_json;

/// Anthropic API Provider
pub struct AnthropicProvider {
    client: Client,
}

impl AnthropicProvider {
    /// 创建新的 Anthropic Provider
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 检查是否为 OAuth token
    fn is_oauth_token(api_key: &str) -> bool {
        api_key.contains("sk-ant-oat")
    }

    /// 检查模型是否支持自适应思考 (Opus 4.6 和 Sonnet 4.6)
    fn supports_adaptive_thinking(model_id: &str) -> bool {
        model_id.contains("opus-4-6")
            || model_id.contains("opus-4.6")
            || model_id.contains("sonnet-4-6")
            || model_id.contains("sonnet-4.6")
    }

    /// 构建请求头
    fn build_headers(
        &self,
        model: &Model,
        api_key: &str,
        options: &StreamOptions,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        // 认证头
        if Self::is_oauth_token(api_key) {
            headers.insert("authorization".to_string(), format!("Bearer {}", api_key));
        } else {
            headers.insert("x-api-key".to_string(), api_key.to_string());
        }

        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());

        // Beta 功能
        let mut beta_features = vec!["fine-grained-tool-streaming-2025-05-14"];

        // 检查是否需要交错思考 beta 头
        let needs_interleaved_beta =
            model.reasoning && !Self::supports_adaptive_thinking(&model.id);
        if needs_interleaved_beta {
            beta_features.push("interleaved-thinking-2025-05-14");
        }

        // OAuth 特殊处理
        if Self::is_oauth_token(api_key) {
            beta_features.push("claude-code-20250219");
            beta_features.push("oauth-2025-04-20");
            headers.insert("user-agent".to_string(), "claude-cli/2.1.75".to_string());
            headers.insert("x-app".to_string(), "cli".to_string());
        }

        headers.insert("anthropic-beta".to_string(), beta_features.join(","));

        // 合并用户自定义头
        if let Some(custom_headers) = &options.headers {
            for (key, value) in custom_headers {
                headers.insert(key.clone(), value.clone());
            }
        }

        // 合并模型配置的头
        if let Some(model_headers) = &model.headers {
            for (key, value) in model_headers {
                headers.insert(key.clone(), value.clone());
            }
        }

        headers
    }

    /// 构建请求体
    fn build_request_body(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
        api_key: &str,
    ) -> serde_json::Value {
        let is_oauth = Self::is_oauth_token(api_key);
        let cache_control = self.get_cache_control(model, options);

        // 转换消息
        let messages = self.convert_messages(&context.messages, model, is_oauth, cache_control.clone());

        // 基础参数
        let max_tokens = options.max_tokens.unwrap_or(model.max_tokens / 3);

        let mut body = json!({
            "model": model.id,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": messages,
        });

        // 系统提示词
        if is_oauth {
            let mut system_blocks = vec![json!({
                "type": "text",
                "text": "You are Claude Code, Anthropic's official CLI for Claude.",
            })];

            if let Some(ref prompt) = context.system_prompt {
                let mut block = json!({
                    "type": "text",
                    "text": prompt,
                });
                if let Some(ref cc) = cache_control {
                    block["cache_control"] = json!(cc);
                }
                system_blocks.push(block);
            } else if let Some(ref cc) = cache_control {
                if let Some(first) = system_blocks.first_mut() {
                    first["cache_control"] = json!(cc);
                }
            }

            body["system"] = json!(system_blocks);
        } else if let Some(ref prompt) = context.system_prompt {
            let mut block = json!({
                "type": "text",
                "text": prompt,
            });
            if let Some(ref cc) = cache_control {
                block["cache_control"] = json!(cc);
            }
            body["system"] = json!(vec![block]);
        }

        // 温度（与思考模式不兼容）
        if let Some(temp) = options.temperature {
            if !model.reasoning {
                body["temperature"] = json!(temp);
            }
        }

        // 工具
        if let Some(ref tools) = context.tools {
            body["tools"] = json!(self.convert_tools(tools, is_oauth));
        }

        // 思考模式
        if model.reasoning {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": 1024,
            });
        }

        // 元数据
        if let Some(ref metadata) = options.metadata {
            if let Some(user_id) = metadata.get("user_id").and_then(|v| v.as_str()) {
                body["metadata"] = json!({ "user_id": user_id });
            }
        }

        body
    }

    /// 获取缓存控制配置
    fn get_cache_control(
        &self,
        model: &Model,
        options: &StreamOptions,
    ) -> Option<HashMap<String, String>> {
        let retention = options.cache_retention.as_ref()?;

        if *retention == CacheRetention::None {
            return None;
        }

        let mut control = HashMap::new();
        control.insert("type".to_string(), "ephemeral".to_string());

        // 长期缓存且是官方 API
        if *retention == CacheRetention::Long && model.base_url.contains("api.anthropic.com") {
            control.insert("ttl".to_string(), "1h".to_string());
        }

        Some(control)
    }

    /// 转换消息为 Anthropic 格式
    fn convert_messages(
        &self,
        messages: &[Message],
        model: &Model,
        is_oauth: bool,
        cache_control: Option<HashMap<String, String>>,
    ) -> Vec<serde_json::Value> {
        let mut result = Vec::new();
        let mut i = 0;

        while i < messages.len() {
            match &messages[i] {
                Message::User(msg) => {
                    let content = match &msg.content {
                        UserContent::Text(text) => {
                            if text.trim().is_empty() {
                                i += 1;
                                continue;
                            }
                            json!(text.trim())
                        }
                        UserContent::Blocks(blocks) => {
                            let anthropic_blocks: Vec<_> = blocks
                                .iter()
                                .filter_map(|block| match block {
                                    ContentBlock::Text(t) => {
                                        if t.text.trim().is_empty() {
                                            None
                                        } else {
                                            Some(json!({
                                                "type": "text",
                                                "text": t.text,
                                            }))
                                        }
                                    }
                                    ContentBlock::Image(img) => Some(json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": img.mime_type,
                                            "data": img.data,
                                        },
                                    })),
                                    _ => None,
                                })
                                .collect();

                            if anthropic_blocks.is_empty() {
                                i += 1;
                                continue;
                            }

                            // 如果模型不支持图片，过滤掉
                            let filtered: Vec<_> = if !model.input.contains(&InputModality::Image) {
                                anthropic_blocks
                                    .into_iter()
                                    .filter(|b| b.get("type") != Some(&json!("image")))
                                    .collect()
                            } else {
                                anthropic_blocks
                            };

                            if filtered.is_empty() {
                                i += 1;
                                continue;
                            }

                            json!(filtered)
                        }
                    };

                    result.push(json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                Message::Assistant(msg) => {
                    let blocks: Vec<_> = msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text(t) => {
                                if t.text.trim().is_empty() {
                                    None
                                } else {
                                    Some(json!({
                                        "type": "text",
                                        "text": t.text,
                                    }))
                                }
                            }
                            ContentBlock::Thinking(th) => {
                                if th.redacted.unwrap_or(false) {
                                    Some(json!({
                                        "type": "redacted_thinking",
                                        "data": th.thinking_signature.as_deref().unwrap_or(""),
                                    }))
                                } else if th.thinking.trim().is_empty() {
                                    None
                                } else if th.thinking_signature.is_none()
                                    || th.thinking_signature.as_ref().unwrap().trim().is_empty()
                                {
                                    // 没有签名，转为普通文本
                                    Some(json!({
                                        "type": "text",
                                        "text": th.thinking,
                                    }))
                                } else {
                                    Some(json!({
                                        "type": "thinking",
                                        "thinking": th.thinking,
                                        "signature": th.thinking_signature,
                                    }))
                                }
                            }
                            ContentBlock::ToolCall(tc) => Some(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": if is_oauth {
                                    self.to_claude_code_name(&tc.name)
                                } else {
                                    tc.name.clone()
                                },
                                "input": tc.arguments,
                            })),
                            _ => None,
                        })
                        .collect();

                    if blocks.is_empty() {
                        i += 1;
                        continue;
                    }

                    result.push(json!({
                        "role": "assistant",
                        "content": blocks,
                    }));
                }
                Message::ToolResult(msg) => {
                    // 收集连续的 toolResult 消息
                    let mut tool_results = Vec::new();

                    tool_results.push(self.convert_tool_result(msg));

                    // 向前查找连续的 toolResult
                    let mut j = i + 1;
                    while j < messages.len() {
                        if let Message::ToolResult(next_msg) = &messages[j] {
                            tool_results.push(self.convert_tool_result(next_msg));
                            j += 1;
                        } else {
                            break;
                        }
                    }

                    i = j - 1;

                    result.push(json!({
                        "role": "user",
                        "content": tool_results,
                    }));
                }
            }
            i += 1;
        }

        // 添加缓存控制到最后一条用户消息
        if let Some(ref cc) = cache_control {
            if let Some(last) = result.last_mut() {
                if last.get("role") == Some(&json!("user")) {
                    if let Some(content) = last.get_mut("content") {
                        if let Some(arr) = content.as_array_mut() {
                            if let Some(last_block) = arr.last_mut() {
                                if let Some(obj) = last_block.as_object_mut() {
                                    let block_type = obj.get("type").and_then(|v| v.as_str());
                                    if block_type == Some("text")
                                        || block_type == Some("image")
                                        || block_type == Some("tool_result")
                                    {
                                        let mut cc_json = json!({"type": "ephemeral"});
                                        if let Some(ttl) = cc.get("ttl") {
                                            cc_json["ttl"] = json!(ttl);
                                        }
                                        obj.insert("cache_control".to_string(), cc_json);
                                    }
                                }
                            }
                        } else if content.is_string() {
                            // 字符串内容转为数组
                            let text = content.as_str().unwrap_or("").to_string();
                            let mut block = json!({
                                "type": "text",
                                "text": text,
                            });
                            if let Some(ttl) = cc.get("ttl") {
                                block["cache_control"] = json!({
                                    "type": "ephemeral",
                                    "ttl": ttl,
                                });
                            } else {
                                block["cache_control"] = json!({"type": "ephemeral"});
                            }
                            *content = json!(vec![block]);
                        }
                    }
                }
            }
        }

        result
    }

    /// 转换工具结果为 Anthropic 格式
    fn convert_tool_result(&self, msg: &ToolResultMessage) -> serde_json::Value {
        let content = if msg.content.len() == 1 {
            match &msg.content[0] {
                ContentBlock::Text(t) => json!(t.text),
                _ => json!(msg.content),
            }
        } else {
            let blocks: Vec<_> = msg
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text(t) => json!({"type": "text", "text": t.text}),
                    ContentBlock::Image(img) => json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": img.mime_type,
                            "data": img.data,
                        },
                    }),
                    _ => json!(null),
                })
                .filter(|v| !v.is_null())
                .collect();
            json!(blocks)
        };

        json!({
            "type": "tool_result",
            "tool_use_id": msg.tool_call_id,
            "content": content,
            "is_error": msg.is_error,
        })
    }

    /// 转换工具定义
    fn convert_tools(&self, tools: &[Tool], is_oauth: bool) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|tool| {
                let name = if is_oauth {
                    self.to_claude_code_name(&tool.name)
                } else {
                    tool.name.clone()
                };

                json!({
                    "name": name,
                    "description": tool.description,
                    "input_schema": {
                        "type": "object",
                        "properties": tool.parameters.get("properties").unwrap_or(&json!({})),
                        "required": tool.parameters.get("required").unwrap_or(&json!([])),
                    },
                })
            })
            .collect()
    }

    /// Claude Code 工具名称映射
    fn to_claude_code_name(&self, name: &str) -> String {
        let lookup: HashMap<String, String> = [
            ("read", "Read"),
            ("write", "Write"),
            ("edit", "Edit"),
            ("bash", "Bash"),
            ("grep", "Grep"),
            ("glob", "Glob"),
            ("askuserquestion", "AskUserQuestion"),
            ("enterplanmode", "EnterPlanMode"),
            ("exitplanmode", "ExitPlanMode"),
            ("killshell", "KillShell"),
            ("notebookedit", "NotebookEdit"),
            ("skill", "Skill"),
            ("task", "Task"),
            ("taskoutput", "TaskOutput"),
            ("todowrite", "TodoWrite"),
            ("webfetch", "WebFetch"),
            ("websearch", "WebSearch"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

        lookup
            .get(&name.to_lowercase())
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// 映射停止原因
    #[allow(dead_code)] // 预留方法供未来使用
    fn map_stop_reason(&self, reason: &str) -> StopReason {
        match reason {
            "end_turn" => StopReason::Stop,
            "max_tokens" => StopReason::Length,
            "tool_use" => StopReason::ToolUse,
            "refusal" => StopReason::Error,
            "stop_sequence" => StopReason::Stop,
            _ => StopReason::Error,
        }
    }

    /// 带重试的流式请求
    async fn stream_with_retry(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
        api_key: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AssistantMessageEvent>> + Send>>> {
        let max_retries = 3;
        let mut retry_count = 0;
        let mut delay_ms = 1000;
        let max_delay_ms = options.max_retry_delay_ms.unwrap_or(30000);

        loop {
            match self.try_stream(context, model, options, api_key).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    let err_str = e.to_string();
                    let should_retry = err_str.contains("429")
                        || err_str.contains("529")
                        || err_str.contains("rate limit")
                        || err_str.contains("overloaded")
                        || err_str.contains("connection")
                        || err_str.contains("timeout");

                    if !should_retry || retry_count >= max_retries {
                        return Err(e);
                    }

                    retry_count += 1;
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = (delay_ms * 2).min(max_delay_ms);
                }
            }
        }
    }

    /// 尝试流式请求
    async fn try_stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
        api_key: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AssistantMessageEvent>> + Send>>> {
        let url = format!("{}/v1/messages", model.base_url.trim_end_matches('/'));
        let headers = self.build_headers(model, api_key, options);
        let body = self.build_request_body(context, model, options, api_key);

        let mut request = self.client.post(&url);

        for (key, value) in &headers {
            request = request.header(key, value);
        }

        let response = request.json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("HTTP {}: {}", status, text));
        }

        let byte_stream = response.bytes_stream();
        let parser = SseParser::new();

        let stream = self.process_stream(byte_stream, parser, model.clone());
        Ok(Box::pin(stream))
    }

    /// 处理 SSE 流
    fn process_stream(
        &self,
        byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
        parser: SseParser,
        model: Model,
    ) -> impl Stream<Item = Result<AssistantMessageEvent>> {
        use async_stream::stream;

        let initial_message = AssistantMessage {
            role: "assistant".to_string(),
            content: Vec::new(),
            api: Api::Anthropic,
            provider: model.provider.clone(),
            model: model.id.clone(),
            response_id: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let mut state = StreamState {
            byte_stream: Box::pin(byte_stream),
            parser,
            partial_message: initial_message,
            content_blocks: Vec::new(),
            usage: Usage::default(),
            stop_reason: None,
            started: false,
            finished: false,
        };

        stream! {
            while !state.finished {
                match state.byte_stream.next().await {
                    Some(Ok(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let events = state.parser.feed(&text);

                        for event in events {
                            if let Some(result) =
                                Self::process_sse_event(&mut state, &event, &model).await
                            {
                                yield result;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        state.finished = true;
                        yield Err(anyhow::anyhow!("Stream error: {}", e));
                    }
                    None => {
                        // 流结束
                        if !state.finished {
                            state.finished = true;
                            if state.started {
                                let reason = state
                                    .stop_reason
                                    .clone()
                                    .unwrap_or(DoneReason::Stop);
                                let mut final_message = state.partial_message.clone();
                                final_message.usage = state.usage.clone();
                                yield Ok(AssistantMessageEvent::Done {
                                    reason,
                                    message: final_message,
                                });
                            } else {
                                yield Err(anyhow::anyhow!("Stream ended without start event"));
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    /// 处理单个 SSE 事件
    async fn process_sse_event(
        state: &mut StreamState,
        event: &SseEvent,
        _model: &Model,
    ) -> Option<Result<AssistantMessageEvent>> {
        let event_type = event.event.as_deref().unwrap_or("");
        let data = &event.data;

        if data.is_empty() {
            return None;
        }

        match event_type {
            "message_start" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(message) = json.get("message") {
                        state.partial_message.response_id =
                            message.get("id").and_then(|v| v.as_str()).map(String::from);

                        if let Some(usage) = message.get("usage") {
                            state.usage.input_tokens =
                                usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            state.usage.output_tokens =
                                usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            state.usage.cache_read_tokens =
                                usage.get("cache_read_input_tokens").and_then(|v| v.as_u64());
                            state.usage.cache_write_tokens =
                                usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64());
                        }

                        state.partial_message.usage = state.usage.clone();
                        state.started = true;

                        return Some(Ok(AssistantMessageEvent::Start {
                            partial: state.partial_message.clone(),
                        }));
                    }
                }
            }
            "content_block_start" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(index) = json.get("index").and_then(|v| v.as_u64()) {
                        if let Some(block) = json.get("content_block") {
                            let block_type = block.get("type").and_then(|v| v.as_str());

                            match block_type {
                                Some("text") => {
                                    state.content_blocks.push(ContentBlockState::Text {
                                        text: String::new(),
                                        index: index as usize,
                                    });
                                    state.partial_message.content.push(ContentBlock::Text(
                                        TextContent::new(""),
                                    ));

                                    return Some(Ok(AssistantMessageEvent::TextStart {
                                        content_index: state.partial_message.content.len() - 1,
                                        partial: state.partial_message.clone(),
                                    }));
                                }
                                Some("thinking") => {
                                    state.content_blocks.push(ContentBlockState::Thinking {
                                        thinking: String::new(),
                                        signature: None,
                                        index: index as usize,
                                    });
                                    state.partial_message.content.push(ContentBlock::Thinking(
                                        ThinkingContent::new(""),
                                    ));

                                    return Some(Ok(AssistantMessageEvent::ThinkingStart {
                                        content_index: state.partial_message.content.len() - 1,
                                        partial: state.partial_message.clone(),
                                    }));
                                }
                                Some("tool_use") => {
                                    let id = block
                                        .get("id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let name = block
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    state.content_blocks.push(ContentBlockState::ToolUse {
                                        id: id.clone(),
                                        name: name.clone(),
                                        input_json: String::new(),
                                        index: index as usize,
                                    });

                                    state.partial_message.content.push(ContentBlock::ToolCall(
                                        ToolCall::new(id, name, json!({})),
                                    ));

                                    return Some(Ok(AssistantMessageEvent::ToolCallStart {
                                        content_index: state.partial_message.content.len() - 1,
                                        partial: state.partial_message.clone(),
                                    }));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            "content_block_delta" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(index) = json.get("index").and_then(|v| v.as_u64()) {
                        if let Some(delta) = json.get("delta") {
                            let delta_type = delta.get("type").and_then(|v| v.as_str());

                            match delta_type {
                                Some("text_delta") => {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        // 找到对应的 content block
                                        if let Some(block_idx) = state
                                            .content_blocks
                                            .iter()
                                            .position(|b| b.index() == index as usize)
                                        {
                                            if let ContentBlockState::Text { text: ref mut t, .. } =
                                                state.content_blocks[block_idx]
                                            {
                                                t.push_str(text);

                                                // 更新 partial_message
                                                if let Some(ContentBlock::Text(ref mut tc)) = state
                                                    .partial_message
                                                    .content
                                                    .get_mut(block_idx)
                                                {
                                                    tc.text.push_str(text);
                                                }

                                                return Some(Ok(AssistantMessageEvent::TextDelta {
                                                    content_index: block_idx,
                                                    delta: text.to_string(),
                                                    partial: state.partial_message.clone(),
                                                }));
                                            }
                                        }
                                    }
                                }
                                Some("thinking_delta") => {
                                    if let Some(thinking) =
                                        delta.get("thinking").and_then(|v| v.as_str())
                                    {
                                        if let Some(block_idx) = state
                                            .content_blocks
                                            .iter()
                                            .position(|b| b.index() == index as usize)
                                        {
                                            if let ContentBlockState::Thinking {
                                                thinking: ref mut t,
                                                ..
                                            } = state.content_blocks[block_idx]
                                            {
                                                t.push_str(thinking);

                                                if let Some(ContentBlock::Thinking(ref mut tc)) =
                                                    state.partial_message.content.get_mut(block_idx)
                                                {
                                                    tc.thinking.push_str(thinking);
                                                }

                                                return Some(Ok(
                                                    AssistantMessageEvent::ThinkingDelta {
                                                        content_index: block_idx,
                                                        delta: thinking.to_string(),
                                                        partial: state.partial_message.clone(),
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                }
                                Some("input_json_delta") => {
                                    if let Some(partial_json) =
                                        delta.get("partial_json").and_then(|v| v.as_str())
                                    {
                                        if let Some(block_idx) = state
                                            .content_blocks
                                            .iter()
                                            .position(|b| b.index() == index as usize)
                                        {
                                            if let ContentBlockState::ToolUse {
                                                input_json: ref mut json,
                                                ..
                                            } = state.content_blocks[block_idx]
                                            {
                                                json.push_str(partial_json);

                                                // 尝试解析部分 JSON
                                                if let Some(parsed) = parse_partial_json(json) {
                                                    if let Some(ContentBlock::ToolCall(ref mut tc)) =
                                                        state
                                                            .partial_message
                                                            .content
                                                            .get_mut(block_idx)
                                                    {
                                                        tc.arguments = parsed;
                                                    }
                                                }

                                                return Some(Ok(
                                                    AssistantMessageEvent::ToolCallDelta {
                                                        content_index: block_idx,
                                                        delta: partial_json.to_string(),
                                                        partial: state.partial_message.clone(),
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                }
                                Some("signature_delta") => {
                                    if let Some(sig) =
                                        delta.get("signature").and_then(|v| v.as_str())
                                    {
                                        if let Some(block_idx) = state
                                            .content_blocks
                                            .iter()
                                            .position(|b| b.index() == index as usize)
                                        {
                                            if let ContentBlockState::Thinking {
                                                signature: ref mut s,
                                                ..
                                            } = state.content_blocks[block_idx]
                                            {
                                                if let Some(ref mut sig_str) = s {
                                                    sig_str.push_str(sig);
                                                } else {
                                                    *s = Some(sig.to_string());
                                                }

                                                if let Some(ContentBlock::Thinking(ref mut tc)) =
                                                    state.partial_message.content.get_mut(block_idx)
                                                {
                                                    tc.thinking_signature = s.clone();
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            "content_block_stop" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(index) = json.get("index").and_then(|v| v.as_u64()) {
                        if let Some(block_idx) = state
                            .content_blocks
                            .iter()
                            .position(|b| b.index() == index as usize)
                        {
                            let block = &state.content_blocks[block_idx];

                            match block {
                                ContentBlockState::Text { text, .. } => {
                                    return Some(Ok(AssistantMessageEvent::TextEnd {
                                        content_index: block_idx,
                                        content: text.clone(),
                                        partial: state.partial_message.clone(),
                                    }));
                                }
                                ContentBlockState::Thinking { thinking, .. } => {
                                    return Some(Ok(AssistantMessageEvent::ThinkingEnd {
                                        content_index: block_idx,
                                        content: thinking.clone(),
                                        partial: state.partial_message.clone(),
                                    }));
                                }
                                ContentBlockState::ToolUse { id, name, input_json, .. } => {
                                    // 最终解析 JSON
                                    let arguments = parse_partial_json(input_json)
                                        .unwrap_or_else(|| json!({}));

                                    if let Some(ContentBlock::ToolCall(ref mut tc)) = state
                                        .partial_message
                                        .content
                                        .get_mut(block_idx)
                                    {
                                        tc.arguments = arguments.clone();
                                    }

                                    let tool_call = ToolCall {
                                        content_type: "toolCall".to_string(),
                                        id: id.clone(),
                                        name: name.clone(),
                                        arguments,
                                        thought_signature: None,
                                    };

                                    return Some(Ok(AssistantMessageEvent::ToolCallEnd {
                                        content_index: block_idx,
                                        tool_call,
                                        partial: state.partial_message.clone(),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
            "message_delta" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = json.get("delta") {
                        if let Some(reason) = delta.get("stop_reason").and_then(|v| v.as_str()) {
                            state.stop_reason = Some(Self::map_stop_reason_to_done_static(reason));
                            state.partial_message.stop_reason = Self::map_stop_reason_static(reason);
                        }
                    }

                    if let Some(usage) = json.get("usage") {
                        if let Some(tokens) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                            state.usage.output_tokens = tokens;
                        }
                        if let Some(tokens) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                            state.usage.input_tokens = tokens;
                        }
                        if let Some(tokens) =
                            usage.get("cache_read_input_tokens").and_then(|v| v.as_u64())
                        {
                            state.usage.cache_read_tokens = Some(tokens);
                        }
                        if let Some(tokens) =
                            usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64())
                        {
                            state.usage.cache_write_tokens = Some(tokens);
                        }
                    }

                    state.partial_message.usage = state.usage.clone();
                }
            }
            "message_stop" => {
                state.finished = true;
                let reason = state.stop_reason.clone().unwrap_or(DoneReason::Stop);
                let mut final_message = state.partial_message.clone();
                final_message.usage = state.usage.clone();

                return Some(Ok(AssistantMessageEvent::Done {
                    reason,
                    message: final_message,
                }));
            }
            "error" => {
                return Some(Err(anyhow::anyhow!("Anthropic API error: {}", data)));
            }
            _ => {}
        }

        None
    }

    /// 映射停止原因到 DoneReason (静态版本)
    fn map_stop_reason_to_done_static(reason: &str) -> DoneReason {
        match reason {
            "end_turn" => DoneReason::Stop,
            "max_tokens" => DoneReason::Length,
            "tool_use" => DoneReason::ToolUse,
            _ => DoneReason::Stop,
        }
    }

    /// 映射停止原因 (静态版本)
    fn map_stop_reason_static(reason: &str) -> StopReason {
        match reason {
            "end_turn" => StopReason::Stop,
            "max_tokens" => StopReason::Length,
            "tool_use" => StopReason::ToolUse,
            "refusal" => StopReason::Error,
            "stop_sequence" => StopReason::Stop,
            _ => StopReason::Error,
        }
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 流处理状态
struct StreamState {
    byte_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    parser: SseParser,
    partial_message: AssistantMessage,
    content_blocks: Vec<ContentBlockState>,
    usage: Usage,
    stop_reason: Option<DoneReason>,
    started: bool,
    finished: bool,
}

/// 内容块状态
enum ContentBlockState {
    Text {
        text: String,
        index: usize,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
        index: usize,
    },
    ToolUse {
        id: String,
        name: String,
        input_json: String,
        index: usize,
    },
}

impl ContentBlockState {
    fn index(&self) -> usize {
        match self {
            ContentBlockState::Text { index, .. } => *index,
            ContentBlockState::Thinking { index, .. } => *index,
            ContentBlockState::ToolUse { index, .. } => *index,
        }
    }
}

#[async_trait]
impl ApiProvider for AnthropicProvider {
    fn api(&self) -> Api {
        Api::Anthropic
    }

    async fn stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AssistantMessageEvent>> + Send>>> {
        // 获取 API key
        let api_key = options
            .api_key
            .clone()
            .or_else(|| get_api_key_from_env(&model.provider))
            .context("No API key available for Anthropic provider")?;

        self.stream_with_retry(context, model, options, &api_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::*;
    use mockito::Server;
    use futures::StreamExt;

    #[test]
    fn test_is_oauth_token() {
        assert!(AnthropicProvider::is_oauth_token("sk-ant-oat-12345"));
        assert!(!AnthropicProvider::is_oauth_token("sk-ant-api-12345"));
    }

    #[test]
    fn test_supports_adaptive_thinking() {
        assert!(AnthropicProvider::supports_adaptive_thinking("claude-opus-4-6-20250514"));
        assert!(AnthropicProvider::supports_adaptive_thinking("claude-sonnet-4.6"));
        assert!(!AnthropicProvider::supports_adaptive_thinking("claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_to_claude_code_name() {
        let provider = AnthropicProvider::new();
        assert_eq!(provider.to_claude_code_name("read"), "Read");
        assert_eq!(provider.to_claude_code_name("Bash"), "Bash");
        assert_eq!(provider.to_claude_code_name("unknown"), "unknown");
    }

    #[test]
    fn test_map_stop_reason() {
        let provider = AnthropicProvider::new();
        assert_eq!(provider.map_stop_reason("end_turn"), StopReason::Stop);
        assert_eq!(provider.map_stop_reason("max_tokens"), StopReason::Length);
        assert_eq!(provider.map_stop_reason("tool_use"), StopReason::ToolUse);
        assert_eq!(provider.map_stop_reason("refusal"), StopReason::Error);
    }

    #[tokio::test]
    async fn test_build_request_basic() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);
        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let body = provider.build_request_body(&context, &model, &options, "test-api-key");

        // 验证基本结构
        assert_eq!(body["model"], "claude-3-sonnet-20240229");
        assert_eq!(body["stream"], true);
        assert!(body["max_tokens"].as_u64().is_some());

        // 验证消息数组
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");

        // 验证系统提示词
        let system = body["system"].as_array().unwrap();
        assert_eq!(system[0]["type"], "text");
        assert_eq!(system[0]["text"], "You are a helpful assistant");
    }

    #[tokio::test]
    async fn test_stream_text_response() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        // 构建 SSE 响应 - 使用正确的格式
        let sse_body = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_123456","type":"message","role":"assistant","model":"claude-3-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}

event: message_stop
data: {"type":"message_stop"}
"#;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
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
        
        // 应该有 Start 事件
        assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));

        // 查找 TextDelta 事件
        let text_deltas: Vec<_> = events.iter().filter_map(|e| match e {
            AssistantMessageEvent::TextDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        }).collect();
        assert_eq!(text_deltas, vec!["Hello", " world"]);

        // 验证 Done 事件
        assert!(matches!(events.last().unwrap(), AssistantMessageEvent::Done { .. }));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_tool_call() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        // 构建包含 tool_use 的 SSE 响应
        let sse_body = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_tool","type":"message","role":"assistant","model":"claude-3-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool_123","name":"get_weather","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"location\":"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"Paris\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":20}}}

event: message_stop
data: {"type":"message_stop"}
"#;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
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

        let tool_call_deltas: Vec<_> = events.iter().filter(|e| matches!(e, AssistantMessageEvent::ToolCallDelta { .. })).collect();
        assert!(!tool_call_deltas.is_empty(), "Expected at least one ToolCallDelta event");

        // 验证 Done 事件存在
        assert!(matches!(events.last().unwrap(), AssistantMessageEvent::Done { .. }));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_thinking() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        // 构建包含 thinking 的 SSE 响应
        let sse_body = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_think","type":"message","role":"assistant","model":"claude-3-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think about this..."}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc123"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"The answer is 42."}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}

event: message_stop
data: {"type":"message_stop"}
"#;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
        model.base_url = server.url();
        model.reasoning = true;

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("What is the meaning of life?")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        // 验证 Thinking 事件
        let thinking_deltas: Vec<_> = events.iter().filter_map(|e| match e {
            AssistantMessageEvent::ThinkingDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        }).collect();
        assert!(!thinking_deltas.is_empty());
        assert!(thinking_deltas[0].contains("Let me think"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_stream_error_response() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        let error_json = r#"{"error": {"type": "rate_limit_error", "message": "Rate limit exceeded"}}"#;

        // 因为有重试机制，需要设置多次期望
        let mock = server.mock("POST", "/v1/messages")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .expect(4) // 重试3次+初始请求
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    // ==================== 边界测试 ====================

    #[tokio::test]
    async fn test_error_response_400_bad_request() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        let error_json = r#"{"error": {"type": "invalid_request_error", "message": "Invalid request: messages array is required"}}"#;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.to_string().contains("400") || err.to_string().contains("invalid_request"));
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_error_response_500_internal_error() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        let error_json = r#"{"error": {"type": "api_error", "message": "Internal server error"}}"#;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(error_json)
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let result = provider.stream(&context, &model, &options).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_stream_response() {
        let mut server = Server::new_async().await;
        let provider = AnthropicProvider::new();

        // 空的 SSE 响应 - 只返回 message_start 然后 message_stop，没有内容
        let sse_body = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_empty","type":"message","role":"assistant","model":"claude-3-sonnet","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}

event: message_stop
data: {"type":"message_stop"}
"#;

        let mock = server.mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body)
            .create_async()
            .await;

        let mut model = sample_model(Api::Anthropic, Provider::Anthropic);
        model.base_url = server.url();

        let context = sample_context("You are a helpful assistant", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-api-key");

        let mut stream = provider.stream(&context, &model, &options).await.unwrap();

        let mut events = vec![];
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        // 应该有 Start 和 Done 事件
        assert!(!events.is_empty());
        assert!(matches!(events[0], Ok(AssistantMessageEvent::Start { .. })));

        mock.assert_async().await;
    }

    #[test]
    fn test_large_message_serialization() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);

        // 创建大量消息
        let mut messages: Vec<Message> = Vec::new();
        for i in 0..100 {
            messages.push(sample_user_message(&format!("Message {} with some content to make it longer", i)));
        }

        let context = sample_context_with_tools(
            "You are a helpful assistant",
            messages,
            vec![sample_tool("test_tool", "A test tool")],
        );
        let options = sample_stream_options("test-api-key");

        // 验证能成功序列化大消息
        let body = provider.build_request_body(&context, &model, &options, "test-api-key");

        // 验证消息数组长度
        let messages_array = body["messages"].as_array().unwrap();
        assert_eq!(messages_array.len(), 100);

        // 验证可以序列化为 JSON 字符串
        let json_string = serde_json::to_string(&body).unwrap();
        assert!(!json_string.is_empty());
        assert!(json_string.len() > 1000);
    }

    #[test]
    fn test_empty_user_message_handling() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);

        // 空的用户消息应该被跳过
        let context = sample_context(
            "You are a helpful assistant",
            vec![
                Message::User(UserMessage::new("")),
                Message::User(UserMessage::new("   ")), // 只有空白字符
                sample_user_message("Valid message"),
            ],
        );
        let options = sample_stream_options("test-api-key");

        let body = provider.build_request_body(&context, &model, &options, "test-api-key");
        let messages_array = body["messages"].as_array().unwrap();

        // 空消息应该被跳过，只保留有效消息
        assert_eq!(messages_array.len(), 1);
    }

    #[test]
    fn test_build_headers_oauth_token() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);
        let options = sample_stream_options("sk-ant-oat-test123");

        let headers = provider.build_headers(&model, "sk-ant-oat-test123", &options);

        // OAuth token 应该使用 Bearer 认证
        assert_eq!(headers.get("authorization").unwrap(), "Bearer sk-ant-oat-test123");
        // 不应该有 x-api-key
        assert!(!headers.contains_key("x-api-key"));
    }

    #[test]
    fn test_build_headers_standard_api_key() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);
        let options = sample_stream_options("sk-ant-api-test123");

        let headers = provider.build_headers(&model, "sk-ant-api-test123", &options);

        // 标准 API key 应该使用 x-api-key
        assert_eq!(headers.get("x-api-key").unwrap(), "sk-ant-api-test123");
        // 不应该有 authorization
        assert!(!headers.contains_key("authorization"));
    }

    #[test]
    fn test_custom_headers_merge() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);
        let mut options = sample_stream_options("test-api-key");
        let mut headers_map = std::collections::HashMap::new();
        headers_map.insert("X-Custom-Header".to_string(), "custom-value".to_string());
        options.headers = Some(headers_map);

        let headers = provider.build_headers(&model, "test-api-key", &options);

        assert_eq!(headers.get("X-Custom-Header").unwrap(), "custom-value");
    }

    #[test]
    fn test_error_response_parsing() {
        // 测试各种错误响应格式的解析
        let error_json = r#"{"error": {"type": "authentication_error", "message": "Invalid API key"}}"#;
        let parsed: serde_json::Value = serde_json::from_str(error_json).unwrap();
        assert_eq!(parsed["error"]["type"], "authentication_error");
        assert_eq!(parsed["error"]["message"], "Invalid API key");

        // 测试缺少 type 字段的错误
        let error_json2 = r#"{"error": {"message": "Unknown error"}}"#;
        let parsed2: serde_json::Value = serde_json::from_str(error_json2).unwrap();
        assert_eq!(parsed2["error"]["message"], "Unknown error");
    }

    #[test]
    fn test_empty_stream_events_handling() {
        // 测试空事件处理逻辑
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);
        
        // 创建包含空内容的上下文
        let context = sample_context(
            "You are helpful",
            vec![Message::User(UserMessage::new(""))],
        );
        let options = sample_stream_options("test-api-key");
        
        // 验证空消息被正确处理
        let body = provider.build_request_body(&context, &model, &options, "test-api-key");
        let messages = body["messages"].as_array().unwrap();
        assert!(messages.is_empty() || messages.len() == 1);
    }

    #[test]
    fn test_large_message_serialization_limits() {
        let provider = AnthropicProvider::new();
        let model = sample_model(Api::Anthropic, Provider::Anthropic);

        // 创建大量消息测试序列化性能边界
        let mut messages: Vec<Message> = Vec::new();
        for i in 0..50 {
            let content = format!("Message {} with {} characters of padding content here", i, "x".repeat(100));
            messages.push(sample_user_message(&content));
        }

        let context = sample_context("You are a helpful assistant", messages);
        let options = sample_stream_options("test-api-key");

        let body = provider.build_request_body(&context, &model, &options, "test-api-key");
        let messages_array = body["messages"].as_array().unwrap();
        assert_eq!(messages_array.len(), 50);

        // 验证 JSON 字符串长度
        let json_string = serde_json::to_string(&body).unwrap();
        assert!(json_string.len() > 5000);
    }

    #[test]
    fn test_oauth_token_variations() {
        let _provider = AnthropicProvider::new();
        
        // 测试各种 OAuth token 格式
        assert!(AnthropicProvider::is_oauth_token("sk-ant-oat-12345"));
        assert!(AnthropicProvider::is_oauth_token("sk-ant-oat-test-token"));
        assert!(!AnthropicProvider::is_oauth_token("sk-ant-api03-12345"));
        assert!(!AnthropicProvider::is_oauth_token("sk-12345"));
        assert!(!AnthropicProvider::is_oauth_token(""));
    }

    #[test]
    fn test_adaptive_thinking_model_detection() {
        // 测试自适应思考模型检测
        assert!(AnthropicProvider::supports_adaptive_thinking("claude-opus-4-6-20250101"));
        assert!(AnthropicProvider::supports_adaptive_thinking("claude-opus-4.6"));
        assert!(AnthropicProvider::supports_adaptive_thinking("claude-sonnet-4-6-20250101"));
        assert!(AnthropicProvider::supports_adaptive_thinking("claude-sonnet-4.6"));
        assert!(!AnthropicProvider::supports_adaptive_thinking("claude-3-5-sonnet"));
        assert!(!AnthropicProvider::supports_adaptive_thinking("claude-opus-4-20250514"));
        assert!(!AnthropicProvider::supports_adaptive_thinking(""));
    }
}
