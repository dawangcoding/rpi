# ITERATION-3 Phase 6 质量提升实施计划

## 现状摘要

- SettingsList（797行）和 CancellableLoader（294行）组件**已完成**，含 24 个单元测试
- `cargo check` 零错误零警告
- `cargo clippy --all-targets` 有 **1 个 deny 错误 + 48 个 warning**
- `cargo doc --no-deps` 零警告，但 rustdoc 有 2 个 `doc_nested_refdefs` 格式问题
- 单元测试 **665 个**，100% 通过
- pi-agent 和 pi-tui 缺乏集成测试

## Task 1: Clippy 警告清理（48 警告 + 1 deny）

**目标：** `cargo clippy --all-targets` 零警告零错误

### 1.1 修复 deny 级错误（阻塞项）
- `pi-tui/src/components/settings_list.rs:744` — `approx_constant`：将 `3.14` 替换为 `3.15` 或其他非近似 PI 的值

### 1.2 unused_variables 修复（~10处）
- `pi-ai/src/providers/anthropic.rs:1681` — `provider` 未使用
- `pi-agent/src/agent.rs:572,586,598,611` — `agent` 未使用（4处）
- `pi-agent/src/agent_loop.rs:962` — `tools` 未使用
- `pi-ai/src/providers/openai.rs:1225,1253` — `mock` 未使用
- `pi-ai/src/providers/azure_openai.rs:1041` — `mock` 未使用
- `pi-ai/src/providers/xai.rs:967` — `mock` 未使用
- `pi-ai/src/providers/openrouter.rs:1054` — `mock` 未使用
- 修复方式：加 `_` 前缀或实际使用

### 1.3 代码风格修复
- `pi-tui/src/components/editor.rs:1655`、`markdown.rs:674` — `len_comparison`：`assert!(x.len() >= 1)` 改为 `assert!(!x.is_empty())`
- `pi-tui/src/components/settings_list.rs:140,201` — `type_complexity`：提取 type alias
- `pi-ai/src/test_fixtures.rs:128` — `unused_enumerate_index`：改用不带 enumerate 的写法
- `pi-coding-agent/src/modes/interactive.rs:1223` — `items_after_test_module`：移动 test module 到文件末尾

### 1.4 修复其余 pi-coding-agent 测试相关警告（~13处）
- unused_imports、dead_code、field_reassign、unnecessary_literal_unwrap 等

### 1.5 rustdoc 格式修复
- `pi-tui/src/lib.rs:14,15` — 修复 `doc_nested_refdefs`（Container/OverlayHandle 链接格式）

**验证：** `cargo clippy --all-targets` 零警告 + `cargo test --lib` 全部通过

## Task 2: 集成测试补充

**目标：** 为 pi-agent 和 pi-tui 建立集成测试框架

### 2.1 pi-agent 集成测试
- 新建 `crates/pi-agent/tests/agent_integration_tests.rs`
- 覆盖场景：
  - Agent 完整对话循环（mock LLM -> agent loop -> tool call -> result）
  - 工具调用超时和错误处理
  - 多轮对话状态管理
  - 事件订阅和通知

### 2.2 pi-tui 集成测试
- 新建 `crates/pi-tui/tests/component_integration_tests.rs`
- 覆盖场景：
  - 多组件组合渲染（Editor + Markdown）
  - 焦点切换和键盘事件传播
  - 终端大小变化处理
  - Overlay/覆盖层交互

### 2.3 现有集成测试增强
- `pi-ai/tests/provider_integration_tests.rs` — 补充边界测试（错误响应解析、空内容处理）
- `pi-coding-agent/tests/tool_integration_tests.rs` — 补充工具组合边界场景

**验证：** `cargo test` 全部通过，新增测试 15-20 个

## Task 3: 公共 API 文档补充

**目标：** 关键公共 API 有完整 `///` 文档和使用示例

### 3.1 pi-agent 文档
- `agent.rs` — Agent、AgentOptions、AgentTool 等核心类型的方法文档
- `agent_loop.rs` — AgentLoop 公共方法文档
- `types.rs` — Message 类型和 Content 类型文档

### 3.2 pi-ai 文档
- `types.rs` — Api/Provider 枚举、StreamEvent 文档
- `stream.rs` — 流式 API 使用示例
- `models.rs` — 模型注册和查询 API 文档

### 3.3 pi-tui 文档
- `tui.rs` — Tui/Component/Focusable trait 文档和示例
- 核心组件（Editor/Markdown/SettingsList）文档

### 3.4 pi-coding-agent 文档
- `agent_session.rs` — AgentSession 公共 API
- `config.rs` — 配置项说明

**验证：** `cargo doc --no-deps` 零警告，关键类型有文档和示例

## 执行顺序

1. **Task 1**（Clippy 清理）— 优先级最高，解除 deny 阻塞，清理全部警告
2. **Task 2**（集成测试）— 在代码稳定后补充
3. **Task 3**（API 文档）— 最后执行，确保文档对应最终代码

Task 1 和 Task 2/3 之间有依赖（先稳定代码再写测试和文档），Task 2 和 Task 3 可并行执行。

## 预估工作量

| Task | 预估时间 |
|------|---------|
| Task 1: Clippy 清理 | 30-45 分钟 |
| Task 2: 集成测试 | 45-60 分钟 |
| Task 3: API 文档 | 30-45 分钟 |
| 总计 | ~2 小时 |
