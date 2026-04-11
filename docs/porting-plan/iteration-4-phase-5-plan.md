# ITERATION-4 Phase 5: 扩展系统与体验优化

## 1. 概述

Phase 5 目标是实现扩展热重载能力和快捷键自定义功能，提升用户体验。核心包括 WASM 扩展动态加载、安全沙箱、热重载机制，以及完整的快捷键配置系统。

### 依赖的前置 Phase

```
Phase 1 (基础设施完善) ──┐
Phase 2 (Notebook 工具) ─┼──> Phase 5 (扩展与体验)
Phase 3 (Vim 编辑器模式) ─┤
Phase 4 (稳定性与协议) ──┘
```

Phase 5 依赖于 Phase 1-4 的稳定性，在核心功能稳定后优化用户体验。

---

## 2. 技术方案选型

### 2.1 扩展动态加载方案

| 方案 | 优点 | 缺点 | 安全性 | 复杂度 |
|------|------|------|--------|--------|
| **dylib (libloading)** | 性能最优，调用开销小 | 平台兼容性差，ABI 不稳定，崩溃风险高 | 低：可访问主进程内存 | 中 |
| **WASM (wasmtime)** | 跨平台，安全隔离，崩溃隔离 | 调用开销略高，需要 WASI 适配 | 高：完整沙箱隔离 | 中高 |
| **IPC (独立进程)** | 完全隔离，语言无关 | 通信开销大，部署复杂 | 最高：进程级隔离 | 高 |

**最终选择：WASM（wasmtime）**

选择理由：
1. **安全性**：WASM 提供完整的沙箱隔离，扩展无法访问主进程内存，崩溃不会影响主程序
2. **跨平台**：WASM 字节码可在所有平台运行，无需为每个平台编译原生库
3. **生态成熟**：wasmtime 是 Bytecode Alliance 维护的成熟运行时，WASI 支持完善
4. **资源控制**：通过 fuel metering 可精确控制 CPU 使用，通过内存限制可控制内存使用
5. **热重载友好**：WASM 模块可动态加载/卸载，无需重启主进程

### 2.2 沙箱隔离方案

基于 WASM + WASI 的完整沙箱实现：

**隔离维度：**

| 维度 | 实现方式 | 效果 |
|------|----------|------|
| **文件系统** | `WasiCtxBuilder` 预打开目录 + `DirPerms`/`FilePerms` 权限控制 | 扩展只能访问显式授权的路径 |
| **网络** | `WasiCtxBuilder` 网络权限配置 | 扩展网络访问需显式声明 |
| **内存** | `StoreLimits` 内存上限配置 | 防止扩展消耗过多内存 |
| **CPU** | fuel metering | 防止无限循环，可精确控制执行成本 |
| **系统调用** | WASI 标准化接口 | 扩展只能使用 WASI 定义的系统调用 |

**安全模型：**

```
┌─────────────────────────────────────────────────────────┐
│                     主进程                               │
│  ┌─────────────────────────────────────────────────┐   │
│  │              WasmExtensionLoader                 │   │
│  │                     │                            │   │
│  │          ┌─────────┴─────────┐                  │   │
│  │          ▼                   ▼                  │   │
│  │  ┌──────────────┐    ┌──────────────┐          │   │
│  │  │   沙箱实例1   │    │   沙箱实例2   │          │   │
│  │  │  (扩展A.wasm) │    │  (扩展B.wasm) │          │   │
│  │  │              │    │              │          │   │
│  │  │ • 文件：/tmp │    │ • 文件：无   │          │   │
│  │  │ • 网络：允许 │    │ • 网络：禁止 │          │   │
│  │  │ • 内存：64MB │    │ • 内存：32MB │          │   │
│  │  └──────────────┘    └──────────────┘          │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

---

## 3. 架构设计

### 3.1 扩展系统架构

```
core/extensions/
├── mod.rs           -- 模块导出
├── types.rs         -- 类型定义（ExtensionManifest, WasmExtensionManifest 等）
├── loader.rs        -- WasmExtensionLoader：WASM 动态加载器
├── runner.rs        -- ExtensionRunner：扩展运行器和生命周期管理
├── sandbox.rs       -- WasmSandbox：安全沙箱实现
├── hot_reload.rs    -- HotReloader：文件监控和热重载
├── api.rs           -- 扩展 API 定义
└── builtin/         -- 内置扩展
```

**核心组件：**

| 组件 | 职责 | 关键功能 |
|------|------|----------|
| **WasmExtensionLoader** | WASM 动态加载 | 扫描 `~/.pi/extensions/`、解析 manifest、加载 `.wasm` 文件 |
| **WasmSandbox** | 安全沙箱 | 文件/网络/资源限制、权限验证、fuel metering |
| **HotReloader** | 热重载 | `notify` 监控目录、debounce 2秒、自动 reload |
| **ExtensionRunner** | 生命周期管理 | 初始化、执行、卸载、错误隔离 |
| **ExtensionRegistry** | 扩展注册表 | 工厂注册、启用/禁用配置 |

**扩展加载流程：**

```
1. ExtensionLoader.scan_extensions()
   └── 扫描 ~/.pi/extensions/ 目录
   └── 解析 manifest.json

