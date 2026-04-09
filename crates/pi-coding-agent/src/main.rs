//! pi - AI coding agent CLI
//!
//! CLI 入口，处理参数解析、模型选择、模式路由

use clap::Parser;
use anyhow::Result;

mod cli;
mod config;
mod core;
mod modes;

use cli::args::CliArgs;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into())
        )
        .init();

    let args = CliArgs::parse();

    // 处理离线模式
    if args.offline {
        std::env::set_var("PI_OFFLINE", "1");
        std::env::set_var("PI_SKIP_VERSION_CHECK", "1");
    }

    // 加载配置
    let config = AppConfig::load().unwrap_or_default();

    // 列出模型
    if args.list_models {
        list_models();
        return Ok(());
    }

    // 确定模型
    let model_id = args.model
        .or(config.default_model.clone())
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let model = pi_ai::models::get_model(&model_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_id))?;

    // 确定 thinking level
    let thinking_level = args.thinking
        .or(config.default_thinking.clone())
        .map(|s| parse_thinking_level_enum(&s))
        .unwrap_or(if model.reasoning {
            pi_ai::types::ThinkingLevel::Medium
        } else {
            pi_ai::types::ThinkingLevel::Off
        });

    // 确定工作目录
    let cwd = args.cwd
        .map(|p| std::path::PathBuf::from(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // 构建初始 prompt
    let initial_prompt = if !args.prompt.is_empty() {
        Some(args.prompt.join(" "))
    } else {
        None
    };

    // 路由到不同模式
    match args.mode.as_str() {
        "interactive" => {
            modes::interactive::run(modes::interactive::InteractiveConfig {
                model: model.clone(),
                thinking_level: thinking_level.clone(),
                system_prompt: args.system_prompt,
                append_system_prompt: args.append_system_prompt,
                context_files: args.context_files,
                cwd: cwd.clone(),
                no_bash: args.no_bash,
                no_edit: args.no_edit,
                app_config: config,
                initial_prompt,
            }).await?;
        }
        "print" | "json" => {
            let prompt = initial_prompt
                .ok_or_else(|| anyhow::anyhow!("Print mode requires a prompt"))?;
            modes::print_mode::run(modes::print_mode::PrintConfig {
                model: model.clone(),
                thinking_level: thinking_level.clone(),
                system_prompt: args.system_prompt,
                append_system_prompt: args.append_system_prompt,
                context_files: args.context_files,
                cwd: cwd.clone(),
                no_bash: args.no_bash,
                no_edit: args.no_edit,
                app_config: config,
                prompt,
                no_stream: args.no_stream,
            }).await?;
        }
        other => {
            anyhow::bail!("Unknown mode: {}", other);
        }
    }

    Ok(())
}

/// 列出可用模型
fn list_models() {
    let models = pi_ai::models::get_models();
    println!("Available models:\n");
    for model in &models {
        println!(
            "  {:<45} {:<20} ctx:{:>8}  ${:.2}/$M in / ${:.2}/$M out",
            model.id,
            format!("{:?}", model.provider),
            model.context_window,
            model.cost.input,
            model.cost.output,
        );
    }
}

/// 解析 thinking level 字符串为枚举
fn parse_thinking_level_enum(s: &str) -> pi_ai::types::ThinkingLevel {
    match s {
        "off" => pi_ai::types::ThinkingLevel::Off,
        "minimal" => pi_ai::types::ThinkingLevel::Minimal,
        "low" => pi_ai::types::ThinkingLevel::Low,
        "medium" => pi_ai::types::ThinkingLevel::Medium,
        "high" => pi_ai::types::ThinkingLevel::High,
        "xhigh" => pi_ai::types::ThinkingLevel::XHigh,
        _ => pi_ai::types::ThinkingLevel::Off,
    }
}
