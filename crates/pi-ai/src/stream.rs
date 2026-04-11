//! 统一流式 API 入口
//!
//! 提供流式和非流式的 LLM 调用接口

use futures::{Future, Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};
use tracing::warn;

use crate::api_registry::resolve_api_provider;
use crate::models::get_model;
use crate::retry::RetryPolicy;
use crate::types::*;

/// 流式调用 LLM（底层 API）
/// 
/// 返回事件流，用于实时接收模型响应
/// 如果 options 中包含 retry_config，则使用重试包装
pub async fn stream(
    context: &Context,
    model: &Model,
    options: &StreamOptions,
) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
    // 如果有重试配置，使用带重试的流式调用
    if options.retry_config.is_some() {
        return stream_with_retry(context, model, options).await;
    }
    
    let provider = resolve_api_provider(&model.api)?;
    provider.stream(context, model, options).await
}

/// 带重试和恢复的流式调用
/// 
/// 在 stream() 调用外层包装重试逻辑，并支持流中断恢复
pub async fn stream_with_retry(
    context: &Context,
    model: &Model,
    options: &StreamOptions,
) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>> {
    let retry_config = options.retry_config.clone().unwrap_or_default();
    let policy = RetryPolicy::new(retry_config);
    
    // 使用重试策略执行流式调用
    let stream_result = policy.execute("stream", || async {
        let provider = resolve_api_provider(&model.api)?;
        provider.stream(context, model, options).await
    }).await;
    
    match stream_result {
        Ok(stream) => {
            // 包装流以支持中断恢复
            let resilient = ResilientStream::new(
                stream,
                context.clone(),
                model.clone(),
                options.clone(),
                policy,
            );
            Ok(Box::pin(resilient) as Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>)
        }
        Err(e) => Err(e),
    }
}

/// 支持恢复机制的流包装器
///
/// 监控流是否正常结束（收到 Done 事件），如果流异常终止，
/// 根据 retry_config 重新发起请求
pub struct ResilientStream {
    /// 当前活动的流
    inner: Option<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>>,
    /// 原始上下文（用于恢复）
    context: Context,
    /// 原始模型（用于恢复）
    model: Model,
    /// 原始选项（用于恢复）
    options: StreamOptions,
    /// 重试策略
    policy: RetryPolicy,
    /// 恢复尝试计数
    recovery_attempts: u32,
    /// 是否已经收到 Done 事件
    received_done: bool,
    /// 是否已经失败（无法恢复）
    failed: bool,
    /// 缓冲的事件（在恢复期间）
    buffer: Vec<AssistantMessageEvent>,
    /// 当前缓冲位置
    buffer_pos: usize,
    /// 恢复 future（用于保持恢复状态）
    recovery_future: Option<Pin<Box<dyn Future<Output = anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>>> + Send>>>,
}

impl ResilientStream {
    /// 创建新的可恢复流
    pub fn new(
        inner: Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>,
        context: Context,
        model: Model,
        options: StreamOptions,
        policy: RetryPolicy,
    ) -> Self {
        Self {
            inner: Some(inner),
            context,
            model,
            options,
            policy,
            recovery_attempts: 0,
            received_done: false,
            failed: false,
            buffer: Vec::new(),
            buffer_pos: 0,
            recovery_future: None,
        }
    }

    /// 创建恢复 future（不借用 self）
    fn create_recovery_future(
        context: Context,
        model: Model,
        options: StreamOptions,
        delay: std::time::Duration,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>>> + Send>> {
        Box::pin(async move {
            tokio::time::sleep(delay).await;
            let provider = resolve_api_provider(&model.api)?;
            let new_stream = provider.stream(&context, &model, &options).await?;
            Ok(new_stream)
        })
    }
}

impl Stream for ResilientStream {
    type Item = anyhow::Result<AssistantMessageEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        // 如果有缓冲的事件，先返回缓冲的事件
        if self.buffer_pos < self.buffer.len() {
            let event = self.buffer[self.buffer_pos].clone();
            self.buffer_pos += 1;
            return Poll::Ready(Some(Ok(event)));
        }

        // 如果已经失败，返回 None
        if self.failed {
            return Poll::Ready(None);
        }

        // 如果正在恢复中，poll 恢复 future
        if let Some(ref mut fut) = self.recovery_future {
            match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(new_stream)) => {
                    self.recovery_future = None;
                    self.inner = Some(new_stream);
                    self.received_done = false;
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Ready(Err(err)) => {
                    self.recovery_future = None;
                    self.failed = true;
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        // 尝试从当前流获取下一个事件
        if let Some(ref mut inner) = self.inner {
            match inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    // 检查是否是 Done 事件
                    if matches!(event, AssistantMessageEvent::Done { .. }) {
                        self.received_done = true;
                    }
                    Poll::Ready(Some(Ok(event)))
                }
                Poll::Ready(Some(Err(e))) => {
                    // 流发生错误，检查是否应该尝试恢复
                    if RetryPolicy::is_retryable(&e) && self.recovery_attempts < self.policy.max_retries() {
                        // 创建恢复 future
                        self.recovery_attempts += 1;
                        let delay = self.policy.delay_for_attempt(self.recovery_attempts);
                        
                        warn!(
                            "Stream interrupted, attempting recovery {}/{} after {:?}...",
                            self.recovery_attempts,
                            self.policy.max_retries(),
                            delay
                        );

                        self.recovery_future = Some(Self::create_recovery_future(
                            self.context.clone(),
                            self.model.clone(),
                            self.options.clone(),
                            delay,
                        ));
                        
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        // 不可重试错误或已达到最大重试次数
                        self.failed = true;
                        Poll::Ready(Some(Err(e)))
                    }
                }
                Poll::Ready(None) => {
                    // 流结束但未收到 Done 事件，尝试恢复
                    if !self.received_done && self.recovery_attempts < self.policy.max_retries() {
                        self.recovery_attempts += 1;
                        let delay = self.policy.delay_for_attempt(self.recovery_attempts);
                        
                        warn!(
                            "Stream interrupted, attempting recovery {}/{} after {:?}...",
                            self.recovery_attempts,
                            self.policy.max_retries(),
                            delay
                        );

                        self.recovery_future = Some(Self::create_recovery_future(
                            self.context.clone(),
                            self.model.clone(),
                            self.options.clone(),
                            delay,
                        ));
                        
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        Poll::Ready(None)
                    }
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(None)
        }
    }
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
        retry_config: options.retry_config.clone(),
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
        retry_config: options.retry_config.clone(),
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
