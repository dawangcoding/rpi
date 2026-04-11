# ITERATION-5 开发计划：功能补全与生产发布

## 一、概述

### ITERATION-4 完成的成果

四次迭代完成了 pi-mono 项目的基础设施完善和前沿功能增强：

- **pi-ai**: 10 个 Provider 实现（Anthropic、OpenAI、Google、Mistral、Bedrock、Azure OpenAI、xAI、OpenRouter、Groq、Cerebras），Token 计数精确化（Mistral、Gemini），流式错误恢复（ResilientStream），指数退避重试策略
- **pi-tui**: TUI 框架保持 100% 完整，新增 Vim 编辑模式（Normal/Insert/Command/Visual），快捷键自定义系统
- **pi-agent**: Agent 运行时核心 100% 完成，保持稳定
- **pi-coding-agent**: 功能大幅完善，包括 Notebook 交互式代码执行工具、OAuth 认证完善（OpenAI、Google）、多格式配置支持（YAML/JSON/TOML/.env）、WASM 扩展系统（动态加载+沙箱+热重载）、权限系统
- **pi-mcp**: 新建 crate，实现 MCP 协议客户端完整支持（传输层、工具集成、Server 管理）

**ITERATION-4 结束时的状态：**
- **pi-ai**: 完成度 90%（核心 100%，Provider 90%，Token 计数 85%）
- **pi-tui**: 完成度 100%（核心 100%，组件 100%，Vim 模式 100%）
- **pi-agent**: 完成度 100%
- **pi-coding-agent**: 完成度 95%（工具 100%，交互 100%，扩展 80%，认证 85%）
- **pi-mcp**: 完成度 80%（核心 100%，协议 100%，工具集成 80%）
- **整体完成度约 88%**

### 已超越原版的功能

1. **Vim 编辑模式** - 完整的 Normal/Insert/Command/Visual 模式支持
2. **MCP 协议支持** - 完整的 Model Context Protocol 客户端实现
3. **权限系统** - 细粒度的工具执行权限控制（permissions.rs）
4. **WASM 扩展系统** - 动态加载 + 安全沙箱 + 热重载三位一体
5. **ResilientStream** - 流式响应中断自动恢复机制
6. **指数退避重试** - 智能重试策略避免频繁失败

### ITERATION-4 遗留的关键问题

1. **扩展系统事件钩子缺失（P0）** - 原版有 20+ 事件类型，rpi3 仅有 WASM 框架，事件分发系统待完成
2. **Google 生态 Provider 不完整（P1）** - 缺少 Google Vertex AI 和 Gemini CLI Provider
3. **其他 LLM Provider 缺失（P1）** - Minimax、Huggingface、Kimi、OpenCode 等 12 个 Provider 待实现
4. **OAuth 完整化（P1）** - 所有 Provider 的 OAuth 认证、Token 刷新统一流程
5. **技能系统缺失（P2）** - 预设技能库框架待实现
6. **RPC 模式缺失（P2）** - JSON-RPC 服务模式待实现
7. **设置管理 UI 缺失（P2）** - TUI 设置界面待实现
8. **测试覆盖率待提升（P2）** - 核心模块覆盖率目标 >70%
9. **系统密钥链集成（P2）** - macOS Keychain/Windows DPAPI/Linux Secret Service

**当前迭代目标：将整体完成度从 88% 提升至 100%，完成全部移植并超越原版，达到生产级发布标准。**

---

## 二、Phase 间依赖关系

```
Phase 1 (扩展系统完善)
    │
    ├──→ Phase 2 (Provider 补全) ────┐
    │                                 │
    ├──→ Phase 3 (OAuth 完整化) ──────┤
    │                                 │
    └──→ Phase 4 (功能特性增强) ──────┤
                                      │
                                      ↓
                              Phase 5 (质量保障与发布准备)
```

**依赖说明：**
- Phase 1 是最高优先级，扩展系统的事件分发机制是其他功能的基础
- Phase 2、3、4 可以在 Phase 1 完成后并行执行
- Phase 5 在所有 Phase 完成后进行，确保发布质量

---

## 三、Phase 详情

### Phase 1: 扩展系统完善（P0 最高优先级）

#### 目标
完善扩展系统的事件钩子机制，实现 20+ 事件类型的定义和分发，支持工具和命令的动态注册，建立事件优先级和取消机制。

#### 任务分解

