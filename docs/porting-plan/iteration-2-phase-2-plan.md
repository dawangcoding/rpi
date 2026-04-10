# 二次迭代第二阶段开发计划

## 概述

第二阶段包含 4 个核心功能模块，按依赖关系和独立性分阶段实施：
- Task 1: Mistral Provider 实现（独立，可最先启动）
- Task 2: 编辑器组件完整集成（独立，可与 Task 1 并行）
- Task 3: 工具权限管理（独立，可与 Task 1/2 并行）
- Task 4: 上下文窗口管理（依赖 pi-ai 的 token 计数，需在 Mistral Provider 之后或并行）

---

## Task 1: Mistral Provider 实现

**目标**: 在 pi-ai crate 中新增 Mistral AI Provider，支持流式聊天完成和工具调用。

**涉及文件**:

| 文件 | 操作 |
|------|------|
| `crates/pi-ai/src/providers/mistral.rs` | 新增 |
| `crates/pi-ai/src/providers/mod.rs` | 修改 |
| `crates/pi-ai/src/lib.rs` | 修改 |
| `crates/pi-ai/src/models.rs` | 修改 |

**子任务**:

### Task 1.1: 实现 MistralProvider 核心结构

新增 `mistral.rs`，参照 `anthropic.rs`（1170 行）的实现模式：

- 定义 `MistralProvider` 结构体（持有 `reqwest::Client`）
- 实现 `ApiProvider` trait 的 `api()` 方法返回 `Api::Mistral`
- 实现 `stream()` 方法：
  - 构建 Mistral Chat Completions API 请求体（兼容 OpenAI 格式但有差异）
  - 发送 SSE 流式请求到 `https://api.mistral.ai/v1/chat/completions`
  - 解析流式响应并转换为 `AssistantMessageEvent` 事件流

关键实现细节：
- 请求头: `Authorization: Bearer {api_key}`, `Content-Type: application/json`
- 消息格式: `{ role, content }` 数组（与 OpenAI 类似）
- 工具调用: Mistral 的 tool_call ID 有 9 字符长度限制，需特殊处理
- 流式事件: `data: {...}` 格式，需解析 `choices[0].delta`

### Task 1.2: 添加 Mistral 模型定义

修改 `models.rs` 的 `builtin_models()` 函数，添加：
- `mistral-large-latest` - 旗舰模型，128K 上下文
- `mistral-medium-latest` - 中等模型
- `mistral-small-latest` - 轻量模型
- `codestral-latest` - 代码专用模型

每个模型需定义：`id`, `name`, `api: Api::Mistral`, `provider: Provider::Mistral`, `base_url`, `context_window`, `max_tokens`, `cost`

### Task 1.3: 注册 Mistral Provider

- 修改 `providers/mod.rs`: 添加 `pub mod mistral;` 和 `pub use mistral::MistralProvider;`
- 修改 `lib.rs` 的 `init_providers()`: 添加 `register_api_provider(Arc::new(providers::MistralProvider::new()));`

---

## Task 2: 编辑器组件完整集成

**目标**: 将 pi-tui 的 Editor 组件集成到交互模式，替换当前的简单 stdin 输入处理。

**涉及文件**:

| 文件 | 操作 |
|------|------|
| `crates/pi-coding-agent/src/modes/interactive.rs` | 修改（重构输入处理） |
| `crates/pi-coding-agent/src/modes/interactive_components.rs` | 修改（添加编辑器渲染） |

**子任务**:

### Task 2.1: 替换输入缓冲区为 Editor 实例

修改 `interactive.rs`：

- 移除 `input_buffer: String` 字段
- 创建 `Editor` 实例，配置：
  ```rust
  EditorConfig {
      placeholder: Some("> Ask anything...".into()),
      max_lines: None,  // 允许多行
      read_only: false,
      line_numbers: false,
      wrap: true,
  }
  ```
- Editor 设为 focused 状态

