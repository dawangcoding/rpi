//! 交互模式 - TUI 交互式会话
//!
//! 这是简化的首版实现，使用 raw mode + print 输出
//! 完整的 TUI 差分渲染可以在后续版本实现

use std::sync::Arc;
use tokio::sync::mpsc;
use pi_agent::types::*;
use pi_ai::types::*;
use pi_tui::terminal::{ProcessTerminal, Terminal};
use crate::core::agent_session::{AgentSession, AgentSessionConfig};
use crate::config::AppConfig;

/// 交互模式配置
pub struct InteractiveConfig {
    pub model: Model,
    pub thinking_level: ThinkingLevel,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub context_files: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub no_bash: bool,
    pub no_edit: bool,
    pub app_config: AppConfig,
    pub initial_prompt: Option<String>,
}

/// 运行交互模式
pub async fn run(config: InteractiveConfig) -> anyhow::Result<()> {
    // 1. 初始化终端
    let mut terminal = ProcessTerminal::new();
    terminal.enable_raw_mode()?;
    // 不进入 alternate screen，直接在主屏幕渲染
    
    // 2. 创建 AgentSession
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
    
    // 3. 设置事件通道（agent 事件 -> UI 更新）
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    
    let tx = event_tx.clone();
    session.agent().subscribe(Arc::new(move |event: AgentEvent, _cancel| {
        let _ = tx.send(event);
    }));
    
    // 4. 设置 stdin 读取
    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match stdin.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if input_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // 5. 打印欢迎信息
    println!("\x1b[1mpi\x1b[0m v0.1.0 | Model: {} | Thinking: {:?}", 
        config.model.name, config.thinking_level);
    println!("Type your message and press Enter to send. Ctrl+C to cancel, Ctrl+D to exit.\n");
    
    // 6. 如果有初始 prompt，直接发送
    if let Some(prompt) = &config.initial_prompt {
        println!("\x1b[36m> {}\x1b[0m\n", prompt);
        session.prompt_text(prompt).await?;
    }
    
    // 7. 主事件循环
    let mut is_streaming = false;
    let mut current_text = String::new();
    let mut input_buffer = String::new();
    let mut should_exit = false;
    
    loop {
        tokio::select! {
            // 处理 Agent 事件
            Some(event) = event_rx.recv() => {
                match event {
                    AgentEvent::AgentStart => {
                        is_streaming = true;
                        current_text.clear();
                    }
                    AgentEvent::MessageStart { .. } => {
                        // 开始新消息
                    }
                    AgentEvent::MessageUpdate { event: msg_event, .. } => {
                        match msg_event {
                            AssistantMessageEvent::TextDelta { delta, .. } => {
                                print!("{}", delta);
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                                current_text.push_str(&delta);
                            }
                            AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                                print!("\x1b[2m{}\x1b[0m", delta); // dim
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                            }
                            AssistantMessageEvent::ToolCallEnd { tool_call, .. } => {
                                println!("\n\x1b[33m⚡ Tool: {} ({})\x1b[0m", 
                                    tool_call.name, tool_call.id);
                            }
                            _ => {}
                        }
                    }
                    AgentEvent::MessageEnd { .. } => {
                        if !current_text.is_empty() {
                            println!(); // 换行
                        }
                    }
                    AgentEvent::ToolExecutionStart { tool_name, .. } => {
                        println!("\x1b[2m  Running {}...\x1b[0m", tool_name);
                    }
                    AgentEvent::ToolExecutionEnd { tool_name, is_error, .. } => {
                        if is_error {
                            println!("\x1b[31m  ✗ {} failed\x1b[0m", tool_name);
                        } else {
                            println!("\x1b[32m  ✓ {} done\x1b[0m", tool_name);
                        }
                    }
                    AgentEvent::TurnEnd { .. } => {
                        current_text.clear();
                    }
                    AgentEvent::AgentEnd { .. } => {
                        is_streaming = false;
                        println!();
                        // 显示统计
                        let stats = session.stats().await;
                        println!("\x1b[2m[tokens: {} in / {} out | cost: ${:.4}]\x1b[0m\n",
                            stats.tokens.input, stats.tokens.output, stats.cost);
                        // 打印提示符
                        print!("\x1b[36m> \x1b[0m");
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }
                    _ => {}
                }
            }
            
            // 处理键盘输入
            Some(data) = input_rx.recv() => {
                let text = String::from_utf8_lossy(&data);
                
                for ch in text.chars() {
                    match ch {
                        '\x03' => {  // Ctrl+C
                            if is_streaming {
                                session.abort().await;
                                println!("\n\x1b[33m[cancelled]\x1b[0m\n");
                                is_streaming = false;
                                print!("\x1b[36m> \x1b[0m");
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                            } else {
                                input_buffer.clear();
                                println!();
                                print!("\x1b[36m> \x1b[0m");
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                            }
                        }
                        '\x04' => {  // Ctrl+D
                            if input_buffer.is_empty() && !is_streaming {
                                should_exit = true;
                            }
                        }
                        '\r' | '\n' => {  // Enter
                            if !is_streaming && !input_buffer.trim().is_empty() {
                                let prompt = input_buffer.trim().to_string();
                                input_buffer.clear();
                                println!();
                                
                                // 特殊命令处理
                                if prompt == "/exit" || prompt == "/quit" {
                                    should_exit = true;
                                } else if prompt == "/stats" {
                                    let stats = session.stats().await;
                                    println!("Session stats:");
                                    println!("  Messages: {} user, {} assistant", 
                                        stats.user_messages, stats.assistant_messages);
                                    println!("  Tool calls: {}", stats.tool_calls);
                                    println!("  Tokens: {} total ({} in, {} out)", 
                                        stats.tokens.total, stats.tokens.input, stats.tokens.output);
                                    println!("  Cost: ${:.4}\n", stats.cost);
                                    print!("\x1b[36m> \x1b[0m");
                                    use std::io::Write;
                                    std::io::stdout().flush().ok();
                                } else if prompt == "/save" {
                                    match session.save().await {
                                        Ok(()) => println!("Session saved.\n"),
                                        Err(e) => println!("Failed to save: {}\n", e),
                                    }
                                    print!("\x1b[36m> \x1b[0m");
                                    use std::io::Write;
                                    std::io::stdout().flush().ok();
                                } else {
                                    // 发送到 agent
                                    if let Err(e) = session.prompt_text(&prompt).await {
                                        println!("\x1b[31mError: {}\x1b[0m\n", e);
                                        print!("\x1b[36m> \x1b[0m");
                                        use std::io::Write;
                                        std::io::stdout().flush().ok();
                                    }
                                }
                            }
                        }
                        '\x7f' => {  // Backspace
                            if !input_buffer.is_empty() {
                                input_buffer.pop();
                                print!("\x08 \x08"); // 擦除字符
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                            }
                        }
                        c if !c.is_control() => {
                            if !is_streaming {
                                input_buffer.push(c);
                                print!("{}", c);
                                use std::io::Write;
                                std::io::stdout().flush().ok();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        if should_exit {
            break;
        }
    }
    
    // 8. 清理
    terminal.disable_raw_mode()?;
    println!("\nGoodbye!");
    
    Ok(())
}
