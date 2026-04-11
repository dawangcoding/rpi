//! 统一流式 API 入口
//!
//! 提供流式和非流式的 LLM 调用接口

use futures::{Stream, StreamExt};
use std::pin::Pin;

use crate::api_registry::resolve_api_provider;
use crate::models::get_model;
use crate::types::*;

/// 流式调用 LLM（底层 API）
/// 
/// 返回事件流，用于实时接收模型响应
pub async fn stream(
    context: &Context,
    model: &Model,
    options: &StreamOptions,
) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
    let provider = resolve_api_provider(&model.api)?;
    provider.stream(context, model, options).await
}

/// 流式调用（简化版）
/// 
/// 使用 SimpleStreamOptions 进行流式调用
pub async fn stream_simple(
    context: &Context,
    model: &Model,
    options: &SimpleStreamOptions,
) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
    // 将 SimpleStreamOptions 转换为 StreamOptions
    let stream_options = StreamOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key.clone(),
        transport: options.transport.clone(),
        cache_retention: options.cache_retention.clone(),
        session_id: options.session_id.clone(),
        headers: options.headers.clone(),
        max_retry_delay_ms: options.max_retry_delay_ms,
        metadata: options.metadata.clone(),
    };
    
    stream(context, model, &stream_options).await
}

/// 非流式调用
/// 
/// 收集所有事件并返回完整消息
pub async fn complete(
    context: &Context,
    model: &Model,
    options: &StreamOptions,
) -> anyhow::Result<AssistantMessage> {
    let mut stream = stream(context, model, options).await?;
    let mut result_message: Option<AssistantMessage> = None;
    
    while let Some(event_result) = stream.next().await {
        let event = event_result?;
        
        match event {
            AssistantMessageEvent::Done { message, .. } => {
                result_message = Some(message);
                break;
            }
            AssistantMessageEvent::Error { error, .. } => {
                return Ok(error);
            }
            _ => {
                // 继续收集其他事件
            }
        }
    }
    
    result_message.ok_or_else(|| anyhow::anyhow!("Stream ended without Done event"))
}

/// 非流式调用（简化版）
/// 
/// 使用 SimpleStreamOptions 进行非流式调用
pub async fn complete_simple(
    context: &Context,
    model: &Model,
    options: &SimpleStreamOptions,
) -> anyhow::Result<AssistantMessage> {
    // 将 SimpleStreamOptions 转换为 StreamOptions
    let stream_options = StreamOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key.clone(),
        transport: options.transport.clone(),
        cache_retention: options.cache_retention.clone(),
        session_id: options.session_id.clone(),
        headers: options.headers.clone(),
        max_retry_delay_ms: options.max_retry_delay_ms,
        metadata: options.metadata.clone(),
    };
    
    complete(context, model, &stream_options).await
}

/// 通过模型 ID 流式调用 LLM
/// 
/// 根据模型 ID 自动查找模型配置并流式调用
pub async fn stream_by_model_id(
    context: &Context,
    model_id: &str,
    options: &StreamOptions,
) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
    let model = get_model(model_id)
        .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;
    stream(context, &model, options).await
}

/// 通过模型 ID 非流式调用 LLM
/// 
/// 根据模型 ID 自动查找模型配置并返回完整消息
pub async fn complete_by_model_id(
    context: &Context,
    model_id: &str,
    options: &StreamOptions,
) -> anyhow::Result<AssistantMessage> {
    let model = get_model(model_id)
        .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;
    complete(context, &model, options).await
}