### Task 2.2: 重构按键处理逻辑

将当前的逐字符处理（第 179-257 行）替换为 Editor 的 `handle_input()`:

- 保留 Ctrl+C（取消）和 Ctrl+D（退出）的特殊处理
- 将所有其他按键传递给 `editor.handle_input(&data)`
- Enter 键逻辑调整：
  - 单行模式：Enter 直接提交
  - 多行模式：Shift+Enter 插入新行，Enter 提交
- 提交时调用 `editor.get_text()` 获取完整输入，然后 `editor.set_text("")` 清空

### Task 2.3: 集成自动完成

- 实现 `CodingAgentAutocompleteProvider`（实现 `AutocompleteProvider` trait）
- 支持 slash 命令补全（`/help`, `/clear`, `/model`, `/exit` 等）
- 支持 `@` 文件路径补全
- 注册到 Editor: `editor.set_autocomplete_provider(Box::new(provider))`

### Task 2.4: 编辑器渲染集成

修改 `interactive_components.rs`：

- 在输入区域使用 `editor.render(width)` 替代简单的文本显示
- 处理光标位置的终端渲染
- 处理自动完成弹窗的渲染叠加

---

## Task 3: 工具权限管理

**目标**: 在 pi-coding-agent 中实现工具权限控制框架，包括首次确认、白/黑名单、执行钩子。

**涉及文件**:

| 文件 | 操作 |
|------|------|
| `crates/pi-coding-agent/src/core/permissions.rs` | 新增 |
| `crates/pi-coding-agent/src/core/mod.rs` | 修改 |
| `crates/pi-coding-agent/src/config.rs` | 修改 |
| `crates/pi-coding-agent/src/core/agent_session.rs` | 修改 |
| `crates/pi-coding-agent/src/core/tools/bash.rs` | 修改 |
| `crates/pi-coding-agent/src/core/tools/mod.rs` | 修改 |

**子任务**:

### Task 3.1: 定义权限类型和管理器

新增 `permissions.rs`，定义核心类型：

```rust
pub enum PermissionLevel {
    AlwaysAllow,    // 始终允许，不提示
    AskFirst,       // 首次使用时询问用户
    AskEveryTime,   // 每次使用都询问
    Deny,           // 禁止使用
}

pub struct ToolPermissionConfig {
    pub default_level: PermissionLevel,
    pub tool_overrides: HashMap<String, PermissionLevel>,
    pub bash_blocked_commands: Vec<String>,      // rm -rf, sudo 等
    pub bash_allowed_commands: Option<Vec<String>>, // 白名单模式
    pub max_execution_time_secs: u64,
}

pub struct PermissionManager {
    config: ToolPermissionConfig,
    granted_tools: HashSet<String>,  // 运行时已授权的工具
}
```

PermissionManager 核心方法：
- `check_tool_permission(tool_name: &str) -> PermissionCheckResult`
- `check_bash_command(command: &str) -> PermissionCheckResult`
- `grant_tool(tool_name: &str)` - 记录用户授权
- `is_dangerous_command(command: &str) -> bool` - 检测危险命令

### Task 3.2: 集成权限配置

修改 `config.rs` 的 `AppConfig`：

```rust
pub struct AppConfig {
    // ... 现有字段
    pub permissions: Option<ToolPermissionConfig>,
}
```

支持从配置文件加载权限设置。

### Task 3.3: 在 AgentSession 中集成权限管理

修改 `agent_session.rs`：

- 在 `AgentSession` 中持有 `PermissionManager` 实例
- 在工具执行前调用权限检查
- 实现 `before_tool_call` 钩子：权限检查 -> 用户确认（如需要）-> 放行/拒绝
- 实现 `after_tool_call` 钩子：记录执行结果

### Task 3.4: 增强 Bash 工具安全性

修改 `bash.rs`：