| 属性 | 值 |
|------|-----|
| **功能名称** | Agent 生命周期事件系统 |
| **当前状态** | 框架代码，无事件分发 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 大 |
| **详细描述** | 实现 Agent 完整生命周期事件：<br>1. 定义 AgentLifecycleEvent 枚举（BeforeStart、AfterStart、BeforeEnd、AfterEnd）<br>2. 实现事件上下文传递（AgentContext、SessionContext）<br>3. 在 agent_loop.rs 中插入事件触发点<br>4. 支持事件处理器注册和注销<br>5. 事件执行错误隔离（单处理器失败不影响其他）<br>6. 异步事件处理支持 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Turn 生命周期事件系统 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 大 |
| **详细描述** | 实现 Turn 完整生命周期事件：<br>1. 定义 TurnLifecycleEvent 枚举（TurnStart、TurnEnd、TurnError）<br>2. 传递 TurnContext（消息历史、当前输入、模型配置）<br>3. 支持 Turn 前置处理（修改输入内容）<br>4. 支持 Turn 后置处理（修改输出内容）<br>5. 实现 Turn 取消机制（前置处理可终止 Turn）<br>6. Turn 统计信息收集（耗时、Token 使用） |

| 属性 | 值 |
|------|-----|
| **功能名称** | 消息事件系统 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 中 |
| **详细描述** | 实现消息级别事件：<br>1. 定义 MessageEvent 枚举（MessageStart、MessageChunk、MessageEnd、MessageError）<br>2. 支持消息内容拦截和修改<br>3. 实现消息渲染前事件（修改 Markdown 内容）<br>4. 实现消息渲染后事件（添加自定义样式）<br>5. 消息元数据扩展（添加自定义字段）<br>6. 消息过滤和路由 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 工具事件系统 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 中 |
| **详细描述** | 实现工具调用完整事件：<br>1. 定义 ToolEvent 枚举（BeforeToolCall、AfterToolCall、ToolExecutionStart、ToolExecutionEnd、ToolError）<br>2. 支持工具参数拦截和修改<br>3. 支持工具结果拦截和修改<br>4. 工具执行超时监控<br>5. 工具执行统计（调用次数、成功率、耗时）<br>6. 工具权限检查事件 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 命令事件系统 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 中 |
| **详细描述** | 实现 Slash 命令事件：<br>1. 定义 CommandEvent 枚举（BeforeCommandExecute、AfterCommandExecute、CommandError）<br>2. 支持命令参数解析前拦截<br>3. 支持命令结果后处理<br>4. 命令执行权限检查<br>5. 自定义命令注册事件<br>6. 命令帮助信息扩展 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 事件优先级和取消机制 |
| **当前状态** | 未实现 |
| **TS 参考** | 事件系统设计模式 |
| **预估工作量** | 中 |
| **详细描述** | 实现事件高级机制：<br>1. 事件处理器优先级（High、Normal、Low）<br>2. 事件传播控制（Continue、StopPropagation）<br>3. 事件取消令牌（CancellationToken）<br>4. 异步事件超时控制<br>5. 事件处理器顺序保证<br>6. 事件链式处理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 工具动态注册机制 |
| **当前状态** | 基础框架 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 大 |
| **详细描述** | 完善扩展工具注册：<br>1. 定义 ExtensionTool trait（名称、描述、参数 Schema、执行函数）<br>2. 实现工具注册表（ToolRegistry）<br>3. 支持运行时动态注册/注销<br>4. 工具参数 JSON Schema 验证<br>5. 工具执行上下文传递（ExtensionContext）<br>6. 工具错误处理和回退机制 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 命令动态注册机制 |
| **当前状态** | 基础框架 |
| **TS 参考** | `packages/coding-agent/src/core/extensions/types.ts` |
| **预估工作量** | 大 |
| **详细描述** | 完善扩展命令注册：<br>1. 定义 ExtensionCommand trait（名称、描述、参数、执行函数）<br>2. 实现命令注册表（CommandRegistry）<br>3. 支持运行时动态注册/注销<br>4. 命令参数解析和验证<br>5. 命令自动完成集成<br>6. 命令帮助信息生成 |

#### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/core/extensions/types.rs` - 扩展事件类型定义
- `crates/pi-coding-agent/src/core/extensions/api.rs` - ExtensionContext 完善
- `crates/pi-coding-agent/src/core/extensions/runner.rs` - ExtensionManager 事件分发
- `crates/pi-coding-agent/src/core/agent_loop.rs` - 事件触发点插入
- `crates/pi-coding-agent/src/modes/interactive.rs` - 命令事件集成

**新增文件：**
- `crates/pi-coding-agent/src/core/extensions/events.rs` - 事件系统核心实现
- `crates/pi-coding-agent/src/core/extensions/registry.rs` - 工具和命令注册表
- `crates/pi-coding-agent/src/core/extensions/dispatcher.rs` - 事件分发器

