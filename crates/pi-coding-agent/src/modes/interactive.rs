//! 交互模式 - TUI 集成的交互式会话
//!
//! 使用 Markdown 渲染、流式差分更新和状态栏。
//! 输入使用 pi-tui 的 Editor 组件。

use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use std::path::PathBuf;
use pi_agent::types::*;
use pi_ai::types::*;
use pi_tui::terminal::{ProcessTerminal, Terminal};
use pi_tui::components::editor::{Editor, EditorConfig};
use pi_tui::autocomplete::{AutocompleteProvider, AutocompleteSuggestions, SlashCommand, SlashCommandProvider};
use pi_tui::tui::{Component, Focusable};
use crate::core::agent_session::{AgentSession, AgentSessionConfig};
use crate::core::export::HtmlExporter;
use crate::core::session_manager::SessionManager;
use crate::core::auth::{TokenStorage, get_oauth_provider, run_oauth_flow};
use crate::config::AppConfig;
use super::interactive_components::{StreamingBlock, render_input_area};

/// 最小渲染间隔 (~60fps = 16ms)
const MIN_RENDER_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

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

/// CodingAgent 自动完成提供者
/// 支持 slash 命令补全
struct CodingAgentAutocompleteProvider {
    slash_provider: SlashCommandProvider,
}

impl CodingAgentAutocompleteProvider {
    fn new() -> Self {
        let mut slash_provider = SlashCommandProvider::new();
        slash_provider.add_command(SlashCommand::new("help", "Show help information"));
        slash_provider.add_command(SlashCommand::new("clear", "Clear the conversation history"));
        slash_provider.add_command(SlashCommand::new("model", "Show or change the current model"));
        slash_provider.add_command(SlashCommand::new("exit", "Exit the application").with_alias("quit"));
        slash_provider.add_command(SlashCommand::new("stats", "Show session statistics"));
        slash_provider.add_command(SlashCommand::new("save", "Save the current session"));
        slash_provider.add_command(SlashCommand::new("fork", "Fork the current session at a message index"));
        slash_provider.add_command(SlashCommand::new("export", "Export session to HTML file").with_alias("export-html"));
        slash_provider.add_command(SlashCommand::new("compact", "Compact conversation history to save context space"));
        slash_provider.add_command(SlashCommand::new("extensions", "List loaded extensions"));
        slash_provider.add_command(SlashCommand::new("login", "Login with OAuth provider (anthropic, github-copilot)"));
        slash_provider.add_command(SlashCommand::new("logout", "Logout from OAuth provider"));
        slash_provider.add_command(SlashCommand::new("auth", "Show current authentication status"));
        Self { slash_provider }
    }
}

impl AutocompleteProvider for CodingAgentAutocompleteProvider {
    fn provide(&self, input: &str, cursor_pos: usize) -> Option<AutocompleteSuggestions> {
        // 优先使用 slash 命令提供者
        if let Some(suggestions) = self.slash_provider.provide(input, cursor_pos) {
            return Some(suggestions);
        }
        None
    }
}

