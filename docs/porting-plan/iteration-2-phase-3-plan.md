# 二次迭代 - 第三阶段开发计划：会话增强与 Bedrock Provider

## 概述

第三阶段是 ITERATION-2 的第三批次实施，包含 4 个功能模块：
1. **会话 Fork** - 支持从任意消息位置创建会话分支
2. **Amazon Bedrock Provider** - 通过 AWS 调用 Claude 模型
3. **HTML 导出** - 将会话导出为自包含的 HTML 文件
4. **会话压缩（Compaction）** - 通过 LLM 生成摘要替代早期消息

---

## Task 1: 会话 Fork 功能（已完成）

**目标**: 支持从任意消息位置创建会话分支，独立管理分支历史

**涉及文件**:
- 改造: `crates/pi-coding-agent/src/core/session_manager.rs`
- 改造: `crates/pi-coding-agent/src/core/agent_session.rs`
- 改造: `crates/pi-coding-agent/src/modes/interactive.rs`

**实现内容**:
1. 扩展 `SessionMetadata` 添加 `parent_session_id` 和 `fork_at_index` 字段（serde(default) 向后兼容）
2. `SessionManager` 新增方法：`fork_session()`、`list_forks()`、`get_session_tree()`
3. `AgentSession` 新增 `fork()` 方法
4. 交互模式添加 `/fork` 和 `/fork N` slash 命令

---

## Task 2: Amazon Bedrock Provider（已完成）

**目标**: 实现 AWS Bedrock Provider，支持通过 AWS 调用 Claude 模型

**涉及文件**:
- 新增: `crates/pi-ai/src/providers/bedrock.rs`
- 改造: `crates/pi-ai/src/providers/mod.rs`
- 改造: `crates/pi-ai/src/models.rs`
- 改造: `crates/pi-ai/src/types.rs`
- 改造: `crates/pi-ai/src/lib.rs`
- 改造: `crates/pi-ai/Cargo.toml`

**实现内容**:
1. 添加 `aws-config` 和 `aws-sdk-bedrockruntime` 依赖
2. 扩展 `Api::AmazonBedrock` 和 `Provider::AmazonBedrock` 枚举
3. 实现 `BedrockProvider`：AWS SDK 认证、Anthropic 兼容消息格式、EventStream 流式响应
4. 注册 Bedrock 模型：Claude 3.5 Sonnet、Claude 3 Opus、Claude 3 Haiku、Claude 3.5 Haiku
5. 支持 AWS 环境变量：`AWS_PROFILE`、`AWS_REGION`、`AWS_ACCESS_KEY_ID`、`AWS_SECRET_ACCESS_KEY`

---

## Task 3: HTML 导出功能（已完成）

**目标**: 将会话导出为自包含的 HTML 文件，包含样式和交互

**涉及文件**:
- 新增: `crates/pi-coding-agent/src/core/export/mod.rs`
- 新增: `crates/pi-coding-agent/src/core/export/html.rs`
- 新增: `crates/pi-coding-agent/src/core/export/html_template.rs`
- 改造: `crates/pi-coding-agent/src/core/mod.rs`
- 改造: `crates/pi-coding-agent/src/modes/interactive.rs`
- 改造: `crates/pi-coding-agent/Cargo.toml`

**实现内容**:
1. `HtmlExporter` 支持 Light/Dark 主题
2. 消息渲染：User/Assistant/ToolResult 不同样式
3. ContentBlock 转换：Text（Markdown→HTML）、Thinking（折叠）、ToolCall（折叠）、Image
4. 自包含 HTML：CSS/JS 内嵌，响应式布局，主题切换按钮
5. 交互模式 `/export` 和 `/export path` 命令
6. 使用 `pulldown-cmark` 进行 Markdown 转 HTML

---

## Task 4: 会话压缩（Compaction）功能（已完成）

**目标**: 实现长会话自动/手动压缩，通过 LLM 生成摘要释放上下文空间

**涉及文件**:
- 新增: `crates/pi-coding-agent/src/core/compaction/mod.rs`
- 新增: `crates/pi-coding-agent/src/core/compaction/compactor.rs`
- 新增: `crates/pi-coding-agent/src/core/compaction/summary_prompt.rs`
- 改造: `crates/pi-coding-agent/src/core/mod.rs`
- 改造: `crates/pi-coding-agent/src/core/session_manager.rs`
- 改造: `crates/pi-coding-agent/src/core/agent_session.rs`
- 改造: `crates/pi-agent/src/context_manager.rs`
- 改造: `crates/pi-coding-agent/src/modes/interactive.rs`

**实现内容**:
1. `SessionCompactor` 核心：token 计数、阈值检测（85%）、压缩范围确定
2. 压缩策略：保留系统消息 + 最近 4 轮对话，压缩中间消息
3. 摘要生成：使用 `pi_ai::stream::complete_simple()` 调用 LLM
4. 结构化摘要提示词：保留文件变更、技术决策、待处理事项
5. `SavedSession` 添加 `compaction_history` 字段
6. `ContextWindowManager` 添加 `needs_compaction()` 方法
7. `AgentSession` 集成 `compact()` 和 `auto_compact_if_needed()`
8. 交互模式 `/compact` 命令

---

## 依赖关系

```
Task 1 (会话 Fork) ──┐
                      ├──→ Task 3 (HTML 导出) ──→ Task 4 (会话压缩)
Task 2 (Bedrock)  ────┘
```

- Task 1 + Task 2: 并行执行（完全独立模块）
- Task 3: 在 Task 1 后执行（共享 session_manager.rs）
- Task 4: 在 Task 1 + Task 3 后执行（依赖 session 层改造稳定）

---

## 验证结果

| 检查项 | 结果 |
|--------|------|
| `cargo check` | 通过（无编译错误） |
| `cargo test` | 26 通过 / 3 失败（均为历史遗留问题） |
| `cargo clippy` | 通过（仅警告，无错误） |

3 个失败的测试均为历史遗留问题，非本次改动引入：
- `mistral::tests::test_truncate_tool_call_id` - Mistral 截断逻辑
- `json_parse::tests::test_streaming_parser` - JSON 流式解析
- `json_parse::tests::test_trailing_comma` - JSON 尾部逗号处理
