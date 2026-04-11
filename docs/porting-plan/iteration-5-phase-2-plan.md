# ITERATION-5 Phase 2: Provider 补全

## 概述

补全所有缺失的 LLM Provider，包括 Google 生态（Vertex AI、Gemini CLI）、GitHub Copilot，以及 6 个 OpenAI 兼容薄包装器 Provider（Minimax、Huggingface、Moonshot、OpenCode、DeepSeek、Qwen）。Provider 总数从 10 增至 19，新增 13+ 模型定义。

## 任务分解

### 核心类型扩展（1 个任务）

| 任务 | 说明 |
|------|------|
| types.rs / models.rs 基础类型扩展 | 新增 Api/Provider 枚举值、模型定义、API Key 环境变量映射 |

### Provider 实现（3 组任务）

| 任务 | Provider | 实现方式 |
|------|----------|----------|
| 薄包装器 Provider 批量实现 | Minimax, Huggingface, Moonshot, OpenCode | 内嵌 `OpenAiProvider` 委托调用 |
| DeepSeek + Qwen Provider | DeepSeek, Qwen | 内嵌 `OpenAiProvider`，新增 Api/Provider 枚举值 |
| 独立实现 Provider | Google Vertex AI, Gemini CLI, GitHub Copilot | 各有独立认证和 API 格式 |

---

## 依赖关系与执行顺序

```
Task 1 (types.rs/models.rs 基础类型) ─────────┐
                                                │
Task 2 (薄包装器: Minimax/HF/Moonshot/OC) ←────┤
                                                │
Task 3 (DeepSeek + Qwen) ←─────────────────────┤
                                                │
Task 4 (Vertex AI + Gemini CLI + Copilot) ←─────┘
         │
         ↓
Task 5 (mod.rs/lib.rs 注册合并 + 验证)
```

---

## Task 1: 基础类型和模型扩展

**范围**: 扩展共享类型文件，为所有新 Provider 提供基础

**修改文件**:
- `crates/pi-ai/src/types.rs`:
  - `Api` 枚举新增 `DeepSeek`, `Qwen` 变体
  - `Provider` 枚举新增 `DeepSeek`, `Qwen` 变体
  - 其余 Provider 复用已预留枚举值（Minimax, Huggingface, Opencode, KimiCoding, GithubCopilot, GoogleVertex, GoogleGeminiCli）
- `crates/pi-ai/src/models.rs`:
  - 新增 13+ 模型定义（DeepSeek V3/R1, Qwen Max/Plus, Minimax abab6.5s, Moonshot v1-8k/128k, HF Llama, Opencode, Vertex AI Gemini, Copilot GPT-4o/Claude）
  - `get_api_key_from_env()` 和 `get_api_key_env_var()` 覆盖所有新 Provider

**验证**: 编译通过，无类型冲突

---

## Task 2: 薄包装器 Provider（Minimax/Huggingface/Moonshot/OpenCode）

**范围**: 4 个 OpenAI 兼容 Provider，统一使用薄包装器模式

**新增文件**:
- `crates/pi-ai/src/providers/minimax.rs` — base URL: `https://api.minimax.chat/v1`
- `crates/pi-ai/src/providers/huggingface.rs` — base URL: `https://api-inference.huggingface.co/v1`
- `crates/pi-ai/src/providers/moonshot.rs` — base URL: `https://api.moonshot.cn/v1`
- `crates/pi-ai/src/providers/opencode.rs` — base URL: `https://api.opencode.ai/v1`

**设计模式**:
```rust
pub struct MinimaxProvider { inner: OpenAiProvider }
impl ApiProvider for MinimaxProvider {
    fn api(&self) -> Api { Api::Other("minimax".to_string()) }
    async fn stream(&self, context, model, options) -> ... {
        self.inner.stream(context, model, options).await
    }
}
```

**验证**: 各 Provider 构造 + 编译通过

---

## Task 3: DeepSeek + Qwen Provider

**范围**: 2 个需要新枚举值的薄包装器 Provider

**新增文件**:
- `crates/pi-ai/src/providers/deepseek.rs` — base URL: `https://api.deepseek.com/v1`
- `crates/pi-ai/src/providers/qwen.rs` — base URL: `https://dashscope.aliyuncs.com/compatible-mode/v1`

**设计**: 与 Task 2 相同的薄包装器模式，但使用独立的 `Api::DeepSeek` / `Api::Qwen` 和 `Provider::DeepSeek` / `Provider::Qwen` 枚举值

**验证**: 编译通过，模型正确注册

---

## Task 4: 独立实现 Provider（Vertex AI / Gemini CLI / GitHub Copilot）

**范围**: 3 个有独立认证和 API 格式的 Provider

**新增文件**:
- `crates/pi-ai/src/providers/vertex_ai.rs`:
  - 独立实现（~931 行），非薄包装器
  - URL 格式: `https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/google/models/{model}:streamGenerateContent`
  - Bearer token 认证
  - 复用 Google Content 消息格式（JSON 手动构建）