/// 运行交互模式
pub async fn run(config: InteractiveConfig) -> anyhow::Result<()> {
    // 1. 初始化终端
    let mut terminal = ProcessTerminal::new();
    terminal.enable_raw_mode()?;
    let mut stdout = std::io::stdout();

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

    // 4. 设置 stdin 异步读取
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
    write!(stdout, "\x1b[1mpi\x1b[0m v0.1.0 | Model: {} | Thinking: {:?}\r\n",
        config.model.name, config.thinking_level)?;
    write!(stdout, "Type your message and press Enter to send. Ctrl+C to cancel, Ctrl+D to exit.\r\n")?;
    write!(stdout, "Use Shift+Enter for new line. /help for commands.\r\n\r\n")?;
    stdout.flush()?;

    // 6. 如果有初始 prompt，直接发送
    if let Some(prompt) = &config.initial_prompt {
        write!(stdout, "\x1b[36m> {}\x1b[0m\r\n\r\n", prompt)?;
        stdout.flush()?;
        session.prompt_text(prompt).await?;
    }

    // 7. 主事件循环
    let mut streaming = StreamingBlock::new();
    let mut is_streaming = false;
    let mut should_exit = false;
    let mut last_render_time = Instant::now();

    // 初始化 Editor 组件
    let mut editor = Editor::new(EditorConfig {
        placeholder: Some("> Ask anything...".to_string()),
        max_lines: None,
        read_only: false,
        line_numbers: false,
        wrap: true,
    });
    editor.set_focused(true);
    
    // 设置自动完成提供者
    let autocomplete_provider = CodingAgentAutocompleteProvider::new();
    editor.set_autocomplete_provider(Box::new(autocomplete_provider));

    // 初始渲染输入区域
    let (term_width, _) = terminal.size();
    let input_render = render_input_area(&editor, term_width);
    write!(stdout, "{}", input_render)?;
    stdout.flush()?;

    loop {
        tokio::select! {
            // 处理 Agent 事件
            Some(event) = event_rx.recv() => {
                let (term_width, _) = terminal.size();

                match event {
                    AgentEvent::AgentStart => {
                        is_streaming = true;
                        streaming = StreamingBlock::new();
                    }
                    AgentEvent::MessageStart { .. } => {
                        // 消息流开始，准备接收 delta
                    }
                    AgentEvent::MessageUpdate { event: msg_event, .. } => {
                        match msg_event {
                            AssistantMessageEvent::TextDelta { delta, .. } => {
                                streaming.push_text(&delta);
                                // 渲染节流：检查是否应该渲染
                                let now = Instant::now();
                                if now.duration_since(last_render_time) >= MIN_RENDER_INTERVAL {
                                    let update = streaming.diff_update(term_width);
                                    write!(stdout, "{}", update)?;
                                    stdout.flush()?;
                                    last_render_time = now;
                                }
                            }
                            AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                                streaming.push_thinking(&delta);
                                // 渲染节流：检查是否应该渲染
                                let now = Instant::now();
                                if now.duration_since(last_render_time) >= MIN_RENDER_INTERVAL {
                                    let update = streaming.diff_update(term_width);
                                    write!(stdout, "{}", update)?;
                                    stdout.flush()?;
                                    last_render_time = now;
                                }
                            }
                            AssistantMessageEvent::ToolCallEnd { tool_call, .. } => {
                                // 工具调用完成，强制 flush 当前流式内容
                                if streaming.has_content() {
                                    let update = streaming.diff_update(term_width);
                                    write!(stdout, "{}", update)?;
                                }
                                write!(stdout, "\r\n\x1b[33m⚡ Tool: {} ({})\x1b[0m",
                                    tool_call.name, tool_call.id)?;
                                stdout.flush()?;
                                // 重置流式块，后续内容（如有）从新位置开始
                                streaming.finish();
                                last_render_time = Instant::now();
                            }
                            _ => {}
                        }
                    }
                    AgentEvent::MessageEnd { .. } => {
                        // 最终渲染一次确保完整（强制渲染，无视节流）
                        if streaming.has_content() {
                            let (term_w, _) = terminal.size();
                            let update = streaming.diff_update(term_w);
                            write!(stdout, "{}", update)?;
                        }
                        streaming.finish();
                        write!(stdout, "\r\n")?;
                        stdout.flush()?;
                        last_render_time = Instant::now();
                    }
                    AgentEvent::ToolExecutionStart { tool_name, .. } => {
                        write!(stdout, "\x1b[2m  Running {}...\x1b[0m\r\n", tool_name)?;
                        stdout.flush()?;
                    }
                    AgentEvent::ToolExecutionEnd { tool_name, is_error, .. } => {
                        if is_error {
                            write!(stdout, "\x1b[31m  ✗ {} failed\x1b[0m\r\n", tool_name)?;
                        } else {
                            write!(stdout, "\x1b[32m  ✓ {} done\x1b[0m\r\n", tool_name)?;
                        }
                        stdout.flush()?;
                    }
                    AgentEvent::TurnEnd { .. } => {
                        // Turn 结束
                    }
                    AgentEvent::AgentEnd { .. } => {
                        is_streaming = false;
                        // 状态栏：token 统计 + 费用
                        let stats = session.stats().await;
                        write!(stdout, "\x1b[2m[tokens: {} in / {} out | cost: ${:.4}]\x1b[0m\r\n\r\n",
                            stats.tokens.input, stats.tokens.output, stats.cost)?;
                        // 渲染输入区域
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    }
                    _ => {}
                }
            }

            // 处理键盘输入
            Some(data) = input_rx.recv() => {
                if is_streaming {
                    // 流式输出时只处理 Ctrl+C 中断
                    let text = String::from_utf8_lossy(&data);
                    if text.contains('\x03') {
                        session.abort().await;
                        streaming.finish();
                        write!(stdout, "\r\n\x1b[33m[cancelled]\x1b[0m\r\n\r\n")?;
                        is_streaming = false;
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    }
                    continue;
                }

                let text = String::from_utf8_lossy(&data);
                
                // 检查 Ctrl+C - 取消/清空
                if text.contains('\x03') {
                    editor.set_text("");
                    let (term_width, _) = terminal.size();
                    let input_render = render_input_area(&editor, term_width);
                    write!(stdout, "{}", input_render)?;
                    stdout.flush()?;
                    continue;
                }
                
                // 检查 Ctrl+D - 退出（仅在编辑器为空时）
                if text.contains('\x04') && editor.is_empty() {
                    should_exit = true;
                    continue;
                }
                
                // 检查是否是 Enter 键（需要特殊处理提交）
                // Enter 键的序列：\r, \n, \r\n, 或 Kitty 协议的 CSI 序列
                let is_enter = text == "\r" || text == "\n" || text == "\r\n";
                let is_shift_enter = text == "\x1b\r" || text == "\x1b\n"; // Shift+Enter 在 Kitty 协议下
                
                if is_enter && !is_shift_enter && !editor.is_empty() {
                    // 提交输入
                    let prompt = editor.get_text().trim().to_string();
                    editor.set_text("");
                    write!(stdout, "\r\n")?;
                    
                    // 特殊命令处理
                    if prompt == "/exit" || prompt == "/quit" {
                        should_exit = true;
                    } else if prompt == "/clear" {
                        // 清屏并重新显示欢迎信息
                        write!(stdout, "\x1b[2J\x1b[H")?;
                        write!(stdout, "\x1b[1mpi\x1b[0m v0.1.0 | Model: {} | Thinking: {:?}\r\n",
                            config.model.name, config.thinking_level)?;
                        write!(stdout, "Conversation cleared.\r\n\r\n")?;
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/model" {
                        write!(stdout, "Current model: {}\r\n\r\n", config.model.name)?;
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/stats" {
                        let stats = session.stats().await;
                        write!(stdout, "Session stats:\r\n")?;
                        write!(stdout, "  Messages: {} user, {} assistant\r\n",
                            stats.user_messages, stats.assistant_messages)?;
                        write!(stdout, "  Tool calls: {}\r\n", stats.tool_calls)?;
                        write!(stdout, "  Tokens: {} total ({} in, {} out)\r\n",
                            stats.tokens.total, stats.tokens.input, stats.tokens.output)?;
                        write!(stdout, "  Cost: ${:.4}\r\n\r\n", stats.cost)?;
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/save" {
                        match session.save().await {
                            Ok(()) => write!(stdout, "Session saved.\r\n\r\n")?,
                            Err(e) => write!(stdout, "Failed to save: {}\r\n\r\n", e)?,
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/fork" || prompt.starts_with("/fork ") {
                        // 解析可选的消息索引参数
                        let fork_at_index = if prompt.len() > 6 {
                            match prompt[6..].trim().parse::<usize>() {
                                Ok(index) => Some(index),
                                Err(_) => {
                                    write!(stdout, "\x1b[31mInvalid index. Usage: /fork or /fork N\x1b[0m\r\n\r\n")?;
                                    let (term_width, _) = terminal.size();
                                    let input_render = render_input_area(&editor, term_width);
                                    write!(stdout, "{}", input_render)?;
                                    stdout.flush()?;
                                    continue;
                                }
                            }
                        } else {
                            None
                        };
                        
                        // 先保存当前会话
                        if let Err(e) = session.save().await {
                            write!(stdout, "\x1b[31mFailed to save session: {}\x1b[0m\r\n\r\n", e)?;
                            let (term_width, _) = terminal.size();
                            let input_render = render_input_area(&editor, term_width);
                            write!(stdout, "{}", input_render)?;
                            stdout.flush()?;
                            continue;
                        }
                        
                        // 执行 fork
                        match session.fork(fork_at_index).await {
                            Ok(new_session_id) => {
                                if let Some(index) = fork_at_index {
                                    write!(stdout, "\x1b[32mForked at message {}. New session ID: {}\x1b[0m\r\n\r\n", index, new_session_id)?;
                                } else {
                                    write!(stdout, "\x1b[32mForked session. New session ID: {}\x1b[0m\r\n\r\n", new_session_id)?;
                                }
                            }
                            Err(e) => {
                                write!(stdout, "\x1b[31mFailed to fork session: {}\x1b[0m\r\n\r\n", e)?;
                            }
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/export" || prompt.starts_with("/export ") {
                        // 解析可选的输出路径参数
                        let output_path = if prompt.len() > 7 {
                            let path_str = prompt[7..].trim();
                            if path_str.is_empty() {
                                None
                            } else {
                                Some(PathBuf::from(path_str))
                            }
                        } else {
                            None
                        };
                        
                        // 先保存当前会话
                        if let Err(e) = session.save().await {
                            write!(stdout, "\x1b[31mFailed to save session: {}\x1b[0m\r\n\r\n", e)?;
                            let (term_width, _) = terminal.size();
                            let input_render = render_input_area(&editor, term_width);
                            write!(stdout, "{}", input_render)?;
                            stdout.flush()?;
                            continue;
                        }
                        
                        // 获取会话 ID 并加载会话数据
                        let session_id = session.session_id_async().await;
                        let sessions_dir = session.sessions_dir()
                            .ok_or_else(|| anyhow::anyhow!("Session manager not available"))?;
                        let session_manager = SessionManager::with_dir(sessions_dir)?;
                        
                        match session_manager.load_session(&session_id).await {
                            Ok(saved_session) => {
                                // 确定输出路径
                                let output = match output_path {
                                    Some(path) => path,
                                    None => {
                                        // 默认路径：当前目录/会话标题.html
                                        let title = saved_session.metadata.title.as_deref()
                                            .unwrap_or(&session_id);
                                        let safe_title = sanitize_filename(title);
                                        PathBuf::from(format!("{}.html", safe_title))
                                    }
                                };
                                
                                // 导出为 HTML
                                let exporter = HtmlExporter::new();
                                match exporter.export_session(&saved_session, &output) {
                                    Ok(()) => {
                                        let abs_path = std::fs::canonicalize(&output)
                                            .unwrap_or(output.clone());
                                        write!(stdout, "\x1b[32mSession exported to: {}\x1b[0m\r\n\r\n", abs_path.display())?;
                                    }
                                    Err(e) => {
                                        write!(stdout, "\x1b[31mFailed to export session: {}\x1b[0m\r\n\r\n", e)?;
                                    }
                                }
                            }
                            Err(e) => {
                                write!(stdout, "\x1b[31mFailed to load session: {}\x1b[0m\r\n\r\n", e)?;
                            }
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/compact" {
                        // 检查是否需要压缩
                        if !session.needs_compaction().await {
                            write!(stdout, "\x1b[33mNo need to compact. Context usage is below threshold.\x1b[0m\r\n\r\n")?;
                            let (term_width, _) = terminal.size();
                            let input_render = render_input_area(&editor, term_width);
                            write!(stdout, "{}", input_render)?;
                            stdout.flush()?;
                            continue;
                        }
                        
                        // 执行压缩
                        write!(stdout, "\x1b[2mCompacting conversation history...\x1b[0m\r\n")?;
                        stdout.flush()?;
                        
                        match session.compact().await {
                            Ok(result) => {
                                let saved_tokens = result.original_tokens.saturating_sub(result.compacted_tokens);
                                write!(stdout, "\x1b[32m✓ Compacted {} messages into summary\x1b[0m\r\n", result.removed_count)?;
                                write!(stdout, "\x1b[2m  Original: {} tokens → Summary: {} tokens (saved: {})\x1b[0m\r\n\r\n", 
                                    result.original_tokens, result.compacted_tokens, saved_tokens)?;
                            }
                            Err(e) => {
                                write!(stdout, "\x1b[31m✗ Failed to compact: {}\x1b[0m\r\n\r\n", e)?;
                            }
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/login" || prompt.starts_with("/login ") {
                        let provider_name = if prompt.len() > 7 {
                            prompt[7..].trim()
                        } else {
                            "anthropic"
                        };
                        
                        match get_oauth_provider(provider_name) {
                            Some(provider_config) => {
                                let token_storage = TokenStorage::new();
                                match run_oauth_flow(&provider_config, &token_storage).await {
                                    Ok(_) => {
                                        write!(stdout, "\x1b[32m✓ Successfully logged in with {}\x1b[0m\r\n\r\n", provider_name)?;
                                    }
                                    Err(e) => {
                                        write!(stdout, "\x1b[31m✗ Login failed: {}\x1b[0m\r\n\r\n", e)?;
                                    }
                                }
                            }
                            None => {
                                write!(stdout, "\x1b[31mUnknown provider: {}. Available: anthropic, github-copilot\x1b[0m\r\n\r\n", provider_name)?;
                            }
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/logout" || prompt.starts_with("/logout ") {
                        let provider_name = if prompt.len() > 8 {
                            prompt[8..].trim()
                        } else {
                            "anthropic"
                        };
                        
                        let token_storage = TokenStorage::new();
                        match token_storage.remove_token(provider_name) {
                            Ok(_) => {
                                write!(stdout, "\x1b[32m✓ Successfully logged out from {}\x1b[0m\r\n\r\n", provider_name)?;
                            }
                            Err(e) => {
                                write!(stdout, "\x1b[31m✗ Logout failed: {}\x1b[0m\r\n\r\n", e)?;
                            }
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/auth" {
                        let token_storage = TokenStorage::new();
                        let providers = token_storage.list_providers();
                        
                        write!(stdout, "\x1b[1mAuthentication Status:\x1b[0m\r\n")?;
                        if providers.is_empty() {
                            write!(stdout, "  No OAuth tokens stored.\r\n")?;
                            write!(stdout, "  Use /login [provider] to authenticate.\r\n")?;
                        } else {
                            for provider in providers {
                                if let Some(token) = token_storage.get_token(&provider) {
                                    let status = if token.is_expired() {
                                        "\x1b[31mexpired\x1b[0m"
                                    } else if token.is_expiring_soon() {
                                        "\x1b[33mexpiring soon\x1b[0m"
                                    } else {
                                        "\x1b[32mvalid\x1b[0m"
                                    };
                                    write!(stdout, "  {} - {}\r\n", provider, status)?;
                                }
                            }
                        }
                        write!(stdout, "\r\n")?;
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/extensions" {
                        // 显示已加载的扩展列表
                        let ext_mgr = session.extension_manager();
                        let extensions = ext_mgr.list_extensions();
                        
                        if extensions.is_empty() {
                            write!(stdout, "\x1b[2mNo extensions loaded.\x1b[0m\r\n\r\n")?;
                        } else {
                            write!(stdout, "\x1b[1mLoaded Extensions ({}):\x1b[0m\r\n", extensions.len())?;
                            for ext in extensions {
                                write!(stdout, "  \x1b[36m{}\x1b[0m v{} - {}\r\n", 
                                    ext.name, ext.version, ext.description)?;
                            }
                            write!(stdout, "\r\n")?;
                        }
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if prompt == "/help" {
                        write!(stdout, "\x1b[1mAvailable Commands:\x1b[0m\r\n")?;
                        write!(stdout, "  /help        - Show this help message\r\n")?;
                        write!(stdout, "  /clear       - Clear conversation history\r\n")?;
                        write!(stdout, "  /model       - Show or change model\r\n")?;
                        write!(stdout, "  /stats       - Show session statistics\r\n")?;
                        write!(stdout, "  /save        - Save current session\r\n")?;
                        write!(stdout, "  /fork        - Fork from current position\r\n")?;
                        write!(stdout, "  /fork N      - Fork at message index N\r\n")?;
                        write!(stdout, "  /compact     - Compact conversation history to save context space\r\n")?;
                        write!(stdout, "  /export      - Export session to HTML\r\n")?;
                        write!(stdout, "  /export path.html - Export to specific path\r\n")?;
                        write!(stdout, "  /extensions  - List loaded extensions\r\n")?;
                        write!(stdout, "  /login       - Login with OAuth (anthropic, github-copilot)\r\n")?;
                        write!(stdout, "  /logout      - Logout from OAuth provider\r\n")?;
                        write!(stdout, "  /auth        - Show authentication status\r\n")?;
                        write!(stdout, "  /exit        - Exit the application\r\n")?;
                        write!(stdout, "  /quit        - Alias for /exit\r\n\r\n")?;
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    } else if !prompt.is_empty() {
                        // 发送到 agent
                        if let Err(e) = session.prompt_text(&prompt).await {
                            write!(stdout, "\x1b[31mError: {}\x1b[0m\r\n\r\n", e)?;
                            let (term_width, _) = terminal.size();
                            let input_render = render_input_area(&editor, term_width);
                            write!(stdout, "{}", input_render)?;
                            stdout.flush()?;
                        }
                    } else {
                        // 空输入，重新渲染
                        let (term_width, _) = terminal.size();
                        let input_render = render_input_area(&editor, term_width);
                        write!(stdout, "{}", input_render)?;
                        stdout.flush()?;
                    }
                } else {
                    // 将输入传递给 Editor 处理
                    let _handled = editor.handle_input(&text);
                    
                    // 重新渲染输入区域
                    let (term_width, _) = terminal.size();
                    let input_render = render_input_area(&editor, term_width);
                    write!(stdout, "{}", input_render)?;
                    stdout.flush()?;
                }
            }
        }

        if should_exit {
            break;
        }
    }

    // 8. 清理终端状态
    terminal.disable_raw_mode()?;
    write!(stdout, "\r\nGoodbye!\r\n")?;
    stdout.flush()?;

    Ok(())
}

/// 清理文件名中的非法字符
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .take(100) // 限制长度
        .collect::<String>()
        .trim()
        .replace(' ', "_")
}
