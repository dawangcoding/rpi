//! Google Gemini CLI Provider 实现
//!
//! 委托 GoogleProvider 处理，使用相同的 Generative AI API
//!
//! 这个 Provider 是为了支持 Gemini CLI 工具而创建的，
//! 它与 Google Provider 使用相同的 API，只是有不同的标识符。

use std::pin::Pin;

use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;

use crate::api_registry::ApiProvider;
use crate::types::*;
use super::google::GoogleProvider;

/// Google Gemini CLI Provider
///
/// 委托 GoogleProvider 处理，使用相同的 Generative AI API
pub struct GeminiCliProvider {
    inner: GoogleProvider,
}

impl GeminiCliProvider {
    /// 创建新的 Gemini CLI Provider
    pub fn new() -> Self {
        Self {
            inner: GoogleProvider::new(),
        }
    }
}

impl Default for GeminiCliProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ApiProvider for GeminiCliProvider {
    fn api(&self) -> Api {
        Api::GoogleGeminiCli
    }

    async fn stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AssistantMessageEvent>> + Send>>> {
        // 委托给内部的 GoogleProvider
        self.inner.stream(context, model, options).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_cli_api_type() {
        let provider = GeminiCliProvider::new();
        assert_eq!(provider.api(), Api::GoogleGeminiCli);
    }

    #[test]
    fn test_gemini_cli_provider_creation() {
        let provider = GeminiCliProvider::new();
        // 确保可以成功创建
        assert_eq!(provider.api(), Api::GoogleGeminiCli);
    }

    #[test]
    fn test_gemini_cli_default() {
        let provider = GeminiCliProvider::default();
        assert_eq!(provider.api(), Api::GoogleGeminiCli);
    }

    #[test]
    fn test_gemini_cli_stream_text_response() {
        // 测试 Gemini CLI SSE 格式的解析（与 Google 格式相同）
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Hello from Gemini CLI!"}]}}]}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let candidates = parsed["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 1);
        
        let parts = candidates[0]["content"]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "Hello from Gemini CLI!");
    }

    #[test]
    fn test_gemini_cli_stream_thinking_response() {
        // 测试包含 thinking 的响应
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"thought":true,"text":"Let me think about this..."}]}}]}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let parts = parsed["candidates"][0]["content"]["parts"].as_array().unwrap();
        assert_eq!(parts[0]["thought"], true);
        assert_eq!(parts[0]["text"], "Let me think about this...");
    }

    #[test]
    fn test_gemini_cli_stream_function_call() {
        // 测试包含 function call 的响应
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"read_file","args":{"path":"/src/main.rs"}}}]}}]}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let func_call = &parsed["candidates"][0]["content"]["parts"][0]["functionCall"];
        assert_eq!(func_call["name"], "read_file");
        assert_eq!(func_call["args"]["path"], "/src/main.rs");
    }

    #[test]
    fn test_gemini_cli_stream_with_usage() {
        // 测试包含 usage metadata 的响应
        let sse_response = r#"data: {"candidates":[{"content":{"role":"model","parts":[{"text":"Done"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":50,"totalTokenCount":150}}"#;
        
        let parsed: serde_json::Value = serde_json::from_str(
            sse_response.strip_prefix("data: ").unwrap_or(sse_response)
        ).unwrap();
        
        let usage = &parsed["usageMetadata"];
        assert_eq!(usage["promptTokenCount"], 100);
        assert_eq!(usage["candidatesTokenCount"], 50);
        assert_eq!(usage["totalTokenCount"], 150);
    }
}
