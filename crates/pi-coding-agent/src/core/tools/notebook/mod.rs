//! Notebook 工具模块
//!
//! 提供 Notebook 执行状态管理和持久化功能

pub mod executor;
pub mod kernel;
pub mod state;

pub use executor::*;
pub use kernel::*;
pub use state::*;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use pi_agent::types::{AgentTool, AgentToolResult};
use pi_ai::types::{ContentBlock, TextContent};

/// Notebook 工具
///
/// 执行 Python 或 Node.js 代码，支持状态持久化和导出
pub struct NotebookTool {
    cwd: PathBuf,
    kernel_manager: Arc<Mutex<KernelManager>>,
    state: Arc<Mutex<Option<NotebookState>>>,
}

impl NotebookTool {
    /// 创建新的 NotebookTool
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd: cwd.clone(),
            kernel_manager: Arc::new(Mutex::new(KernelManager::new(cwd))),
            state: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl AgentTool for NotebookTool {
    fn name(&self) -> &str {
        "notebook"
    }

    fn label(&self) -> &str {
        "Execute Code"
    }

    fn description(&self) -> &str {
        "Execute Python or Node.js code in an isolated notebook environment. \
         Supports code execution with output capture, timeout control, and state persistence. \
         Use this tool when you need to run code, perform calculations, data analysis, \
         or generate visualizations. Results are captured and can be exported as Jupyter notebooks."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["execute", "save", "export", "status"],
                    "description": "Action to perform: 'execute' to run code, 'save' to save notebook state, 'export' to export as .ipynb, 'status' to check available kernels"
                },
                "language": {
                    "type": "string",
                    "enum": ["python", "javascript"],
                    "description": "Programming language for code execution (required for 'execute' action)"
                },
                "code": {
                    "type": "string",
                    "description": "Code to execute (required for 'execute' action)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Execution timeout in seconds (default: 30, max: 300)"
                },
                "export_path": {
                    "type": "string",
                    "description": "File path for save/export actions"
                },
                "format": {
                    "type": "string",
                    "enum": ["pinb", "ipynb"],
                    "description": "Export format (default: 'ipynb')"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        params: serde_json::Value,
        cancel: CancellationToken,
        on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
    ) -> anyhow::Result<AgentToolResult> {
        let action = params["action"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;

        match action {
            "execute" => self.handle_execute(params, cancel, on_update).await,
            "save" => self.handle_save(params).await,
            "export" => self.handle_export(params).await,
            "status" => self.handle_status().await,
            _ => Ok(AgentToolResult::error(format!("Unknown action: {}", action))),
        }
    }
}

impl NotebookTool {
    /// 解析并验证导出路径，确保在工作目录内
    fn resolve_export_path(&self, path_str: &str) -> anyhow::Result<std::path::PathBuf> {
        let path = std::path::Path::new(path_str);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        };

        // 规范化路径（解析 .. 等）
        // 注意：canonicalize 需要路径存在，对于新文件需要 canonicalize 父目录
        let parent = resolved
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid export path: {}", path_str))?;

        // 确保父目录存在
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }

        let canonical_parent = parent.canonicalize().map_err(|e| {
            anyhow::anyhow!("Cannot resolve export path '{}': {}", path_str, e)
        })?;
        let canonical_cwd = self.cwd.canonicalize().map_err(|e| {
            anyhow::anyhow!("Cannot resolve working directory: {}", e)
        })?;

        let file_name = resolved
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Export path has no filename: {}", path_str))?;
        let final_path = canonical_parent.join(file_name);

        if !canonical_parent.starts_with(&canonical_cwd) {
            return Err(anyhow::anyhow!(
                "Export path '{}' is outside the working directory '{}'",
                path_str,
                self.cwd.display()
            ));
        }

        Ok(final_path)
    }

