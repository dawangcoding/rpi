# Phase 3: OAuth 认证完整实现 - 开发计划

## 现状分析

当前 OAuth 系统处于 Alpha 阶段（功能完成约 60-70%）：

| 模块 | 文件 | 行数 | 状态 |
|------|------|------|------|
| OAuth 流程 | `oauth_server.rs` | 220 | 完成（PKCE + State + 回调服务器） |
| Token 存储 | `token_storage.rs` | 164 | 完成但明文存储，有 refresh 方法 |
| Provider 配置 | `providers.rs` | 40 | 仅 Anthropic + GitHub Copilot |
| Slash 命令 | `interactive.rs` | /login /logout /auth | 已集成 |
| API Key 优先级 | `config.rs` | OAuth > 配置 > 环境变量 | 已集成 |

**核心差距：** Token 明文存储、缺少自动刷新调度、Provider 仅 2 个、零测试覆盖。

---

## Task 1: Token 加密存储（P0 安全关键）

**目标：** 将明文 JSON 存储升级为加密存储，优先使用系统密钥链，回退到 AES-GCM 文件加密。

**修改文件：**
- `crates/pi-coding-agent/src/core/auth/token_storage.rs` — 重构存储层，增加加密逻辑
- `crates/pi-coding-agent/Cargo.toml` — 添加 `keyring`、`aes-gcm`、`argon2` 依赖

**实现方案：**

1. **两层存储策略：**
   - 首选：`keyring` crate 集成系统密钥链（macOS Keychain / Linux Secret Service / Windows Credential Manager）
   - 回退：AES-256-GCM 文件加密（密钥通过 Argon2 从机器指纹派生）

2. **加密存储接口：**
   ```rust
   trait SecureStorage {
       fn save(&self, key: &str, data: &[u8]) -> Result<()>;
       fn load(&self, key: &str) -> Result<Option<Vec<u8>>>;
       fn delete(&self, key: &str) -> Result<()>;
   }
   
   struct KeychainStorage;         // 系统密钥链
   struct EncryptedFileStorage;    // AES-GCM 文件加密回退
   ```

3. **迁移兼容：** 检测到旧版明文 `tokens.json` 时自动迁移为加密格式

4. **机器指纹生成（用于文件加密密钥派生）：**
   - macOS: `IOPlatformSerialNumber`
   - Linux: `/etc/machine-id`
   - 通过 Argon2id 派生 256-bit AES 密钥

**新增依赖：**
```toml
keyring = "3"           # 系统密钥链
aes-gcm = "0.10"        # AES-256-GCM 加密
argon2 = "0.5"          # 密钥派生
```

**验证标准：**
- Token 不再以明文形式存在于磁盘
- 系统密钥链可用时使用密钥链存储
- 密钥链不可用时回退到 AES-GCM 文件加密
- 旧版明文文件自动迁移

---

## Task 2: Token 自动刷新调度（P1）

**目标：** 实现 Token 过期前自动刷新，刷新失败时提示用户重新登录。

**修改文件：**
- `crates/pi-coding-agent/src/core/auth/token_storage.rs` — 增加 `get_valid_token_or_refresh` 方法
- `crates/pi-coding-agent/src/core/auth/mod.rs` — 导出新方法
- `crates/pi-coding-agent/src/config.rs` — 在 `get_api_key` 中集成自动刷新

**实现方案：**

1. **懒刷新策略（推荐）：** 在 `get_api_key` 调用时检测是否即将过期（5 分钟内），若是则自动触发刷新，无需后台任务。

2. **新增方法：**
   ```rust
   impl TokenStorage {
       /// 获取有效 token，如果即将过期则自动刷新
       pub async fn get_valid_token_or_refresh(
           &self, 
           provider: &str,
       ) -> Result<Option<String>>;
   }
   ```

3. **刷新失败降级：**
   - 第一次失败：返回当前 token（如果尚未过期）并记录警告
   - Token 已过期且刷新失败：返回 None，在下次交互时提示 `/login` 重新认证
   - 记录刷新失败原因到日志

4. **config.rs 集成：** 将 `get_api_key` 改为 async，调用 `get_valid_token_or_refresh`

**验证标准：**
- Token 即将过期时 API 调用自动触发刷新
- 刷新成功后存储更新的 Token
- 刷新失败时给出用户可读的错误信息
- 完全过期时提示重新登录

---

## Task 3: 新增 OpenAI OAuth Provider（P0）

**目标：** 添加 OpenAI OAuth 支持。

**修改文件：**
- `crates/pi-coding-agent/src/core/auth/providers.rs` — 添加 OpenAI Provider 配置

**实现方案：**

```rust
"openai" => Some(OAuthProviderConfig {
    name: "openai".to_string(),
    authorize_url: "https://auth.openai.com/authorize".to_string(),
    token_url: "https://auth.openai.com/oauth/token".to_string(),
    client_id: "...".to_string(), // OpenAI CLI client_id
    scopes: vec!["openai.public".to_string()],
    use_pkce: true,
}),
```

**验证标准：**
- `/login openai` 可启动 OAuth 流程
- 授权后 Token 正确存储
- API 调用时使用 OAuth Token

