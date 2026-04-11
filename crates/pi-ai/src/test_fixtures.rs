//! 测试辅助模块
//!
//! 提供测试用的 fixtures 和辅助函数

#[cfg(test)]
pub mod fixtures {
    use crate::types::*;

    /// 创建一个简单的用户消息
    pub fn sample_user_message(content: &str) -> Message {
        Message::User(UserMessage::new(content))
    }

    /// 创建一个简单的助手消息
    pub fn sample_assistant_message(content: &str) -> AssistantMessage {
        let mut msg = AssistantMessage::new(Api::Anthropic, Provider::Anthropic, "claude-3-sonnet");
        msg.content = vec![ContentBlock::Text(TextContent::new(content))];
        msg
    }

    /// 创建一个工具调用消息
    pub fn sample_tool_call(id: &str, name: &str, args: &str) -> ToolCall {
        let arguments = serde_json::from_str(args).unwrap_or_else(|_| serde_json::json!({}));
        ToolCall::new(id, name, arguments)
    }

    /// 创建 Anthropic SSE 流式响应样本 - message_start
    pub fn anthropic_sse_message_start(message_id: &str) -> String {
        format!(
            r#"event: message_start
            data: {{"type":"message_start","message":{{"id":"{}","type":"message","role":"assistant","model":"claude-3-sonnet-20240229","content":[],"stop_reason":null,"stop_sequence":null,"usage":{{"input_tokens":10,"output_tokens":1}}}}}}"#,
            message_id
        )
    }

    /// 创建 Anthropic SSE 流式响应样本 - content_block_start (text)
    pub fn anthropic_sse_text_start(index: usize) -> String {
        format!(
            r#"event: content_block_start
            data: {{"type":"content_block_start","index":{},"content_block":{{"type":"text","text":""}}}}"#,
            index
        )
    }

    /// 创建 Anthropic SSE 流式响应样本 - content_block_delta (text)
    pub fn anthropic_sse_text_delta(index: usize, text: &str) -> String {
        format!(
            r#"event: content_block_delta
            data: {{"type":"content_block_delta","index":{},"delta":{{"type":"text_delta","text":"{}"}}}}"#,
            index,
            text.replace('"', "\\\"")
        )
    }

    /// 创建 Anthropic SSE 流式响应样本 - content_block_stop
    pub fn anthropic_sse_content_block_stop(index: usize) -> String {
        format!(
            r#"event: content_block_stop
            data: {{"type":"content_block_stop","index":{}}}"#,
            index
        )
    }

    /// 创建 Anthropic SSE 流式响应样本 - message_delta
    pub fn anthropic_sse_message_delta(stop_reason: &str) -> String {
        format!(
            r#"event: message_delta
            data: {{"type":"message_delta","delta":{{"stop_reason":"{}","stop_sequence":null}},"usage":{{"output_tokens":10}}}}"#,
            stop_reason
        )
    }

    /// 创建 Anthropic SSE 流式响应样本 - message_stop
    pub fn anthropic_sse_message_stop() -> String {
        r#"event: message_stop
        data: {"type":"message_stop"}"#.to_string()
    }

    /// 创建完整的 Anthropic SSE 流式文本响应
    pub fn anthropic_sse_text_response(text: &str) -> String {
        let mut lines = vec![];
        lines.push(anthropic_sse_message_start("msg_123456"));
        lines.push(anthropic_sse_text_start(0));
        
        // 将文本分成小块以模拟流式响应
        for (i, chunk) in text.chars().collect::<Vec<_>>().chunks(10).enumerate() {
            let chunk_str: String = chunk.iter().collect();
            lines.push(anthropic_sse_text_delta(i, &chunk_str));
        }
        
        lines.push(anthropic_sse_content_block_stop(0));
        lines.push(anthropic_sse_message_delta("end_turn"));
        lines.push(anthropic_sse_message_stop());
        lines.join("\n\n")
    }

    /// 创建 Anthropic SSE 流式响应样本 - content_block_start (tool_use)
    pub fn anthropic_sse_tool_call_start(index: usize, id: &str, name: &str) -> String {
        format!(
            r#"event: content_block_start
            data: {{"type":"content_block_start","index":{},"content_block":{{"type":"tool_use","id":"{}","name":"{}","input":{{}}}}}}"#,
            index, id, name
        )
    }

    /// 创建 Anthropic SSE 流式响应样本 - content_block_delta (tool_use input_json_delta)
    pub fn anthropic_sse_tool_call_delta(index: usize, partial_json: &str) -> String {
        format!(
            r#"event: content_block_delta
            data: {{"type":"content_block_delta","index":{},"delta":{{"type":"input_json_delta","partial_json":"{}"}}}}"#,
            index,
            partial_json.replace('"', "\\\"")
        )
    }

