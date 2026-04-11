# pi-mono Rust 移植计划

## 项目结构

```
/Users/lzmcoding/Code/rpi/
├── Cargo.toml                    # workspace 根配置
├── crates/
│   ├── pi-ai/                    # LLM 统一 API 层
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs          # Message, Content, Tool, Model 等核心类型
│   │       ├── stream.rs         # stream/complete 统一入口
│   │       ├── api_registry.rs   # Provider 注册系统
│   │       ├── models.rs         # 模型注册表
│   │       ├── providers/
│   │       │   ├── mod.rs
│   │       │   ├── anthropic.rs  # Claude Messages API
│   │       │   ├── openai.rs     # OpenAI Chat Completions
│   │       │   └── google.rs     # Google Generative AI
│   │       └── utils/
│   │           ├── mod.rs
│   │           ├── event_stream.rs  # SSE 解析
│   │           └── json_parse.rs    # 增量 JSON 解析
│   ├── pi-tui/                   # 终端 UI 框架 (crossterm 自定义)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tui.rs            # 核心差分渲染引擎
│   │       ├── terminal.rs       # 终端抽象
│   │       ├── keys.rs           # 键盘输入解析 (Kitty protocol)
│   │       ├── keybindings.rs    # 快捷键管理
│   │       ├── autocomplete.rs   # 自动完成
│   │       ├── fuzzy.rs          # 模糊匹配
│   │       ├── kill_ring.rs      # Emacs 风格剪贴板
│   │       ├── undo_stack.rs     # 撤销/重做
│   │       ├── utils.rs          # 宽字符、截断、换行
│   │       ├── terminal_image.rs # Kitty/iTerm2 图像协议
│   │       └── components/
│   │           ├── mod.rs
│   │           ├── editor.rs     # 多行文本编辑器
│   │           ├── input.rs      # 单行输入
│   │           ├── markdown.rs   # Markdown 渲染
│   │           ├── select_list.rs
│   │           ├── box_component.rs
│   │           ├── text.rs
│   │           ├── loader.rs
│   │           ├── image.rs
│   │           └── spacer.rs
│   ├── pi-agent/                 # Agent 运行时核心
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs          # AgentTool, AgentEvent, AgentState 等
│   │       ├── agent.rs          # Agent 结构体 (prompt, steer, followUp)
│   │       └── agent_loop.rs     # 核心循环 (stream -> tool -> loop)
│   └── pi-coding-agent/          # 编码 agent CLI
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # CLI 入口
│           ├── lib.rs            # 库导出
│           ├── config.rs         # 配置管理
│           ├── cli/
│           │   ├── mod.rs
│           │   ├── args.rs       # clap 参数解析
│           │   └── initial_message.rs
│           ├── core/
│           │   ├── mod.rs
│           │   ├── agent_session.rs   # 会话管理核心
│           │   ├── system_prompt.rs   # 系统提示词构建
│           │   ├── session_manager.rs # 会话持久化
│           │   └── tools/
│           │       ├── mod.rs
│           │       ├── bash.rs        # shell 命令执行
│           │       ├── read.rs        # 文件读取
│           │       ├── edit.rs        # 文件编辑
│           │       ├── write.rs       # 文件写入
│           │       ├── grep.rs        # 全文搜索
│           │       ├── find.rs        # 文件查找
│           │       ├── ls.rs          # 目录列表
│           │       └── truncate.rs    # 输出截断
│           └── modes/
│               ├── mod.rs
│               ├── interactive.rs     # TUI 交互模式
│               └── print_mode.rs      # 非交互打印模式
```

## 核心 Rust 依赖

| 功能 | Crate | 说明 |
|------|-------|------|
| 异步运行时 | `tokio` | 全功能异步 (rt-multi-thread, macros, fs, process, signal) |
| HTTP 客户端 | `reqwest` + `reqwest-eventsource` | SSE 流式请求 |
| JSON | `serde` + `serde_json` | 序列化/反序列化 |
| JSON Schema | `schemars` | 工具参数 schema 生成 |
| 终端控制 | `crossterm` | 底层终端操作 |
| CLI 参数 | `clap` | 命令行参数解析 |
| 宽字符 | `unicode-width` | 东亚字符宽度 |
| Markdown | `pulldown-cmark` | Markdown 解析 |
| ANSI 样式 | `anstyle` / `owo-colors` | 终端颜色 |
| Glob | `globset` / `ignore` | 文件匹配 (含 .gitignore 支持) |
| Diff | `similar` | 统一 diff 生成 |
| Base64 | `base64` | 图像编码 |
| 正则 | `regex` | grep 工具 |
| 子进程 | `tokio::process` | bash 工具 |
| 配置 | `directories` + `serde_yaml` | 配置路径和文件 |

