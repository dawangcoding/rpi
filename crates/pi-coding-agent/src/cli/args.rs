//! CLI 参数解析
//!
//! 使用 clap derive 宏定义命令行参数

use clap::Parser;

/// CLI 参数结构
#[derive(Parser, Debug)]
#[command(name = "pi", about = "AI coding agent", version)]
pub struct CliArgs {
    /// Initial prompt (positional, optional, accepts multiple values)
    #[arg(trailing_var_arg = true)]
    pub prompt: Vec<String>,

    /// LLM model (provider:model format)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Provider name
    #[arg(long)]
    pub provider: Option<String>,

    /// API key
    #[arg(long = "api-key")]
    pub api_key: Option<String>,

    /// Thinking/reasoning level
    #[arg(short, long, value_parser = parse_thinking_level)]
    pub thinking: Option<String>,

    /// Custom system prompt
    #[arg(long = "system-prompt")]
    pub system_prompt: Option<String>,

    /// Append to system prompt
    #[arg(long = "append-system-prompt")]
    pub append_system_prompt: Option<String>,

    /// Session file path
    #[arg(short, long)]
    pub session: Option<String>,

    /// Session ID
    #[arg(long = "session-id")]
    pub session_id: Option<String>,

    /// Session directory
    #[arg(long = "session-dir")]
    pub session_dir: Option<String>,

    /// Continue previous session
    #[arg(short, long)]
    pub r#continue: bool,

    /// Resume a session (select from list)
    #[arg(short, long)]
    pub resume: bool,

    /// Fork session from path or ID
    #[arg(long)]
    pub fork: Option<String>,

    /// No session (ephemeral mode)
    #[arg(long = "no-session")]
    pub no_session: bool,

    /// Run mode
    #[arg(long, default_value = "interactive")]
    pub mode: String,

    /// Print mode (non-interactive)
    #[arg(short, long)]
    pub print: bool,

    /// Add file to initial message
    #[arg(long = "file", num_args = 1)]
    pub files: Vec<String>,

    /// Project context file
    #[arg(long = "context-file")]
    pub context_files: Vec<String>,

    /// Disable bash tool
    #[arg(long)]
    pub no_bash: bool,

    /// Disable edit tool
    #[arg(long)]
    pub no_edit: bool,

    /// Disable all tools
    #[arg(long = "no-tools")]
    pub no_tools: bool,

    /// Enable specific tools (comma-separated)
    #[arg(long)]
    pub tools: Option<String>,

    /// Disable streaming
    #[arg(long = "no-stream")]
    pub no_stream: bool,

    /// Export session to HTML
    #[arg(long)]
    pub export: Option<String>,

    /// Working directory
    #[arg(long, short = 'C')]
    pub cwd: Option<String>,

    /// List available models
    #[arg(long = "list-models")]
    pub list_models: bool,

    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,

    /// Offline mode
    #[arg(long)]
    pub offline: bool,

    // 注意：clap 自动生成 --help 和 --version，不需要显式定义
}

/// 解析 thinking level
fn parse_thinking_level(s: &str) -> Result<String, String> {
    match s {
        "off" | "minimal" | "low" | "medium" | "high" | "xhigh" => Ok(s.to_string()),
        _ => Err(format!(
            "Invalid thinking level: {s}. Valid: off, minimal, low, medium, high, xhigh"
        )),
    }
}