    /// 创建 OpenAI SSE 流式响应样本
    pub fn openai_sse_text_response(text: &str) -> String {
        let mut lines = vec![];
        
        // 将文本分成小块以模拟流式响应
        let chunks: Vec<String> = text
            .chars()
            .collect::<Vec<_>>()
            .chunks(10)
            .map(|c| c.iter().collect())
            .collect();
        
        for chunk in chunks.iter() {
            let line = format!(
                r#"data: {{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{{"index":0,"delta":{{"role":"assistant","content":"{}"}},"finish_reason":null}}]}}"#,
                chunk.replace('"', "\\\"")
            );
            lines.push(line);
        }
        
        // 结束标记
        lines.push(r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#.to_string());
        lines.push("data: [DONE]".to_string());
        
        lines.join("\n\n")
    }

    /// 创建 OpenAI SSE 工具调用响应
    pub fn openai_sse_tool_call_response(tool_calls: Vec<(&str, &str, &str)>) -> String {
        let mut lines = vec![];
        
        for (i, (id, name, args)) in tool_calls.iter().enumerate() {
            let line = format!(
                r#"data: {{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{{"index":0,"delta":{{"role":"assistant","tool_calls":[{{"index":{},"id":"{}","type":"function","function":{{"name":"{}","arguments":"{}"}}}}]}},"finish_reason":null}}]}}"#,
                i, id, name, args.replace('"', "\\\"")
            );
            lines.push(line);
        }
        
        lines.push(r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#.to_string());
        lines.push("data: [DONE]".to_string());
        
        lines.join("\n\n")
    }

    /// 创建一个标准的 Context
    pub fn sample_context(system_prompt: &str, messages: Vec<Message>) -> Context {
        Context::new(messages).with_system_prompt(system_prompt)
    }

    /// 创建一个标准的 StreamOptions
    pub fn sample_stream_options(api_key: &str) -> StreamOptions {
        StreamOptions {
            temperature: Some(0.7),
            max_tokens: Some(4096),
            api_key: Some(api_key.to_string()),
            transport: Some(Transport::Sse),
            cache_retention: None,
            session_id: None,
            headers: None,
            max_retry_delay_ms: None,
            metadata: None,
        }
    }

    /// 创建一个标准的 Model（用于测试）
    pub fn sample_model(api: Api, provider: Provider) -> Model {
        let (id, name, base_url) = match provider {
            Provider::Anthropic => (
                "claude-3-sonnet-20240229",
                "Claude 3 Sonnet",
                "https://api.anthropic.com/v1/messages",
            ),
            Provider::Openai => (
                "gpt-4",
                "GPT-4",
                "https://api.openai.com/v1/chat/completions",
            ),
            Provider::Google => (
                "gemini-pro",
                "Gemini Pro",
                "https://generativelanguage.googleapis.com/v1beta",
            ),
            Provider::Mistral => (
                "mistral-large",
                "Mistral Large",
                "https://api.mistral.ai/v1/chat/completions",
            ),
            _ => ("test-model", "Test Model", "https://api.test.com/v1"),
        };

        Model {
            id: id.to_string(),
            name: name.to_string(),
            api: api.clone(),
            provider: provider.clone(),
            base_url: base_url.to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            },
            context_window: 200000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        }
    }

    /// 创建一个带工具的 Context
    pub fn sample_context_with_tools(
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Vec<Tool>,
    ) -> Context {
        Context::new(messages)
            .with_system_prompt(system_prompt)
            .with_tools(tools)
    }

    /// 创建一个简单的工具定义
    pub fn sample_tool(name: &str, description: &str) -> Tool {
        Tool::new(
            name,
            description,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    }
                },
                "required": ["query"]
            }),
        )
    }

    /// 创建工具结果消息
    pub fn sample_tool_result(tool_call_id: &str, tool_name: &str, content: &str) -> Message {
        Message::ToolResult(ToolResultMessage::new(
            tool_call_id,
            tool_name,
            vec![ContentBlock::Text(TextContent::new(content))],
        ))
    }

    /// 创建错误响应样本
    pub fn sample_error_response(error_message: &str) -> serde_json::Value {
        serde_json::json!({
            "error": {
                "message": error_message,
                "type": "api_error",
                "code": "internal_error"
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use crate::{Api, Provider};

    #[test]
    fn test_sample_user_message() {
        let msg = sample_user_message("Hello, world!");
        match msg {
            crate::types::Message::User(user_msg) => {
                assert_eq!(user_msg.role, "user");
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn test_sample_assistant_message() {
        let msg = sample_assistant_message("Hello from assistant!");
        assert_eq!(msg.role, "assistant");
        assert!(!msg.content.is_empty());
    }

    #[test]
    fn test_sample_context() {
        let messages = vec![sample_user_message("Test")];
        let ctx = sample_context("You are a helpful assistant", messages);
        assert_eq!(ctx.system_prompt, Some("You are a helpful assistant".to_string()));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn test_sample_model() {
        let model = sample_model(Api::Anthropic, Provider::Anthropic);
        assert_eq!(model.provider, Provider::Anthropic);
        assert_eq!(model.api, Api::Anthropic);
        assert!(!model.id.is_empty());
    }

    #[test]
    fn test_anthropic_sse_response() {
        let response = anthropic_sse_text_response("Hello, world!");
        assert!(response.contains("message_start"));
        assert!(response.contains("content_block_start"));
        assert!(response.contains("message_stop"));
    }

    #[test]
    fn test_openai_sse_response() {
        let response = openai_sse_text_response("Hello, world!");
        assert!(response.contains("chat.completion.chunk"));
        assert!(response.contains("[DONE]"));
    }
}