## 任务分解

### Task 1: 项目骨架搭建

创建 Cargo workspace 和所有 4 个 crate 的基础结构，配置依赖。

**产出**: 可编译的空 workspace，各 crate 间依赖关系正确。

---

### Task 2: pi-ai -- 核心类型系统

移植 `packages/ai/src/types.ts` (403 行) 的完整类型定义到 Rust：

- `Message` 枚举 (UserMessage, AssistantMessage, ToolResultMessage)
- `ContentBlock` 枚举 (Text, Thinking, Image, ToolCall)
- `Tool` 结构体 (name, description, parameters as serde_json::Value)
- `Model` 结构体 (id, api, provider, cost, context_window 等)
- `StreamOptions` 结构体
- `AssistantMessageEvent` 枚举 (13 种事件类型)
- `AssistantMessageEventStream` trait (异步迭代器)
- `StopReason`, `ThinkingLevel`, `Api`, `Provider` 等枚举

**关键映射**:
- TS `interface` -> Rust `struct` (derive Serialize, Deserialize, Clone, Debug)
- TS union type -> Rust `enum` (tagged with serde)
- TS `Record<string, any>` -> `serde_json::Value` 或 `HashMap<String, Value>`
- TS `AsyncIterable` -> Rust `Stream<Item=Result<Event>>` (futures::Stream)
- TypeBox TSchema -> `serde_json::Value` (JSON Schema 格式)

---

### Task 3: pi-ai -- Provider 注册和流式 API

移植 Provider 注册系统和统一流式接口：

- `ApiRegistry`: Provider 注册表 (`HashMap<Api, Box<dyn ApiProvider>>`)
- `ApiProvider` trait: 定义 `stream()` 方法签名
- `stream()` / `complete()` 统一入口函数
- 模型注册表 (`models.rs`): 内置模型定义 (Anthropic/OpenAI/Google 主要模型)
- SSE 事件流解析工具 (`event_stream.rs`)
- 增量 JSON 解析 (`json_parse.rs`)

**ApiProvider trait 设计**:
```rust
#[async_trait]
pub trait ApiProvider: Send + Sync {
    fn api(&self) -> Api;
    async fn stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<AssistantMessageEvent>> + Send>>>;
}
```

---

### Task 4: pi-ai -- Anthropic Provider

移植 `providers/anthropic.ts` (27.2 KB)：

- Anthropic Messages API 实现 (`/v1/messages`, streaming)
- 请求构建: messages 格式转换 (pi Message -> Anthropic format)
- SSE 流解析: `message_start`, `content_block_start`, `content_block_delta`, `message_delta`, `message_stop`
- 缓存控制: `cache_control` 头和 `cacheRetention` 选项
- Thinking 支持: `thinking` content block 处理
- Tool use 支持: `tool_use` content block -> ToolCall 事件
- 错误处理: rate limit, overloaded, 网络错误重试

---

### Task 5: pi-ai -- OpenAI Provider

移植 `providers/openai-completions.ts` (29.7 KB) 和相关文件：

- OpenAI Chat Completions API (`/v1/chat/completions`, streaming)
- 请求构建: pi Message -> OpenAI message format
- SSE 流解析: `chat.completion.chunk` 事件
- Tool calling: `function` 格式和新版 `tool` 格式
- Streaming function arguments 增量解析
- 兼容 OpenAI API 格式的其他服务 (通过 `base_url` 配置)

---

### Task 6: pi-ai -- Google Provider

移植 `providers/google.ts` (14.4 KB) 和 `google-shared.ts` (11.8 KB)：

- Google Generative AI API (`/v1beta/models/{model}:streamGenerateContent`)
- 请求构建: pi Message -> Google Content format
- 流解析: JSON 行格式 (非 SSE)
- Tool 调用: `functionCall` part 处理
- Safety settings, generation config

