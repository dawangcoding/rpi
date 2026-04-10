# 二次迭代 - 第四阶段开发计划：扩展系统、OAuth、测试与性能

## 概述

第四阶段是 ITERATION-2 的最终批次，包含 4 个功能模块：
1. **扩展系统基础框架** - 定义 Extension trait、加载器、运行器、API 上下文
2. **OAuth 认证** - 本地回调服务器、Token 存储/刷新、多提供商支持
3. **测试覆盖** - 修复历史失败测试、Mock Server、各层单元/集成测试
4. **性能优化** - 大文件增量读取、流式渲染批量更新、虚拟滚动

---

## Task 0: 修复历史遗留测试失败（已完成）

**目标**: 修复 3 个已知失败测试，建立干净的测试基线

**涉及文件**:
- `crates/pi-ai/src/providers/mistral.rs` - 修复 `test_truncate_tool_call_id`
- `crates/pi-ai/src/utils/json_parse.rs` - 修复 `test_streaming_parser` 和 `test_trailing_comma`

---

## Task 1: 测试基础设施搭建（已完成）

**目标**: 添加测试工具依赖，建立 Mock Server 和测试 fixtures 框架

**涉及文件**:
- 改造: 所有 Cargo.toml（添加 mockito、tokio-test、tempfile、assert_cmd、predicates）
- 新增: `crates/pi-ai/src/test_fixtures.rs` - Provider 测试 fixtures
- 新增: `crates/pi-agent/src/test_fixtures.rs` - Agent 测试 fixtures

---

## Task 2: Provider 单元测试完善（已完成）

**目标**: 使用 Mock Server 为 5 个 Provider 编写完整的单元测试

**涉及文件**:
- 扩展: `crates/pi-ai/src/providers/anthropic.rs` - 流式响应解析测试
- 扩展: `crates/pi-ai/src/providers/openai.rs` - 文本/工具/推理响应测试
- 扩展: `crates/pi-ai/src/providers/google.rs` - JSON 行流式测试
- 扩展: `crates/pi-ai/src/providers/mistral.rs` - ID 截断和流式测试
- 扩展: `crates/pi-ai/src/providers/bedrock.rs` - 请求构建测试

---

## Task 3: Agent Loop 和工具测试（已完成）

**目标**: 为 Agent 核心循环和 7 个内置工具编写测试

**涉及文件**:
- 扩展: `crates/pi-agent/src/agent.rs` - Agent 创建和状态测试
- 扩展: `crates/pi-agent/src/agent_loop.rs` - 工具转换和上下文测试
- 扩展: `crates/pi-coding-agent/src/core/tools/*.rs` - 7 个工具的完整测试

---

## Task 4: 会话系统和 TUI 组件测试（已完成）

**目标**: 为会话管理、压缩、导出和 TUI 组件编写测试

**涉及文件**:
- 扩展: `crates/pi-coding-agent/src/core/session_manager.rs` - 会话 CRUD 和 Fork 测试
- 扩展: `crates/pi-coding-agent/src/core/compaction/compactor.rs` - 阈值和范围测试
- 扩展: `crates/pi-coding-agent/src/core/export/html.rs` - 导出和主题测试
- 扩展: `crates/pi-tui/src/components/markdown.rs` - Markdown 渲染测试
- 扩展: `crates/pi-tui/src/components/editor.rs` - 编辑器操作测试

---

## Task 5: 扩展系统基础框架（已完成）

**目标**: 实现扩展系统的核心抽象，支持工具注册、命令注册和生命周期管理

**涉及文件**:
- 新增: `crates/pi-coding-agent/src/core/extensions/mod.rs`
- 新增: `crates/pi-coding-agent/src/core/extensions/types.rs` - Extension trait、SlashCommand
- 新增: `crates/pi-coding-agent/src/core/extensions/loader.rs` - 扩展加载器
- 新增: `crates/pi-coding-agent/src/core/extensions/runner.rs` - ExtensionManager
- 新增: `crates/pi-coding-agent/src/core/extensions/api.rs` - ExtensionContext
- 改造: `crates/pi-coding-agent/src/core/agent_session.rs` - 集成扩展系统
- 改造: `crates/pi-coding-agent/src/modes/interactive.rs` - 支持 /extensions 命令

---

## Task 6: OAuth 认证系统（已完成）

**目标**: 实现完整的 OAuth 授权流程

**涉及文件**:
- 新增: `crates/pi-coding-agent/src/core/auth/mod.rs`
- 新增: `crates/pi-coding-agent/src/core/auth/providers.rs` - Anthropic/GitHub Copilot 配置
- 新增: `crates/pi-coding-agent/src/core/auth/oauth_server.rs` - 本地回调服务器
- 新增: `crates/pi-coding-agent/src/core/auth/token_storage.rs` - Token 存储/刷新
- 改造: `crates/pi-coding-agent/src/config.rs` - 优先使用 OAuth token
- 改造: `crates/pi-coding-agent/src/modes/interactive.rs` - /login /logout /auth 命令

---

## Task 7: 性能优化（已完成）

**目标**: 优化大文件处理、流式渲染和内存使用

**涉及文件**:
- 改造: `crates/pi-coding-agent/src/core/tools/truncate.rs` - 流式截断
- 改造: `crates/pi-coding-agent/src/core/tools/read.rs` - BufReader 分块读取
- 改造: `crates/pi-tui/src/tui.rs` - 批量更新、VirtualViewport
- 改造: `crates/pi-tui/src/components/markdown.rs` - 渲染缓存
- 改造: `crates/pi-coding-agent/src/modes/interactive.rs` - 渲染节流(16ms)
- 改造: `crates/pi-coding-agent/src/modes/interactive_components.rs` - 增量追加

---

## Task 8: E2E 测试和清理（已完成）

**目标**: CLI 端到端测试，清理编译警告

**涉及文件**:
- 新增: `crates/pi-coding-agent/tests/cli_tests.rs`
- 改造: 各文件清理 dead_code 警告

---

## 依赖关系

```
Task 0 (修复历史测试) ──┐
                         ├──→ Task 1 (测试基础设施) ──→ Task 2/3/4 (测试)
                         ├──→ Task 5 (扩展系统)
                         ├──→ Task 6 (OAuth 认证)
                         ├──→ Task 7 (性能优化)
                         └──→ Task 8 (E2E + 清理)
```

---

## 新增依赖

**workspace dependencies**:
- `hyper` (1.x) - OAuth 回调服务器
- `hyper-util` (0.1) - Hyper 工具
- `http-body-util` (0.1) - HTTP Body 处理

**dev-dependencies**:
- `mockito` (1.x) - HTTP Mock Server
- `tempfile` (3.x) - 临时文件/目录
- `assert_cmd` (2.x) - CLI 测试
- `predicates` (3.x) - 断言辅助
- `tokio-test` (0.4) - Tokio 测试工具

---

## 验证结果

| 检查项 | 结果 |
|--------|------|
| `cargo check` | 通过 |
| `cargo test` | 全部通过 |
| `cargo clippy` | 通过 |
