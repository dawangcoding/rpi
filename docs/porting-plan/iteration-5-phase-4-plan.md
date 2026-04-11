# ITERATION-5 Phase 4: 功能特性增强

## 概述

实现四大功能模块：技能系统框架（Skill Registry + 5 个内置技能）、JSON-RPC 2.0 服务模式（hyper 1.x HTTP 服务器 + 10 个 RPC 方法）、TUI 设置管理界面（分类导航 + 多类型设置项编辑）、配置热重载（notify crate 文件监控）+ CLI 增强（退出码标准化 + JSON 输出 + 文件 I/O + 批处理）。

## 任务分解

### 功能模块（4 个独立任务）

| 任务 | 模块 | 说明 |
|------|------|------|
| 技能系统 | `core/skills/` | Skill 类型定义 + SkillRegistry + 5 个内置技能 |
| RPC 服务模式 | `rpc/` | JSON-RPC 2.0 类型 + HTTP 服务器 + 10 个方法处理器 |
| 设置管理 TUI | `modes/settings.rs` | SettingsManager + 12 个默认设置项 + 分类导航 |
| CLI 增强 + 配置热重载 | `cli/args.rs`, `config.rs`, `modes/print_mode.rs` | 退出码、JSON 输出、文件 I/O、ConfigWatcher |

---

## 依赖关系与执行顺序

```
Task 1 (技能系统) ─────────────────────────┐
                                            │
Task 2 (RPC 服务模式) ─────────────────────┤  ← 4 个任务完全独立可并行
                                            │
Task 3 (设置管理 TUI) ─────────────────────┤
                                            │
Task 4 (CLI 增强 + 配置热重载) ────────────┤
                                            │
                                            ↓
Task 5 (集成验证)
```

---

## Task 1: 技能系统框架

**范围**: 实现完整的技能系统框架

**新增文件**:
- `crates/pi-coding-agent/src/core/skills/mod.rs` — 模块入口，导出所有类型
- `crates/pi-coding-agent/src/core/skills/types.rs`:
  - `Skill` 结构体（id, name, description, category, prompt_template, parameters, tags）
  - `SkillParameter`（name, description, param_type, required, default_value）
  - `SkillCategory` 枚举（CodeQuality, Refactoring, Documentation, Analysis, Performance）
- `crates/pi-coding-agent/src/core/skills/registry.rs`:
  - `SkillRegistry` — `HashMap<String, Skill>` 存储
  - 方法：`register()`, `get()`, `list()`, `search()`, `list_by_category()`
  - 内置技能自动注册
- `crates/pi-coding-agent/src/core/skills/builtin.rs`:
  - 5 个内置技能:
    1. `code-review` — 代码审查
    2. `refactoring` — 代码重构
    3. `doc-generation` — 文档生成
    4. `bug-analysis` — Bug 分析
    5. `performance-optimization` — 性能优化
  - 每个技能包含完整的 prompt_template 和参数定义

**修改文件**:
- `crates/pi-coding-agent/src/core/mod.rs` — 添加 `pub mod skills`

**验证**: 注册表 CRUD + 搜索 + 分类测试通过

---

## Task 2: RPC 服务模式

**范围**: 实现 JSON-RPC 2.0 HTTP 服务

**新增文件**:
- `crates/pi-coding-agent/src/rpc/mod.rs` — 模块入口
- `crates/pi-coding-agent/src/rpc/types.rs`:
  - `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` 结构体
  - 标准错误码常量（PARSE_ERROR, INVALID_REQUEST, METHOD_NOT_FOUND, INTERNAL_ERROR）
  - `RpcId` 枚举（Number/String/Null）
- `crates/pi-coding-agent/src/rpc/server.rs`:
  - `RpcServer` — 基于 hyper 1.x 的 HTTP 服务器
  - 支持单个请求和批量请求
  - CORS 支持
  - 优雅关闭（graceful shutdown）