---

### Task 7: pi-tui -- 核心引擎和终端抽象

移植 TUI 核心：

- **Terminal trait**: 终端抽象 (write, flush, size, raw mode, alternate screen)
- **ProcessTerminal**: 基于 crossterm 的实现
- **Component trait**: `render(width) -> Vec<String>`, `handle_input(data)`, `invalidate()`
- **Focusable trait**
- **Container**: 子组件容器
- **TUI struct**: 差分渲染引擎
  - 逐行 diff (新旧 buffer 比较)
  - 光标定位 (CURSOR_MARKER)
  - 覆盖层系统 (OverlayHandle)
  - 焦点管理
- **StdinBuffer**: 异步 stdin 读取 (tokio)

**差分渲染核心逻辑** (约 400 行):
- 前后 frame buffer 对比
- 仅输出变更行 (cursor move + clear line + write)
- 覆盖层合成 (z-order)

---

### Task 8: pi-tui -- 键盘输入和快捷键

移植键盘处理系统：

- **keys.rs**: 按键解析
  - ANSI CSI 序列解析
  - Kitty keyboard protocol 支持
  - 修饰符 (Shift/Ctrl/Alt/Meta) 追踪
  - Key 结构体: id, modifiers, event_type
- **keybindings.rs**: 快捷键管理
  - KeybindingsManager
  - 快捷键定义和冲突检测
  - 默认快捷键映射 (TUI_KEYBINDINGS)

---

### Task 9: pi-tui -- 编辑器和输入组件

移植核心 UI 组件：

- **editor.rs** (对标 2231 行的 editor.ts): 多行文本编辑器
  - 光标移动 (上/下/左/右/Home/End)
  - 文本输入/删除/选择
  - 撤销/重做 (UndoStack)
  - Kill ring (Emacs 风格剪贴板)
  - 自动完成集成
  - 粘贴标记处理
  - 自动换行
- **input.rs**: 单行输入 (编辑器简化版)
- **autocomplete.rs**: 自动完成系统
- **fuzzy.rs**: 模糊匹配算法

---

### Task 10: pi-tui -- Markdown 渲染和其他组件

移植剩余 UI 组件：

- **markdown.rs**: Markdown -> ANSI 渲染
  - 使用 pulldown-cmark 解析
  - 代码块语法高亮 (syntect)
  - 列表、标题、链接、引用等样式
- **select_list.rs**: 列表选择器 (带模糊搜索)
- **box_component.rs**: 边框容器
- **text.rs**: 纯文本/截断文本
- **loader.rs**: 加载动画
- **image.rs**: Kitty/iTerm2 图像协议
- **terminal_image.rs**: 图像编码和终端能力检测
- **utils.rs**: `visible_width()`, `truncate_to_width()`, `wrap_text()`

---

### Task 11: pi-agent -- 类型系统和 Agent 循环

移植 Agent 核心：

- **types.rs**: 
  - `AgentTool` trait (name, description, parameters, execute)
  - `AgentToolResult` 结构体
  - `AgentEvent` 枚举 (12 种事件)
  - `AgentLoopConfig` 结构体
  - `AgentContext`, `AgentState`
  - `ToolExecutionMode` 枚举 (Sequential/Parallel)
  - `BeforeToolCallContext`, `AfterToolCallContext`
- **agent_loop.rs**: Agent 循环核心 (对标 632 行)
  - `run_agent_loop()`: 从新 prompt 开始
  - `run_agent_loop_continue()`: 从已有消息继续
  - 工具执行: 串行/并行模式
  - beforeToolCall/afterToolCall 钩子
  - steering/followUp 消息注入
  - AbortSignal -> `tokio::sync::watch` 或 `CancellationToken`
- **agent.rs**: Agent 结构体 (对标 540 行)
  - `subscribe()`: 事件监听
  - `prompt()`: 发送消息并运行循环
  - `steer()` / `follow_up()`: 消息队列
  - `abort()` / `reset()` / `wait_for_idle()`

---

### Task 12: pi-coding-agent -- CLI 和配置

移植 CLI 入口和配置：

