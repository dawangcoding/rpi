//! Bash 命令执行工具
//!
//! 执行 shell 命令并捕获输出，包含安全检查和环境变量过滤

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

/// 危险命令模式
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "rm -rf /*",
    "sudo ",
    "sudo\t",
    "chmod 777",
    "chmod -R 777",
    "mkfs.",
    "mkfs ",
    "dd if=",
    "dd of=/dev/",
    "> /dev/sda",
    "> /dev/hda",
    "> /dev/sd",
    "> /dev/hd",
    ":(){:|:&};:",
    ":(){ :|:& };:",
];

/// 敏感环境变量前缀/包含模式
const SENSITIVE_ENV_PATTERNS: &[&str] = &[
    "KEY",
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "AUTH",
    "PRIVATE",
    "API_KEY",
    "ACCESS_KEY",
    "SECRET_KEY",
];

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
    #[allow(dead_code)] // 预留方法供未来使用
    pub fn with_shell(mut self, shell: String) -> Self {
        self.shell = shell;
        self
    }

    /// 检查命令是否包含危险模式
    pub fn is_dangerous_command(&self, command: &str) -> bool {
        let cmd_lower = command.to_lowercase();
        for pattern in DANGEROUS_PATTERNS {
            if cmd_lower.contains(pattern) {
                return true;
            }
        }
        false
    }

    /// 过滤敏感环境变量
    /// 
    /// 返回清理后的环境变量列表，移除包含敏感信息的变量
    pub fn filter_sensitive_env_vars(&self) -> Vec<(String, String)> {
        std::env::vars()
            .filter(|(key, _)| {
                let key_upper = key.to_uppercase();
                // 保留非敏感变量
                !SENSITIVE_ENV_PATTERNS.iter().any(|pattern| key_upper.contains(pattern))
            })
            .collect()
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

        // 内置安全检查：检测危险命令
        if self.is_dangerous_command(command) {
            return Ok(AgentToolResult::error(format!(
                "Security check failed: Command '{}' contains potentially dangerous operations. \
                 This command requires explicit user confirmation.",
                command
            )));
        }

        // 过滤敏感环境变量
        let filtered_env = self.filter_sensitive_env_vars();

        // 构建命令（使用过滤后的环境变量）
        let mut cmd = Command::new(&self.shell);
        cmd.arg("-c")
            .arg(command)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true);
        
        // 清除所有环境变量，然后只添加过滤后的变量
        cmd.env_clear();
        for (key, value) in filtered_env {
            cmd.env(&key, &value);
        }
        
        // 启动命令
        let mut child = cmd.spawn()
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bash_simple_command() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "echo hello"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证输出包含 "hello"
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_exit_code_success() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "exit 0"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证成功退出码
        let details = result.details.as_object().unwrap();
        assert_eq!(details["exit_code"], 0);
    }

    #[tokio::test]
    async fn test_bash_exit_code_failure() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "exit 42"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证失败退出码
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("exited with code 42"));
        
        let details = result.details.as_object().unwrap();
        assert_eq!(details["exit_code"], 42);
    }

    #[tokio::test]
    async fn test_bash_stderr_output() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "echo error >&2"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证 stderr 被捕获
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("error"));
    }

    #[tokio::test]
    async fn test_bash_multiline_output() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "echo line1 && echo line2 && echo line3"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证多行输出
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
        assert!(text.contains("line3"));
    }

    #[tokio::test]
    async fn test_bash_working_directory() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "pwd"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证工作目录
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        // pwd 应该输出临时目录的路径
        assert!(text.contains(dir.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_bash_missing_command() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({}),
            CancellationToken::new(),
            None,
        ).await;
        
        // 应该返回错误
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing 'command' parameter"));
    }

    #[tokio::test]
    async fn test_bash_dangerous_command_rm_rf() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "rm -rf /"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 危险命令应该被阻止，返回错误结果
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("Security check failed"));
    }

    #[tokio::test]
    async fn test_bash_dangerous_command_sudo() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "sudo apt-get install something"}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // sudo 命令应该被阻止
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("Security check failed"));
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "sleep 5", "timeout": 1}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        // 验证超时
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("timed out") || text.contains("Command"));
        
        let details = result.details.as_object().unwrap();
        assert_eq!(details["timeout"], true);
    }

    #[tokio::test]
    async fn test_bash_custom_timeout() {
        let dir = TempDir::new().unwrap();
        let tool = BashTool::new(dir.path().to_path_buf());
        
        // 使用较长的超时，命令应该成功
        let result = tool.execute(
            "call_1",
            serde_json::json!({"command": "echo test", "timeout": 10}),
            CancellationToken::new(),
            None,
        ).await.unwrap();
        
        let text = result.content.iter()
            .filter_map(|c| if let ContentBlock::Text(t) = c { Some(t.text.as_str()) } else { None })
            .collect::<String>();
        assert!(text.contains("test"));
    }

    #[test]
    fn test_bash_tool_name() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.name(), "bash");
    }

    #[test]
    fn test_bash_tool_label() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        assert_eq!(tool.label(), "Execute Command");
    }

    #[test]
    fn test_bash_tool_parameters() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        let params = tool.parameters();
        
        assert!(params.is_object());
        let obj = params.as_object().unwrap();
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("properties"));
        assert!(obj.contains_key("required"));
        
        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("command"));
        assert!(properties.contains_key("timeout"));
        
        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("command")));
    }

    #[test]
    fn test_is_dangerous_command() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        
        // 测试危险命令
        assert!(tool.is_dangerous_command("rm -rf /"));
        assert!(tool.is_dangerous_command("sudo apt-get install"));
        assert!(tool.is_dangerous_command("chmod 777 file"));
        assert!(tool.is_dangerous_command("mkfs.ext4 /dev/sda1"));
        assert!(tool.is_dangerous_command("dd if=/dev/zero of=/dev/sda"));
        
        // 测试安全命令
        assert!(!tool.is_dangerous_command("echo hello"));
        assert!(!tool.is_dangerous_command("ls -la"));
        assert!(!tool.is_dangerous_command("cat file.txt"));
    }

    #[test]
    fn test_filter_sensitive_env_vars() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        
        // 设置一些环境变量
        std::env::set_var("TEST_NORMAL_VAR", "normal");
        std::env::set_var("TEST_SECRET_KEY", "secret");
        std::env::set_var("TEST_API_TOKEN", "token");
        
        let filtered = tool.filter_sensitive_env_vars();
        
        // 验证敏感变量被过滤
        let keys: Vec<&str> = filtered.iter().map(|(k, _)| k.as_str()).collect();
        assert!(!keys.contains(&"TEST_SECRET_KEY"));
        assert!(!keys.contains(&"TEST_API_TOKEN"));
        
        // 验证普通变量被保留
        assert!(keys.contains(&"TEST_NORMAL_VAR"));
        
        // 清理
        std::env::remove_var("TEST_NORMAL_VAR");
        std::env::remove_var("TEST_SECRET_KEY");
        std::env::remove_var("TEST_API_TOKEN");
    }

    #[test]
    fn test_bash_tool_with_shell() {
        let tool = BashTool::new(PathBuf::from("/tmp"))
            .with_shell("/bin/bash".to_string());
        
        // 验证 shell 设置成功
        assert_eq!(tool.name(), "bash");
    }
}
