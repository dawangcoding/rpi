# 三次迭代计划

## 概述

二次迭代（ITERATION-2）完成了 pi-mono 项目的大部分核心功能，包括：
- **pi-ai**: 5 个 Provider 实现（Anthropic、OpenAI、Google、Mistral、Bedrock），核心类型系统和流式 API 完整
- **pi-tui**: TUI 框架核心完整，包含差分渲染引擎、编辑器组件、Markdown 渲染、键盘处理等
- **pi-agent**: Agent 运行时核心 100% 完成，包含完整的 agent loop、消息队列、工具调用框架
- **pi-coding-agent**: 基础功能完成，包括 7 个内置工具、会话管理、HTML 导出、会话压缩、Fork 功能、扩展系统框架、OAuth 认证框架

**ITERATION-2 结束时的状态：**
- **pi-ai**: 完成度 45%（核心 100%，Provider 35%）
- **pi-tui**: 完成度 91%（核心 100%，组件 83%）
- **pi-agent**: 完成度 100%
- **pi-coding-agent**: 完成度 35%（工具 100%，交互 14%，扩展 30%）
- **整体完成度约 60%**

**ITERATION-2 遗留的关键问题：**
1. 交互模式仍使用简化的 raw mode（659 行），未完整集成 pi-tui 差分渲染引擎和编辑器组件
2. Provider 覆盖不足（缺少 Azure OpenAI、xAI、OpenRouter 等）
3. OAuth 认证为框架阶段，未实现完整流程
4. 会话压缩和 Fork 功能需要完善
5. 扩展系统仅为基础框架，事件钩子和动态加载待实现
6. TUI 组件缺失（SettingsList、TruncatedText、CancellableLoader）

三次迭代的目标是：**将交互模式升级为完整的 TUI 体验，扩展 Provider 支持，完善认证和会话系统，增强扩展能力，最终达到生产可用状态。**

---

## Phase 1: 交互模式 TUI 完整集成（P0 最高优先级）

### 目标
将交互模式从当前简化的 raw mode（659 行）升级为完整集成 pi-tui 差分渲染引擎，提供接近原版的用户体验。

### 任务列表

| 属性 | 值 |
|------|-----|
| **功能名称** | 消息历史渲染重构 |
| **当前状态** | 简单行缓冲输出 |
| **TS 参考** | `packages/coding-agent/src/modes/interactive/interactive-mode.ts` (4750 行) |
| **预估工作量** | 大 |
| **详细描述** | 将简单行缓冲输出升级为完整的消息组件树渲染：<br>1. 创建消息列表容器组件<br>2. 实现用户消息组件（带编辑标记）<br>3. 实现助手消息组件（Markdown 渲染、Thinking 折叠）<br>4. 实现工具调用显示组件（可折叠）<br>5. 实现加载状态组件<br>6. 支持消息间的分隔和样式 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 编辑器组件集成 |
| **当前状态** | 单行输入（stdin 逐字节读取） |
| **TS 参考** | `packages/tui/src/components/editor.ts` (72KB), `packages/coding-agent/src/modes/interactive/components/custom-editor.ts` |
| **预估工作量** | 大 |
| **详细描述** | 将 pi-tui 的 Editor 组件完整集成到交互模式：<br>1. 替换现有简单输入循环<br>2. 实现多行输入支持（Shift+Enter 换行）<br>3. 集成自动完成系统（Slash 命令、@文件引用）<br>4. 实现粘贴处理（大粘贴折叠）<br>5. 支持完整的 Emacs 快捷键（Ctrl+A/E/K/Y 等）<br>6. 实现输入历史（Up/Down 导航） |