**依赖文件：**
- `crates/pi-coding-agent/src/core/tools/mod.rs` - 工具系统集成
- `crates/pi-coding-agent/src/core/extensions/loader.rs` - 扩展加载集成

#### 验证标准

1. 扩展可以注册 Agent 生命周期事件处理器
2. 扩展可以注册 Turn 生命周期事件处理器
3. 扩展可以注册消息事件处理器并修改消息内容
4. 扩展可以注册工具事件处理器并拦截工具调用
5. 扩展可以注册命令事件处理器并拦截命令执行
6. 事件处理器支持优先级配置
7. 事件可以取消后续处理器执行
8. 扩展可以动态注册自定义工具
9. 扩展可以动态注册自定义 Slash 命令
10. 事件系统性能开销 < 5%

#### 预估工作量

**总计：3-4 周**（1 人全职）

---

### Phase 2: Provider 补全（P1）

#### 目标
补全 Google 生态 Provider（Vertex AI、Gemini CLI），实现其他 12 个 LLM Provider（Minimax、Huggingface、Kimi、OpenCode 等）。

#### 任务分解

| 属性 | 值 |
|------|-----|
| **功能名称** | Google Vertex AI Provider |
| **当前状态** | 未实现 |
| **TS 参考** | Google Cloud Vertex AI 文档 |
| **预估工作量** | 大 |
| **详细描述** | 实现 Google Vertex AI Provider：<br>1. 支持 Google Cloud 认证（Service Account、OAuth）<br>2. 实现 Vertex AI endpoint 调用（https://{region}-aiplatform.googleapis.com）<br>3. 支持 Gemini 模型在 Vertex AI 上的部署<br>4. 支持 Claude 模型在 Vertex AI 上的部署<br>5. 实现流式响应（Server-Sent Events）<br>6. 错误处理（Google Cloud 特定错误码）<br>7. 区域和项目配置支持 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Google Gemini CLI Provider |
| **当前状态** | 未实现 |
| **TS 参考** | Gemini CLI API 文档 |
| **预估工作量** | 中 |
| **详细描述** | 实现 Google Gemini CLI 专用 Provider：<br>1. 支持 Gemini CLI 认证方式<br>2. 实现 Gemini CLI 特定 API 端点<br>3. 支持 Gemini 1.5 Pro/Flash 系列模型<br>4. 支持多模态输入（文本、图像、音频）<br>5. 实现流式响应<br>6. 与现有 Google Provider 区分定位 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Minimax Provider |
| **当前状态** | 未实现 |
| **TS 参考** | Minimax API 文档（OpenAI 兼容） |
| **预估工作量** | 小 |
| **详细描述** | 实现 Minimax Provider：<br>1. 复用 OpenAI 兼容接口逻辑<br>2. 配置 Minimax base URL（https://api.minimax.chat）<br>3. 支持 Minimax 模型系列（abab6、abab5.5）<br>4. 实现流式响应<br>5. 注册到模型注册表 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Huggingface Provider |
| **当前状态** | 未实现 |
| **TS 参考** | Huggingface Inference API 文档 |
| **预估工作量** | 中 |
| **详细描述** | 实现 Huggingface Inference Provider：<br>1. 支持 Huggingface Token 认证<br>2. 实现 Inference API 调用<br>3. 支持文本生成模型（Llama、Mistral、Qwen 等）<br>4. 支持模型路由（自动选择部署）<br>5. 实现流式响应<br>6. 错误处理（速率限制、模型加载） |

| 属性 | 值 |
|------|-----|
| **功能名称** | Moonshot (Kimi) Provider |
| **当前状态** | 未实现 |
| **TS 参考** | Moonshot API 文档（OpenAI 兼容） |
| **预估工作量** | 小 |
| **详细描述** | 实现 Moonshot Provider：<br>1. 复用 OpenAI 兼容接口逻辑<br>2. 配置 Moonshot base URL（https://api.moonshot.cn）<br>3. 支持 Kimi 模型系列（kimi-k1、moonshot-v1）<br>4. 实现流式响应<br>5. 注册到模型注册表 |

| 属性 | 值 |
|------|-----|
| **功能名称** | OpenCode Provider |
| **当前状态** | 未实现 |
| **TS 参考** | OpenCode API 文档 |
| **预估工作量** | 小 |
| **详细描述** | 实现 OpenCode Provider：<br>1. 复用 OpenAI 兼容接口逻辑<br>2. 配置 OpenCode base URL<br>3. 支持 OpenCode 模型系列<br>4. 实现流式响应<br>5. 注册到模型注册表 |

