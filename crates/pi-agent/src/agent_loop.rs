//! Agent 循环核心
//!
//! 处理 Agent 的主要循环逻辑，包括消息流、工具执行等

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use futures::future::BoxFuture;
use pi_ai::types::*;

use crate::types::*;

/// Agent 循环配置
pub struct AgentLoopConfig {
    pub model: Model,
    pub thinking_level: ThinkingLevel,
    pub thinking_budgets: Option<ThinkingBudgets>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub transport: Option<Transport>,
    pub cache_retention: Option<CacheRetention>,
    pub session_id: Option<String>,
    pub max_retry_delay_ms: Option<u64>,

    /// 消息转换函数（AgentMessage -> LLM Message）
    pub convert_to_llm: Arc<dyn Fn(&[AgentMessage]) -> Vec<Message> + Send + Sync>,

    /// 上下文变换（可选，在每次 LLM 调用前执行）
    pub transform_context: Option<
        Arc<
            dyn Fn(Vec<AgentMessage>, CancellationToken) -> BoxFuture<'static, Vec<AgentMessage>>
                + Send
                + Sync,
        >,
    >,

    /// API Key 获取
    pub get_api_key: Option<Arc<dyn Fn(&str) -> Option<String> + Send + Sync>>,

    /// 转向消息（mid-turn注入）
    pub get_steering_messages: Option<Arc<dyn Fn() -> Vec<AgentMessage> + Send + Sync>>,

    /// 后续消息
    pub get_follow_up_messages: Option<Arc<dyn Fn() -> Vec<AgentMessage> + Send + Sync>>,

    /// 工具执行模式
    pub tool_execution: ToolExecutionMode,

    /// beforeToolCall 钩子
    pub before_tool_call: Option<
        Arc<
            dyn Fn(&ToolCallContext, CancellationToken) -> BoxFuture<'static, Option<BeforeToolCallResult>>
                + Send
                + Sync,
        >,
    >,

    /// afterToolCall 钩子
    pub after_tool_call: Option<
        Arc<
            dyn Fn(
                    &ToolCallContext,
                    &AgentToolResult,
                    bool,
                    CancellationToken,
                ) -> BoxFuture<'static, Option<AfterToolCallResult>>
                + Send
                + Sync,
        >,
    >,
}