    /// 处理 execute action
    async fn handle_execute(
        &self,
        params: serde_json::Value,
        cancel: CancellationToken,
        on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
    ) -> anyhow::Result<AgentToolResult> {
        // 1. 解析参数
        let language_str = params["language"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'language' parameter for execute action"))?;
        let code = params["code"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'code' parameter for execute action"))?;
        let timeout_secs = params["timeout"].as_u64();

        // 2. 解析语言类型
        let kernel_type: KernelType = language_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!("{}", e))?;

        // 3. 发现可用 Kernel
        let mut km = self.kernel_manager.lock().await;
        if km.available_kernels().is_empty() {
            km.discover_kernels().await;
        }

        // 4. 获取可执行文件路径
        let executable = km
            .get_executable(kernel_type)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{} is not available on this system. Please install it first.",
                    kernel_type.display_name()
                )
            })?
            .clone();
        drop(km); // 释放锁

        // 5. 配置执行器超时
        let config = if let Some(t) = timeout_secs {
            ExecutionConfig::new(self.cwd.clone()).with_timeout(t)
        } else {
            ExecutionConfig::new(self.cwd.clone())
        };
        let executor = CodeExecutor::new(config);

        // 6. 创建 on_update 包装器
        // 由于 executor.execute 需要一个引用，我们需要创建一个包装结构
        let update_callback: Option<Box<dyn Fn(String) + Send + Sync>> = on_update.map(|cb| {
            Box::new(move |line: String| {
                cb(AgentToolResult {
                    content: vec![ContentBlock::Text(TextContent::new(&line))],
                    details: serde_json::json!({}),
                });
            }) as Box<dyn Fn(String) + Send + Sync>
        });

        // 7. 执行代码
        let result = if let Some(ref callback) = update_callback {
            executor.execute(kernel_type, code, &executable, cancel, Some(callback)).await?
        } else {
            executor.execute(kernel_type, code, &executable, cancel, None).await?
        };

        // 8. 更新 Notebook 状态
        let mut state_guard = self.state.lock().await;
        let state = state_guard.get_or_insert_with(|| {
            let session_id = format!(
                "session_{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            );
            NotebookState::new(session_id, kernel_type.language_name())
        });

        // 添加代码单元格
        let cell_index = state.add_code_cell(code.to_string(), kernel_type.language_name());

        // 构建输出
        let mut cell_outputs = Vec::new();
        if !result.stdout.is_empty() {
            cell_outputs.push(CellOutput::Stream {
                name: "stdout".to_string(),
                text: result.stdout.clone(),
            });
        }
        if !result.stderr.is_empty() {
            if result.is_success() {
                cell_outputs.push(CellOutput::Stream {
                    name: "stderr".to_string(),
                    text: result.stderr.clone(),
                });
            } else {
                cell_outputs.push(CellOutput::Error {
                    ename: "ExecutionError".to_string(),
                    evalue: result.stderr.lines().last().unwrap_or("").to_string(),
                    traceback: result.stderr.lines().map(String::from).collect(),
                });
            }
        }
        
        // 处理非零退出但无 stderr 的情况
        if !result.is_success() && result.stderr.is_empty() {
            cell_outputs.push(CellOutput::Error {
                ename: "ExecutionError".to_string(),
                evalue: format!("Process exited with code {:?}", result.exit_code),
                traceback: vec![],
            });
        }

        let _ = state.update_cell_output(cell_index, cell_outputs);
        let _ = state.set_execution_count(cell_index);
        drop(state_guard);

        // 9. 构建返回结果
        let output_text = result.formatted_output();
        let details = serde_json::json!({
            "language": kernel_type.language_name(),
            "exit_code": result.exit_code,
            "execution_time_ms": result.execution_time_ms,
            "timeout": result.is_timeout,
            "cancelled": result.is_cancelled,
            "image_count": result.images.len(),
        });

        Ok(AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(
                if output_text.is_empty() {
                    "[No output]".to_string()
                } else {
                    output_text
                },
            ))],
            details,
        })
    }

    /// 处理 save action
    async fn handle_save(&self, params: serde_json::Value) -> anyhow::Result<AgentToolResult> {
        let export_path = params["export_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'export_path' parameter for save action"))?;
        let path = self.resolve_export_path(export_path)?;

        let state_guard = self.state.lock().await;
        if let Some(state) = state_guard.as_ref() {
            state.save_pinb(&path)?;
            Ok(AgentToolResult {
                content: vec![ContentBlock::Text(TextContent::new(format!(
                    "Notebook saved to {}",
                    export_path
                )))],
                details: serde_json::json!({"path": export_path}),
            })
        } else {
            Ok(AgentToolResult::error(
                "No notebook state to save. Execute some code first.",
            ))
        }
    }

    /// 处理 export action
    async fn handle_export(&self, params: serde_json::Value) -> anyhow::Result<AgentToolResult> {
        let export_path = params["export_path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'export_path' parameter for export action"))?;
        let format = params["format"].as_str().unwrap_or("ipynb");
        let path = self.resolve_export_path(export_path)?;

        let state_guard = self.state.lock().await;
        if let Some(state) = state_guard.as_ref() {
            match format {
                "ipynb" => state.export_ipynb(&path)?,
                "pinb" => state.save_pinb(&path)?,
                _ => return Ok(AgentToolResult::error(format!("Unknown format: {}", format))),
            }
            Ok(AgentToolResult {
                content: vec![ContentBlock::Text(TextContent::new(format!(
                    "Notebook exported to {} (format: {})",
                    export_path, format
                )))],
                details: serde_json::json!({"path": export_path, "format": format}),
            })
        } else {
            Ok(AgentToolResult::error(
                "No notebook state to export. Execute some code first.",
            ))
        }
    }

    /// 处理 status action
    async fn handle_status(&self) -> anyhow::Result<AgentToolResult> {
        let mut km = self.kernel_manager.lock().await;
        km.discover_kernels().await;
        let kernels = km.available_kernels();

        let mut status_lines = vec!["Available kernels:".to_string()];
        if kernels.is_empty() {
            status_lines.push(
                "  No kernels found. Please install Python 3 or Node.js.".to_string(),
            );
        } else {
            for spec in kernels {
                status_lines.push(format!(
                    "  - {} ({})",
                    spec.display_name,
                    spec.executable.display()
                ));
            }
        }

        // Notebook 状态
        let state_guard = self.state.lock().await;
        if let Some(state) = state_guard.as_ref() {
            status_lines.push(format!(
                "\nNotebook: {} cells, {} executed",
                state.cell_count(),
                state.get_execution_history().len(),
            ));
        } else {
            status_lines.push("\nNotebook: No active session".to_string());
        }

        Ok(AgentToolResult {
            content: vec![ContentBlock::Text(TextContent::new(status_lines.join("\n")))],
            details: serde_json::json!({"kernel_count": kernels.len()}),
        })
    }
}