| 属性 | 值 |
|------|-----|
| **功能名称** | 自动完成增强 |
| **当前状态** | 基础框架 |
| **TS 参考** | `packages/tui/src/autocomplete.ts` (22KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现完整的自动完成功能：<br>1. Slash 命令补全（/compact, /fork, /export 等）<br>2. @文件引用补全（项目文件模糊搜索）<br>3. 模型名称补全<br>4. 会话名称补全<br>5. 补全项分类和图标显示 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 粘贴处理优化 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/tui/src/editor-component.ts` |
| **预估工作量** | 中 |
| **详细描述** | 实现大粘贴折叠处理：<br>1. 检测粘贴开始/结束标记（bracketed paste）<br>2. 大内容粘贴时折叠显示<br>3. 提供展开/折叠控制<br>4. 防止粘贴期间的中间渲染 |

| 属性 | 值 |
|------|-----|
| **功能名称** | IME 支持完善 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/tui/src/tui.ts` (CURSOR_MARKER) |
| **预估工作量** | 中 |
| **详细描述** | 完善输入法编辑器支持：<br>1. 候选窗口定位优化（使用 CURSOR_MARKER）<br>2. 组合字符处理<br>3. 不同终端的 IME 兼容性 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 主题系统基础 |
| **当前状态** | 硬编码颜色 |
| **TS 参考** | `packages/coding-agent/src/modes/interactive/theme/` |
| **预估工作量** | 中 |
| **详细描述** | 实现基础主题系统：<br>1. 定义主题配置结构（颜色、样式）<br>2. 实现 Light/Dark 主题<br>3. 组件样式与主题绑定<br>4. 主题切换命令（/theme） |

### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/modes/interactive.rs` (当前 659 行 → 目标 ~2000 行)
- `crates/pi-coding-agent/src/modes/interactive_components.rs` (扩展)

**新增文件：**
- `crates/pi-coding-agent/src/modes/theme.rs` - 主题定义和管理

**依赖文件：**
- `crates/pi-tui/src/tui.rs` - 差分渲染引擎
- `crates/pi-tui/src/components/editor.rs` - 编辑器组件
- `crates/pi-tui/src/components/markdown.rs` - Markdown 渲染
- `crates/pi-tui/src/autocomplete.rs` - 自动完成

### 验证标准

1. 交互模式启动后显示完整 TUI 界面（消息历史 + 编辑器 + 状态栏）
2. 多行输入支持 Shift+Enter 换行
3. 输入 `/` 触发 Slash 命令补全
4. 输入 `@` 触发文件引用补全
5. 大粘贴内容自动折叠
6. 支持 Ctrl+C 中断、Ctrl+D 退出
7. 主题切换后界面颜色正确更新
8. 终端大小变化时布局正确调整

### 预估工作量

**总计：5-6 周**（1 人全职）

---

## Phase 2: Provider 扩展（P1）

### 目标
从当前 5 个 Provider 扩展到覆盖主流平台，支持 Azure OpenAI、xAI (Grok)、OpenRouter 统一网关。

### 任务列表

| 属性 | 值 |
|------|-----|
| **功能名称** | Azure OpenAI Provider |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/ai/src/providers/azure-openai-responses.ts` (7KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现 Azure OpenAI Provider：<br>1. 支持 Azure AD 认证（API Key 和 OAuth）<br>2. 支持部署名称映射（deployment name → model）<br>3. 复用 OpenAI Provider 的大部分逻辑<br>4. 支持 Azure 特有的 endpoint 格式<br>5. 错误处理（Azure 特定错误码） |

| 属性 | 值 |
|------|-----|
| **功能名称** | xAI (Grok) Provider |
| **当前状态** | 未实现 |
| **TS 参考** | 兼容 OpenAI 接口格式 |
| **预估工作量** | 小 |
| **详细描述** | 实现 xAI Provider：<br>1. 复用 OpenAI Provider 逻辑<br>2. 配置 xAI base URL（https://api.x.ai）<br>3. 支持 Grok 模型系列<br>4. 注册到模型注册表 |

| 属性 | 值 |
|------|-----|
| **功能名称** | OpenRouter 统一网关 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/ai/src/providers/openai-completions.ts`（兼容模式） |
| **预估工作量** | 中 |
| **详细描述** | 实现 OpenRouter Provider：<br>1. 支持 OpenRouter API（https://openrouter.ai/api/v1）<br>2. 通过 OpenRouter 支持 Groq、Cerebras、Moonshot 等后端<br>3. 处理 OpenRouter 特有的响应头（模型路由信息）<br>4. 支持免费模型标识 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 模型注册表扩展 |
| **当前状态** | 5 个 Provider 的模型 |
| **TS 参考** | `packages/ai/src/models.generated.ts` (353KB) |
| **预估工作量** | 小 |
| **详细描述** | 在 `models.rs` 中注册新 Provider 的模型：<br>1. Azure OpenAI：GPT-4o、GPT-4o-mini、o1、o3-mini<br>2. xAI：Grok-2、Grok-2-vision、Grok-beta<br>3. OpenRouter：映射各后端模型 |

### 涉及文件

**新增文件：**
- `crates/pi-ai/src/providers/azure.rs` - Azure OpenAI Provider
- `crates/pi-ai/src/providers/xai.rs` - xAI Provider
- `crates/pi-ai/src/providers/openrouter.rs` - OpenRouter Provider

**修改文件：**
- `crates/pi-ai/src/providers/mod.rs` - 注册新 Provider
- `crates/pi-ai/src/models.rs` - 添加新模型定义
- `crates/pi-ai/src/types.rs` - 扩展 Api 和 Provider 枚举
- `crates/pi-ai/src/lib.rs` - 导出新模块

### 验证标准

1. Azure OpenAI Provider 可通过 Azure AD 认证调用
2. xAI Provider 可调用 Grok-2 模型
3. OpenRouter Provider 可路由到 Groq/Cerebras 后端
4. 所有新 Provider 支持流式响应
5. 模型列表命令显示新添加的模型
6. 配置文件中可配置新 Provider 的 API Key

### 预估工作量

**总计：2-3 周**（1 人全职）

---

## Phase 3: OAuth 认证完整实现（P1）

### 目标
从框架阶段升级为可用的完整 OAuth 流程，支持 Anthropic、OpenAI、Google 的 OAuth 认证。

### 任务列表

| 属性 | 值 |
|------|-----|
| **功能名称** | OAuth 流程完整实现 |
| **当前状态** | 框架代码（结构定义） |
| **TS 参考** | `packages/coding-agent/src/core/auth-storage.ts` (12KB) |
| **预估工作量** | 中 |
| **详细描述** | 实现完整的 OAuth 授权码流程：<br>1. 本地 HTTP 回调服务器（hyper）<br>2. 授权 URL 生成（PKCE 支持）<br>3. 授权码交换 Token<br>4. 浏览器自动打开授权页面<br>5. 错误处理（用户拒绝、超时） |

| 属性 | 值 |
|------|-----|
| **功能名称** | Token 存储加密 |
| **当前状态** | 框架代码 |
| **TS 参考** | `packages/coding-agent/src/core/auth-storage.ts` |
| **预估工作量** | 中 |
| **详细描述** | 实现安全的本地 Token 持久化：<br>1. 使用系统密钥链（macOS Keychain、Windows DPAPI、Linux Secret Service）<br>2. 或使用文件加密（AES-GCM，密钥派生自机器指纹）<br>3. Token 存储格式（JSON + 元数据）<br>4. 读取时自动解密 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Token 自动刷新 |
| **当前状态** | 未实现 |
| **TS 参考** | OAuth 2.0 Refresh Token 流程 |
| **预估工作量** | 中 |
| **详细描述** | 实现 Token 过期前自动刷新：<br>1. 检测 Token 过期时间<br>2. 过期前自动调用 refresh_token 端点<br>3. 更新存储的 Token<br>4. 刷新失败时提示重新登录 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 多 Provider OAuth 支持 |
| **当前状态** | 框架代码 |
| **TS 参考** | 各 Provider OAuth 文档 |
| **预估工作量** | 中 |
| **详细描述** | 支持多个 Provider 的 OAuth：<br>1. Anthropic OAuth（console.anthropic.com）<br>2. OpenAI OAuth（platform.openai.com）<br>3. Google OAuth（Google Cloud Console）<br>4. 每个 Provider 的配置（client_id、scopes、endpoints） |

### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/core/auth/mod.rs` - OAuth 流程实现
- `crates/pi-coding-agent/src/core/auth/oauth_server.rs` - 本地回调服务器
- `crates/pi-coding-agent/src/core/auth/providers.rs` - Provider OAuth 配置
- `crates/pi-coding-agent/src/core/auth/token_storage.rs` - Token 存储和加密

**集成修改：**
- `crates/pi-coding-agent/src/config.rs` - 优先使用 OAuth token
- `crates/pi-coding-agent/src/modes/interactive.rs` - /login /logout /auth 命令

### 验证标准

1. `/login anthropic` 启动 OAuth 流程，浏览器打开授权页面
2. 授权后本地服务器接收回调，获取并存储 Token
3. Token 加密存储在系统密钥链或加密文件
4. API 调用时自动使用 OAuth Token
5. Token 过期前自动刷新
6. `/logout` 清除存储的 Token
7. `/auth status` 显示当前认证状态

### 预估工作量

**总计：2-3 周**（1 人全职）

---

## Phase 4: 会话系统完善（P1）

### 目标
完善会话压缩和 Fork 功能，增强 Token 计数和成本统计。

### 任务列表

| 属性 | 值 |
|------|-----|
| **功能名称** | 会话压缩完整实现 |
| **当前状态** | 基础框架（阈值检测、摘要生成） |
| **TS 参考** | `packages/coding-agent/src/core/compaction/compaction.ts` (25KB) |
| **预估工作量** | 中 |
| **详细描述** | 完善 LLM 驱动摘要和自动触发逻辑：<br>1. 优化压缩范围确定算法<br>2. 完善摘要提示词（保留关键信息）<br>3. 实现压缩历史记录<br>4. 支持手动触发（/compact 命令）<br>5. 支持自动触发（阈值达到时）<br>6. 压缩后消息重新加载 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 会话 Fork 完善 |
| **当前状态** | 基础实现 |
| **TS 参考** | `packages/coding-agent/src/core/session-manager.ts` (41KB) |
| **预估工作量** | 中 |
| **详细描述** | 补全 Fork 的边界情况处理和 UI 集成：<br>1. 从任意消息位置创建分支<br>2. 分支历史独立管理<br>3. 分支切换命令（/switch）<br>4. 分支列表显示（/forks）<br>5. 分支可视化（树形结构）<br>6. 分支删除和清理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 会话统计增强 |
| **当前状态** | 基础统计 |
| **TS 参考** | `packages/coding-agent/src/core/agent-session.ts` |
| **预估工作量** | 小 |
| **详细描述** | 完善 Token 计数和成本统计：<br>1. 实时 Token 计数显示<br>2. 成本估算（按模型定价）<br>3. 上下文窗口使用率<br>4. 会话统计持久化<br>5. /stats 命令显示详细统计 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Token 计数优化 |
| **当前状态** | 基础 EstimateTokenCounter |
| **TS 参考** | `packages/ai/src/token-counter.ts` |
| **预估工作量** | 中 |
| **详细描述** | 完善 Token 计数实现：<br>1. 支持更多模型的精确计数<br>2. 集成 tiktoken（OpenAI 模型）<br>3. 集成 anthropic-tokenizer（Claude 模型）<br>4. 其他模型的字符估算优化 |

### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/core/compaction/compactor.rs` - 压缩逻辑完善
- `crates/pi-coding-agent/src/core/compaction/summary_prompt.rs` - 摘要提示词优化
- `crates/pi-coding-agent/src/core/session_manager.rs` - Fork 管理完善
- `crates/pi-coding-agent/src/core/agent_session.rs` - 统计集成
- `crates/pi-ai/src/token_counter.rs` - Token 计数优化

**集成修改：**
- `crates/pi-coding-agent/src/modes/interactive.rs` - /compact、/fork、/stats 命令

### 验证标准

1. 长会话（>85% 上下文窗口）自动触发压缩
2. /compact 命令手动触发压缩
3. /fork N 从第 N 条消息创建分支
4. /forks 显示所有分支列表
5. /stats 显示 Token 使用、成本、消息数
6. Token 计数与 Provider 返回的 usage 接近
7. 压缩后会话可正常继续对话

### 预估工作量

**总计：2-3 周**（1 人全职）

---

## Phase 5: 扩展系统增强（P1-P2）

### 目标
增强扩展系统能力，补全事件钩子，完善工具和命令注册机制。

### 任务列表

| 属性 | 值 |
|------|-----|
| **功能名称** | 扩展 Trait 完善 |
| **当前状态** | 基础事件（5 种） |
| **TS 参考** | `packages/coding-agent/src/core/extensions/` (145KB+) |
| **预估工作量** | 大 |
| **详细描述** | 补全事件钩子（从基础扩展到 20+ 种事件）：<br>1. Agent 生命周期：BeforeAgentStart、AgentStart、AgentEnd<br>2. Turn 生命周期：TurnStart、TurnEnd<br>3. 消息事件：MessageStart、MessageEnd、MessageUpdate<br>4. 工具事件：BeforeToolCall、AfterToolCall、ToolExecutionStart/End<br>5. 渲染事件：MessageRenderer、ToolOutputRenderer<br>6. 命令事件：BeforeCommandExecute、AfterCommandExecute |

| 属性 | 值 |
|------|-----|
| **功能名称** | 工具注册机制 |
| **当前状态** | 基础框架 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 中 |
| **详细描述** | 完善动态工具注册和发现：<br>1. 扩展动态注册工具<br>2. 工具参数 Schema 定义<br>3. 工具执行上下文传递<br>4. 工具权限控制<br>5. 工具帮助信息生成 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 命令注册机制 |
| **当前状态** | 基础框架 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 中 |
| **详细描述** | 完善 Slash 命令的扩展注册：<br>1. 扩展注册自定义 Slash 命令<br>2. 命令参数解析<br>3. 命令自动完成<br>4. 命令帮助信息<br>5. 命令执行上下文 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 扩展加载策略 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/loader.ts` |
| **预估工作量** | 大 |
| **详细描述** | 评估并实现编译时链接或动态库加载方案：<br>1. 调研 Rust 扩展加载方案（dylib、WASM、IPC）<br>2. 实现扩展发现机制（~/.pi/extensions/）<br>3. 扩展加载和初始化<br>4. 扩展隔离和错误处理<br>5. 扩展热重载（可选） |

### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/core/extensions/types.rs` - 扩展 Trait 完善
- `crates/pi-coding-agent/src/core/extensions/api.rs` - ExtensionContext 完善
- `crates/pi-coding-agent/src/core/extensions/loader.rs` - 加载器实现
- `crates/pi-coding-agent/src/core/extensions/runner.rs` - ExtensionManager 完善

**集成修改：**
- `crates/pi-coding-agent/src/core/agent_session.rs` - 事件分发
- `crates/pi-coding-agent/src/modes/interactive.rs` - 扩展命令集成

### 验证标准

1. 扩展示例可以注册自定义工具
2. 扩展示例可以注册自定义 Slash 命令
3. 扩展可以监听 Agent 生命周期事件
4. 扩展可以修改消息内容（通过事件钩子）
5. /extensions list 显示已加载扩展
6. 扩展加载失败不影响主程序运行

### 预估工作量

**总计：3-4 周**（1 人全职）

---

## Phase 6: TUI 组件补全与质量提升（P2）

### 目标
补全缺失组件，提升测试覆盖，清理编译警告，补充文档。

### 任务列表

| 属性 | 值 |
|------|-----|
| **功能名称** | SettingsList 组件 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/tui/src/components/settings-list.ts` (7.7KB) |
| **预估工作量** | 中 |
| **详细描述** | 新建设置列表组件：<br>1. 设置项显示（名称、描述、当前值）<br>2. 支持不同类型（布尔、字符串、数字、枚举）<br>3. 设置值编辑<br>4. 分类和分组显示<br>5. 搜索过滤 |

| 属性 | 值 |
|------|-----|
| **功能名称** | TruncatedText 组件 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/tui/src/components/truncated-text.ts` (1.8KB) |
| **预估工作量** | 小 |
| **详细描述** | 新建截断文本组件：<br>1. 超出宽度时显示省略号<br>2. 支持头部/尾部截断<br>3. 支持鼠标悬停显示完整内容（可选）<br>4. ANSI 样式保留 |

| 属性 | 值 |
|------|-----|
| **功能名称** | CancellableLoader 组件 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/tui/src/components/cancellable-loader.ts` (1.0KB) |
| **预估工作量** | 小 |
| **详细描述** | 新建可取消加载器：<br>1. 加载动画（旋转器）<br>2. 取消按钮/快捷键<br>3. 加载进度显示（可选）<br>4. 取消事件回调 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 测试覆盖提升 |
| **当前状态** | 基础测试 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 为核心模块补充单元测试和集成测试：<br>1. pi-ai：Provider 边界情况测试<br>2. pi-tui：组件渲染测试<br>3. pi-agent：Agent 状态机测试<br>4. pi-coding-agent：工具集成测试<br>5. 目标：核心模块覆盖率 > 70% |

| 属性 | 值 |
|------|-----|
| **功能名称** | 编译警告清理 |
| **当前状态** | 存在 dead_code 和 unused 警告 |
| **TS 参考** | N/A |
| **预估工作量** | 小 |
| **详细描述** | 消除所有编译警告：<br>1. 清理 dead_code 警告<br>2. 清理 unused 警告<br>3. 清理 clippy 警告<br>4. 确保 `cargo check` 和 `cargo clippy` 零警告 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 文档补充 |
| **当前状态** | 部分文档 |
| **TS 参考** | N/A |
| **预估工作量** | 中 |
| **详细描述** | 为公共 API 补充 rustdoc 文档注释：<br>1. pi-ai：types、stream、Provider trait<br>2. pi-tui：Component trait、TUI 结构体<br>3. pi-agent：Agent、AgentLoop<br>4. pi-coding-agent：工具、会话管理<br>5. 生成并发布文档 |

### 涉及文件

**新增文件：**
- `crates/pi-tui/src/components/settings_list.rs` - 设置列表组件
- `crates/pi-tui/src/components/truncated_text.rs` - 截断文本组件
- `crates/pi-tui/src/components/cancellable_loader.rs` - 可取消加载器

**修改文件：**
- `crates/pi-tui/src/components/mod.rs` - 导出新组件
- 各 crate 的测试文件 - 补充测试
- 各 crate 的 lib.rs - 补充文档

### 验证标准

1. SettingsList 组件可正确显示和编辑设置
2. TruncatedText 组件正确截断长文本
3. CancellableLoader 组件显示加载状态并可取消
4. `cargo test` 通过率 > 95%
5. 代码覆盖率 > 70%
6. `cargo clippy` 零警告
7. `cargo doc` 生成完整文档

### 预估工作量

**总计：2-3 周**（1 人全职）

---

## Phase 间依赖关系

```
Phase 1 (TUI 完整集成)
    │
    ├──→ Phase 2 (Provider 扩展) ──┐
    │                               │
    ├──→ Phase 3 (OAuth 认证) ──────┤
    │                               │
    ├──→ Phase 4 (会话系统) ────────┤
    │                               │
    └──→ Phase 5 (扩展系统) ────────┤
                                    │
                                    ↓
                            Phase 6 (质量提升)
```

**依赖说明：**
- Phase 1 是其他所有 Phase 的基础，因为交互模式是用户主要入口
- Phase 2、3、4、5 可以并行进行（在 Phase 1 完成后）
- Phase 6 依赖于所有其他 Phase 完成（代码稳定后提升质量）

---

## 总体时间线估算

| Phase | 名称 | 预估时间 | 累计时间 |
|-------|------|----------|----------|
| Phase 1 | 交互模式 TUI 完整集成 | 5-6 周 | 5-6 周 |
| Phase 2 | Provider 扩展 | 2-3 周 | 7-9 周 |
| Phase 3 | OAuth 认证完整实现 | 2-3 周 | 9-12 周 |
| Phase 4 | 会话系统完善 | 2-3 周 | 11-15 周 |
| Phase 5 | 扩展系统增强 | 3-4 周 | 14-19 周 |
| Phase 6 | TUI 组件补全与质量提升 | 2-3 周 | 16-22 周 |

**总计：16-22 周**（约 4-5 个月，1 人全职）

**并行优化：**
- 如果 Phase 2-5 并行开发，可缩短至 **12-16 周**
- 需要 2-3 名开发者协作

---

## 完成后的预期状态

### 各模块完成度目标

| 模块 | 当前完成度 | 目标完成度 | 关键改进 |
|------|-----------|-----------|----------|
| **pi-ai** | 45% | 75% | +3 个 Provider，Token 计数优化 |
| **pi-tui** | 91% | 100% | +3 个组件，文档完善 |
| **pi-agent** | 100% | 100% | 保持稳定 |
| **pi-coding-agent** | 35% | 85% | TUI 完整集成，OAuth，扩展增强 |
| **整体** | 60% | **90%** | 生产可用 |

### 功能完整性对比

| 功能 | ITERATION-2 | ITERATION-3 目标 | 原版 |
|------|-------------|------------------|------|
| TUI 交互模式 | 简化版 | 完整版 | 100% |
| Provider 支持 | 5 个 | 8+ 个 | 22+ 个 |
| OAuth 认证 | 框架 | 完整可用 | 100% |
| 会话压缩 | 基础 | 完善 | 100% |
| 会话 Fork | 基础 | 完善 | 100% |
| 扩展系统 | 框架 | 可用 | 80% |
| 主题系统 | 无 | 基础 | 100% |
| 文档 | 部分 | 完整 | 100% |
| 测试覆盖 | 基础 | >70% | 80% |

### 用户使用场景验证

1. **新用户首次使用**
   - `pi` 启动进入完整 TUI 交互模式
   - `/login anthropic` 完成 OAuth 认证
   - 输入问题，获得流式回复

2. **长会话管理**
   - 会话自动压缩，保持上下文窗口健康
   - `/fork 10` 从早期消息创建分支
   - `/stats` 查看 Token 使用和成本

3. **高级用户**
   - 安装扩展增加自定义工具
   - 使用 `/theme dark` 切换主题
   - 通过 OpenRouter 访问多个后端模型

4. **开发者**
   - 完整的 API 文档
   - 扩展开发 SDK
   - 测试覆盖保证稳定性

---

## 风险与缓解措施

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| TUI 集成复杂度超预期 | Phase 1 延期 | 分阶段交付：先基础集成，后高级功能 |
| OAuth 安全审计 | Phase 3 延期 | 使用成熟库（oauth2、keyring），提前安全审查 |
| 扩展加载方案不确定 | Phase 5 延期 | 前期技术调研，准备备选方案（WASM/dylib） |
| 测试覆盖提升缓慢 | Phase 6 延期 | 在开发阶段同步编写测试，不堆积到最后 |

---

## 附录：新增依赖预估

| Crate | 用途 | Phase |
|-------|------|-------|
| `oauth2` | OAuth 2.0 流程 | Phase 3 |
| `keyring` | 系统密钥链访问 | Phase 3 |
| `tiktoken-rs` | OpenAI Token 计数 | Phase 4 |
| `wasmtime` | 扩展 WASM 运行时（可选） | Phase 5 |
| `criterion` | 性能测试 | Phase 6 |

---

*文档版本: 1.0*
*创建日期: 2026-04-10*
*基于: ITERATION-2 完成状态*