/// 从新提示启动 Agent 循环
pub async fn run_agent_loop(
    prompts: Vec<AgentMessage>,
    context: &mut AgentContext,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<Vec<AgentMessage>> {
    let mut new_messages = prompts.clone();

    // 将 prompts 添加到 context.messages
    context.messages.extend(prompts.clone());

    // 发出 agent_start
    emit(AgentEvent::AgentStart);

    // 发出 turn_start
    emit(AgentEvent::TurnStart);

    // 为每个 prompt 发出 message_start 和 message_end
    for prompt in &prompts {
        emit(AgentEvent::MessageStart {
            message: prompt.clone(),
        });
        emit(AgentEvent::MessageEnd {
            message: prompt.clone(),
        });
    }

    // 进入主循环
    run_loop(context, &mut new_messages, config, emit, cancel.clone()).await?;

    // 发出 agent_end
    emit(AgentEvent::AgentEnd {
        messages: new_messages.clone(),
    });

    Ok(new_messages)
}

/// 从已有消息继续循环（用于重试）
pub async fn run_agent_loop_continue(
    context: &mut AgentContext,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<Vec<AgentMessage>> {
    let mut new_messages: Vec<AgentMessage> = Vec::new();

    // 检查上下文有效性
    if context.messages.is_empty() {
        anyhow::bail!("Cannot continue: no messages in context");
    }

    // 检查最后一条消息不是 assistant
    if let Some(AgentMessage::Llm(Message::Assistant(_))) = context.messages.last() {
        anyhow::bail!("Cannot continue from message role: assistant");
    }

    // 发出 agent_start
    emit(AgentEvent::AgentStart);

    // 发出 turn_start
    emit(AgentEvent::TurnStart);

    // 进入主循环
    run_loop(context, &mut new_messages, config, emit, cancel.clone()).await?;

    // 发出 agent_end
    emit(AgentEvent::AgentEnd {
        messages: new_messages.clone(),
    });

    Ok(new_messages)
}

/// 主循环逻辑
async fn run_loop(
    context: &mut AgentContext,
    new_messages: &mut Vec<AgentMessage>,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let mut first_turn = true;
    let mut pending_messages: Vec<AgentMessage> = config
        .get_steering_messages
        .as_ref()
        .map(|f| f())
        .unwrap_or_default();

    // 外层循环：处理后续消息
    loop {
        let mut has_more_tool_calls = true;

        // 内层循环：处理工具调用和转向消息
        while has_more_tool_calls || !pending_messages.is_empty() {
            if !first_turn {
                emit(AgentEvent::TurnStart);
            } else {
                first_turn = false;
            }

            // 处理待处理消息
            if !pending_messages.is_empty() {
                for message in &pending_messages {
                    emit(AgentEvent::MessageStart {
                        message: message.clone(),
                    });
                    emit(AgentEvent::MessageEnd {
                        message: message.clone(),
                    });
                    context.messages.push(message.clone());
                    new_messages.push(message.clone());
                }
                pending_messages.clear();
            }

            // 流式获取助手响应
            let message = stream_assistant_response(context, config, emit, cancel.clone()).await?;
            new_messages.push(message.clone());

            // 检查是否是错误/中止
            let stop_reason = match &message {
                AgentMessage::Llm(Message::Assistant(assistant)) => Some(assistant.stop_reason.clone()),
                _ => None,
            };

            if matches!(stop_reason, Some(StopReason::Error) | Some(StopReason::Aborted)) {
                emit(AgentEvent::TurnEnd {
                    message,
                    tool_results: Vec::new(),
                });
                emit(AgentEvent::AgentEnd {
                    messages: new_messages.clone(),
                });
                return Ok(());
            }

            // 检查工具调用
            let tool_calls = extract_tool_calls(&message);
            has_more_tool_calls = !tool_calls.is_empty();

            let mut tool_results: Vec<ToolResultMessage> = Vec::new();
            if has_more_tool_calls {
                tool_results = execute_tool_calls(context, &message, tool_calls, config, emit, cancel.clone()).await?;

                for result in &tool_results {
                    let tool_result_msg = AgentMessage::Llm(Message::ToolResult(result.clone()));
                    context.messages.push(tool_result_msg.clone());
                    new_messages.push(tool_result_msg);
                }
            }

            emit(AgentEvent::TurnEnd {
                message,
                tool_results,
            });

            // 获取转向消息
            pending_messages = config
                .get_steering_messages
                .as_ref()
                .map(|f| f())
                .unwrap_or_default();
        }

        // 检查后续消息
        let follow_up = config
            .get_follow_up_messages
            .as_ref()
            .map(|f| f())
            .unwrap_or_default();

        if !follow_up.is_empty() {
            pending_messages = follow_up;
            continue;
        }

        // 没有更多消息，退出循环
        break;
    }

    Ok(())
}

/// 流式获取助手响应
async fn stream_assistant_response(
    context: &mut AgentContext,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<AgentMessage> {
    // 应用上下文变换
    let messages = if let Some(transform) = &config.transform_context {
        transform(context.messages.clone(), cancel.clone()).await
    } else {
        context.messages.clone()
    };

    // 转换为 LLM 消息
    let llm_messages = (config.convert_to_llm)(&messages);

    // 构建 LLM 上下文
    let _llm_context = Context {
        system_prompt: Some(context.system_prompt.clone()),
        messages: llm_messages,
        tools: None, // 工具通过 AgentTool trait 处理
    };

    // 解析 API key
    let _api_key = config
        .get_api_key
        .as_ref()
        .and_then(|f| f(&format!("{:?}", config.model.provider)));

    // TODO: 调用 stream_simple 获取事件流
    // 目前返回一个模拟的 AssistantMessage
    let assistant_message = AssistantMessage::new(
        config.model.api.clone(),
        config.model.provider.clone(),
        &config.model.id,
    );

    let agent_message = AgentMessage::Llm(Message::Assistant(assistant_message.clone()));

    // 发出 message_start
    emit(AgentEvent::MessageStart {
        message: agent_message.clone(),
    });

    // 添加到上下文
    context.messages.push(agent_message.clone());

    // TODO: 消费事件流，发出 message_update

    // 发出 message_end
    emit(AgentEvent::MessageEnd {
        message: agent_message.clone(),
    });

    Ok(agent_message)
}

/// 从消息中提取工具调用
fn extract_tool_calls(message: &AgentMessage) -> Vec<ToolCall> {
    match message {
        AgentMessage::Llm(Message::Assistant(assistant)) => assistant
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolCall(tc) => Some(tc.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// 执行工具调用
async fn execute_tool_calls(
    context: &AgentContext,
    assistant_message: &AgentMessage,
    tool_calls: Vec<ToolCall>,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<Vec<ToolResultMessage>> {
    match config.tool_execution {
        ToolExecutionMode::Sequential => {
            execute_tools_sequential(context, assistant_message, tool_calls, config, emit, cancel).await
        }
        ToolExecutionMode::Parallel => {
            execute_tools_parallel(context, assistant_message, tool_calls, config, emit, cancel).await
        }
    }
}

/// 串行执行工具
async fn execute_tools_sequential(
    context: &AgentContext,
    assistant_message: &AgentMessage,
    tool_calls: Vec<ToolCall>,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<Vec<ToolResultMessage>> {
    let mut results = Vec::new();

    let assistant = match assistant_message {
        AgentMessage::Llm(Message::Assistant(a)) => a.clone(),
        _ => anyhow::bail!("Expected assistant message"),
    };

    for tool_call in tool_calls {
        emit(AgentEvent::ToolExecutionStart {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            args: tool_call.arguments.clone(),
        });

        let (result_msg, _is_error) = execute_tool_call(
            context,
            &assistant,
            &tool_call,
            config,
            emit,
            cancel.clone(),
        )
        .await?;

        results.push(result_msg);
    }

    Ok(results)
}

/// 并行执行工具
async fn execute_tools_parallel(
    context: &AgentContext,
    assistant_message: &AgentMessage,
    tool_calls: Vec<ToolCall>,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<Vec<ToolResultMessage>> {
    let mut results = Vec::new();
    let mut pending_futures = Vec::new();

    let assistant = match assistant_message {
        AgentMessage::Llm(Message::Assistant(a)) => a.clone(),
        _ => anyhow::bail!("Expected assistant message"),
    };

    for tool_call in tool_calls {
        emit(AgentEvent::ToolExecutionStart {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            args: tool_call.arguments.clone(),
        });

        // 准备工具调用
        let tool = context.tools.iter().find(|t| t.name() == tool_call.name);

        if tool.is_none() {
            // 工具未找到，创建错误结果
            let error_result = AgentToolResult::error(format!("Tool {} not found", tool_call.name));
            let tool_result = ToolResultMessage::new(
                &tool_call.id,
                &tool_call.name,
                error_result.content.clone(),
            )
            .with_error(true)
            .with_details(error_result.details.clone());

            emit(AgentEvent::ToolExecutionEnd {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                result: error_result,
                is_error: true,
            });

            results.push(tool_result);
            continue;
        }

        // 创建异步任务
        let tool = tool.unwrap().clone();
        let tool_call_clone = tool_call.clone();
        let assistant_clone = assistant.clone();
        let _config_clone = AgentContext {
            system_prompt: context.system_prompt.clone(),
            messages: context.messages.clone(),
            tools: context.tools.clone(),
        };
        let cancel_clone = cancel.clone();

        let future = async move {
            let ctx = ToolCallContext {
                assistant_message: assistant_clone,
                tool_call: tool_call_clone.clone(),
                args: tool_call_clone.arguments.clone(),
            };

            // 执行工具
            let result = tool
                .execute(
                    &tool_call_clone.id,
                    tool_call_clone.arguments.clone(),
                    cancel_clone.clone(),
                    None,
                )
                .await;

            (tool_call_clone, result, ctx)
        };

        pending_futures.push(future);
    }

    // 等待所有工具执行完成
    let executions = futures::future::join_all(pending_futures).await;

    for (tool_call, result, ctx) in executions {
        let (tool_result, _is_error) = match result {
            Ok(agent_result) => {
                // 应用 afterToolCall 钩子
                let (final_result, final_is_error) = if let Some(after_hook) = &config.after_tool_call {
                    if let Some(after_result) = after_hook(&ctx, &agent_result, false, cancel.clone()).await {
                        let content = after_result.content.unwrap_or(agent_result.content);
                        let details = after_result.details.unwrap_or(agent_result.details);
                        let is_err = after_result.is_error.unwrap_or(false);
                        (
                            AgentToolResult { content, details },
                            is_err,
                        )
                    } else {
                        (agent_result, false)
                    }
                } else {
                    (agent_result, false)
                };

                emit(AgentEvent::ToolExecutionEnd {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    result: final_result.clone(),
                    is_error: final_is_error,
                });

                let msg = ToolResultMessage::new(&tool_call.id, &tool_call.name, final_result.content)
                    .with_error(final_is_error)
                    .with_details(final_result.details);

                (msg, final_is_error)
            }
            Err(e) => {
                let error_result = AgentToolResult::error(e.to_string());

                emit(AgentEvent::ToolExecutionEnd {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    result: error_result.clone(),
                    is_error: true,
                });

                let msg = ToolResultMessage::new(&tool_call.id, &tool_call.name, error_result.content)
                    .with_error(true)
                    .with_details(error_result.details);

                (msg, true)
            }
        };

        // 发出 tool result 消息事件
        let agent_msg = AgentMessage::Llm(Message::ToolResult(tool_result.clone()));
        emit(AgentEvent::MessageStart {
            message: agent_msg.clone(),
        });
        emit(AgentEvent::MessageEnd {
            message: agent_msg,
        });

        results.push(tool_result);
    }

    Ok(results)
}

/// 执行单个工具调用
async fn execute_tool_call(
    context: &AgentContext,
    assistant_message: &AssistantMessage,
    tool_call: &ToolCall,
    config: &AgentLoopConfig,
    emit: &dyn Fn(AgentEvent),
    cancel: CancellationToken,
) -> anyhow::Result<(ToolResultMessage, bool)> {
    // 查找工具
    let tool = context.tools.iter().find(|t| t.name() == tool_call.name);

    if let Some(tool) = tool {
        let tool = tool.clone();
        let ctx = ToolCallContext {
            assistant_message: assistant_message.clone(),
            tool_call: tool_call.clone(),
            args: tool_call.arguments.clone(),
        };

        // 应用 beforeToolCall 钩子
        if let Some(before_hook) = &config.before_tool_call {
            if let Some(before_result) = before_hook(&ctx, cancel.clone()).await {
                if before_result.block {
                    let error_result = AgentToolResult::error(
                        before_result.reason.unwrap_or_else(|| "Tool execution was blocked".to_string()),
                    );

                    emit(AgentEvent::ToolExecutionEnd {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        result: error_result.clone(),
                        is_error: true,
                    });

                    let msg = ToolResultMessage::new(&tool_call.id, &tool_call.name, error_result.content)
                        .with_error(true)
                        .with_details(error_result.details);

                    // 发出 tool result 消息事件
                    let agent_msg = AgentMessage::Llm(Message::ToolResult(msg.clone()));
                    emit(AgentEvent::MessageStart {
                        message: agent_msg.clone(),
                    });
                    emit(AgentEvent::MessageEnd { message: agent_msg });

                    return Ok((msg, true));
                }
            }
        }

        // 执行工具
        let result = tool
            .execute(&tool_call.id, tool_call.arguments.clone(), cancel.clone(), None)
            .await;

        match result {
            Ok(agent_result) => {
                // 应用 afterToolCall 钩子
                let (final_result, final_is_error) = if let Some(after_hook) = &config.after_tool_call {
                    if let Some(after_result) = after_hook(&ctx, &agent_result, false, cancel.clone()).await {
                        let content = after_result.content.unwrap_or(agent_result.content);
                        let details = after_result.details.unwrap_or(agent_result.details);
                        let is_err = after_result.is_error.unwrap_or(false);
                        (
                            AgentToolResult { content, details },
                            is_err,
                        )
                    } else {
                        (agent_result, false)
                    }
                } else {
                    (agent_result, false)
                };

                emit(AgentEvent::ToolExecutionEnd {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    result: final_result.clone(),
                    is_error: final_is_error,
                });

                let msg = ToolResultMessage::new(&tool_call.id, &tool_call.name, final_result.content)
                    .with_error(final_is_error)
                    .with_details(final_result.details);

                // 发出 tool result 消息事件
                let agent_msg = AgentMessage::Llm(Message::ToolResult(msg.clone()));
                emit(AgentEvent::MessageStart {
                    message: agent_msg.clone(),
                });
                emit(AgentEvent::MessageEnd { message: agent_msg });

                Ok((msg, final_is_error))
            }
            Err(e) => {
                let error_result = AgentToolResult::error(e.to_string());

                emit(AgentEvent::ToolExecutionEnd {
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    result: error_result.clone(),
                    is_error: true,
                });

                let msg = ToolResultMessage::new(&tool_call.id, &tool_call.name, error_result.content)
                    .with_error(true)
                    .with_details(error_result.details);

                // 发出 tool result 消息事件
                let agent_msg = AgentMessage::Llm(Message::ToolResult(msg.clone()));
                emit(AgentEvent::MessageStart {
                    message: agent_msg.clone(),
                });
                emit(AgentEvent::MessageEnd { message: agent_msg });

                Ok((msg, true))
            }
        }
    } else {
        // 工具未找到
        let error_result = AgentToolResult::error(format!("Tool {} not found", tool_call.name));

        emit(AgentEvent::ToolExecutionEnd {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            result: error_result.clone(),
            is_error: true,
        });

        let msg = ToolResultMessage::new(&tool_call.id, &tool_call.name, error_result.content)
            .with_error(true)
            .with_details(error_result.details);

        // 发出 tool result 消息事件
        let agent_msg = AgentMessage::Llm(Message::ToolResult(msg.clone()));
        emit(AgentEvent::MessageStart {
            message: agent_msg.clone(),
        });
        emit(AgentEvent::MessageEnd { message: agent_msg });

        Ok((msg, true))
    }
}
