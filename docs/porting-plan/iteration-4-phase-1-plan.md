# ITERATION-4 Phase 1: 基础设施完善

## 概述

补齐基础设施级别的关键功能，包括 OAuth Provider 完善（OpenAI/Google）、Token 自动刷新增强、Token 计数精度提升（Mistral/Gemini Tokenizer）、多格式配置支持（.env/JSON/TOML）。

## 依赖关系与执行顺序

```
Work Stream A (OAuth):     Task 1 + Task 2 (并行) --> Task 3
Work Stream B (Tokenizer): Task 4 + Task 5 (并行)
Work Stream C (Config):    Task 6 + Task 7 + Task 8 (一起实现)
```

三个 Work Stream 完全独立，可全部并行。

---

## Task 1: OpenAI OAuth 支持激活与集成

**范围**: 验证 `providers.rs` 中 OpenAI 配置的激活状态，确保 `/login openai` 命令正确调用 `run_oauth_flow()`

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/providers.rs` -- 验证 OpenAI 配置（L36-46），移除 `list_oauth_providers` 的 dead_code 标记
- `crates/pi-coding-agent/src/core/auth/mod.rs` -- 导出 `list_oauth_providers`
- `crates/pi-coding-agent/src/modes/interactive.rs` -- `/login` 命令支持所有 4 个 Provider，动态显示可用列表

**验证**: `/login openai` 启动浏览器 OAuth 流程，回调成功获取 token 并存储

---

## Task 2: Google OAuth 支持激活与集成

**范围**: 验证 `providers.rs` 中 Google 配置（L47-60）的激活状态，处理 Google 特有的 `access_type=offline` 和 `prompt=consent` 参数

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/providers.rs` -- 验证 Google 配置
- `crates/pi-coding-agent/src/modes/interactive.rs` -- 确保 `/login google` 命令路由
- `crates/pi-coding-agent/src/core/auth/oauth_server.rs` -- 验证 Google 返回 scope 缩减情况的兼容性

**验证**: `/login google` 启动浏览器 OAuth 流程，含 offline access 和 consent 提示

---

## Task 3: Token 自动刷新完善

**依赖**: Task 1 + Task 2

**范围**: 增强 `token_storage.rs` 的刷新机制

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/token_storage.rs`:
  - 添加 `tokio::sync::Mutex` 并发刷新保护（per-provider 级别锁）
  - 基础重试机制（1 次重试 + 1 秒间隔）
  - 刷新失败通过 `tracing::warn!` 通知上层需要重新登录
- `crates/pi-coding-agent/src/core/agent_session.rs` -- 验证集成

**验证**: Token 过期前 5 分钟自动刷新；并发请求只触发一次刷新；刷新失败提示重新登录

---

## Task 4: Mistral Tokenizer 集成

**范围**: 在 `token_counter.rs` 中新增 `MistralTokenCounter`

**修改文件**:
- `crates/pi-ai/src/token_counter.rs`:
  - 新增 `MistralTokenCounter` 结构体，使用 `tokenizers::Tokenizer::from_pretrained("mistralai/Mistral-7B-v0.1")`
  - `OnceLock` 缓存 tokenizer 实例
  - 加载失败回退到 `ModelTokenCounter`（3.8 比率）
  - 新增 `is_mistral_model()` 辅助函数
  - 更新 `create_token_counter()` 工厂函数
- `crates/pi-ai/Cargo.toml` -- 添加 `tokenizers = "0.22"` 依赖

**验证**: Mistral 模型 Token 计数误差 < 5%

---

## Task 5: Gemini Tokenizer 集成

**范围**: 在 `token_counter.rs` 中新增 `GeminiTokenCounter`

**修改文件**:
- `crates/pi-ai/src/token_counter.rs`:
  - 新增 `GeminiTokenCounter` 结构体，使用 `tokenizers::Tokenizer::from_pretrained("google/gemma-2b")`
  - 改进本地估算器（CJK 字符、代码、普通文本不同比率）
  - 多模态图片 token 按尺寸动态估算
  - 新增 `is_gemini_model()` 辅助函数
  - 更新 `create_token_counter()` 工厂函数

**验证**: Gemini 模型 Token 计数误差 < 5%

---

## Task 6: .env 配置文件支持

**范围**: 在应用启动时从 `~/.pi/.env` 加载环境变量

**修改文件**:
- `crates/pi-coding-agent/src/config.rs` -- 新增 `load_env_file()` 函数，使用 `dotenvy` crate
- `crates/pi-coding-agent/Cargo.toml` -- 添加 `dotenvy = "0.15"` 依赖

**验证**: `~/.pi/.env` 中的环境变量被正确加载且不覆盖已有变量

---

## Task 7: JSON 配置文件支持

**范围**: 支持 `~/.pi/config.json` 配置文件

**修改文件**:
- `crates/pi-coding-agent/src/config.rs`:
  - 新增 `ConfigFormat` 枚举和 `parse_config()` 方法
  - 修改 `load()` 方法，添加格式自动检测（优先级：YAML > JSON > TOML）
  - JSON 语法错误提供行号定位

**验证**: `~/.pi/config.json` 正确解析，与 YAML 配置等效

---

## Task 8: TOML 配置文件支持

**范围**: 支持 `~/.pi/config.toml` 配置文件

**修改文件**:
- `crates/pi-coding-agent/src/config.rs` -- 添加 TOML 格式支持
- `crates/pi-coding-agent/Cargo.toml` -- 添加 `toml = "0.8"` 依赖

**验证**: `~/.pi/config.toml` 正确解析，格式错误时有清晰提示

---

## 交付结果

### 修改文件清单（9 个文件，+839/-25 行）

| 文件 | 改动 | 说明 |
|------|------|------|
| `crates/pi-ai/Cargo.toml` | +1 | 添加 tokenizers 依赖 |
| `crates/pi-ai/src/token_counter.rs` | +510 | MistralTokenCounter、GeminiTokenCounter、辅助函数、13 个测试 |
| `crates/pi-coding-agent/Cargo.toml` | +3 | 添加 dotenvy、toml、serial_test 依赖 |
| `crates/pi-coding-agent/src/config.rs` | +66 | .env 加载、JSON/TOML 格式支持、自动检测 |
| `crates/pi-coding-agent/src/core/agent_session.rs` | +4/-4 | 集成验证调整 |
| `crates/pi-coding-agent/src/core/auth/mod.rs` | +1 | 导出 list_oauth_providers |
| `crates/pi-coding-agent/src/core/auth/providers.rs` | -1 | 移除 dead_code 标记 |
| `crates/pi-coding-agent/src/core/auth/token_storage.rs` | +269 | 并发刷新保护、重试机制、失败通知 |
| `crates/pi-coding-agent/src/modes/interactive.rs` | +9/-4 | /login 命令支持所有 Provider |

### 新增测试文件

| 文件 | 测试数 | 说明 |
|------|--------|------|
| `crates/pi-coding-agent/tests/config_tests.rs` | 12 | 多格式配置加载、.env 支持、错误提示 |

### 验证结果

- `cargo check` 通过（2 个预留方法 dead_code 警告）
- `cargo test` 全部通过：pi-ai 142 + pi-coding-agent 302 + config_tests 12 + 其他
- `cargo clippy` 零错误

---

*文档版本: 1.0*
*创建日期: 2026-04-11*
*基于: ITERATION-3 完成状态 + ITERATION-4 Phase 1 交付*
