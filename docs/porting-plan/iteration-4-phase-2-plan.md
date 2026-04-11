# ITERATION-4 Phase 2: Notebook 工具实现计划

## 背景

Phase 1（OAuth 支持、Tokenizer 集成、配置格式支持）已全部完成。Phase 2 目标是实现交互式代码执行 Notebook 工具，支持 Python/Node.js 代码块执行、输出捕获和状态管理。

## 依赖关系

```
Task 1 (Kernel 管理) ──┐
                        ├──> Task 2 (执行沙箱) ──> Task 4 (工具集成)
Task 3 (状态持久化) ───┘                              |
                                                       v
                                              Task 5 (TUI 渲染增强)
```

- Task 1 和 Task 3 可并行
- Task 2 依赖 Task 1
- Task 4 依赖 Task 2 + Task 3
- Task 5 依赖 Task 4（需要工具接口定义）

## 新建文件清单

```
crates/pi-coding-agent/src/core/tools/notebook/
  mod.rs          -- 模块定义、NotebookTool 实现（AgentTool trait）
  kernel.rs       -- Kernel 发现、生命周期管理、健康监控
  executor.rs     -- 代码执行沙箱、超时控制、输出捕获
  state.rs        -- 执行状态、变量序列化、.pinb 格式、Jupyter 导入导出
```

## 修改文件清单

- `crates/pi-coding-agent/src/core/tools/mod.rs` -- 添加 `pub mod notebook` 和 `pub use notebook::NotebookTool`
- `crates/pi-coding-agent/src/core/agent_session.rs` -- 在工具列表中注册 NotebookTool
- `crates/pi-coding-agent/src/core/system_prompt.rs` -- 添加 notebook 相关指南
- `crates/pi-tui/src/components/markdown.rs` -- 增强代码块渲染（可选执行标记）
- `crates/pi-coding-agent/Cargo.toml` -- 添加新依赖（若需要）

---

## Task 1: Kernel 管理 (kernel.rs)

**目标**: 实现 Python/Node.js Kernel 的发现、启动、停止和健康监控。

**新建文件**: `crates/pi-coding-agent/src/core/tools/notebook/kernel.rs`

**核心结构体**:

```rust
pub enum KernelType { Python, NodeJs }

pub enum KernelStatus { Starting, Running, Stopped, Crashed }

pub struct KernelSpec {
    pub kernel_type: KernelType,
    pub executable: PathBuf,   // python3 / node 路径
    pub version: String,       // 版本号
    pub display_name: String,  // 显示名称
}

pub struct KernelInstance {
    pub id: String,
    pub spec: KernelSpec,
    pub status: KernelStatus,
    pub process: Option<tokio::process::Child>,
    pub stdin: Option<tokio::process::ChildStdin>,
    pub stdout: Option<tokio::process::ChildStdout>,
    pub stderr: Option<tokio::process::ChildStderr>,
    pub started_at: Instant,
}

pub struct KernelManager {
    kernels: HashMap<String, KernelInstance>,
    cwd: PathBuf,
}
```

**核心方法**:
- `discover_kernels()` -- 使用 `which python3` / `which node` 检测可用运行时，获取版本
- `start_kernel(kernel_type)` -- 启动子进程，配置 stdin/stdout/stderr 管道
- `stop_kernel(kernel_id)` -- 优雅关闭（SIGTERM），超时后强制 kill
- `restart_kernel(kernel_id)` -- 停止后重启
- `health_check(kernel_id)` -- 通过简单 echo 命令检测存活
- `get_status(kernel_id)` -- 返回 Kernel 当前状态

**Python Kernel 启动方式**: 使用 `python3 -i -u` （交互模式 + 无缓冲输出），通过 stdin 写入代码，stdout/stderr 捕获输出。

**Node.js Kernel 启动方式**: 使用 `node --interactive`，类似方式。

**单元测试**: 测试 Kernel 发现（mock 环境）、启动/停止生命周期、健康检查。

---

## Task 2: 代码执行沙箱 (executor.rs)

