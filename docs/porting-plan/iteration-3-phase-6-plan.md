# ITERATION-3 Phase 6: TUI 组件补全与质量提升

## 现状摘要

- **编译警告**: 92 个 cargo check 警告 + 99 个 clippy 警告，主要为 dead_code、unused 字段/方法、ptr_arg 等
- **测试**: 881 个测试全部通过，但缺乏跨 crate 集成测试
- **文档**: pi-tui 组件文档完整，pi-agent/pi-ai/pi-coding-agent 缺少模块级文档
- **组件**: TruncatedText 已在 `text.rs` 中实现（第 95-173 行），无需新建；SettingsList 和 CancellableLoader 缺失

## 关键调整说明

原计划中 TruncatedText 组件标注为"未实现"，但调研发现**已在 `crates/pi-tui/src/components/text.rs` 中完整实现**（含截断、省略号自定义、样式应用），因此跳过该任务。

---

## Task 1: SettingsList 组件实现

**目标**: 新建设置列表组件，支持多种设置类型的显示和编辑

**文件操作**:
- 新建 `crates/pi-tui/src/components/settings_list.rs`
- 修改 `crates/pi-tui/src/components/mod.rs` 添加导出

**实现要点**:
1. 定义 `SettingItem` 枚举（Boolean/String/Number/Enum 四种类型）
2. 定义 `SettingsCategory` 结构（名称 + 设置项列表）
3. 实现 `SettingsList` 结构体，参考 `SelectList`（`select_list.rs`）的导航和滚动逻辑
4. 实现 Component trait（render 方法渲染分类标题、设置名称、描述、当前值）
5. 实现 Focusable trait，支持键盘导航和值编辑
6. 支持搜索过滤（复用模糊匹配逻辑）
7. 包含至少 8 个单元测试

**参考文件**: `crates/pi-tui/src/components/select_list.rs`（442 行，布局和导航逻辑）

---

## Task 2: CancellableLoader 组件实现

**目标**: 扩展现有 Loader 组件为可取消版本

**文件操作**:
- 新建 `crates/pi-tui/src/components/cancellable_loader.rs`
- 修改 `crates/pi-tui/src/components/mod.rs` 添加导出

**实现要点**:
1. 基于 `Loader`（`loader.rs`，135 行）的动画逻辑
2. 添加取消提示文本（如 "Press Esc to cancel"）
3. 添加可选进度百分比显示
4. 实现 `on_cancel` 回调
5. handle_input 处理 Esc 键取消
6. 包含至少 5 个单元测试

**参考文件**: `crates/pi-tui/src/components/loader.rs`（135 行）

---

## Task 3: 编译警告清理

**目标**: 消除所有 `cargo check` 和 `cargo clippy` 警告，达到零警告

**涉及文件**（按 crate 分组）:

### pi-coding-agent（75 个警告，最多）
- `src/modes/theme.rs` - 多个未读字段
- `src/core/permissions.rs` - 多个未使用方法
- `src/core/tools/find.rs:62` - clippy ptr_arg（`&PathBuf` -> `&Path`）
- `src/core/tools/read.rs:243` - clippy map_identity
- `src/core/tools/write.rs:93` - clippy let_underscore_future
- `src/core/tools/grep.rs:84` - clippy needless_range_loop
- `src/core/extensions/builtin/example_counter.rs:24` - clippy missing_default
- `src/core/agent_session.rs:183` - unused_must_use
- `src/modes/interactive.rs:138` - unused_must_use
- `src/modes/print_mode.rs:48` - unused_must_use

### pi-ai（10 个警告）
- `src/providers/openai.rs` - 反序列化结构体未读字段
- `src/providers/mistral.rs` - 字段未读

### pi-tui（5 个警告）
- `src/keys.rs` - 未使用的函数 + rustdoc bare URL

### pi-agent（2 个警告）
- `src/agent_loop.rs` - `final_message` 未使用变量