2. WasmExtensionLoader.load_wasm(manifest)
   └── WasmSandbox::from_manifest_with_dir()
   └── 创建 Engine（fuel metering）
   └── 构建 WasiCtx（权限配置）
   └── Module::from_file()
   └── Linker::default()
   └── Store::new()

3. WasmExtension::initialize()
   └── 调用 WASM 导出函数 init()
   └── 错误隔离（catch_unwind）

4. HotReloader.start_watching()
   └── 监控 .wasm 文件变更
   └── 发送 HotReloadEvent
   └── ExtensionRunner 处理重载
```

### 3.2 快捷键系统架构

```
pi-tui/src/keybindings.rs              -- 核心快捷键管理
pi-coding-agent/src/modes/keybindings_config.rs  -- 配置 UI
```

**核心组件：**

| 组件 | 职责 | 关键功能 |
|------|------|----------|
| **KeybindingsConfig** | 配置持久化 | TOML/JSON 格式、文件加载/保存 |
| **KeybindingsManager** | 运行时管理 | 绑定增删查、冲突检测、上下文匹配 |
| **KeybindingsConfigView** | 配置 UI | 列表显示、按键捕获、预设选择 |
| **KeybindingsPreset** | 预设方案 | Emacs/Vim/VSCode 三套预设 |

**配置文件格式（TOML）：**

```toml
# ~/.pi/keybindings.toml
preset = "Emacs"

[bindings]
"ctrl+c" = "cancel"
"ctrl+v" = "paste"
"ctrl+x" = "cut"
```

**预设方案绑定数量：**

| 预设 | 绑定数量 | 特点 |
|------|----------|------|
| Emacs | 30+ | ctrl+a/e 行首尾，ctrl+k 删除到行尾，alt+f/b 词移动 |
| Vim | 40+ | h/j/k/l 移动，dd/yy 删除复制，i/a 进入插入模式 |
| VSCode | 50+ | ctrl+s 保存，ctrl+p 快速打开，ctrl+shift+p 命令面板 |

**导入/导出流程：**

```
导出：
  KeybindingsManager.to_config()
  └── KeybindingsConfig.export_to_file(path)
      └── 根据扩展名选择 TOML/JSON 格式
      └── 写入文件

导入：
  KeybindingsConfig.import_from_file(path)
  └── 自动检测格式（先 TOML 后 JSON）
  └── KeybindingsManager.merge_import(&config)
      └── 返回被覆盖的绑定列表
