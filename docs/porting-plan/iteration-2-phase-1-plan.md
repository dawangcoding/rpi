# ITERATION-2 第一阶段开发计划：核心管线贯通

> 状态：**已完成**

## Context

首次复刻完成了 pi-mono 项目的四个 crate 骨架（pi-ai、pi-agent、pi-tui、pi-coding-agent），但三个核心子系统之间是断开的：

1. **Provider 已实现但从未注册** — Anthropic/OpenAI/Google 三个 Provider 代码完整（共 ~2300 行），但全局 `ApiRegistry` 为空，调用 `pi_ai::stream()` 会报 "No API provider registered"
2. **Agent Loop 的 LLM 调用是 stub** — `stream_assistant_response()` 直接返回空的 `AssistantMessage`，不与任何 API 通信
3. **交互模式使用 raw print** — 有完整的差分渲染引擎但未接入，用 `print!`/`println!` 直接输出

第一阶段目标：**将这三层贯通为一条端到端的工作管线**，使用户可以在终端中与 LLM 进行真实对话。

用户有 OpenRouter API key（OpenAI 兼容格式），优先验证 OpenAI Provider 路径。

---

## 实施顺序与依赖关系

```
Task 1: Provider 注册   ──► Task 2: Agent Loop 流式接入  ──► Task 3: TUI 中度集成
(打通 pi-ai 层)            (打通 pi-agent 层)                (打通显示层)
```

- Task 1 → Task 2 是强依赖（没有 Provider 注册就无法 stream）
- Task 2 完成后，print mode 就能端到端工作，是关键里程碑
- Task 3 依赖 Task 2 才能有真实流式数据来验证 TUI 渲染

---

## Task 1: Provider 注册初始化

### 要解决的问题
Provider 代码已完成，但全局 `ApiRegistry` 从未被填充。

### 修改文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/pi-ai/src/providers/mod.rs` | 修改 | 添加 Provider struct 的 re-export |
| `crates/pi-ai/src/lib.rs` | 修改 | 添加 `init_providers()` 公开函数 |
| `crates/pi-ai/src/models.rs` | 修改 | 添加 OpenRouter 兼容模型条目 |
| `crates/pi-coding-agent/src/main.rs` | 修改 | 在启动时调用 `pi_ai::init_providers()` |

### 实现细节

#### 1.1 `crates/pi-ai/src/providers/mod.rs`
添加 re-export：
```rust
pub use anthropic::AnthropicProvider;
pub use openai::OpenAiProvider;
pub use google::GoogleProvider;
```

#### 1.2 `crates/pi-ai/src/lib.rs`
添加公开初始化函数：
```rust
pub fn init_providers() {
    use std::sync::Arc;
    // 避免重复注册
    if has_api_provider(&Api::Anthropic) {
        return;
    }
    register_api_provider(Arc::new(providers::AnthropicProvider::new()));
    register_api_provider(Arc::new(providers::OpenAiProvider::new()));
    register_api_provider(Arc::new(providers::GoogleProvider::new()));
    tracing::debug!("Registered 3 built-in providers: Anthropic, OpenAI(ChatCompletions), Google");
}
```

注意：各 Provider 的 `new()` 只创建 `reqwest::Client`，不做网络调用，离线也不会失败。

Provider `api()` 返回值与 Model 注册表的 `Api` 变体映射关系：
- `AnthropicProvider::api()` → `Api::Anthropic`
- `OpenAiProvider::api()` → `Api::OpenAiChatCompletions`
- `GoogleProvider::api()` → `Api::Google`

#### 1.3 `crates/pi-ai/src/models.rs`
添加 3 个 OpenRouter 兼容模型：

| 模型 ID | 说明 | API |
|---------|------|-----|
| `openrouter/auto` | OpenRouter 通用自动路由 | `Api::OpenAiChatCompletions` |
| `openrouter/anthropic/claude-sonnet-4-20250514` | Claude Sonnet 4 via OpenRouter | `Api::OpenAiChatCompletions` |
| `openrouter/google/gemini-2.5-flash-preview-04-17` | Gemini Flash via OpenRouter | `Api::OpenAiChatCompletions` |

所有 OpenRouter 模型使用 `Provider::Openrouter`，`base_url: "https://openrouter.ai/api/v1"`，对应环境变量 `OPENROUTER_API_KEY`。

#### 1.4 `crates/pi-coding-agent/src/main.rs`
在 `tracing_subscriber` 初始化之后、`CliArgs::parse()` 之前调用：
```rust
pi_ai::init_providers();
```