**目标**: 在隔离子进程中安全执行代码，支持超时控制、内存限制和输出捕获。

**新建文件**: `crates/pi-coding-agent/src/core/tools/notebook/executor.rs`

**核心设计**: 不使用长运行 Kernel 进程的 REPL 模式（复杂度高、状态管理难），而是采用**每次执行创建独立子进程**的方式（类似 BashTool 模式），更安全可靠。

**核心结构体**:

```rust
pub struct ExecutionConfig {
    pub timeout_secs: u64,        // 默认 30s，最大 300s
    pub max_memory_mb: Option<u64>, // 内存限制（MB）
    pub cwd: PathBuf,
}

pub struct ExecutionOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub execution_time_ms: u64,
    pub is_timeout: bool,
    pub is_cancelled: bool,
    pub output_images: Vec<ImageOutput>,  // 图像输出（matplotlib 等）
}

pub struct ImageOutput {
    pub mime_type: String,
    pub data: Vec<u8>,  // base64 decoded
    pub filename: String,
}

pub struct CodeExecutor {
    config: ExecutionConfig,
}
```

**核心方法**:
- `execute_python(code, cancel_token)` -- 将代码写入临时文件，用 `python3 <tmpfile>` 执行
- `execute_nodejs(code, cancel_token)` -- 将代码写入临时文件，用 `node <tmpfile>` 执行
- `execute(language, code, cancel_token, on_update)` -- 统一入口，分发到对应语言执行器

**执行流程**（参考 BashTool 模式）:
1. 将代码写入临时文件（`tempfile` crate）
2. 构建 `tokio::process::Command`，配置 stdout/stderr 管道
3. 使用 `tokio::time::timeout` 包裹执行
4. 通过 `tokio::select!` 监听取消信号、stdout、stderr
5. 通过 `on_update` 回调发送实时输出
6. 执行完成后清理临时文件

**图像输出捕获**: Python 脚本注入 matplotlib 后端配置，将图像保存到临时目录，执行后扫描收集。

**安全措施**:
- 代码在独立子进程执行，kill_on_drop(true)
- 超时自动 kill
- 环境变量过滤（复用 BashTool 的 filter_sensitive_env_vars 逻辑）
- 临时文件执行后立即清理

---

## Task 3: 状态持久化 (state.rs)

**目标**: 管理执行状态、实现 .pinb 格式和 Jupyter 导入导出。

**新建文件**: `crates/pi-coding-agent/src/core/tools/notebook/state.rs`

**核心结构体**:

```rust
pub struct NotebookCell {
    pub cell_type: CellType,       // Code / Markdown
    pub source: String,            // 源代码
    pub language: Option<String>,  // python / javascript
    pub execution_count: Option<u32>,
    pub outputs: Vec<CellOutput>,
    pub metadata: serde_json::Value,
}

pub enum CellType { Code, Markdown }

pub enum CellOutput {
    Stream { name: String, text: String },
    ExecuteResult { data: HashMap<String, String>, execution_count: u32 },
    Error { ename: String, evalue: String, traceback: Vec<String> },
    DisplayData { data: HashMap<String, String> },
}

pub struct NotebookState {
    pub metadata: NotebookMetadata,
    pub cells: Vec<NotebookCell>,
    pub session_id: String,
}

pub struct NotebookMetadata {
    pub kernel_spec: KernelSpecInfo,
    pub language_info: LanguageInfo,
    pub created_at: String,
    pub modified_at: String,
}
```

**核心方法**:
- `save_pinb(path)` -- 保存为 .pinb 格式（JSON）
- `load_pinb(path)` -- 从 .pinb 加载
- `export_ipynb(path)` -- 导出为 Jupyter .ipynb 格式
- `import_ipynb(path)` -- 从 Jupyter .ipynb 导入
- `add_cell(cell)` -- 添加执行单元
- `update_cell_output(index, output)` -- 更新执行结果
- `get_execution_history()` -- 获取执行历史

**.pinb 格式**: 基于 JSON，与 .ipynb 结构兼容但增加 pi 特有元数据。