- **args.rs**: clap 参数定义
  - model, thinking, system-prompt, session, mode, file, context-file 等
- **config.rs**: 配置管理
  - 配置文件路径 (~/.pi/)
  - API key 读取 (环境变量 + 配置文件)
  - 模型默认设置
- **main.rs**: CLI 入口逻辑
  - 参数解析
  - 模型选择
  - 会话创建/恢复
  - 模式路由 (interactive/print)

---

### Task 13: pi-coding-agent -- 内置工具集

移植 7 个核心工具：

- **bash.rs**: shell 命令执行
  - `tokio::process::Command`
  - stdout/stderr 流式捕获
  - 超时 (SIGKILL)
  - 输出截断
- **read.rs**: 文件读取 (行数/字节限制)
- **write.rs**: 文件创建/覆盖 (原子写入)
- **edit.rs**: 行范围替换/插入
  - 统一 diff 生成 (`similar` crate)
  - 自动创建父目录
- **grep.rs**: 全文搜索
  - `grep` crate 或自实现
  - .gitignore 支持 (`ignore` crate)
- **find.rs**: 文件查找 (`ignore` crate walkdir)
- **ls.rs**: 目录列表
- **truncate.rs**: 输出截断工具函数

---

### Task 14: pi-coding-agent -- 会话和系统提示词

移植会话管理和提示词构建：

- **system_prompt.rs**: 系统提示词生成
  - 基础提示词模板
  - 工具描述注入
  - 自定义指南
  - 上下文文件注入
  - 当前日期和工作目录
- **agent_session.rs**: 会话管理核心 (精简版)
  - 创建 Agent 实例
  - 注册工具
  - 事件处理和分发
  - 会话统计 (tokens, cost)
- **session_manager.rs**: 会话持久化
  - JSON 格式保存/加载
  - 会话列表

---

### Task 15: pi-coding-agent -- 交互模式

移植 TUI 交互模式：

- **interactive.rs**: 主交互循环
  - TUI 初始化
  - 编辑器组件 (用户输入)
  - Markdown 渲染 (助手回复)
  - 工具调用显示
  - 加载状态
  - 快捷键处理 (Ctrl+C 中断, Ctrl+D 退出等)
- **print_mode.rs**: 非交互模式
  - 流式文本输出
  - 工具调用日志

---

### Task 16: 集成测试和验证

- 端到端测试: CLI 启动 -> 用户输入 -> LLM 调用 -> 工具执行 -> 结果显示
- Provider 测试: 各 Provider 的流式响应解析 (使用 mock server)
- TUI 测试: 组件渲染输出正确性
- 工具测试: 各工具的边界情况

## 执行顺序和依赖关系

```
Task 1 (骨架)
    |
    v
Task 2 (ai 类型) ---> Task 3 (Provider 注册)
    |                       |
    |                  +----+----+----+
    |                  |    |    |    |
    |               Task4 Task5 Task6 (三个 Provider 可并行)
    |                  |    |    |    |
    |                  +----+----+----+
    |                       |
    v                       v
Task 7 (tui 核心) ---> Task 11 (agent 核心)
    |                       |
    +---+---+               v
    |   |   |          Task 12 (CLI+配置)
Task8 Task9 Task10         |
    |   |   |          Task 13 (工具集)
    +---+---+               |
        |              Task 14 (会话+提示词)
        v                   |
   Task 15 (交互模式) <-----+
        |
        v
   Task 16 (集成测试)
```

## 简化策略 (首版不实现)

以下功能在首版中暂不移植，后续按需添加:

1. **扩展系统** (extensions/): 20+ 事件钩子，复杂度高，首版不需要
2. **OAuth 认证**: 首版使用 API Key 即可
3. **会话压缩** (compaction/): 首版不做上下文压缩
4. **HTML 导出** (export-html/): 非核心功能
5. **RPC 模式** (rpc-mode.ts): 首版只需 interactive + print
6. **技能系统** (skills.ts): 首版不支持
7. **数据迁移** (migrations.ts): 新项目无需迁移
8. **settings-list / settings 界面**: 首版通过配置文件和 CLI 参数管理
9. **proxy 工具** (agent/proxy.ts): 非核心
10. **自动生成模型列表** (models.generated.ts 353KB): 首版手动定义主要模型