| 属性 | 值 |
|------|-----|
| **功能名称** | GitHub Copilot Provider |
| **当前状态** | 未实现 |
| **TS 参考** | GitHub Copilot API 文档 |
| **预估工作量** | 中 |
| **详细描述** | 实现 GitHub Copilot Provider：<br>1. 支持 GitHub Token 认证<br>2. 实现 Copilot Chat API 调用<br>3. 支持 Copilot 模型系列（GPT-4、Claude）<br>4. 处理 Copilot 特定请求格式<br>5. 实现流式响应 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 其他 Provider 批量实现 |
| **当前状态** | 未实现 |
| **TS 参考** | 各 Provider API 文档 |
| **预估工作量** | 中 |
| **详细描述** | 批量实现剩余 OpenAI 兼容 Provider：<br>1. DeepSeek Provider<br>2. Qwen (通义千问) Provider<br>3. ChatGLM Provider<br>4. Baichuan Provider<br>5. Stepfun Provider<br>6. 360 AI Provider<br>7. 复用 OpenAI 兼容逻辑，主要配置差异 |

#### 涉及文件

**新增文件：**
- `crates/pi-ai/src/providers/vertex_ai.rs` - Google Vertex AI Provider
- `crates/pi-ai/src/providers/gemini_cli.rs` - Google Gemini CLI Provider
- `crates/pi-ai/src/providers/minimax.rs` - Minimax Provider
- `crates/pi-ai/src/providers/huggingface.rs` - Huggingface Provider
- `crates/pi-ai/src/providers/moonshot.rs` - Moonshot Provider
- `crates/pi-ai/src/providers/opencode.rs` - OpenCode Provider
- `crates/pi-ai/src/providers/github_copilot.rs` - GitHub Copilot Provider
- `crates/pi-ai/src/providers/deepseek.rs` - DeepSeek Provider
- `crates/pi-ai/src/providers/qwen.rs` - Qwen Provider
- `crates/pi-ai/src/providers/chatglm.rs` - ChatGLM Provider

**修改文件：**
- `crates/pi-ai/src/providers/mod.rs` - 注册新 Provider
- `crates/pi-ai/src/models.rs` - 添加新模型定义
- `crates/pi-ai/src/types.rs` - 扩展 Api 和 Provider 枚举

#### 验证标准

1. Google Vertex AI Provider 可通过 Service Account 认证调用
2. Google Gemini CLI Provider 支持多模态输入
3. Minimax Provider 可调用 abab6 模型
4. Huggingface Provider 可调用 Inference API
5. Moonshot Provider 可调用 kimi-k1 模型
6. 所有新 Provider 支持流式响应
7. 模型列表命令显示所有新添加的模型
8. 配置文件中可配置新 Provider 的 API Key
9. Provider 错误处理符合统一规范

#### 预估工作量

**总计：2-3 周**（1 人全职）

---

### Phase 3: OAuth 完整化（P1）

#### 目标
实现统一的 OAuth 流程框架，完成所有 Provider 的特定 OAuth 实现，完善 Token 刷新和错误恢复机制，集成系统密钥链。

#### 任务分解

| 属性 | 值 |
|------|-----|
| **功能名称** | 统一 OAuth 框架完善 |
| **当前状态** | 基础框架（Anthropic、OpenAI、Google 部分实现） |
| **TS 参考** | `packages/coding-agent/src/core/auth/` |
| **预估工作量** | 大 |
| **详细描述** | 完善统一 OAuth 流程框架：<br>1. 抽象 OAuthProvider trait（授权 URL 生成、Token 交换、刷新）<br>2. 实现 PKCE 支持（所有 Provider）<br>3. 统一错误处理（用户拒绝、超时、网络错误）<br>4. 状态管理（防止 CSRF）<br>5. Scope 配置标准化<br>6. 回调服务器统一处理 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 系统密钥链集成 |
| **当前状态** | 文件存储（未加密） |
| **TS 参考** | `keyring` crate |
| **预估工作量** | 中 |
| **详细描述** | 集成系统密钥链存储 Token：<br>1. macOS Keychain 集成<br>2. Windows DPAPI/Credential Manager 集成<br>3. Linux Secret Service/Keyring 集成<br>4. 自动检测系统类型<br>5. 密钥链访问错误处理<br>6. 降级方案（文件加密存储） |