**Jupyter 兼容**: 严格遵循 nbformat v4 规范，确保导出的 .ipynb 可在 JupyterLab 中打开。

---

## Task 4: NotebookTool 工具集成 (mod.rs)

**目标**: 实现 `AgentTool` trait，将 Notebook 功能注册为 Agent 工具，并修改工具注册链路。

**新建文件**: `crates/pi-coding-agent/src/core/tools/notebook/mod.rs`

**修改文件**:
- `crates/pi-coding-agent/src/core/tools/mod.rs` -- 添加模块导出
- `crates/pi-coding-agent/src/core/agent_session.rs` -- 注册工具
- `crates/pi-coding-agent/src/core/system_prompt.rs` -- 添加 notebook 指南

**NotebookTool 实现**:

```rust
pub struct NotebookTool {
    cwd: PathBuf,
    executor: CodeExecutor,
    state: Arc<Mutex<Option<NotebookState>>>,
}

#[async_trait]
impl AgentTool for NotebookTool {
    fn name(&self) -> &str { "notebook" }
    fn label(&self) -> &str { "Execute Code" }
    fn description(&self) -> &str {
        "Execute Python or Node.js code in an isolated notebook environment. ..."
    }
    fn parameters(&self) -> serde_json::Value {
        // language: "python" | "javascript"
        // code: string
        // timeout: optional integer
        // action: "execute" | "save" | "export" | "status"
    }
    async fn execute(...) -> anyhow::Result<AgentToolResult> { ... }
}
```

**参数 schema**:
```json
{
  "type": "object",
  "properties": {
    "action": { "type": "string", "enum": ["execute", "save", "export", "status"] },
    "language": { "type": "string", "enum": ["python", "javascript"] },
    "code": { "type": "string" },
    "timeout": { "type": "integer" },
    "export_path": { "type": "string" },
    "format": { "type": "string", "enum": ["pinb", "ipynb"] }
  },
  "required": ["action"]
}
```

**工具注册** (agent_session.rs):
```rust
// 在工具列表中添加
tool_list.push(Arc::new(tools::NotebookTool::new(config.cwd.clone())));
```

**系统提示词** (system_prompt.rs): 添加 notebook 工具使用指南，引导 LLM 在需要代码执行时调用 notebook 工具。

---

## Task 5: TUI 代码块渲染增强

**目标**: 在 Markdown 渲染中增强代码块显示，添加语言标识和执行状态标记。

**修改文件**: `crates/pi-tui/src/components/markdown.rs`

**增强内容**:
- 代码块头部显示语言类型和执行状态标识（如 "python [executed]"）
- 执行输出区域渲染（区分 stdout、stderr、图像占位符）
- 错误堆栈高亮显示（stderr 用红色渲染）

**实现方式**: 在 `Tag::CodeBlock` 处理中，检测语言标识是否为可执行语言（python/javascript），若是则添加视觉标记。执行结果通过 Agent 输出的文本内容自然渲染，无需特殊处理。

---

## 验收标准

1. Agent 检测到代码执行需求时调用 notebook 工具
2. Python 代码在隔离进程中执行，stdout/stderr 正确捕获
3. Node.js 代码在隔离进程中执行，stdout/stderr 正确捕获
4. 无限循环代码被超时中断（默认 30s）
5. 代码执行错误正确显示堆栈跟踪
6. Notebook 状态可保存为 .pinb 格式并恢复
7. 支持导出为 .ipynb 格式（可在 JupyterLab 打开）
8. 与 FileEditTool 等其他工具协作正常
9. `cargo build` 编译通过
10. `cargo test` 现有测试不回归

## 执行顺序

1. **并行**: Task 1 (kernel.rs) + Task 3 (state.rs) -- 两者无依赖
2. **串行**: Task 2 (executor.rs) -- 依赖 Task 1
3. **串行**: Task 4 (mod.rs + 工具注册) -- 依赖 Task 2 + Task 3
4. **串行**: Task 5 (TUI 增强) -- 依赖 Task 4
5. **最后**: 编译验证 + 集成测试
