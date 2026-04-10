# ITERATION-3 Phase 5: 扩展系统增强 - 开发计划

## 概述

将扩展系统从基础框架升级为功能完善的扩展框架，包含 20+ 事件钩子、完善的工具/命令注册机制、编译时链接扩展加载策略，以及示例扩展验证端到端流程。

## 现状（Phase 5 开始前）

| 组件 | 文件 | 状态 |
|------|------|------|
| Extension trait | types.rs (66行) | 基础 trait: manifest/activate/deactivate/registered_tools/registered_commands/on_event |
| ExtensionContext | api.rs (26行) | 仅提供 cwd/config/session_id/data_dir |
| ExtensionManager | runner.rs (101行) | 基础生命周期管理 |
| ExtensionLoader | loader.rs (78行) | 仅扫描 manifest.json |
| 事件类型 | pi-agent/types.rs | 11 种事件 |
| 集成 | agent_session.rs | 仅初始化，未加载扩展 |

## 任务分解

### Task 1: 扩展事件系统增强
- 在 AgentEvent 枚举中新增 BeforeAgentStart, BeforeToolCall, AfterToolCall, BeforeCommandExecute, AfterCommandExecute, ExtensionLoaded, ExtensionError 事件
- 新增 EventResult 枚举（Continue/Block/Modified）
- Extension trait 的 on_event 改为异步并返回 EventResult
- agent_loop.rs 补充 BeforeToolCall/AfterToolCall emit

### Task 2: ExtensionContext API 完善
- 新增 ExtensionLogger 日志记录器
- ExtensionContext 增加 log_prefix, extension_dir, tool_registry, command_registry
- 实现 register_tool, register_command, read_data, write_data, logger 方法

### Task 3: 工具注册机制完善
- 新增 ExtensionToolWrapper（带权限和来源信息）
- ExtensionManager 增加动态工具管理（register_tool/unregister_tool/get_extension_tools/get_tool_source）

### Task 4: 命令注册机制完善
- 新增 CommandSource, CommandArgs, CommandResult 类型
- 增强 SlashCommand（usage, aliases, source）
- interactive.rs 增加动态命令查找和执行
- 自动补全支持扩展命令

### Task 5: 扩展加载策略
- 新增 ExtensionFactory trait 和 ExtensionRegistry
- 编译时链接 + 配置激活方案
- config.rs 增加 ExtensionsConfig

### Task 6: 示例扩展实现
- example-counter 扩展：统计工具调用和消息数
- 注册 /counter-stats 命令和 counter_reset 工具
- 监听 ToolExecutionEnd 和 MessageEnd 事件

### Task 7: 集成与命令增强
- agent_session.rs 使用 ExtensionRegistry 加载扩展
- 扩展工具合并到 Agent 工具列表
- /extensions 支持 list/info/enable/disable 子命令

### Task 8: 单元测试和集成测试
- types.rs 测试：EventResult, CommandArgs, SlashCommand, ExtensionToolWrapper
- runner.rs 测试：ExtensionManager 完整生命周期和动态工具
- loader.rs 测试：ExtensionRegistry 工厂注册和加载
- example_counter.rs 端到端测试

## 实施顺序

```
批次 1: Task 1 (事件增强) + Task 2 (Context API)     -- 并行
批次 2: Task 3 (工具注册) + Task 5 (加载策略)         -- 并行
         Task 4 (命令注册)                             -- 依赖 Task 1 + Task 5
批次 3: Task 6 (示例扩展) + Task 7 (集成增强)         -- 串行
批次 4: Task 8 (测试)
```

## 文件变更汇总

| 文件 | 操作 | 说明 |
|------|------|------|
| crates/pi-agent/src/types.rs | 修改 | 新增 7 种事件枚举变体 |
| crates/pi-agent/src/agent_loop.rs | 修改 | 补充 BeforeToolCall/AfterToolCall emit |
| crates/pi-coding-agent/src/core/extensions/types.rs | 重构 | EventResult, CommandSource/Args/Result, ExtensionToolWrapper |
| crates/pi-coding-agent/src/core/extensions/api.rs | 重构 | ExtensionContext 交互能力, ExtensionLogger |
| crates/pi-coding-agent/src/core/extensions/runner.rs | 重构 | 动态工具/命令管理, 异步 dispatch_event |
| crates/pi-coding-agent/src/core/extensions/loader.rs | 重构 | ExtensionFactory, ExtensionRegistry |
| crates/pi-coding-agent/src/core/extensions/mod.rs | 修改 | 导出新类型和 builtin 模块 |
| crates/pi-coding-agent/src/core/extensions/builtin/mod.rs | 新增 | 内置扩展模块 |
| crates/pi-coding-agent/src/core/extensions/builtin/example_counter.rs | 新增 | 示例扩展 |
| crates/pi-coding-agent/src/core/agent_session.rs | 修改 | ExtensionRegistry 集成, 扩展工具合并 |
| crates/pi-coding-agent/src/modes/interactive.rs | 修改 | 动态命令路由, /extensions 子命令 |
| crates/pi-coding-agent/src/modes/autocomplete_providers.rs | 修改 | 扩展命令自动补全 |
| crates/pi-coding-agent/src/config.rs | 修改 | ExtensionsConfig 配置项 |

## 验证标准

1. 示例扩展注册自定义工具（counter_reset）
2. 示例扩展注册自定义 Slash 命令（/counter-stats）
3. 扩展监听 Agent 生命周期事件
4. /extensions list 显示已加载扩展
5. /extensions info 显示扩展详情
6. 扩展加载失败不影响主程序
7. cargo check 通过
8. cargo test 全部通过
9. cargo clippy 零警告