### 验证方式
```bash
cargo check              # 编译通过
cargo run -- --list-models  # 列出模型（含 OpenRouter）
```

---

## Task 2: Agent Loop 流式响应接入

### 要解决的问题
`crates/pi-agent/src/agent_loop.rs` 中的 `stream_assistant_response()` 是 stub，返回空的 `AssistantMessage`。需要接入 `pi_ai::stream()` 实现真实的 LLM 流式调用。

### 修改文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/pi-agent/src/types.rs` | 修改 | 添加 `agent_tool_to_llm_tool()` 辅助函数 |
| `crates/pi-agent/src/agent_loop.rs` | 修改 | 重写 `stream_assistant_response()`，添加 `StreamExt` import |

### 实现细节

#### 2.1 `crates/pi-agent/src/types.rs`
添加工具格式转换函数：
```rust
pub fn agent_tool_to_llm_tool(tool: &dyn AgentTool) -> pi_ai::types::Tool {
    pi_ai::types::Tool {
        name: tool.name().to_string(),
        description: tool.description().to_string(),
        parameters: tool.parameters(),
    }
}
```

#### 2.2 `crates/pi-agent/src/agent_loop.rs` — 重写 `stream_assistant_response()`

核心流程（~140 行实现）：

**Step A: 构建 LLM Context**
- 将 `context.tools` 通过 `agent_tool_to_llm_tool()` 转换为 `Vec<Tool>`
- 构建 `pi_ai::types::Context { system_prompt, messages, tools }`

**Step B: 解析 API Key**
```rust
let api_key = config.get_api_key.as_ref()
    .and_then(|f| f(&format!("{:?}", config.model.provider).to_lowercase()))
    .or_else(|| {
        let env_var = pi_ai::get_api_key_env_var(&config.model.provider);
        std::env::var(env_var).ok()
    });
```

**Step C: 构建 StreamOptions 并调用 `pi_ai::stream()`**
```rust
let stream_options = StreamOptions {
    temperature: config.temperature.map(|t| t as f32),
    max_tokens: config.max_tokens.map(|t| t as u64),
    api_key,
    // ... 其他字段
};
let mut event_stream = pi_ai::stream(&llm_context, &config.model, &stream_options).await?;
```

**Step D: `tokio::select! { biased }` 消费流（带取消支持）**
```rust
loop {
    tokio::select! {
        biased;  // 优先检查取消
        _ = cancel.cancelled() => {
            // 构建 Aborted 消息
            break;
        }
        next = event_stream.next() => {
            match next {
                Some(Ok(event)) => {
                    // Start → emit MessageStart
                    // TextDelta/ThinkingDelta/ToolCallEnd → 更新 partial, emit MessageUpdate
                    // Done → 设置 final_message, break
                    // Error → 设置 error message, break
                }
                Some(Err(e)) => { /* 流错误处理 */ break; }
                None => { /* 流意外结束 */ break; }
            }
        }
    }
}
```

**Step E: 推送最终消息到 context，发出 MessageEnd**

### 关键设计决策
1. **`biased` 选择器**：确保取消检查优先于流消费，实现快速响应 Ctrl+C
2. **MessageStart 延迟发出**：在收到首个 `Start` 事件时发出，而非函数入口
3. **所有 `AssistantMessageEvent` 变体通过 `partial` 字段更新当前消息**
4. **类型转换**：temperature `f64→f32`，max_tokens `u32→u64`

### 验证方式
```bash
cargo check
# print mode 端到端测试
OPENROUTER_API_KEY=sk-or-... cargo run -- -m openrouter/auto -p "say hello" --mode print
```

---

## Task 3: TUI 中度集成到交互模式

### 要解决的问题
当前 `interactive.rs`（258 行）使用 `print!`/`println!` 直接输出，不利用 pi-tui 的组件系统。

### 集成范围（中度）
- **使用 Markdown 组件渲染助手回复**（pulldown-cmark 解析 + ANSI 样式）
- **流式差分渲染**（光标回退 + 行清除实现就地更新）
- **状态栏**（token 统计 + 费用）
- **Raw mode 下使用 `\r\n`** 替代 `println!` 的 `\n`
- **输入仍使用简化的 stdin 字符读取**（Editor 组件集成是第二阶段）

### 实现方案选择