- 在 `execute()` 方法开头添加命令检查
- 实现危险命令检测：`rm -rf`, `sudo`, `chmod 777`, `mkfs`, `dd` 等
- 添加环境变量过滤（移除 API keys 等敏感信息）
- 命令黑名单匹配（支持正则模式）

---

## Task 4: 上下文窗口管理

**目标**: 实现 token 计数、上下文窗口监控和智能消息裁剪。

**涉及文件**:

| 文件 | 操作 |
|------|------|
| `crates/pi-ai/src/token_counter.rs` | 新增 |
| `crates/pi-ai/src/lib.rs` | 修改 |
| `crates/pi-agent/src/context_manager.rs` | 新增 |
| `crates/pi-agent/src/lib.rs` | 修改 |
| `crates/pi-agent/src/agent_loop.rs` | 修改 |
| `crates/pi-coding-agent/src/core/agent_session.rs` | 修改 |

**子任务**:

### Task 4.1: 实现 Token 计数模块

新增 `pi-ai/src/token_counter.rs`：

```rust
pub trait TokenCounter: Send + Sync {
    fn count_text(&self, text: &str) -> usize;
    fn count_message(&self, message: &Message) -> usize;
    fn count_messages(&self, messages: &[Message]) -> usize;
}

pub struct EstimateTokenCounter;  // 启发式：字符数 / 4
pub struct ModelTokenCounter { model_family: String }  // 模型特定规则
```

采用分层策略：
- 默认使用启发式估算（字符数 / 4，对英文准确度约 80%）
- 为主流模型提供微调系数
- 预留精确计数器接口（未来可接入 tiktoken-rs）

### Task 4.2: 实现上下文窗口管理器

新增 `pi-agent/src/context_manager.rs`：

```rust
pub struct ContextWindowManager {
    token_counter: Arc<dyn TokenCounter>,
    context_window_size: usize,    // 模型的上下文窗口大小
    reserve_for_output: usize,     // 为输出预留的 token 数
    warning_threshold: f64,        // 警告阈值（如 0.8 = 80%）
}
```

核心功能：
- `estimate_usage(messages: &[AgentMessage]) -> ContextUsage` - 估算当前使用量
- `should_warn(usage: &ContextUsage) -> bool` - 是否需要警告
- `trim_messages(messages: &mut Vec<AgentMessage>, target_tokens: usize)` - 智能裁剪
  - 保留系统消息和最近 N 轮对话
  - 优先移除旧的工具调用结果（通常最占空间）
  - 保留文件操作历史的摘要

### Task 4.3: 在 Agent Loop 中集成上下文管理

修改 `agent_loop.rs`：

- 在 `run_agent_loop()` 调用 LLM 前，检查上下文窗口使用情况
- 如超出阈值，自动执行消息裁剪
- 发出 `AgentEvent::ContextWarning` 事件通知上层
- 记录每轮的 token 使用统计

### Task 4.4: 在 AgentSession 中暴露上下文统计

修改 `agent_session.rs`：

- 创建 `ContextWindowManager` 实例并传入 agent loop
- 在状态栏中显示上下文使用百分比
- 提供 `/context` 命令查看详细上下文统计

---

## 执行顺序和依赖关系

```
阶段 A（并行启动）:
  ├── Task 1 (Mistral Provider) - 独立模块，pi-ai crate
  ├── Task 2 (编辑器集成) - 独立模块，pi-coding-agent crate
  └── Task 3 (工具权限管理) - 独立模块，pi-coding-agent crate

阶段 B（Task 3 完成后）:
  └── Task 4 (上下文窗口管理) - 跨 pi-ai、pi-agent、pi-coding-agent

阶段 C（全部完成后）:
  └── 编译验证 + 集成测试
```

---

## 新增依赖

无需新增外部依赖。Token 计数采用内置启发式估算，避免引入 tiktoken-rs 等重量级依赖。所有网络请求使用已有的 `reqwest` + `reqwest-eventsource`。