- `crates/pi-coding-agent/src/rpc/methods.rs`:
  - `RpcMethodHandler` — 10 个方法:
    1. `initialize` — 初始化会话
    2. `shutdown` — 关闭服务
    3. `sendMessage` — 发送消息
    4. `getMessages` — 获取消息历史
    5. `executeTool` — 执行工具
    6. `getTools` — 获取可用工具列表
    7. `setModel` — 切换模型
    8. `getModels` — 获取可用模型
    9. `getStatus` — 获取服务状态
    10. `compactSession` — 压缩会话

**修改文件**:
- `crates/pi-coding-agent/src/lib.rs` — 添加 `pub mod rpc`

**验证**: JSON-RPC 请求/响应序列化测试、方法路由测试通过

---

## Task 3: 设置管理 TUI 界面

**范围**: 实现 TUI 设置管理

**新增文件**:
- `crates/pi-coding-agent/src/modes/settings.rs`:
  - `SettingsManager` 结构体
  - `SettingItem`（key, label, description, category, value_type, value, default_value）
  - `SettingValueType` 枚举（Bool, String, Number, Enum）
  - `SettingCategory` 枚举（General, Provider, Editor, Extensions）
  - **12 个默认设置项**（4 个分类各 3 个）:
    - General: theme, language, auto_save
    - Provider: default_provider, temperature, max_tokens
    - Editor: vim_mode, tab_size, word_wrap
    - Extensions: auto_update, sandbox_enabled, max_extensions
  - **操作方法**:
    - 分类导航（`select_category()`）
    - 列表导航（`move_up()`, `move_down()`）
    - Bool 切换（`toggle_bool()`）
    - Enum 循环（`cycle_enum()`）
    - 字符串/数字编辑（`update_value()`）
    - 重置默认（`reset_to_default()`）
    - 导出配置（`export_settings()`）

**修改文件**:
- `crates/pi-coding-agent/src/modes/mod.rs` — 添加 `pub mod settings`

**验证**: 设置项 CRUD + 分类切换 + 导出测试通过

---

## Task 4: CLI 增强与配置热重载

**范围**: 命令行模式增强 + 配置文件热重载

**修改文件**:
- `crates/pi-coding-agent/src/cli/args.rs`:
  - 新增 CLI 参数:
    - `--rpc` / `--rpc-port` — 启动 RPC 模式
    - `--input-file` — 从文件读取提示词
    - `--output-file` — 输出到文件
    - `--json` — JSON 格式输出
    - `--batch` — 非交互式批量处理
- `crates/pi-coding-agent/src/modes/print_mode.rs`:
  - 退出码常量:
    - `SUCCESS = 0`, `GENERAL_ERROR = 1`, `AUTH_ERROR = 2`, `MODEL_ERROR = 3`
  - `JsonOutput` 结构体（response, model, tokens_used, duration_ms）
  - `read_input_file()` — 从文件读取输入
  - `write_output_file()` — 写入输出到文件
  - `format_json_output()` — 格式化 JSON 输出
- `crates/pi-coding-agent/src/config.rs`:
  - `ConfigWatcher` 结构体:
    - 使用 `notify` crate 监控配置文件变更
    - 支持 YAML/JSON/TOML 格式验证
    - 配置变更事件通知
    - 验证失败时保持原配置

**验证**: CLI 参数解析测试 + ConfigWatcher 单元测试通过

---

## 新增文件清单

| 文件 | 说明 | 行数（约） |
|------|------|-----------|
| `crates/pi-coding-agent/src/core/skills/mod.rs` | 技能系统模块入口 | ~20 |
| `crates/pi-coding-agent/src/core/skills/types.rs` | Skill/SkillParameter/SkillCategory 类型 | ~100 |
| `crates/pi-coding-agent/src/core/skills/registry.rs` | SkillRegistry 注册表 | ~200 |
| `crates/pi-coding-agent/src/core/skills/builtin.rs` | 5 个内置技能 | ~200 |
| `crates/pi-coding-agent/src/rpc/mod.rs` | RPC 模块入口 | ~20 |
| `crates/pi-coding-agent/src/rpc/types.rs` | JSON-RPC 2.0 类型 | ~150 |
| `crates/pi-coding-agent/src/rpc/server.rs` | HTTP 服务器（hyper 1.x） | ~250 |
| `crates/pi-coding-agent/src/rpc/methods.rs` | 10 个 RPC 方法处理器 | ~300 |
| `crates/pi-coding-agent/src/modes/settings.rs` | TUI 设置管理界面 | ~400 |

