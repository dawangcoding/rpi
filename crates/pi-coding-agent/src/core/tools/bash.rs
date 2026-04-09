//! Bash 命令执行工具
//!
//! 执行 shell 命令并捕获输出

use std::path::PathBuf;
use std::process::Stdio;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;
use pi_agent::types::*;
use pi_ai::types::*;
use super::truncate::*;

/// Bash 命令执行工具
pub struct BashTool {
    cwd: PathBuf,
    shell: String,
}

impl BashTool {
    /// 创建新的 BashTool
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
        }
    }

    /// 设置 shell
    pub fn with_shell(mut self, shell: String) -> Self {
        self.shell = shell;
        self
    }
}

#[async_trait]
impl AgentTool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn label(&self) -> &str {
        "Execute Command"
    }

    fn description(&self) -> &str {
        "Execute a shell command. Use for running scripts, installing packages, building projects, or any task that requires shell access. Returns stdout and stderr combined. Output is truncated to last 500 lines or 1MB (whichever is hit first)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120, max: 600)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
        on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
    ) -> anyhow::Result<AgentToolResult> {
        let command = params["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;
        
        let timeout_secs = params["timeout"]
            .as_u64()
            .map(|t| t.min(600))
            .unwrap_or(120);

        // 检查工作目录是否存在
        if !self.cwd.exists() {
            return Err(anyhow::anyhow!(
                "Working directory does not exist: {}\nCannot execute bash commands.",
                self.cwd.display()
            ));
        }

        // 启动命令
        let mut child = Command::new(&self.shell)
            .arg("-c")
            .arg(command)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn shell: {}", e))?;

        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        // 收集输出
        let mut output_lines: Vec<String> = Vec::new();
        let mut is_cancelled = false;
        let mut is_timeout = false;

        // 使用超时
        let result = timeout(Duration::from_secs(timeout_secs), async {
            loop {
                tokio::select! {
                    // 检查取消信号
                    _ = cancel.cancelled() => {
                        is_cancelled = true;
                        let _ = child.kill().await;
                        break;
                    }
                    // 读取 stdout
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                output_lines.push(line);
                                // 发送更新
                                if let Some(ref callback) = on_update {
                                    let current_output = output_lines.join("\n");
                                    let (truncated, _) = truncate_output_tail(&current_output, DEFAULT_MAX_LINES, DEFAULT_MAX_BYTES);
                                    callback(AgentToolResult {
                                        content: vec![ContentBlock::Text(TextContent::new(truncated))],
                                        details: serde_json::json!({}),
                                    });
                                }
                            }
                            Ok(None) => {
                                // stdout 结束，继续读取 stderr
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                }
            }

            // 继续读取 stderr
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                output_lines.push(line);
            }

            // 等待进程结束
            child.wait().await
        }).await;

        let exit_status = match result {
            Ok(status) => status,
            Err(_) => {
                is_timeout = true;
                let _ = child.kill().await;
                child.wait().await
            }
        };

        // 合并输出
        let full_output = output_lines.join("\n");
        
        // 应用尾部截断（保留最后的内容，错误通常在最后）
        let (truncated_output, truncation_result) = truncate_output_tail(&full_output, DEFAULT_MAX_LINES, DEFAULT_MAX_BYTES);

        // 构建结果
        let mut output_text = truncated_output;
        let mut is_error = false;

        // 添加状态信息
        if is_cancelled {
            if !output_text.is_empty() {
                output_text.push_str("\n\n");
            }
            output_text.push_str("Command aborted");
            is_error = true;
        } else if is_timeout {
            if !output_text.is_empty() {
                output_text.push_str("\n\n");
            }
            output_text.push_str(&format!("Command timed out after {} seconds", timeout_secs));
            is_error = true;
        } else if let Ok(status) = exit_status {
            if !status.success() {
                let code = status.code().unwrap_or(-1);
                if !output_text.is_empty() {
                    output_text.push_str("\n\n");
                }
                output_text.push_str(&format!("Command exited with code {}", code));
                is_error = true;
            }
        }

        // 构建 details
        let details = serde_json::json!({
            "truncation": if truncation_result.was_truncated {
                serde_json::to_value(&truncation_result)?
            } else {
                serde_json::Value::Null
            },
            "exit_code": exit_status.ok().and_then(|s| s.code()),
            "timeout": is_timeout,
            "cancelled": is_cancelled,
        });

        let result = AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(output_text))],
            details,
        };

        if is_error {
            // 对于错误情况，我们仍然返回结果，但标记为错误
            // 注意：AgentToolResult 本身没有 is_error 字段，这是通过返回 Err 来处理的
            // 但为了保留输出内容，我们可能需要特殊处理
        }

        Ok(result)
    }
}
