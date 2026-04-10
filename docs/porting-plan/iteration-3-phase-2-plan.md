# ITERATION-3 Phase 2: Provider 扩展开发计划

## 概述

本阶段目标是从当前 5 个 Provider 扩展到 8 个，新增 Azure OpenAI、xAI (Grok)、OpenRouter 独立 Provider，并扩展模型注册表。

## 现状分析

**已有基础设施：**
- `Api` 枚举已预留 `AzureOpenAiResponses`、`Xai`、`Openrouter` 变体
- `Provider` 枚举已预留对应变体，`get_api_key_from_env()` 已处理所有三个 Provider 的环境变量
- OpenAI Provider 的 `detect_compat()` 已通过兼容层支持 xAI 和 OpenRouter
- OpenRouter 已有 3 个模型注册（使用 `Api::OpenAiChatCompletions`），xAI 尚无模型注册
- Azure OpenAI 无 Provider 实现，无模型注册

**设计决策：**
- **Azure OpenAI**：需要独立 Provider（endpoint 格式、认证方式完全不同）
- **xAI**：创建薄包装 Provider，内部委托 OpenAI 逻辑，使用独立 `Api::Xai` 路由
- **OpenRouter**：创建薄包装 Provider，使用独立 `Api::Openrouter` 路由，处理 OpenRouter 特有的 Provider 路由头

---

## Task 1: Azure OpenAI Provider

| 属性 | 值 |
|------|-----|
| **新增文件** | `crates/pi-ai/src/providers/azure_openai.rs` |
| **Api 枚举** | `Api::AzureOpenAiResponses` |
| **Provider 枚举** | `Provider::AzureOpenAiResponses` |
| **预估代码量** | 400-600 行 |

### 实现要点

1. 结构体 `AzureOpenAiProvider { client: Client }`
2. Endpoint 格式：`{base_url}/openai/deployments/{deployment-id}/chat/completions?api-version=2024-12-01-preview`
3. 认证：请求头 `api-key: {key}` 而非 Bearer token
4. 复用 OpenAI 的消息转换逻辑（convert_messages、convert_tools、StreamState）
5. 支持流式 SSE 处理
6. 内置重试逻辑（指数退避，最多 3 次）
7. 环境变量：`AZURE_OPENAI_API_KEY`、`AZURE_OPENAI_ENDPOINT`

---

## Task 2: xAI (Grok) Provider

| 属性 | 值 |
|------|-----|
| **新增文件** | `crates/pi-ai/src/providers/xai.rs` |
| **Api 枚举** | `Api::Xai` |
| **Provider 枚举** | `Provider::Xai` |
| **预估代码量** | 300-400 行 |

### 实现要点

1. 结构体 `XaiProvider { client: Client }`
2. Endpoint：`https://api.x.ai/v1/chat/completions`
3. 认证：标准 Bearer token
4. xAI 特有处理：
   - 不支持 `store` 参数
   - 不支持 `developer` 角色
   - 不支持 `reasoning_effort`
   - 使用 `max_completion_tokens`
5. 环境变量：`XAI_API_KEY`

---

## Task 3: OpenRouter Provider

| 属性 | 值 |
|------|-----|
| **新增文件** | `crates/pi-ai/src/providers/openrouter.rs` |
| **Api 枚举** | `Api::Openrouter` |
| **Provider 枚举** | `Provider::Openrouter` |
| **预估代码量** | 300-400 行 |

### 实现要点

1. 结构体 `OpenRouterProvider { client: Client }`
2. Endpoint：`https://openrouter.ai/api/v1/chat/completions`
3. 认证：标准 Bearer token
4. OpenRouter 特有请求头：
   - `HTTP-Referer`：应用标识
   - `X-Title`：应用名称
5. thinking_format 设为 `"openrouter"`
6. 环境变量：`OPENROUTER_API_KEY`

---

## Task 4: 模型注册表扩展

**修改文件：** `crates/pi-ai/src/models.rs`

### 新增模型

**Azure OpenAI（4 个）：**
- `azure/gpt-4o` — GPT-4o 部署
- `azure/gpt-4o-mini` — GPT-4o Mini 部署
- `azure/o3-mini` — o3-mini 推理模型部署
- `azure/o1` — o1 推理模型部署

**xAI（3 个）：**
- `grok-3` — Grok 3
- `grok-3-mini` — Grok 3 Mini（推理模型）
- `grok-2-vision` — Grok 2 Vision（多模态）

**OpenRouter 补充（3 个）：**
- `openrouter/openai/gpt-4o` — GPT-4o via OpenRouter
- `openrouter/meta-llama/llama-4-maverick` — Llama 4 Maverick via OpenRouter
- `openrouter/deepseek/deepseek-r1` — DeepSeek R1 via OpenRouter

**注意：** 现有 3 个 OpenRouter 模型的 `api` 从 `Api::OpenAiChatCompletions` 改为 `Api::Openrouter`。

---

## Task 5: 注册与导出

**修改文件：**
- `crates/pi-ai/src/providers/mod.rs` — 添加新模块导出
- `crates/pi-ai/src/lib.rs` — 在 `init_providers()` 中注册新 Provider

---

## Task 6: 编译验证

1. `cargo check -p pi-ai` 通过
2. `cargo clippy -p pi-ai` 无新增警告
3. `cargo test -p pi-ai` 全部通过

---

## 执行依赖

```
Task 1-4 (并行) → Task 5 (注册+导出) → Task 6 (验证)
```

---

*文档版本: 1.0*
*创建日期: 2026-04-10*
*基于: ITERATION-3 Phase 2 计划*
