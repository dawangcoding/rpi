# ITERATION-3 Phase 4: 会话系统完善开发计划

## 概述

Phase 4 的目标是完善会话压缩和 Fork 功能，增强 Token 计数和成本统计。基于现有实现（压缩系统~85%、Fork~60%、统计~50%、Token计数~60%），进行补全和优化。

## 现状分析

| 功能 | 现状 | 完成度 | 剩余工作 |
|------|------|--------|----------|
| 会话压缩 | compactor.rs 520行，自动/手动触发完整 | ~85% | 压缩范围算法优化、摘要提示词增强 |
| 会话 Fork | fork_session/list_forks/get_session_tree 完整 | ~60% | /forks、/switch 命令、分支删除、树形可视化 |
| 会话统计 | SessionStats/TokenStats 结构完整，/stats 命令存在 | ~50% | 成本计算逻辑、统计持久化、输出增强 |
| Token 计数 | EstimateTokenCounter + ModelTokenCounter | ~60% | tiktoken-rs 集成、精度提升 |

已有 Slash 命令: /help, /clear, /model, /exit, /stats, /save, /fork, /export, /compact, /extensions, /login, /logout, /auth, /theme (共14条)

缺失命令: /forks, /switch, /delete-fork, /tree

---

## Task 1: Token 计数精度提升（基础层）

**目标**: 集成 tiktoken-rs 为 OpenAI 模型提供精确 token 计数，优化其他模型估算。

**修改文件**:
- `crates/pi-ai/Cargo.toml` - 添加 tiktoken-rs 依赖
- `crates/pi-ai/src/token_counter.rs` - 新增 TiktokenCounter 实现

**具体工作**:
1. 在 Cargo.toml 添加依赖: `tiktoken-rs = "0.6"`
2. 新增 TiktokenCounter 结构体，实现 TokenCounter trait
   - 使用 `tiktoken_rs::get_bpe_from_model` 加载对应模型的 tokenizer
   - `count_text()` 使用真实 BPE 编码计数
   - `count_message()` / `count_messages()` 考虑消息格式开销
3. 新增 `create_token_counter(model: &str) -> Arc<dyn TokenCounter>` 工厂函数
4. 优化 ModelTokenCounter 的字符/token 比率
5. 补充单元测试

**依赖**: 无

---

## Task 2: 成本计算逻辑实现

**目标**: 利用 ModelCost 定价信息，实现精确的会话成本计算。

**修改文件**:
- `crates/pi-coding-agent/src/core/agent_session.rs` - 增强统计逻辑

**具体工作**:
1. 在 AgentSession 中添加 `model_cost: Option<ModelCost>` 字段
2. 在 handle_event 的 MessageEnd 分支中计算成本
3. 累加到 SessionStats.cost 字段
4. 根据当前模型从 models.rs 查找 ModelCost
5. 补充单元测试

**依赖**: Task 1

---

## Task 3: Fork 命令补全与增强

**目标**: 补全 /forks、/switch、/delete-fork、/tree 命令，实现分支删除和树形可视化。

**修改文件**:
- `crates/pi-coding-agent/src/core/session_manager.rs` - 添加 delete/format 方法
- `crates/pi-coding-agent/src/modes/interactive.rs` - 新增命令处理
- `crates/pi-coding-agent/src/modes/autocomplete_providers.rs` - 补全项更新

**具体工作**:
1. session_manager.rs: 新增 delete_session、delete_fork_tree、format_session_tree 方法
2. interactive.rs: 实现 /forks、/switch、/delete-fork、/tree 命令
3. autocomplete_providers.rs: 注册新命令

**依赖**: 无

---

## Task 4: 会话统计增强与持久化

**目标**: 增强 /stats 输出，实现统计持久化，优化状态栏实时显示。

**修改文件**:
- `crates/pi-coding-agent/src/core/agent_session.rs` - 统计持久化
- `crates/pi-coding-agent/src/core/session_manager.rs` - SavedSession 扩展
- `crates/pi-coding-agent/src/modes/interactive.rs` - /stats 输出增强
- `crates/pi-coding-agent/src/modes/message_components.rs` - 状态栏增强

**依赖**: Task 2, Task 3

---

## Task 5: 压缩算法优化

**目标**: 优化压缩范围确定算法和摘要提示词质量。

**修改文件**:
- `crates/pi-coding-agent/src/core/compaction/compactor.rs`
- `crates/pi-coding-agent/src/core/compaction/summary_prompt.rs`

**具体工作**:
1. 改进 determine_compress_range: 按 token 预算动态确定保留数量
2. 添加工具调用边界对齐
3. 优化摘要提示词（添加 CWD、Active Files、Error Context）
4. 优化 build_summary_prompt（截断策略调整）
5. 压缩统计显示增强

**依赖**: Task 1

---

## Task 6: 集成测试与验证

**目标**: 确保所有 Phase 4 功能端到端工作正常。

**依赖**: Task 4, Task 5

---

## 执行顺序

```
Task 1 (Token计数) ──┬──→ Task 2 (成本计算) ──→ Task 4 (统计增强)
                     │
                     └──→ Task 5 (压缩优化)

Task 3 (Fork命令) ────────────────────────────→ Task 4 (统计增强)

Task 4 + Task 5 ──→ Task 6 (集成验证)
```

---

*文档版本: 1.0*
*创建日期: 2026-04-10*
*基于: ITERATION-3.md Phase 4*
