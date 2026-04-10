# 二次迭代计划

## 概述

首次复刻完成了 pi-mono 项目的核心骨架，包括：
- **pi-ai**: LLM 统一 API 层，实现了 Anthropic、OpenAI、Google 三个主要 Provider 的基础流式接口
- **pi-tui**: TUI 框架核心，实现了差分渲染引擎、终端抽象、编辑器组件等基础设施
- **pi-agent**: Agent 运行时核心，实现了基本的 agent loop、消息队列、工具调用框架
- **pi-coding-agent**: CLI 入口，实现了基础的交互模式和打印模式

**首次复刻的简化/跳过内容：**
1. 交互模式使用简化的 raw mode + print，未完整集成 TUI 差分渲染引擎和编辑器组件
2. 扩展系统未实现
3. OAuth 认证未实现
4. 会话压缩（compaction）功能未实现
5. HTML 导出功能未实现
6. 更多 Provider（Mistral、Bedrock、Azure OpenAI 等）未实现
7. Agent loop 中的实际 API 流式响应为 stub 状态，需要接入 Provider

二次迭代的目标是完善核心功能，提升用户体验，扩展 Provider 支持，并建立扩展系统基础。

---

## P0 - 核心功能完善（必须完成）

### 1. TUI 完整集成到交互模式

| 属性 | 值 |
|------|-----|
| **功能名称** | TUI 差分渲染引擎接入交互模式 |
| **当前状态** | 简化实现（raw mode + print 输出） |
| **TS 参考** | `packages/coding-agent/src/modes/interactive/interactive-mode.ts` (4750 行), `packages/tui/src/tui.ts` |
| **预估工作量** | 大 |
| **详细描述** | 当前交互模式使用简单的 raw mode 和直接 print 输出，需要完整接入 pi-tui 的差分渲染引擎。包括：<br>1. 创建 TUI 实例并管理生命周期<br>2. 将 Agent 事件流转换为 TUI 组件更新<br>3. 实现消息历史渲染组件（用户消息、助手消息、工具调用）<br>4. 实现底部状态栏组件<br>5. 实现输入编辑器组件集成<br>6. 处理终端大小变化、焦点管理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 编辑器组件替换简化输入 |
| **当前状态** | 未集成（使用简单 stdin 读取） |
| **TS 参考** | `packages/tui/src/components/editor.ts` (72KB), `packages/coding-agent/src/modes/interactive/components/custom-editor.ts` |
| **预估工作量** | 中 |
| **详细描述** | 当前交互模式使用简单的 stdin 逐字节读取和基础字符处理。需要：<br>1. 将 pi-tui 的 Editor 组件集成到交互模式<br>2. 实现多行输入支持（Shift+Enter）<br>3. 实现粘贴处理（大粘贴折叠）<br>4. 实现自动完成（slash 命令、@文件引用）<br>5. 支持 Emacs 风格快捷键（Ctrl+A/E/K/Y 等） |

### 2. Agent Loop 流式响应接入

| 属性 | 值 |
|------|-----|
| **功能名称** | Agent Loop 接入实际 Provider 流式响应 |
| **当前状态** | Stub（返回空 AssistantMessage） |
| **TS 参考** | `packages/agent/src/agent-loop.ts`, `packages/pi-agent/src/agent_loop.rs` (line 288-314) |
| **预估工作量** | 中 |
| **详细描述** | 当前 `agent_loop.rs` 中的 `stream_assistant_response` 函数是 stub 实现，直接返回空消息。需要：<br>1. 接入 pi-ai 的 Provider registry<br>2. 实现消息转换（AgentMessage -> LLM Message）<br>3. 消费 SSE 流并发出对应事件<br>4. 处理流中断、错误重试<br>5. 支持取消操作 |

### 3. Provider 实际 API 调试与完善