- `crates/pi-ai/src/providers/gemini_cli.rs`:
  - 委托 `GoogleProvider`，自定义 base URL
  - 面向 Gemini CLI 场景
- `crates/pi-ai/src/providers/github_copilot.rs`:
  - 两步认证：GitHub Token → Copilot 短期 token → OpenAI 兼容调用
  - 使用 `Api::Other("github-copilot".to_string())` 避免与 OpenAI Provider 冲突
  - Token 缓存（`Arc<RwLock<Option<CopilotToken>>>`）

**验证**: 编译通过，需要真实 API 的测试标记 `#[ignore]`

---

## Task 5: Provider 注册合并与验证

**范围**: 将所有新 Provider 注册到全局注册表，全量验证

**修改文件**:
- `crates/pi-ai/src/providers/mod.rs` — 声明 9 个新模块 + pub use 导出
- `crates/pi-ai/src/lib.rs` — `init_providers()` 注册 19 个 Provider

**验证**: `cargo check --workspace` + `cargo test --workspace` 全通过

---

## 新增文件清单

| 文件 | 说明 | 行数（约） |
|------|------|-----------|
| `crates/pi-ai/src/providers/minimax.rs` | Minimax 薄包装器 | ~150 |
| `crates/pi-ai/src/providers/huggingface.rs` | Huggingface 薄包装器 | ~150 |
| `crates/pi-ai/src/providers/moonshot.rs` | Moonshot/Kimi 薄包装器 | ~150 |
| `crates/pi-ai/src/providers/opencode.rs` | OpenCode 薄包装器 | ~150 |
| `crates/pi-ai/src/providers/deepseek.rs` | DeepSeek 薄包装器 | ~200 |
| `crates/pi-ai/src/providers/qwen.rs` | Qwen 薄包装器 | ~200 |
| `crates/pi-ai/src/providers/vertex_ai.rs` | Google Vertex AI（独立实现） | ~931 |
| `crates/pi-ai/src/providers/gemini_cli.rs` | Gemini CLI（委托 GoogleProvider） | ~200 |
| `crates/pi-ai/src/providers/github_copilot.rs` | GitHub Copilot（两步认证） | ~350 |

## 修改文件清单

| 文件 | 改动说明 |
|------|----------|
| `crates/pi-ai/src/types.rs` | 新增 `Api::DeepSeek`, `Api::Qwen`, `Provider::DeepSeek`, `Provider::Qwen` |
| `crates/pi-ai/src/models.rs` | 新增 13+ 模型定义 + API key 环境变量支持 |
| `crates/pi-ai/src/providers/mod.rs` | 注册 9 个新 Provider 模块（总计 19 个） |
| `crates/pi-ai/src/lib.rs` | `init_providers()` 注册 19 个 Provider |
| `crates/pi-ai/tests/provider_integration_tests.rs` | 模型 ID 格式验证修复（支持大写字母） |

---

## 关键设计决策

### 1. 薄包装器模式

大多数新 Provider 使用 OpenAI Chat Completions 兼容 API，通过内嵌 `OpenAiProvider` 并委托所有 API 调用实现。仅自定义 `api()` 返回值和构造函数（base URL、默认 headers）。

优势：代码复用率高，维护成本低。

### 2. GitHub Copilot Api 标识

Copilot 使用 OpenAI Chat Completions 格式，但不能与 OpenAI Provider 共用 `Api::OpenAiChatCompletions`（会导致 Registry 冲突）。解决方案：使用 `Api::Other("github-copilot".to_string())`。

### 3. Vertex AI 独立实现

Vertex AI 使用 Google 专有的 Content API 格式（非 OpenAI 兼容），需要独立的请求构建、响应解析和认证流程，因此不使用薄包装器模式。

---

## 验证结果

### 编译状态
- `cargo check --workspace` 通过，零错误

### 测试状态
- `cargo test --workspace` 全部通过：603+ 测试，0 失败，8 ignored

### 验收标准

| # | 标准 | 状态 |
|---|------|------|
| 1 | Google Vertex AI Provider 实现完整 | ✅ |
| 2 | Google Gemini CLI Provider 委托 GoogleProvider | ✅ |
| 3 | GitHub Copilot 两步认证实现 | ✅ |
| 4 | 6 个薄包装器 Provider 编译通过 | ✅ |
| 5 | 13+ 新模型注册到模型注册表 | ✅ |
| 6 | Provider 总数从 10 增至 19 | ✅ |
| 7 | 所有新 Provider 支持流式响应 | ✅ |
| 8 | API Key 环境变量映射完整 | ✅ |
| 9 | 集成测试全通过 | ✅ |

---

*文档版本: 1.0*
*创建日期: 2026-04-11*
*基于: ITERATION-5 Phase 2 交付*