## 修改文件清单

| 文件 | 改动说明 |
|------|----------|
| `crates/pi-coding-agent/src/core/mod.rs` | 添加 `pub mod skills` |
| `crates/pi-coding-agent/src/lib.rs` | 添加 `pub mod rpc` |
| `crates/pi-coding-agent/src/modes/mod.rs` | 添加 `pub mod settings` |
| `crates/pi-coding-agent/src/cli/args.rs` | 新增 6 个 CLI 参数 |
| `crates/pi-coding-agent/src/modes/print_mode.rs` | 退出码 + JSON 输出 + 文件 I/O |
| `crates/pi-coding-agent/src/config.rs` | ConfigWatcher 配置热重载 |

---

## 关键设计决策

### 1. 技能系统架构

采用 Registry 模式：`SkillRegistry` 持有 `HashMap<String, Skill>`，支持内置技能自动注册 + 用户自定义技能动态注册。每个 Skill 包含参数化的 prompt_template，使用时替换参数占位符。

### 2. RPC 服务器选型

使用 `hyper 1.x` 直接构建 HTTP 服务器（而非高层框架如 axum/actix），保持依赖最小化。JSON-RPC 2.0 协议手动实现，支持标准错误码、批量请求和 CORS。

### 3. 设置系统类型安全

`SettingValueType` 枚举限制值类型（Bool/String/Number/Enum），每个 `SettingItem` 携带默认值，支持类型检查的值更新。分类系统使用 `SettingCategory` 枚举而非字符串。

### 4. 配置热重载策略

`ConfigWatcher` 使用 `notify` crate 的文件系统事件（而非轮询），变更时先验证新配置格式，验证失败则保持原配置不变，避免错误配置导致运行时崩溃。

### 5. 退出码标准化

遵循 Unix 惯例：0=成功，1=通用错误，2=认证错误，3=模型错误。JSON 输出模式下，结构化信息写入 stdout，错误信息写入 stderr。

---

## 验证结果

### 编译状态
- `cargo check --workspace` 通过，零错误

### 测试状态
- `cargo test --workspace` 全部通过：603+ 测试，0 失败，8 ignored

### 验收标准

| # | 标准 | 状态 |
|---|------|------|
| 1 | SkillRegistry 支持注册/查询/搜索/分类 | ✅ |
| 2 | 5 个内置技能可正常获取 | ✅ |
| 3 | JSON-RPC 2.0 请求/响应格式正确 | ✅ |
| 4 | RPC 服务器支持单个和批量请求 | ✅ |
| 5 | 10 个 RPC 方法路由正确 | ✅ |
| 6 | SettingsManager 12 个默认设置项 | ✅ |
| 7 | 设置分类导航 + 多类型编辑 | ✅ |
| 8 | ConfigWatcher 文件监控 + 格式验证 | ✅ |
| 9 | CLI 新参数解析正确 | ✅ |
| 10 | 退出码标准化 + JSON 输出 | ✅ |
| 11 | 所有测试通过 | ✅ |

---

## 已知限制

1. **RPC 服务器 hyper 1.x API**: hyper 1.x API 变动较大，部分高级功能（如连接池、超时控制）需后续完善
2. **技能模板渲染**: 当前使用简单字符串替换，后续可引入 Tera/Handlebars 等模板引擎
3. **设置持久化**: SettingsManager 当前为内存管理，需与 ConfigWatcher 协同实现持久化

---

*文档版本: 1.0*
*创建日期: 2026-04-11*
*基于: ITERATION-5 Phase 4 交付*
