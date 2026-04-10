//! 打印模式 - 非交互单次执行
//!
//! 用于 `pi -p "prompt"` 或 `pi --mode print "prompt"`
//! 执行单次 prompt 后退出

use std::sync::Arc;
use tokio::sync::mpsc;
use pi_agent::types::*;
use pi_ai::types::*;
use crate::core::agent_session::{AgentSession, AgentSessionConfig};
use crate::config::AppConfig;

/// 打印模式配置
pub struct PrintConfig {
    pub model: Model,
    pub thinking_level: ThinkingLevel,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub context_files: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub no_bash: bool,
    pub no_edit: bool,
    pub app_config: AppConfig,
    pub prompt: String,
    pub no_stream: bool,
}

/// 运行打印模式（非交互，执行单次 prompt 后退出）
pub async fn run(config: PrintConfig) -> anyhow::Result<()> {
    // 1. 创建 AgentSession
    let session = AgentSession::new(AgentSessionConfig {
        model: config.model.clone(),
        thinking_level: config.thinking_level.clone(),
        system_prompt: config.system_prompt,
        append_system_prompt: config.append_system_prompt,
        context_files: config.context_files,
        cwd: config.cwd.clone(),
        no_bash: config.no_bash,
        no_edit: config.no_edit,
        app_config: config.app_config,
        session_id: None,
    }).await?;
    
    // 2. 设置事件监听器（流式输出到 stdout）
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    
    let tx = event_tx.clone();
    let _ = session.agent().subscribe(Arc::new(move |event: AgentEvent, _cancel| {
        let _ = tx.send(event);
    }));
    
    // 3. 发送 prompt
    session.prompt_text(&config.prompt).await?;
    
    // 4. 消费事件流输出
    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::MessageUpdate { event: msg_event, .. } => {
                match msg_event {
                    AssistantMessageEvent::TextDelta { delta, .. } => {
                        if !config.no_stream {
                            print!("{}", delta);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        }
                    }
                    AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                        if !config.no_stream {
                            eprint!("{}", delta); // thinking 输出到 stderr
                        }
                    }
                    _ => {}
                }
            }
            AgentEvent::ToolExecutionStart { tool_name, .. } => {
                eprintln!("[tool] Running {}...", tool_name);
            }
            AgentEvent::ToolExecutionEnd { tool_name, is_error, .. } => {
                if is_error {
                    eprintln!("[tool] {} failed", tool_name);
                } else {
                    eprintln!("[tool] {} done", tool_name);
                }
            }
            AgentEvent::AgentEnd { .. } => {
                if !config.no_stream {
                    println!(); // 最后换行
                }
                break;
            }
            _ => {}
        }
    }
    
    // 5. 等待完成
    session.wait_for_idle().await;
    
    // 6. 输出统计到 stderr
    let stats = session.stats().await;
    eprintln!("[stats] tokens: {} in / {} out | cost: ${:.4}",
        stats.tokens.input, stats.tokens.output, stats.cost);
    
    Ok(())
}