```

---

## 4. 实施任务清单

### Task 1: WASM 动态加载（已完成）

**范围**：新增 `loader.rs`/`runner.rs`/`types.rs` 模块，引入 `wasmtime`/`wasmtime-wasi`

**实现内容**：
- `WasmExtensionLoader`：扫描扩展目录、加载 `.wasm` 文件
- `WasmExtension`：WASM 扩展实例封装
- `WasmExtensionManifest`：WASM 扩展 manifest 格式
- `ExtensionLoadError`：加载错误类型

**验收**：扩展文件放入 `~/.pi/extensions/` 后可被扫描和加载

---

### Task 2: 安全沙箱（已完成）

**范围**：新增 `sandbox.rs`，基于 WASI 的完整沙箱

**实现内容**：
- `WasmSandbox`：沙箱主结构
- `WasiSandboxCtx`：WASI 上下文封装
- 文件系统权限控制（`PathPermission`、`DirPerms`、`FilePerms`）
- 网络权限控制（`NetworkPermission`）
- 资源限制（`ResourceLimits`：内存、CPU fuel）
- 权限解析器（`Permission::from_str()`）

**验收**：
- 扩展无法访问未授权的文件路径
- 内存超限触发 OOM 错误
- fuel 耗尽触发执行中断

---

### Task 3: 热重载（已完成）

**范围**：新增 `hot_reload.rs`，使用 `notify` 监控目录

**实现内容**：
- `HotReloader`：热重载主结构
- `HotReloadEvent`：重载事件枚举
- `HotReloadStatus`：重载状态跟踪
- 2 秒 debounce 防抖
- `.wasm` 文件过滤

**验收**：修改 `.wasm` 文件后 3 秒内自动重载

---

### Task 4: 快捷键配置（已完成）

**范围**：扩展 `keybindings.rs` 与 `config.rs`，新增 `keybindings_config.rs` UI

**实现内容**：
- `KeybindingsConfig`：TOML/JSON 配置持久化
- `KeybindingsManager`：运行时绑定管理
- `KeybindingsConfigView`：TUI 配置界面
- `KeybindingsPreset`：三套预设方案
- 导入/导出功能
- 冲突检测

**验收**：
- `/keybindings` 命令打开配置界面
- 配置保存到 `~/.pi/keybindings.toml`
- 预设方案可一键切换

---

### Task 5: 导入导出与预设（已完成）

**范围**：增强 `keybindings.rs` 导入/导出/预设功能

**实现内容**：
- `export_to_json()` / `export_to_toml()`
- `import_from_file()` 自动检测格式
- `apply_preset()` 应用预设
- `merge_import()` 合并导入

**验收**：
- 支持 TOML 和 JSON 两种格式
- 导入时显示被覆盖的绑定

---

## 5. 新增/修改的文件清单

### 新增文件

| 文件 | 行数 | 说明 |
|------|------|------|
| `crates/pi-coding-agent/src/core/extensions/loader.rs` | 890 | WASM 动态加载器、扩展注册表 |
| `crates/pi-coding-agent/src/core/extensions/runner.rs` | 960 | 扩展运行器和生命周期管理 |
| `crates/pi-coding-agent/src/core/extensions/sandbox.rs` | 800 | WASM 安全沙箱 |
| `crates/pi-coding-agent/src/core/extensions/hot_reload.rs` | 486 | 热重载机制 |
| `crates/pi-coding-agent/src/core/extensions/types.rs` | 990 | 类型定义 |
| `crates/pi-coding-agent/src/core/extensions/api.rs` | 140 | 扩展 API |
| `crates/pi-coding-agent/src/modes/keybindings_config.rs` | 793 | 快捷键配置 UI |

### 修改文件

| 文件 | 改动 | 说明 |
|------|------|------|
| `crates/pi-coding-agent/Cargo.toml` | +5 | 添加 wasmtime、wasmtime-wasi、notify、shellexpand 依赖 |
| `crates/pi-coding-agent/src/core/extensions/mod.rs` | 修改 | 模块导出 |
| `crates/pi-tui/Cargo.toml` | +2 | 添加 serde_json、toml 依赖 |
| `crates/pi-tui/src/keybindings.rs` | +1622 | 完整快捷键管理系统 |
| `crates/pi-tui/src/lib.rs` | 修改 | 导出 keybindings 模块 |

---

## 6. 新增依赖

| Crate | 版本 | 用途 | 添加位置 |
|-------|------|------|----------|
| `wasmtime` | 29 | WASM 运行时 | pi-coding-agent |
| `wasmtime-wasi` | 29 | WASI 支持（文件系统、网络等） | pi-coding-agent |
| `notify` | 7 | 文件系统监控（热重载） | pi-coding-agent |
| `serde_json` | 1.0 | JSON 序列化（快捷键导入导出） | pi-tui |
| `toml` | 0.8 | TOML 配置支持 | pi-tui |
| `shellexpand` | 3 | 路径扩展（`~` 支持） | pi-coding-agent |

---

## 7. 测试覆盖

**测试统计：1120 个测试全部通过**

| 模块 | 测试数 | 关键测试 |
|------|--------|----------|
| pi-ai | 142+ | Tokenizer、Provider |
| pi-tui | 包含 keybindings | 配置加载、预设应用、导入导出 |
| pi-coding-agent | 302+ | 扩展加载、沙箱权限、热重载 |
| config_tests | 12 | 多格式配置 |

**快捷键系统测试覆盖：**
- `test_keybinding_definition`：定义创建
- `test_keybindings_manager`：管理器操作
- `test_conflict_detection`：冲突检测
- `test_keybindings_config_toml_roundtrip`：TOML 序列化
- `test_keybindings_config_file_roundtrip`：文件读写
- `test_preset_emacs_bindings`：Emacs 预设
- `test_preset_vim_bindings`：Vim 预设
- `test_preset_vscode_bindings`：VSCode 预设
- `test_apply_preset`：预设应用
- `test_merge_import`：导入合并
- `test_export_import_roundtrip`：导出导入闭环

---

## 8. 已知限制和后续工作

### 已知限制

1. **dead_code warnings**：`loader.rs` 中有 `#[allow(dead_code)]` 标记的预留方法
   - `ExtensionLoader::with_dir()`
   - `ExtensionFactory::description()`
   - `ExtensionRegistry::available_extensions()`
   
2. **扩展系统集成待完善**：WASM 扩展系统基础设施已完成，但上层 Agent 工具集成尚未完成
   - 扩展工具调用接口待实现
   - 扩展 manifest 中的工具声明待解析

3. **热重载测试覆盖**：热重载功能的集成测试需要真实文件系统操作

### 后续工作

1. **扩展工具集成**：
   - 解析扩展 manifest 中的工具声明
   - 将扩展工具注册到 Agent 工具列表
   - 实现工具调用路由

2. **扩展 API 完善**：
   - 定义标准的扩展 API（文件访问、HTTP 请求等）
   - 提供扩展 SDK 和示例

3. **快捷键 UI 增强**：
   - 支持多键序列绑定（如 `ctrl+x ctrl+s`）
   - 支持模式特定绑定切换

---

*文档版本: 1.0*
*创建日期: 2026-04-11*
*基于: ITERATION-4 Phase 5 交付*