| 属性 | 值 |
|------|-----|
| **功能名称** | Token 刷新统一机制 |
| **当前状态** | 基础实现 |
| **TS 参考** | OAuth 2.0 Refresh Token 流程 |
| **预估工作量** | 中 |
| **详细描述** | 完善 Token 自动刷新：<br>1. 统一刷新调度器（RefreshScheduler）<br>2. 过期前自动刷新（默认 5 分钟前）<br>3. 刷新失败重试机制<br>4. 刷新失败通知（提示重新登录）<br>5. 并发刷新控制（避免重复刷新）<br>6. 刷新日志记录 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Azure OpenAI OAuth 支持 |
| **当前状态** | 仅 API Key |
| **TS 参考** | Azure AD OAuth 文档 |
| **预估工作量** | 中 |
| **详细描述** | 实现 Azure OpenAI OAuth：<br>1. Azure AD 授权端点配置<br>2. 支持企业应用注册<br>3. 支持 Managed Identity<br>4. Azure 特定 Scope 配置<br>5. Token 缓存和刷新 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Mistral OAuth 支持 |
| **当前状态** | 未实现 |
| **TS 参考** | Mistral OAuth 文档 |
| **预估工作量** | 小 |
| **详细描述** | 实现 Mistral OAuth：<br>1. Mistral 授权端点配置<br>2. 支持 Mistral 控制台应用<br>3. Scope 配置<br>4. Token 刷新 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 其他 Provider OAuth 支持 |
| **当前状态** | 未实现 |
| **TS 参考** | 各 Provider OAuth 文档 |
| **预估工作量** | 中 |
| **详细描述** | 实现其他 Provider OAuth：<br>1. GitHub Copilot OAuth<br>2. Huggingface OAuth<br>3. OpenRouter OAuth<br>4. 统一配置格式<br>5. 统一登录命令 |

#### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/core/auth/mod.rs` - OAuth 框架完善
- `crates/pi-coding-agent/src/core/auth/oauth_server.rs` - 回调服务器完善
- `crates/pi-coding-agent/src/core/auth/providers.rs` - Provider OAuth 配置扩展
- `crates/pi-coding-agent/src/core/auth/token_storage.rs` - Token 存储和刷新

**新增文件：**
- `crates/pi-coding-agent/src/core/auth/keychain.rs` - 系统密钥链集成
- `crates/pi-coding-agent/src/core/auth/refresh.rs` - Token 刷新调度器

**依赖文件：**
- `crates/pi-coding-agent/src/config.rs` - 认证配置集成
- `crates/pi-coding-agent/src/modes/interactive.rs` - /login 命令集成

#### 验证标准

1. `/login` 命令支持所有 Provider 的 OAuth 流程
2. Token 安全存储在系统密钥链
3. Token 过期前 5 分钟自动刷新
4. 刷新失败时提示用户重新登录
5. 支持 Azure AD 企业认证
6. 支持 GitHub Copilot OAuth
7. 密钥链访问失败时降级到加密文件
8. OAuth 状态防止 CSRF 攻击
9. 登录状态持久化（重启后保持）

#### 预估工作量

**总计：2-3 周**（1 人全职）

---

### Phase 4: 功能特性增强（P2）

#### 目标
实现技能系统框架、RPC 模式（JSON-RPC 服务）、设置管理 TUI 界面，提升用户体验。

#### 任务分解

