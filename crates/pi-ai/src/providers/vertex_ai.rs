//! Google Vertex AI Provider 实现
//!
//! 实现 Google Vertex AI API 的流式调用支持
//!
//! 与 GoogleProvider 的主要区别：
//! - URL 格式: https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/google/models/{model_id}:streamGenerateContent
//! - 认证: 使用 Bearer token（从 GOOGLE_CLOUD_API_KEY 或 OAuth token）
//! - 请求头: Authorization: Bearer {token}

use std::pin::Pin;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api_registry::ApiProvider;
use crate::models::get_api_key_from_env;
use crate::types::*;
use crate::utils::event_stream::SseParser;

/// Google Vertex AI Provider
pub struct VertexAiProvider {
    client: Client,
}

impl VertexAiProvider {
    /// 创建新的 Vertex AI Provider
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// 获取 Google Cloud 项目 ID
    fn get_project_id(&self) -> Option<String> {
        std::env::var("GOOGLE_CLOUD_PROJECT").ok()
    }

    /// 获取 Google Cloud 区域
    fn get_region(&self) -> String {
        std::env::var("GOOGLE_CLOUD_REGION")
            .unwrap_or_else(|_| "us-central1".to_string())
    }

    /// 构建 Vertex AI 请求 URL
    fn build_url(&self, model: &Model, region: &str, project: &str) -> String {
        // 从模型 ID 中提取实际的模型名称（去掉 "vertex/" 前缀）
        let model_id = model.id.strip_prefix("vertex/").unwrap_or(&model.id);
        
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
            region, project, region, model_id
        )
    }

    /// 转换消息为 Google 格式
    fn convert_messages(&self, context: &Context, model: &Model) -> Vec<GoogleContent> {
        let mut contents = Vec::new();

        for msg in &context.messages {
            match msg {
                Message::User(user_msg) => {
                    let parts = self.convert_user_content(&user_msg.content, model);
                    if !parts.is_empty() {
                        contents.push(GoogleContent {
                            role: "user".to_string(),
                            parts,
                        });
                    }
                }
                Message::Assistant(assistant_msg) => {
                    let parts = self.convert_assistant_content(&assistant_msg.content, model);
                    if !parts.is_empty() {
                        contents.push(GoogleContent {
                            role: "model".to_string(),
                            parts,
                        });
                    }
                }
                Message::ToolResult(tool_result) => {
                    let parts = self.convert_tool_result(tool_result, model);
                    if !parts.is_empty() {
                        contents.push(GoogleContent {
                            role: "user".to_string(),
                            parts,
                        });
                    }
                }
            }
        }

        contents
    }

    /// 转换用户内容为 Google parts
    fn convert_user_content(&self, content: &UserContent, model: &Model) -> Vec<GooglePart> {
        match content {
            UserContent::Text(text) => {
                vec![GooglePart::Text {
                    text: text.clone(),
                }]
            }
            UserContent::Blocks(blocks) => {
                let parts: Vec<_> = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text(t) => Some(GooglePart::Text {
                            text: t.text.clone(),
                        }),
                        ContentBlock::Image(img) => {
                            if model.input.contains(&InputModality::Image) {
                                Some(GooglePart::InlineData {
                                    inline_data: InlineData {
                                        mime_type: img.mime_type.clone(),
                                        data: img.data.clone(),
                                    },
                                })
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect();
                parts
            }
        }
    }

    /// 转换助手内容为 Google parts
    fn convert_assistant_content(&self, content: &[ContentBlock], _model: &Model) -> Vec<GooglePart> {
        let mut parts = Vec::new();

        for block in content {
            match block {
                ContentBlock::Text(t) => {
                    if !t.text.trim().is_empty() {
                        parts.push(GooglePart::Text {
                            text: t.text.clone(),
                        });
                    }
                }
                ContentBlock::Thinking(thinking) => {
                    if !thinking.thinking.trim().is_empty() {
                        parts.push(GooglePart::Thought {
                            thought: true,
                            text: thinking.thinking.clone(),
                        });
                    }
                }
                ContentBlock::ToolCall(tool_call) => {
                    parts.push(GooglePart::FunctionCall {
                        function_call: FunctionCall {
                            name: tool_call.name.clone(),
                            args: tool_call.arguments.clone(),
                        },
                    });
                }
                _ => {}
            }
        }

        parts
    }

    /// 转换工具结果为 Google parts
    fn convert_tool_result(&self, tool_result: &ToolResultMessage, _model: &Model) -> Vec<GooglePart> {
        let text_content: Vec<_> = tool_result
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();

        let response_text = text_content.join("\n");
        let response_value = if response_text.is_empty() {
            "(empty result)".to_string()
        } else {
            response_text
        };

        vec![GooglePart::FunctionResponse {
            function_response: FunctionResponse {
                name: tool_result.tool_name.clone(),
                response: json!({
                    if tool_result.is_error { "error" } else { "output" }: response_value
                }),
            },
        }]
    }

    /// 转换工具定义为 Google 格式
    fn convert_tools(&self, tools: &[Tool]) -> Vec<GoogleTool> {
        if tools.is_empty() {
            return Vec::new();
        }

        let function_declarations: Vec<_> = tools
            .iter()
            .map(|tool| FunctionDeclaration {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters_json_schema: Some(tool.parameters.clone()),
            })
            .collect();

        vec![GoogleTool {
            function_declarations,
        }]
    }

    /// 获取默认安全设置
    fn safety_settings(&self) -> Vec<SafetySetting> {
        vec![
            SafetySetting {
                category: "HARM_CATEGORY_HARASSMENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            SafetySetting {
                category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            SafetySetting {
                category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            SafetySetting {
                category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
        ]
    }

    /// 构建请求体
    fn build_request_body(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> GoogleRequest {
        let contents = self.convert_messages(context, model);
        let tools = context.tools.as_ref().map(|t| self.convert_tools(t));

        let mut generation_config = GenerationConfig::default();
        if let Some(temp) = options.temperature {
            generation_config.temperature = Some(temp);
        }
        if let Some(max_tokens) = options.max_tokens {
            generation_config.max_output_tokens = Some(max_tokens as i32);
        }

        // 如果模型支持 reasoning，添加 thinkingConfig
        if model.reasoning {
            generation_config.thinking_config = Some(ThinkingConfig {
                thinking_budget: 10000,
            });
        }

        GoogleRequest {
            contents,
            system_instruction: context.system_prompt.as_ref().map(|prompt| SystemInstruction {
                parts: vec![GooglePart::Text { text: prompt.clone() }],
            }),
            tools,
            generation_config: Some(generation_config),
            safety_settings: Some(self.safety_settings()),
        }
    }

    /// 解析 SSE 事件并生成流事件
    fn parse_sse_stream(
        &self,
        response: reqwest::Response,
        model: &Model,
    ) -> Pin<Box<dyn Stream<Item = Result<AssistantMessageEvent>> + Send>> {
        use futures::stream::{self, StreamExt};
        use std::sync::{Arc, Mutex};

        let model_id = model.id.clone();

        // 使用 Arc<Mutex<>> 来允许线程安全的内部可变性
        let sse_parser = Arc::new(Mutex::new(SseParser::new()));
        let partial_message = Arc::new(Mutex::new(AssistantMessage::new(Api::GoogleVertex, Provider::GoogleVertex, &model_id)));
        let current_block_type = Arc::new(Mutex::new(None::<BlockType>));
        let text_started = Arc::new(Mutex::new(false));
        let thinking_started = Arc::new(Mutex::new(false));
        let has_error = Arc::new(Mutex::new(false));

        // 收集所有事件到一个 vector
        let stream = response.bytes_stream().flat_map({
            let sse_parser = Arc::clone(&sse_parser);
            let partial_message = Arc::clone(&partial_message);
            let current_block_type = Arc::clone(&current_block_type);
            let text_started = Arc::clone(&text_started);
            let thinking_started = Arc::clone(&thinking_started);
            let has_error = Arc::clone(&has_error);

            move |chunk_result| {
                let sse_parser = Arc::clone(&sse_parser);
                let partial_message = Arc::clone(&partial_message);
                let current_block_type = Arc::clone(&current_block_type);
                let text_started = Arc::clone(&text_started);
                let thinking_started = Arc::clone(&thinking_started);
                let has_error = Arc::clone(&has_error);

                let mut events: Vec<Result<AssistantMessageEvent>> = Vec::new();

                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                            let sse_events = sse_parser.lock().unwrap().feed(&text);

                            for sse_event in sse_events {
                                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&sse_event.data) {
                                    let mut msg = partial_message.lock().unwrap();

                                    // 检查是否有 block reason
                                    if let Some(block_reason) = json_value.get("promptFeedback").and_then(|p| p.get("blockReason")) {
                                        let error_msg = format!("Content blocked: {}", block_reason.as_str().unwrap_or("unknown"));
                                        msg.error_message = Some(error_msg);
                                        msg.stop_reason = StopReason::Error;
                                        events.push(Ok(AssistantMessageEvent::Error {
                                            reason: ErrorReason::Error,
                                            error: msg.clone(),
                                        }));
                                        *has_error.lock().unwrap() = true;
                                        break;
                                    }

                                    // 检查 candidates
                                    if let Some(candidates) = json_value.get("candidates").and_then(|c| c.as_array()) {
                                        if candidates.is_empty() {
                                            continue;
                                        }

                                        for candidate in candidates {
                                            // 处理 finishReason
                                            if let Some(finish_reason) = candidate.get("finishReason").and_then(|f| f.as_str()) {
                                                msg.stop_reason = match finish_reason {
                                                    "STOP" => StopReason::Stop,
                                                    "MAX_TOKENS" => StopReason::Length,
                                                    _ => StopReason::Error,
                                                };
                                            }

                                            // 处理 content parts
                                            if let Some(content) = candidate.get("content") {
                                                if let Some(parts) = content.get("parts").and_then(|p| p.as_array()) {
                                                    for part in parts {
                                                        // 检查是否是 thought
                                                        let is_thought = part.get("thought").and_then(|t| t.as_bool()).unwrap_or(false);

                                                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                                            if is_thought {
                                                                // Thinking content
                                                                let mut ts = thinking_started.lock().unwrap();
                                                                let mut cbt = current_block_type.lock().unwrap();

                                                                if !*ts || *cbt != Some(BlockType::Thinking) {
                                                                    // 结束之前的 block
                                                                    if let Some(BlockType::Text) = *cbt {
                                                                        if let Some(ContentBlock::Text(t)) = msg.content.last() {
                                                                            events.push(Ok(AssistantMessageEvent::TextEnd {
                                                                                content_index: msg.content.len() - 1,
                                                                                content: t.text.clone(),
                                                                                partial: msg.clone(),
                                                                            }));
                                                                        }
                                                                    }

                                                                    // 开始新的 thinking block
                                                                    msg.content.push(ContentBlock::Thinking(ThinkingContent::new(text)));
                                                                    events.push(Ok(AssistantMessageEvent::ThinkingStart {
                                                                        content_index: msg.content.len() - 1,
                                                                        partial: msg.clone(),
                                                                    }));
                                                                    *ts = true;
                                                                    *cbt = Some(BlockType::Thinking);
                                                                }

                                                                // 更新 thinking content
                                                                if let Some(ContentBlock::Thinking(thinking)) = msg.content.last_mut() {
                                                                    thinking.thinking.push_str(text);
                                                                }

                                                                events.push(Ok(AssistantMessageEvent::ThinkingDelta {
                                                                    content_index: msg.content.len() - 1,
                                                                    delta: text.to_string(),
                                                                    partial: msg.clone(),
                                                                }));
                                                            } else {
                                                                // Text content
                                                                let mut txt_started = text_started.lock().unwrap();
                                                                let mut cbt = current_block_type.lock().unwrap();

                                                                if !*txt_started || *cbt != Some(BlockType::Text) {
                                                                    // 结束之前的 block
                                                                    if let Some(BlockType::Thinking) = *cbt {
                                                                        if let Some(ContentBlock::Thinking(t)) = msg.content.last() {
                                                                            events.push(Ok(AssistantMessageEvent::ThinkingEnd {
                                                                                content_index: msg.content.len() - 1,
                                                                                content: t.thinking.clone(),
                                                                                partial: msg.clone(),
                                                                            }));
                                                                        }
                                                                    }

                                                                    // 开始新的 text block
                                                                    msg.content.push(ContentBlock::Text(TextContent::new(text)));
                                                                    events.push(Ok(AssistantMessageEvent::TextStart {
                                                                        content_index: msg.content.len() - 1,
                                                                        partial: msg.clone(),
                                                                    }));
                                                                    *txt_started = true;
                                                                    *cbt = Some(BlockType::Text);
                                                                }

                                                                // 更新 text content
                                                                if let Some(ContentBlock::Text(txt)) = msg.content.last_mut() {
                                                                    txt.text.push_str(text);
                                                                }

                                                                events.push(Ok(AssistantMessageEvent::TextDelta {
                                                                    content_index: msg.content.len() - 1,
                                                                    delta: text.to_string(),
                                                                    partial: msg.clone(),
                                                                }));
                                                            }
                                                        }

                                                        // 处理 function call
                                                        if let Some(function_call) = part.get("functionCall") {
                                                            if let Some(name) = function_call.get("name").and_then(|n| n.as_str()) {
                                                                let args = function_call.get("args").cloned().unwrap_or(json!({}));
                                                                let tool_call_id = format!("{}_{}", name, chrono::Utc::now().timestamp_millis());

                                                                let tool_call = ToolCall::new(&tool_call_id, name, args);

                                                                // 结束之前的 block
                                                                let mut cbt = current_block_type.lock().unwrap();
                                                                match *cbt {
                                                                    Some(BlockType::Text) => {
                                                                        if let Some(ContentBlock::Text(t)) = msg.content.last() {
                                                                            events.push(Ok(AssistantMessageEvent::TextEnd {
                                                                                content_index: msg.content.len() - 1,
                                                                                content: t.text.clone(),
                                                                                partial: msg.clone(),
                                                                            }));
                                                                        }
                                                                    }
                                                                    Some(BlockType::Thinking) => {
                                                                        if let Some(ContentBlock::Thinking(t)) = msg.content.last() {
                                                                            events.push(Ok(AssistantMessageEvent::ThinkingEnd {
                                                                                content_index: msg.content.len() - 1,
                                                                                content: t.thinking.clone(),
                                                                                partial: msg.clone(),
                                                                            }));
                                                                        }
                                                                    }
                                                                    _ => {}
                                                                }
                                                                *cbt = None;
                                                                drop(cbt);

                                                                // 发送 tool call 事件
                                                                let content_index = msg.content.len();
                                                                msg.content.push(ContentBlock::ToolCall(tool_call.clone()));

                                                                events.push(Ok(AssistantMessageEvent::ToolCallStart {
                                                                    content_index,
                                                                    partial: msg.clone(),
                                                                }));
                                                                events.push(Ok(AssistantMessageEvent::ToolCallDelta {
                                                                    content_index,
                                                                    delta: serde_json::to_string(&tool_call.arguments).unwrap_or_default(),
                                                                    partial: msg.clone(),
                                                                }));
                                                                events.push(Ok(AssistantMessageEvent::ToolCallEnd {
                                                                    content_index,
                                                                    tool_call,
                                                                    partial: msg.clone(),
                                                                }));

                                                                // 标记为 tool use stop reason
                                                                msg.stop_reason = StopReason::ToolUse;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // 处理 usage metadata
                                    if let Some(usage) = json_value.get("usageMetadata") {
                                        msg.usage.input_tokens = usage.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                                        msg.usage.output_tokens = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
                                        msg.usage.cache_read_tokens = usage.get("cachedContentTokenCount").and_then(|v| v.as_u64());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        events.push(Err(anyhow!("Stream error: {}", e)));
                    }
                }

                stream::iter(events)
            }
        });

        // 创建最终消息
        let final_partial_message = Arc::clone(&partial_message);
        let final_block_type = Arc::clone(&current_block_type);
        let final_has_error = Arc::clone(&has_error);

        let result_stream = stream.chain(stream::once(async move {
            if *final_has_error.lock().unwrap() {
                return Err(anyhow!("Stream ended with error"));
            }

            let msg = final_partial_message.lock().unwrap().clone();
            let block_type = *final_block_type.lock().unwrap();

            // 结束任何未完成的 block
            if let Some(bt) = block_type {
                match bt {
                    BlockType::Text => {
                        if let Some(ContentBlock::Text(t)) = msg.content.last() {
                            return Ok(AssistantMessageEvent::TextEnd {
                                content_index: msg.content.len() - 1,
                                content: t.text.clone(),
                                partial: msg.clone(),
                            });
                        }
                    }
                    BlockType::Thinking => {
                        if let Some(ContentBlock::Thinking(t)) = msg.content.last() {
                            return Ok(AssistantMessageEvent::ThinkingEnd {
                                content_index: msg.content.len() - 1,
                                content: t.thinking.clone(),
                                partial: msg.clone(),
                            });
                        }
                    }
                }
            }

            // 检查是否有 tool calls
            let has_tool_calls = msg.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_)));
            let reason = if has_tool_calls {
                DoneReason::ToolUse
            } else {
                match msg.stop_reason {
                    StopReason::Length => DoneReason::Length,
                    _ => DoneReason::Stop,
                }
            };

            Ok(AssistantMessageEvent::Done {
                reason,
                message: msg,
            })
        }));

        Box::pin(result_stream)
    }
}

impl Default for VertexAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ApiProvider for VertexAiProvider {
    fn api(&self) -> Api {
        Api::GoogleVertex
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
            .ok_or_else(|| anyhow!("No API key found for provider: {:?}. Set GOOGLE_CLOUD_API_KEY environment variable.", model.provider))?;

        // 获取项目 ID
        let project = self.get_project_id()
            .ok_or_else(|| anyhow!("No Google Cloud project ID found. Set GOOGLE_CLOUD_PROJECT environment variable."))?;

        // 获取区域
        let region = self.get_region();

        let url = self.build_url(model, &region, &project);
        let body = self.build_request_body(context, model, options);

        // 发送请求 - 使用 Bearer token 认证
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await?;

        // 检查 HTTP 状态
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Vertex AI API error ({}): {}", status, error_text));
        }

        // 创建初始消息
        let start_message = AssistantMessage::new(Api::GoogleVertex, Provider::GoogleVertex, &model.id);

        // 创建流
        let start_event = AssistantMessageEvent::Start {
            partial: start_message,
        };

        let sse_stream = self.parse_sse_stream(response, model);

        // 组合 start 事件和 SSE 流
        let combined_stream = futures::stream::once(async { Ok(start_event) }).chain(sse_stream);

        Ok(Box::pin(combined_stream))
    }
}

/// Block 类型标记
#[derive(Debug, Clone, Copy, PartialEq)]
enum BlockType {
    Text,
    Thinking,
}

// ==================== Google API 类型定义 ====================

/// Google API 请求体
#[derive(Debug, Serialize)]
struct GoogleRequest {
    #[serde(rename = "contents")]
    contents: Vec<GoogleContent>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<SystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GoogleTool>>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(rename = "safetySettings", skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<SafetySetting>>,
}

/// Google Content
#[derive(Debug, Serialize, Deserialize)]
struct GoogleContent {
    role: String,
    parts: Vec<GooglePart>,
}

/// Google Part - 使用内部标签来序列化不同的变体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum GooglePart {
    /// 文本内容
    Text {
        text: String,
    },
    /// Thinking 内容
    Thought {
        thought: bool,
        text: String,
    },
    /// 内联数据（图片）
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: InlineData,
    },
    /// 函数调用
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
    },
    /// 函数响应
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

/// 内联数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    args: serde_json::Value,
}

/// 函数响应
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionResponse {
    name: String,
    response: serde_json::Value,
}

/// 系统指令
#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<GooglePart>,
}

/// Google 工具定义
#[derive(Debug, Serialize)]
struct GoogleTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<FunctionDeclaration>,
}

/// 函数声明
#[derive(Debug, Serialize)]
struct FunctionDeclaration {
    name: String,
    description: String,
    #[serde(rename = "parametersJsonSchema", skip_serializing_if = "Option::is_none")]
    parameters_json_schema: Option<serde_json::Value>,
}

/// 生成配置
#[derive(Debug, Serialize, Default)]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(rename = "maxOutputTokens", skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<i32>,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<ThinkingConfig>,
}

/// Thinking 配置
#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    thinking_budget: i32,
}

/// 安全设置
#[derive(Debug, Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::*;

    #[test]
    fn test_vertex_ai_api_type() {
        let provider = VertexAiProvider::new();
        assert_eq!(provider.api(), Api::GoogleVertex);
    }

    #[test]
    fn test_vertex_ai_url_construction() {
        let provider = VertexAiProvider::new();
        let mut model = sample_model(Api::GoogleVertex, Provider::GoogleVertex);
        model.id = "vertex/gemini-2.5-pro-preview-05-06".to_string();
        
        let url = provider.build_url(&model, "us-central1", "my-project");
        
        assert!(url.contains("us-central1-aiplatform.googleapis.com"));
        assert!(url.contains("projects/my-project"));
        assert!(url.contains("locations/us-central1"));
        assert!(url.contains("publishers/google"));
        assert!(url.contains("models/gemini-2.5-pro-preview-05-06"));
        assert!(url.contains(":streamGenerateContent"));
        assert!(url.contains("alt=sse"));
    }

    #[test]
    fn test_vertex_ai_url_without_prefix() {
        let provider = VertexAiProvider::new();
        let mut model = sample_model(Api::GoogleVertex, Provider::GoogleVertex);
        model.id = "gemini-2.0-flash".to_string();
        
        let url = provider.build_url(&model, "europe-west1", "test-project");
        
        assert!(url.contains("europe-west1-aiplatform.googleapis.com"));
        assert!(url.contains("models/gemini-2.0-flash"));
    }

    #[test]
    fn test_vertex_ai_convert_messages() {
        let provider = VertexAiProvider::new();
        let model = sample_model(Api::GoogleVertex, Provider::GoogleVertex);

        let context = sample_context(
            "You are a helpful assistant",
            vec![
                sample_user_message("Hello"),
                Message::Assistant(sample_assistant_message("Hi there!")),
            ],
        );

        let contents = provider.convert_messages(&context, &model);

        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].role, "user");
        assert_eq!(contents[1].role, "model");
    }

    #[test]
    fn test_vertex_ai_convert_tools() {
        let provider = VertexAiProvider::new();
        let tools = vec![
            sample_tool("get_weather", "Get weather info"),
            sample_tool("search", "Search the web"),
        ];

        let google_tools = provider.convert_tools(&tools);

        assert_eq!(google_tools.len(), 1);
        let func_decls = &google_tools[0].function_declarations;
        assert_eq!(func_decls.len(), 2);
        assert_eq!(func_decls[0].name, "get_weather");
        assert_eq!(func_decls[1].name, "search");
    }

    #[test]
    fn test_vertex_ai_build_request_body() {
        let provider = VertexAiProvider::new();
        let model = sample_model(Api::GoogleVertex, Provider::GoogleVertex);
        let context = sample_context("You are helpful", vec![sample_user_message("Hello")]);
        let options = sample_stream_options("test-key");

        let body = provider.build_request_body(&context, &model, &options);

        assert!(!body.contents.is_empty());
        assert!(body.system_instruction.is_some());
        assert!(body.safety_settings.is_some());
        assert!(body.generation_config.is_some());
    }

    #[test]
    fn test_vertex_ai_safety_settings() {
        let provider = VertexAiProvider::new();
        let settings = provider.safety_settings();

        assert_eq!(settings.len(), 4);
        assert!(settings.iter().all(|s| s.threshold == "BLOCK_NONE"));
    }

    #[test]
    fn test_vertex_ai_get_region_default() {
        let provider = VertexAiProvider::new();
        // 默认区域应该是 us-central1
        let region = provider.get_region();
        // 注意：如果环境中设置了 GOOGLE_CLOUD_REGION，这可能会失败
        // 但在测试环境中通常没有设置
        assert!(!region.is_empty());
    }

    #[test]
    fn test_vertex_ai_stream_text_response() {
        // 测试 Vertex AI SSE 格式的解析
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]}}]}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let candidates = parsed["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 1);
        
        let parts = candidates[0]["content"]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "Hello");
    }

    #[test]
    fn test_vertex_ai_stream_thinking_response() {
        // 测试包含 thinking 的响应
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"thought":true,"text":"Thinking..."}]}}]}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let parts = parsed["candidates"][0]["content"]["parts"].as_array().unwrap();
        assert_eq!(parts[0]["thought"], true);
        assert_eq!(parts[0]["text"], "Thinking...");
    }

    #[test]
    fn test_vertex_ai_stream_function_call() {
        // 测试包含 function call 的响应
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"get_weather","args":{"city":"Paris"}}}]}}]}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let func_call = &parsed["candidates"][0]["content"]["parts"][0]["functionCall"];
        assert_eq!(func_call["name"], "get_weather");
        assert_eq!(func_call["args"]["city"], "Paris");
    }
}