| 属性 | 值 |
|------|-----|
| **功能名称** | Anthropic Provider 实际 API 调试 |
| **当前状态** | 代码完成，未实际测试 |
| **TS 参考** | `packages/ai/src/providers/anthropic.ts` (27KB) |
| **预估工作量** | 中 |
| **详细描述** | Anthropic Provider 代码已完整移植，但需要进行实际 API 测试：<br>1. 测试标准 API key 认证<br>2. 测试 OAuth token 认证<br>3. 测试思考模式（thinking）<br>4. 测试工具调用流式解析<br>5. 测试缓存控制<br>6. 测试错误处理和重试 |

| 属性 | 值 |
|------|-----|
| **功能名称** | OpenAI Provider 实际 API 调试 |
| **当前状态** | 代码完成，未实际测试 |
| **TS 参考** | `packages/ai/src/providers/openai.ts` (30KB) |
| **预估工作量** | 中 |
| **详细描述** | OpenAI Provider 支持多种兼容模式，需要测试：<br>1. 标准 OpenAI API<br>2. OpenRouter 兼容模式<br>3. Groq/Cerebras/XAI 等非标准兼容<br>4. 工具调用解析<br>5. Reasoning 内容处理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Google Provider 实际 API 调试 |
| **当前状态** | 代码完成，未实际测试 |
| **TS 参考** | `packages/ai/src/providers/google.ts` (14KB), `packages/ai/src/providers/google-gemini-cli.ts` (30KB) |
| **预估工作量** | 中 |
| **详细描述** | Google Provider 需要测试：<br>1. Gemini API 流式响应<br>2. 工具调用（function calling）<br>3. 思考内容处理<br>4. 安全设置（safety settings）<br>5. 图片输入支持 |

---

## P1 - 重要功能补充（应该完成）

### 4. 更多 Provider 支持