原始计划使用 `Tui` 差分渲染引擎全量管理屏幕，但存在以下问题：
- Tui 引擎为固定高度视口设计，对滚动式聊天界面的支持不佳
- 整个对话历史放入 Tui buffer 会导致每次渲染重新解析所有 Markdown

**实际采用方案：混合式渲染**
- 使用 `Markdown` 组件的 `render(width)` 方法进行内容格式化
- 使用自定义 `StreamingBlock` 结构管理流式区域的就地差分更新
- 非流式内容（用户消息、工具状态、统计信息）仍使用 `write!` 直接输出

### 修改/创建文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/pi-coding-agent/src/modes/interactive_components.rs` | **新建** | 渲染辅助模块 |
| `crates/pi-coding-agent/src/modes/mod.rs` | 修改 | 添加模块声明 |
| `crates/pi-coding-agent/src/modes/interactive.rs` | 修改 | 重构为 TUI 组件驱动 |

### 实现细节

#### 3.1 `interactive_components.rs` — 渲染辅助模块

**`render_markdown_lines(content, width) -> Vec<String>`**
使用 `pi_tui::components::markdown::Markdown` 组件将文本渲染为 ANSI 格式终端行。

**`render_thinking_lines(content) -> Vec<String>`**
将思考内容渲染为 dim 样式（`\x1b[2m`）终端行。

**`StreamingBlock`** — 核心的流式差分渲染结构：

```rust
pub struct StreamingBlock {
    prev_line_count: usize,  // 上次渲染占用的终端行数
    text: String,            // 累积的文本内容
    thinking: String,        // 累积的思考内容
}
```

渲染流程（`diff_update(width) -> String`）：
1. 回退光标到流式区域起始行（`\x1b[NA`，N = prev_line_count - 1）
2. 使用 Markdown 组件重新渲染累积内容
3. 逐行清除旧内容（`\x1b[2K`）并输出新行
4. 清除新内容不足时的多余旧行
5. 更新 `prev_line_count`

包含 9 个单元测试覆盖各种场景。

#### 3.2 `interactive.rs` — 重构事件循环

主要变化：
- 所有输出使用 `write!(stdout, "...\r\n")` 替代 `println!`（raw mode 兼容）
- `TextDelta` / `ThinkingDelta` → `streaming.push_text/push_thinking` + `streaming.diff_update`
- `ToolCallEnd` → flush 流式块 + 输出工具信息 + 重置流式块
- `MessageEnd` → 最终渲染 + finish 流式块
- `AgentEnd` → 输出 token 统计状态栏
- 终端宽度通过 `ProcessTerminal::size()` 获取
- 使用 `write!(stdout, ...)` + `stdout.flush()` 统一输出

---

## 总文件修改清单

| 文件 | Task | 操作 |
|------|------|------|
| `crates/pi-ai/src/providers/mod.rs` | 1 | 修改（添加 re-export） |
| `crates/pi-ai/src/lib.rs` | 1 | 修改（添加 init_providers） |
| `crates/pi-ai/src/models.rs` | 1 | 修改（添加 3 个 OpenRouter 模型） |
| `crates/pi-coding-agent/src/main.rs` | 1 | 修改（调用 init_providers） |
| `crates/pi-agent/src/types.rs` | 2 | 修改（添加 agent_tool_to_llm_tool） |
| `crates/pi-agent/src/agent_loop.rs` | 2 | 修改（重写 stream_assistant_response，~140 行） |
| `crates/pi-coding-agent/src/modes/interactive_components.rs` | 3 | **新建**（StreamingBlock + 渲染辅助 + 9 个测试） |
| `crates/pi-coding-agent/src/modes/mod.rs` | 3 | 修改（添加模块声明） |
| `crates/pi-coding-agent/src/modes/interactive.rs` | 3 | 修改（重构为 TUI 组件驱动） |

## 验证检查清单

1. `cargo check` — 全 workspace 编译通过（零错误）
2. `cargo test -p pi-coding-agent` — 12 个测试全部通过
3. `cargo run -- --list-models` — 模型列表正常显示（含 OpenRouter）
4. `OPENROUTER_API_KEY=... cargo run -- -m openrouter/auto -p "hello" --mode print` — print mode 端到端工作
5. `OPENROUTER_API_KEY=... cargo run -- -m openrouter/auto` — 交互模式 Markdown 渲染正常
6. Ctrl+C — 取消当前流式输出
7. Ctrl+D — 退出程序，终端恢复正常
8. 多轮对话 — 消息正确累积显示