**清理策略**:
1. 先运行 `cargo clippy --fix` 自动修复可处理的问题
2. 手动处理未使用的字段/方法：确认是未来预留则添加 `#[allow(dead_code)]` 注释说明原因，否则删除
3. 修复 rustdoc 格式（bare URL 包裹 `<>`、未闭合 HTML tag）
4. 目标：`cargo check` 和 `cargo clippy` 零警告

---

## Task 4: 测试覆盖提升

**目标**: 为核心模块补充单元测试和集成测试

**涉及文件**:

### pi-ai Provider 边界测试
- `src/providers/anthropic.rs` - 错误响应、速率限制、超长内容
- `src/providers/openai.rs` - 流式中断、非标准响应
- `src/providers/google.rs` - 安全过滤响应
- `src/providers/bedrock.rs` - AWS 凭证过期

### pi-tui 组件渲染测试
- `src/components/editor.rs` - 多行编辑边界、Unicode 宽度
- `src/components/markdown.rs` - 嵌套列表、代码块、表格
- `src/components/box_component.rs` - 极端宽度、嵌套 box

### pi-agent 状态机测试
- `src/agent_loop.rs` - 工具调用超时、并发消息、状态转换

### pi-coding-agent 工具集成测试
- `tests/` 目录 - 工具组合调用、权限检查、会话持久化

### 新增集成测试
- 新建 `crates/pi-coding-agent/tests/tool_integration_tests.rs` - 跨工具场景
- 新建 `crates/pi-ai/tests/provider_integration_tests.rs` - Provider 统一行为验证

**目标覆盖率**: 核心模块 > 70%

---

## Task 5: 公共 API 文档补充

**目标**: 为所有公共 API 补充 rustdoc 文档注释，`cargo doc` 零警告

**涉及文件**:

### pi-agent
- `src/lib.rs` - 添加 crate 级文档（`//!`）
- `src/agent.rs` - Agent 结构体和方法文档
- `src/agent_loop.rs` - AgentLoop 工作流程文档
- `src/types.rs` - 核心类型文档

### pi-ai
- `src/lib.rs` - crate 级文档和使用示例
- `src/types.rs` - Api、Provider 枚举文档
- `src/stream.rs` - 流式 API 文档
- `src/api_registry.rs` - Provider 注册机制文档
- `src/token_counter.rs` - Token 计数器文档

### pi-coding-agent
- `src/lib.rs` - crate 级文档
- `src/core/agent_session.rs` - 会话管理文档
- `src/core/tools/mod.rs` - 工具系统概述
- `src/core/extensions/types.rs` - 扩展 Trait 文档

### pi-tui（已有组件级文档，补充 crate 级）
- `src/lib.rs` - crate 级文档和架构概述
- `src/tui.rs` - Component/Focusable trait 使用示例

**文档规范**:
- 每个 crate 的 `lib.rs` 包含 `//!` 模块级描述
- 每个 public struct/trait/enum 包含 `///` 描述
- 关键方法包含用法示例（`/// # Examples`）
- 修复现有 rustdoc 警告（bare URL、未闭合 tag）

---

## Task 间依赖关系

```
Task 1 (SettingsList) ──┐
                        ├──→ Task 4 (测试覆盖，含新组件测试)
Task 2 (CancellableLoader)┘         │
                                     ├──→ Task 5 (文档，含新组件文档)
Task 3 (警告清理) ──────────────────┘
```

- Task 1 和 Task 2 可并行开发
- Task 3 可与 Task 1/2 并行（不同文件）
- Task 4 依赖 Task 1/2 完成（需要测试新组件）
- Task 5 依赖 Task 3/4 完成（文档需反映最终代码状态）

---

## 验证标准

1. SettingsList 组件可正确显示和编辑设置项
2. CancellableLoader 组件显示加载状态并支持 Esc 取消
3. `cargo check` 零警告
4. `cargo clippy` 零警告
5. `cargo test` 全部通过，通过率 > 95%
6. `cargo doc --no-deps` 零警告，文档完整生成

---

*文档版本: 1.0*
*创建日期: 2026-04-10*
*基于: ITERATION-3 Phase 6 计划*