| 属性 | 值 |
|------|-----|
| **功能名称** | Mistral Provider 实现 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/ai/src/providers/mistral.ts` (18KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现 Mistral AI Provider：<br>1. 使用 mistralai SDK 或 REST API<br>2. 支持工具调用<br>3. 支持流式响应<br>4. 处理 Mistral 特有的 tool call ID 长度限制 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Amazon Bedrock Provider 实现 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/ai/src/providers/amazon-bedrock.ts` (25KB) |
| **预估工作量** | 大 |
| **详细描述** | 实现 AWS Bedrock Provider：<br>1. 使用 aws-sdk-client-bedrock-runtime<br>2. 支持 Claude 模型<br>3. 支持跨区域推理<br>4. 支持缓存（prompt caching）<br>5. 处理 AWS 特有的认证和配置 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Azure OpenAI Provider 实现 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/ai/src/providers/azure-openai-responses.ts` (7KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现 Azure OpenAI Provider：<br>1. 支持 Azure AD 认证<br>2. 支持部署名称映射<br>3. 复用 OpenAI Provider 的大部分逻辑 |

### 5. 会话系统完善

| 属性 | 值 |
|------|-----|
| **功能名称** | 会话 Fork 功能 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/session-manager.ts` (41KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现会话 Fork（分支）功能：<br>1. 支持从任意消息创建分支<br>2. 分支历史独立管理<br>3. 分支切换和可视化<br>4. 分支合并（可选） |

| 属性 | 值 |
|------|-----|
| **功能名称** | 会话压缩（Compaction） |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/compaction/compaction.ts` (25KB), `packages/coding-agent/src/core/compaction/branch-summarization.ts` (11KB) |
| **预估工作量** | 大 |
| **详细描述** | 实现长会话的自动/手动压缩：<br>1. 检测上下文窗口接近上限<br>2. 使用 LLM 生成对话摘要<br>3. 保留文件操作历史<br>4. 支持分支摘要<br>5. 压缩后重新加载会话 |

| 属性 | 值 |
|------|-----|
| **功能名称** | HTML 导出功能 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/export-html/` (7 个文件，包含模板) |
| **预估工作量** | 中 |
| **详细描述** | 实现会话导出为 HTML：<br>1. ANSI 转 HTML<br>2. Markdown 渲染<br>3. 工具调用折叠/展开<br>4. 主题颜色导出<br>5. 独立 HTML 文件（包含 CSS/JS） |

### 6. Agent 功能完善

| 属性 | 值 |
|------|-----|
| **功能名称** | Steering/FollowUp 完整逻辑 |
| **当前状态** | 基础实现，需完善 |
| **TS 参考** | `packages/agent/src/agent.ts` (15KB), `packages/agent/src/agent-loop.ts` (16KB) |
| **预估工作量** | 中 |
| **详细描述** | 完善消息队列处理：<br>1. Steering 消息（mid-turn 注入）<br>2. FollowUp 消息（turn 结束后自动发送）<br>3. 队列模式支持（OneAtATime、Coalesce、Queue）<br>4. 消息优先级处理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 上下文窗口管理 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/agent-session.ts` (98KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现智能上下文管理：<br>1. Token 计数和估算<br>2. 上下文窗口接近上限警告<br>3. 自动触发压缩<br>4. 智能消息裁剪策略 |

### 7. 工具系统增强

| 属性 | 值 |
|------|-----|
| **功能名称** | 工具权限管理 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/coding-agent/src/core/tools/` (14 个文件) |
| **预估工作量** | 中 |
| **详细描述** | 增强工具权限控制：<br>1. 首次使用工具时确认<br>2. 允许/拒绝列表<br>3. 工具调用前钩子（beforeToolCall）<br>4. 工具调用后钩子（afterToolCall） |

| 属性 | 值 |
|------|-----|
| **功能名称** | Bash 工具沙箱增强 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/coding-agent/src/core/tools/bash.ts` (15KB), `packages/coding-agent/src/core/bash-executor.ts` (5KB) |
| **预估工作量** | 中 |
| **详细描述** | 增强 Bash 工具安全性：<br>1. 命令白名单/黑名单<br>2. 超时控制<br>3. 环境变量隔离<br>4. 危险命令确认 |

---

## P2 - 增强功能（可选完成）

### 8. 扩展系统

| 属性 | 值 |
|------|-----|
| **功能名称** | 扩展系统基础框架 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/` (5 个文件，145KB+) |
| **预估工作量** | 大 |
| **详细描述** | 实现扩展系统：<br>1. 扩展类型定义（types.ts）<br>2. 扩展加载器（loader.ts）<br>3. 扩展运行器（runner.ts）<br>4. 扩展 API 上下文<br>5. 工具注册机制<br>6. 命令注册机制<br>7. UI 交互 API |

| 属性 | 值 |
|------|-----|
| **功能名称** | 扩展市场/管理 |
| **当前状态** | 未实现 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 扩展的发布和管理：<br>1. 扩展安装/卸载<br>2. 扩展配置管理<br>3. 扩展更新检查 |

### 9. 认证系统

| 属性 | 值 |
|------|-----|
| **功能名称** | OAuth 认证支持 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/auth-storage.ts` (12KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现 OAuth 认证流程：<br>1. 本地 HTTP 回调服务器<br>2. Token 存储和刷新<br>3. 支持 Anthropic、GitHub Copilot 等 OAuth 提供商 |

### 10. TUI 组件完善

| 属性 | 值 |
|------|-----|
| **功能名称** | Markdown 渲染组件增强 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/tui/src/components/markdown.ts` (26KB), `packages/pi-tui/src/components/markdown.rs` |
| **预估工作量** | 中 |
| **详细描述** | 增强 Markdown 组件：<br>1. 代码块语法高亮<br>2. 表格渲染<br>3. 列表缩进优化<br>4. 链接处理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 图片显示组件 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/tui/src/components/image.ts`, `packages/tui/src/terminal-image.ts` (10KB) |
| **预估工作量** | 中 |
| **详细描述** | 完善终端图片显示：<br>1. Kitty 图形协议支持<br>2. iTerm2 图像协议支持<br>3. 图片缩放和裁剪<br>4. 回退到字符画 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 模糊搜索组件 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/tui/src/fuzzy.ts` (3KB) |
| **预估工作量** | 小 |
| **详细描述** | 增强模糊搜索：<br>1. 高性能模糊匹配算法<br>2. 高亮匹配字符<br>3. 多字段搜索 |

### 11. 测试覆盖

| 属性 | 值 |
|------|-----|
| **功能名称** | 单元测试覆盖 |
| **当前状态** | 部分组件有测试 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 建立完整测试体系：<br>1. Provider 单元测试（使用 mock server）<br>2. Agent loop 测试<br>3. TUI 组件测试<br>4. 工具测试<br>5. 集成测试 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 端到端测试 |
| **当前状态** | 无 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 端到端测试：<br>1. CLI 命令测试<br>2. 交互模式测试<br>3. 会话持久化测试 |

### 12. 性能优化

| 属性 | 值 |
|------|-----|
| **功能名称** | 大文件处理优化 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/coding-agent/src/core/tools/truncate.ts` (7KB) |
| **预估工作量** | 中 |
| **详细描述** | 优化大文件处理：<br>1. 智能截断策略<br>2. 增量读取<br>3. 内存使用优化 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 流式渲染优化 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/tui/src/tui.ts` |
| **预估工作量** | 中 |
| **详细描述** | 优化 TUI 流式渲染：<br>1. 减少不必要的重绘<br>2. 批量更新<br>3. 虚拟滚动（大历史记录） |

---

## 依赖关系图

```
P0 核心功能:
├── TUI 完整集成到交互模式
│   └── 依赖: TUI 差分渲染引擎 (已完成)
├── 编辑器组件替换简化输入
│   └── 依赖: Editor 组件 (已完成)
├── Agent Loop 流式响应接入
│   └── 依赖: Provider 实现 (已完成)
└── Provider 实际 API 调试与完善

P1 重要功能:
├── 更多 Provider 支持
│   └── 依赖: Provider 接口稳定
├── 会话系统完善
│   └── 依赖: 会话管理器 (已完成)
├── Agent 功能完善
│   └── 依赖: Agent Loop 稳定
└── 工具系统增强
    └── 依赖: 工具框架 (已完成)

P2 增强功能:
├── 扩展系统
│   └── 依赖: 所有核心功能稳定
├── 认证系统
│   └── 依赖: 配置系统
├── TUI 组件完善
│   └── 依赖: TUI 核心 (已完成)
├── 测试覆盖
│   └── 依赖: 功能稳定
└── 性能优化
    └── 依赖: 功能完整
```

---

## 时间估算汇总

| 优先级 | 项目数 | 总工作量 | 预估时间（1人） |
|--------|--------|----------|-----------------|
| P0 | 6 | 4大 2中 | 6-8 周 |
| P1 | 9 | 2大 6中 1小 | 8-10 周 |
| P2 | 7 | 3大 3中 1小 | 6-8 周 |
| **总计** | **22** | **9大 11中 2小** | **20-26 周** |

---

## 建议实施顺序

### 第一阶段（4-6 周）：核心稳定
1. Provider 实际 API 调试（Anthropic、OpenAI、Google）
2. Agent Loop 流式响应接入
3. 基础 TUI 集成到交互模式

### 第二阶段（4-6 周）：功能完善
1. 编辑器组件完整集成
2. Mistral Provider 实现
3. 工具权限管理
4. 上下文窗口管理

### 第三阶段（4-6 周）：会话增强
1. 会话压缩功能
2. 会话 Fork 功能
3. HTML 导出功能
4. Amazon Bedrock Provider

### 第四阶段（6-8 周）：扩展与优化
1. 扩展系统基础框架
2. OAuth 认证
3. 测试覆盖
4. 性能优化
