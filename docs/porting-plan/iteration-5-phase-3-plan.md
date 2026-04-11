# ITERATION-5 Phase 3: OAuth 完整化

## 概述

完善 OAuth 认证框架：新增 4 个 Provider 的 OAuth 配置（Azure OpenAI、Mistral、Huggingface、OpenRouter），实现 Token 自动刷新调度器（RefreshScheduler），增强系统密钥链集成（keyring crate + AES-256-GCM 加密文件降级），支持 Device Code Flow，Token 版本化存储（v1→v2 自动迁移）。

## 任务分解

### OAuth Provider 扩展（1 个任务）

| 任务 | 说明 |
|------|------|
| 新增 OAuth Provider 配置 | 在 providers.rs 中添加 Azure OpenAI、Mistral、Huggingface、OpenRouter 的 OAuth 端点配置 |

### Token 管理增强（2 个任务）

| 任务 | 说明 |
|------|------|
| Token 刷新调度器 | 实现 RefreshScheduler 后台定时检查 + 自动刷新 |
| 密钥链增强 + Token 版本化 | keyring 集成、健康检查、Token 格式 v1→v2 迁移、错误分类 |

---

## 依赖关系与执行顺序

```
Task 1 (OAuth Provider 配置扩展) ──────────┐
                                            │
Task 2 (RefreshScheduler 刷新调度器) ───────┤  ← 可并行
                                            │
Task 3 (密钥链增强 + Token 版本化) ─────────┤  ← 可并行
                                            │
                                            ↓
Task 4 (Device Code Flow + 集成验证)
```

---

## Task 1: OAuth Provider 配置扩展

**范围**: 新增 4 个 Provider 的 OAuth 配置

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/providers.rs`:
  - `AzureOpenAiOAuth` — Azure AD 授权端点，scope: `https://cognitiveservices.azure.com/.default`
  - `MistralOAuth` — Mistral 控制台 OAuth，scope: `openid profile email`
  - `HuggingfaceOAuth` — Huggingface Hub OAuth，scope: `openid profile`
  - `OpenRouterOAuth` — OpenRouter OAuth，scope: `openid profile email`
  - 每个配置包含: `auth_url`, `token_url`, `client_id`, `scope`, `redirect_uri`

**验证**: 编译通过，OAuth 端点 URL 格式正确

---

## Task 2: Token 刷新调度器

**范围**: 实现后台 Token 自动刷新

**新增文件**:
- `crates/pi-coding-agent/src/core/auth/refresh.rs`:
  - `RefreshScheduler` 结构体
    - `start()` 启动后台 tokio task，默认每 5 分钟检查一次
    - `stop()` 通过 watch channel 优雅关闭
  - `RefreshEvent` 枚举:
    - `Refreshed { provider, expires_at }` — 刷新成功
    - `Failed { provider, error }` — 刷新失败
    - `ReloginRequired { provider }` — 需要重新登录
  - 并发刷新控制（同一 Provider 不重复刷新）
  - 刷新日志记录

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/mod.rs` — 导出 `refresh` 模块

**验证**: 单元测试覆盖调度器启停、事件发送

---

## Task 3: 密钥链增强与 Token 版本化

**范围**: 增强 Token 安全存储，支持版本迁移

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/token_storage.rs`:
  - **KeychainStorage 增强**:
    - 使用 `keyring` crate 访问系统密钥链（macOS Keychain / Windows DPAPI / Linux Secret Service）
    - 独立索引文件 `keychain_index.json` 管理 Token 列表
    - 健康检查方法 `health_check()`
    - 降级方案：密钥链不可用时使用 AES-256-GCM 加密文件存储
  - **Token 版本化**:
    - `TokenVersion` 枚举（V1, V2）
    - V2 格式包含: `version`, `data`, `created_at`, `migrated_from`
    - 自动迁移：读取 V1 格式时自动升级为 V2
  - **错误分类**:
    - `RefreshError` 枚举: `NetworkError`, `AuthError`, `Other`
    - 错误类型决定后续行为（重试 vs 重新登录）

**验证**: Token 存取正确，版本迁移测试通过

---

## Task 4: Device Code Flow 与集成验证

**范围**: 支持 Device Code Flow 认证方式

**修改文件**:
- `crates/pi-coding-agent/src/core/auth/oauth_server.rs`:
  - `DeviceCodeFlow` 支持:
    - `request_device_code()` — 请求设备码
    - `poll_for_token()` — 轮询获取 Token
    - 用户提示（显示 URL 和设备码）
  - 适用于无法打开浏览器的远程/终端环境

**验证**: `cargo check --workspace` + `cargo test --workspace` 全通过

---

## 新增文件清单

| 文件 | 说明 | 行数（约） |
|------|------|-----------|
| `crates/pi-coding-agent/src/core/auth/refresh.rs` | RefreshScheduler Token 刷新调度器 | ~250 |

## 修改文件清单

| 文件 | 改动说明 |
|------|----------|
| `crates/pi-coding-agent/src/core/auth/providers.rs` | 新增 4 个 OAuth Provider 配置 |
| `crates/pi-coding-agent/src/core/auth/token_storage.rs` | 密钥链增强 + Token 版本化 + 错误分类 |
| `crates/pi-coding-agent/src/core/auth/oauth_server.rs` | Device Code Flow 支持 |
| `crates/pi-coding-agent/src/core/auth/mod.rs` | 导出 refresh 模块 |

---

## 关键设计决策

### 1. 密钥链优先 + 加密文件降级

```
TokenStorage::store()
  ├── 尝试 keyring → 成功 → 返回
  └── keyring 失败 → AES-256-GCM 加密文件 → 返回
```

优先使用系统密钥链（最安全），不可用时自动降级到加密文件存储，确保所有平台都能正常工作。

### 2. Token 版本化迁移策略

采用"读时迁移"策略：读取 Token 时检测版本，V1 自动升级为 V2 并写回存储。避免一次性批量迁移的风险。

### 3. RefreshScheduler 事件驱动

使用 `tokio::sync::watch` channel 实现优雅关闭，`RefreshEvent` 枚举提供明确的刷新结果分类，调用方可据此决定是否提示用户重新登录。

### 4. Device Code Flow

为远程 SSH / 无头服务器环境提供认证支持。流程：请求设备码 → 显示 URL + 用户码 → 轮询 Token → 存储。

---

## 验证结果

### 编译状态
- `cargo check --workspace` 通过，零错误

### 测试状态
- `cargo test --workspace` 全部通过：603+ 测试，0 失败，8 ignored

### 验收标准

| # | 标准 | 状态 |
|---|------|------|
| 1 | 4 个新 OAuth Provider 配置完整 | ✅ |
| 2 | RefreshScheduler 后台定时刷新 | ✅ |
| 3 | 密钥链集成（keyring crate） | ✅ |
| 4 | 加密文件降级方案 | ✅ |
| 5 | Token V1→V2 自动迁移 | ✅ |
| 6 | Device Code Flow 支持 | ✅ |
| 7 | RefreshError 错误分类 | ✅ |
| 8 | 密钥链健康检查 | ✅ |
| 9 | 所有测试通过 | ✅ |

---

*文档版本: 1.0*
*创建日期: 2026-04-11*
*基于: ITERATION-5 Phase 3 交付*