| 属性 | 值 |
|------|-----|
| **功能名称** | 技能系统框架 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/core/skills/` |
| **预估工作量** | 大 |
| **详细描述** | 实现预设技能库框架：<br>1. 定义 Skill 结构（名称、描述、提示词模板、参数）<br>2. 实现技能注册表（SkillRegistry）<br>3. 支持技能参数注入<br>4. 技能分类和标签<br>5. 技能搜索和推荐<br>6. 技能导入/导出<br>7. 内置技能库（代码审查、重构、文档生成等） |

| 属性 | 值 |
|------|-----|
| **功能名称** | RPC 服务模式 |
| **当前状态** | 未实现 |
| **TS 参考** | JSON-RPC 2.0 规范 |
| **预估工作量** | 大 |
| **详细描述** | 实现 JSON-RPC 服务模式：<br>1. JSON-RPC 2.0 协议实现<br>2. HTTP 服务器（std 或 tokio）<br>3. 方法注册和路由<br>4. 请求/响应序列化<br>5. 错误处理（标准 JSON-RPC 错误码）<br>6. 通知支持（单向调用）<br>7. 批处理支持<br>8. 认证中间件 |

| 属性 | 值 |
|------|-----|
| **功能名称** | RPC 方法实现 |
| **当前状态** | 未实现 |
| **TS 参考** | API 设计 |
| **预估工作量** | 中 |
| **详细描述** | 实现核心 RPC 方法：<br>1. `initialize` - 初始化会话<br>2. `sendMessage` - 发送消息<br>3. `getMessages` - 获取消息历史<br>4. `executeTool` - 执行工具<br>5. `getTools` - 获取可用工具<br>6. `setModel` - 切换模型<br>7. `getModels` - 获取可用模型<br>8. `compactSession` - 压缩会话 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 设置管理 TUI 界面 |
| **当前状态** | 未实现 |
| **TS 参考** | `packages/coding-agent/src/modes/settings/` |
| **预估工作量** | 中 |
| **详细描述** | 实现 TUI 设置管理界面：<br>1. 设置分类导航（General、Provider、Editor、Extensions）<br>2. 设置项编辑器（布尔、字符串、数字、枚举、列表）<br>3. 设置值验证<br>4. 设置变更实时预览<br>5. 设置导入/导出<br>6. 恢复默认设置<br>7. 与 SettingsList 组件集成 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 设置热重载 |
| **当前状态** | 未实现 |
| **TS 参考** | 文件监控 |
| **预估工作量** | 小 |
| **详细描述** | 实现配置文件热重载：<br>1. 文件系统监控（notify crate）<br>2. 配置文件变更检测<br>3. 自动重载配置<br>4. 配置验证（错误时保持原配置）<br>5. 重载事件通知 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 命令行模式增强 |
| **当前状态** | 基础实现 |
| **TS 参考** | CLI 设计 |
| **预估工作量** | 小 |
| **详细描述** | 增强命令行模式功能：<br>1. 支持从文件读取提示词<br>2. 支持输出到文件<br>3. 支持非交互式批量处理<br>4. 支持 JSON 输出格式<br>5. 退出码标准化 |

#### 涉及文件

**主要修改：**
- `crates/pi-coding-agent/src/core/mod.rs` - 技能系统集成
- `crates/pi-coding-agent/src/modes/interactive.rs` - 设置界面集成
- `crates/pi-coding-agent/src/cli/mod.rs` - 命令行增强

**新增文件：**
- `crates/pi-coding-agent/src/core/skills/mod.rs` - 技能系统模块
- `crates/pi-coding-agent/src/core/skills/registry.rs` - 技能注册表
- `crates/pi-coding-agent/src/core/skills/builtin.rs` - 内置技能
- `crates/pi-coding-agent/src/rpc/mod.rs` - RPC 服务模块
- `crates/pi-coding-agent/src/rpc/server.rs` - JSON-RPC 服务器
- `crates/pi-coding-agent/src/rpc/methods.rs` - RPC 方法实现
- `crates/pi-coding-agent/src/modes/settings.rs` - 设置管理界面

**依赖文件：**
- `crates/pi-tui/src/components/settings_list.rs` - 设置列表组件

#### 验证标准

1. `/skills` 命令打开技能选择界面
2. 技能可以参数化并应用到当前会话
3. RPC 服务器启动后可通过 HTTP 调用
4. 所有 RPC 方法符合 JSON-RPC 2.0 规范
5. `/settings` 命令打开 TUI 设置界面
6. 设置变更实时生效
7. 配置文件修改后自动热重载
8. CLI 支持 `--input-file` 和 `--output-file` 参数
9. 非交互式模式返回标准化退出码

#### 预估工作量

**总计：2-3 周**（1 人全职）

---

### Phase 5: 质量保障与发布准备（P2）

#### 目标
提升核心模块测试覆盖率至 70% 以上，实现 E2E 集成测试，清理编译警告，补全 Rustdoc 文档，完成性能基准测试。

#### 任务分解

| 属性 | 值 |
|------|-----|
| **功能名称** | pi-ai 测试覆盖提升 |
| **当前状态** | 基础测试 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 提升 pi-ai 模块测试覆盖率：<br>1. Provider 单元测试（模拟 HTTP 响应）<br>2. Token 计数器测试<br>3. 流式响应解析测试<br>4. 重试逻辑测试<br>5. 错误处理测试<br>6. 目标覆盖率 > 70% |

| 属性 | 值 |
|------|-----|
| **功能名称** | pi-tui 测试覆盖提升 |
| **当前状态** | 基础测试 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 提升 pi-tui 模块测试覆盖率：<br>1. 组件渲染测试<br>2. 编辑器功能测试（Emacs/Vim 模式）<br>3. 键盘处理测试<br>4. 差分渲染测试<br>5. 主题系统测试<br>6. 目标覆盖率 > 70% |

| 属性 | 值 |
|------|-----|
| **功能名称** | pi-coding-agent 测试覆盖提升 |
| **当前状态** | 基础测试 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 提升 pi-coding-agent 模块测试覆盖率：<br>1. 工具集成测试<br>2. 会话管理测试<br>3. 扩展系统测试<br>4. OAuth 流程测试（模拟）<br>5. 压缩和 Fork 测试<br>6. 目标覆盖率 > 70% |

| 属性 | 值 |
|------|-----|
| **功能名称** | E2E 集成测试 |
| **当前状态** | 未实现 |
| **TS 参考** | N/A |
| **预估工作量** | 大 |
| **详细描述** | 实现端到端集成测试：<br>1. 完整会话流程测试<br>2. 工具调用链测试<br>3. 多 Provider 切换测试<br>4. 扩展加载测试<br>5. 配置热重载测试<br>6. 使用模拟 Provider 避免外部依赖 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 编译警告清零 |
| **当前状态** | 存在 dead_code 和 unused 警告 |
| **TS 参考** | N/A |
| **预估工作量** | 中 |
| **详细描述** | 消除所有编译警告：<br>1. 清理 dead_code 警告<br>2. 清理 unused 警告<br>3. 清理 clippy 警告（所有级别）<br>4. 确保 `cargo check` 零警告<br>5. 确保 `cargo clippy -- -D warnings` 通过<br>6. 添加 CI 检查 |

| 属性 | 值 |
|------|-----|
| **功能名称** | Rustdoc 文档补全 |
| **当前状态** | 部分文档 |
| **TS 参考** | N/A |
| **预估工作量** | 中 |
| **详细描述** | 为公共 API 补充完整文档：<br>1. pi-ai：所有 Provider、Model、TokenCounter<br>2. pi-tui：所有组件、TUI 结构体<br>3. pi-agent：Agent、AgentLoop、所有类型<br>4. pi-coding-agent：工具、会话、扩展 API<br>5. pi-mcp：MCP 客户端、协议类型<br>6. 文档示例代码 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 性能基准测试 |
| **当前状态** | 未实现 |
| **TS 参考** | `criterion` crate |
| **预估工作量** | 中 |
| **详细描述** | 实现性能基准测试：<br>1. Token 计数性能基准<br>2. Markdown 渲染性能基准<br>3. 编辑器操作性能基准<br>4. 流式响应处理性能基准<br>5. 扩展加载性能基准<br>6. 生成性能报告 |

| 属性 | 值 |
|------|-----|
| **功能名称** | 发布准备 |
| **当前状态** | 未实现 |
| **TS 参考** | Rust 发布流程 |
| **预估工作量** | 小 |
| **详细描述** | 完成发布前准备工作：<br>1. 版本号更新（遵循 SemVer）<br>2. CHANGELOG 编写<br>3. README 完善<br>4. 安装脚本编写<br>5. 发布检查清单 |

#### 涉及文件

**主要修改：**
- `crates/pi-ai/tests/` - 补充测试
- `crates/pi-tui/tests/` - 补充测试
- `crates/pi-coding-agent/tests/` - 补充测试
- 各 crate 的 `lib.rs` - 补充文档
- `Cargo.toml` - 版本号更新

**新增文件：**
- `benches/` - 性能基准测试目录
- `benches/token_counter.rs` - Token 计数基准
- `benches/markdown_render.rs` - Markdown 渲染基准
- `tests/e2e/` - E2E 测试目录
- `CHANGELOG.md` - 变更日志

#### 验证标准

1. `cargo test` 通过率 100%
2. 核心模块代码覆盖率 > 70%
3. `cargo clippy -- -D warnings` 零警告
4. `cargo doc` 生成完整文档无警告
5. 所有公共 API 有文档注释和示例
6. 基准测试可正常运行并生成报告
7. E2E 测试覆盖主要用户场景
8. 版本号符合 SemVer 规范
9. CHANGELOG 记录所有重要变更

#### 预估工作量

**总计：2-3 周**（1 人全职）

---

## 四、总体时间线

| Phase | 名称 | 预估时间 | 累计时间 |
|-------|------|----------|----------|
| Phase 1 | 扩展系统完善 | 3-4 周 | 3-4 周 |
| Phase 2 | Provider 补全 | 2-3 周 | 5-7 周 |
| Phase 3 | OAuth 完整化 | 2-3 周 | 5-7 周 |
| Phase 4 | 功能特性增强 | 2-3 周 | 5-7 周 |
| Phase 5 | 质量保障与发布准备 | 2-3 周 | 7-10 周 |

**总计：7-10 周**（约 2-2.5 个月，1 人全职）

**并行优化：**
- Phase 2、3、4 可在 Phase 1 完成后并行开发
- 如果 2-3 人协作，可缩短至 **5-7 周**

---

## 五、完成后的预期状态

### 各模块完成度目标

| 模块 | 当前完成度 | 目标完成度 | 关键改进 |
|------|-----------|-----------|----------|
| **pi-ai** | 90% | 100% | +12 个 Provider，Token 计数完善 |
| **pi-tui** | 100% | 100% | 保持稳定 |
| **pi-agent** | 100% | 100% | 保持稳定 |
| **pi-coding-agent** | 95% | 100% | 扩展事件系统、技能系统、RPC 模式 |
| **pi-mcp** | 80% | 100% | 完善工具集成 |
| **整体** | 88% | **100%** | 生产级发布标准 |

### 功能完整性对比

| 功能 | ITERATION-4 | ITERATION-5 目标 | 原版 |
|------|-------------|------------------|------|
| Provider 支持 | 10 个 | 22+ 个 | 22+ 个 |
| OAuth 认证 | 部分 Provider | 完整（所有支持 Provider） | 100% |
| 扩展系统 | WASM 框架 | 完整（20+ 事件类型） | 100% |
| 技能系统 | 无 | 完整框架 | 100% |
| RPC 模式 | 无 | 完整 JSON-RPC | 90% |
| 设置管理 UI | 无 | 完整 TUI 界面 | 100% |
| 系统密钥链 | 无 | 完整支持 | 100% |
| 测试覆盖率 | ~50% | >70% | 80% |
| 文档完整性 | 部分 | 完整 Rustdoc | 100% |

### 用户使用场景验证

1. **扩展开发**
   - 开发者可以注册自定义事件处理器
   - 扩展可以动态注册工具和命令
   - 扩展可以通过事件修改消息内容

2. **多 Provider 选择**
   - 用户可以选择 22+ 个 Provider
   - 包括 Google Vertex AI、Kimi、DeepSeek 等
   - 所有 Provider 支持 OAuth 认证

3. **技能应用**
   - `/skills` 查看可用技能库
   - 选择"代码审查"技能自动应用提示词模板
   - 自定义技能导入和分享

4. **RPC 集成**
   - 启动 RPC 模式 `pi --rpc`
   - 通过 HTTP API 发送消息
   - 集成到第三方工具（如编辑器插件）

5. **设置管理**
   - `/settings` 打开 TUI 设置界面
   - 实时修改配置并预览效果
   - 导出配置分享给其他用户

6. **安全认证**
   - Token 安全存储在系统密钥链
   - 自动刷新避免频繁登录
   - 支持企业级 Azure AD 认证

7. **生产部署**
   - 完整文档和示例
   - 稳定 API 保证
   - 性能基准报告

8. **质量保证**
   - 测试覆盖率 >70%
   - 零编译警告
   - E2E 测试保障核心流程

---

## 六、风险与缓解措施

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 扩展事件系统复杂度超预期 | Phase 1 延期 | 分阶段实现：先核心事件，后高级功能 |
| Provider API 变更 | Phase 2 延期 | 使用 OpenAI 兼容格式，降低变更影响 |
| OAuth 平台审核 | Phase 3 延期 | 优先实现已审核平台，其他使用 API Key |
| 系统密钥链兼容性问题 | Phase 3 延期 | 提供降级方案（文件加密存储） |
| 技能系统设计争议 | Phase 4 延期 | 参考原版设计，保持兼容性 |
| 测试覆盖率提升缓慢 | Phase 5 延期 | 在开发阶段同步编写测试 |
| 性能基准不达标 | Phase 5 延期 | 提前进行性能分析，预留优化时间 |

---

## 七、新增依赖预估

| Crate | 用途 | Phase |
|-------|------|-------|
| `keyring` | 系统密钥链访问 | Phase 3 |
| `jsonrpsee` | JSON-RPC 服务器 | Phase 4 |
| `criterion` | 性能基准测试 | Phase 5 |
| `cargo-tarpaulin` | 代码覆盖率检测 | Phase 5 |
| `mockall` | 测试模拟 | Phase 5 |
| `tempfile` | 测试临时文件 | Phase 5 |
| `wiremock` | HTTP 模拟测试 | Phase 5 |

---

*文档版本: 1.0*
*创建日期: 2026-04-11*
*基于: ITERATION-4 完成状态*