---

## Task 4: 新增 Google OAuth Provider（P0）

**目标：** 添加 Google OAuth 支持（用于 Google AI / Gemini）。

**修改文件：**
- `crates/pi-coding-agent/src/core/auth/providers.rs` — 添加 Google Provider 配置

**实现方案：**

```rust
"google" => Some(OAuthProviderConfig {
    name: "google".to_string(),
    authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
    token_url: "https://oauth2.googleapis.com/token".to_string(),
    client_id: "...".to_string(),
    scopes: vec![
        "https://www.googleapis.com/auth/generative-language".to_string(),
    ],
    use_pkce: true,
}),
```

**注意：** Google OAuth 需要额外参数 `access_type=offline`（获取 refresh_token）和 `prompt=consent`。需要在 `oauth_server.rs` 中支持 Provider 特有的授权 URL 参数。

**修改文件（额外）：**
- `crates/pi-coding-agent/src/core/auth/providers.rs` — `OAuthProviderConfig` 增加 `extra_auth_params` 字段
- `crates/pi-coding-agent/src/core/auth/oauth_server.rs` — 构建授权 URL 时追加 extra params

**验证标准：**
- `/login google` 可启动 Google OAuth 流程
- 获取到 refresh_token（通过 `access_type=offline`）
- Token 正确存储和刷新

---

## Task 5: 单元测试覆盖（P1）

**目标：** 为 auth 模块添加全面的单元测试。

**修改文件：**
- `crates/pi-coding-agent/src/core/auth/token_storage.rs` — 底部添加 `#[cfg(test)] mod tests`
- `crates/pi-coding-agent/src/core/auth/providers.rs` — 底部添加 `#[cfg(test)] mod tests`
- `crates/pi-coding-agent/src/core/auth/oauth_server.rs` — 底部添加 `#[cfg(test)] mod tests`

**测试用例：**

1. **token_storage 测试：**
   - `test_save_and_get_token` — 存储后读取
   - `test_get_valid_token_expired` — 过期 Token 返回 None
   - `test_get_valid_token_valid` — 有效 Token 返回值
   - `test_is_expiring_soon` — 即将过期检测
   - `test_remove_token` — 删除后读取返回 None
   - `test_list_providers` — 列出所有已存储 Provider
   - `test_encrypted_storage_roundtrip` — 加密存储写入读取往返测试
   - `test_migration_from_plaintext` — 明文迁移到加密

2. **providers 测试：**
   - `test_get_known_provider` — 获取已知 Provider
   - `test_get_unknown_provider` — 未知 Provider 返回 None
   - `test_list_providers` — 列出所有支持的 Provider
   - `test_provider_urls_valid` — 验证 URL 格式

3. **oauth_server 测试：**
   - `test_generate_code_verifier_length` — verifier 长度合规
   - `test_sha256_base64url` — PKCE challenge 正确性
   - `test_generate_state_uniqueness` — state 唯一性
   - `test_parse_query_string` — 查询字符串解析
   - `test_urlencoding` — URL 编码正确性

**验证标准：**
- `cargo test -p pi-coding-agent` 全部通过
- auth 模块测试覆盖关键路径

---

## Task 6: 编译验证与集成检查

**目标：** 确保所有修改编译通过，集成正常。

**验证项目：**
- `cargo check --workspace` 零错误
- `cargo test -p pi-coding-agent` 全部通过
- `cargo clippy --workspace` 无新增警告
- `/login anthropic`、`/login openai`、`/login google` 命令可用
- `/auth` 显示所有 Provider 的认证状态
- `/logout` 正确清除 Token

---

## 任务依赖关系

```
Task 1 (加密存储)  ←─── Task 5 (测试) 部分依赖
    │
Task 2 (自动刷新)  ←─── 依赖 Task 1
    │
Task 3 (OpenAI)   ──┐
                     ├── Task 4 (Google) 需要 extra_auth_params
Task 4 (Google)   ──┘
    │
Task 6 (验证) ←─── 依赖所有 Task
```

**推荐执行顺序：**
1. Task 1（加密存储） + Task 3/4（Provider，可并行）
2. Task 2（自动刷新，依赖 Task 1）
3. Task 5（测试，依赖 Task 1-4）
4. Task 6（集成验证）

---

## 涉及文件汇总

| 文件 | 操作 | Task |
|------|------|------|
| `auth/token_storage.rs` | 重构（加密 + 自动刷新 + 测试） | 1, 2, 5 |
| `auth/providers.rs` | 扩展（+2 Provider + extra_params + 测试） | 3, 4, 5 |
| `auth/oauth_server.rs` | 小改（extra_auth_params 支持 + 测试） | 4, 5 |
| `auth/mod.rs` | 小改（导出新增内容） | 1, 2 |
| `config.rs` | 改造（async get_api_key + 自动刷新） | 2 |
| `Cargo.toml` | 添加依赖 | 1 |

## 新增依赖

```toml
keyring = "3"
aes-gcm = "0.10"
argon2 = "0.5"
```

---

*文档版本: 1.0*
*创建日期: 2026-04-10*
*基于: ITERATION-3 Phase 3 规划*
