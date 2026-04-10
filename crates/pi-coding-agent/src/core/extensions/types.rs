use async_trait::async_trait;
use pi_agent::types::{AgentTool, AgentEvent, AgentToolResult};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use std::pin::Pin;
use std::future::Future;
use serde::{Serialize, Deserialize};

/// 扩展元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub entry_point: PathBuf,
}

/// 事件处理结果
#[derive(Debug, Clone, Default)]
pub enum EventResult {
    #[default]
    Continue,                    // 继续正常流程
    Block(String),               // 阻止操作（用于 Before* 事件）
    Modified(serde_json::Value), // 修改数据继续（用于 After* 事件）
}

/// 扩展 trait（Trait Object 方案，首版不用 WASM/动态库）
#[async_trait]
pub trait Extension: Send + Sync {
    /// 获取扩展元信息
    fn manifest(&self) -> &ExtensionManifest;
    
    /// 激活扩展
    async fn activate(&mut self, ctx: &super::api::ExtensionContext) -> anyhow::Result<()>;
    
    /// 停用扩展
    async fn deactivate(&mut self) -> anyhow::Result<()>;
    
    /// 获取扩展注册的工具
    fn registered_tools(&self) -> Vec<Arc<dyn AgentTool>>;
    
    /// 获取扩展注册的 slash 命令
    fn registered_commands(&self) -> Vec<SlashCommand>;
    
    /// 处理 Agent 事件（异步，可返回控制信号）
    async fn on_event(&self, event: &AgentEvent) -> anyhow::Result<EventResult> {
        let _ = event;
        Ok(EventResult::Continue)
    }
}

/// 命令来源
#[derive(Debug, Clone)]
pub enum CommandSource {
    Builtin,
    Extension(String), // 扩展名称
}

/// 命令参数
#[derive(Debug, Clone)]
pub struct CommandArgs {
    /// 原始参数字符串
    pub raw: String,
    /// 分割后的参数列表
    pub parts: Vec<String>,
}

impl CommandArgs {
    pub fn new(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let parts: Vec<String> = raw.split_whitespace().map(|s| s.to_string()).collect();
        Self { raw, parts }
    }

    /// 获取第一个参数
    pub fn first(&self) -> Option<&str> {
        self.parts.first().map(|s| s.as_str())
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.raw.trim().is_empty()
    }
}

/// 命令执行结果
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// 要显示的消息
    pub message: String,
    /// 是否需要重新渲染
    pub should_render: bool,
}

impl CommandResult {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            should_render: true,
        }
    }

    pub fn silent() -> Self {
        Self {
            message: String::new(),
            should_render: false,
        }
    }
}

/// 命令处理器类型
pub type SlashCommandHandler = Arc<
    dyn Fn(CommandArgs) -> Pin<Box<dyn Future<Output = anyhow::Result<CommandResult>> + Send>> + Send + Sync
>;

/// Slash 命令定义
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub usage: Option<String>,       // 用法示例, e.g. "/counter-stats [--verbose]"
    pub aliases: Vec<String>,        // 命令别名
    pub source: CommandSource,       // 来源
    pub handler: SlashCommandHandler,
}

impl Clone for SlashCommand {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            usage: self.usage.clone(),
            aliases: self.aliases.clone(),
            source: self.source.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl std::fmt::Debug for SlashCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlashCommand")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("usage", &self.usage)
            .field("aliases", &self.aliases)
            .field("source", &self.source)
            .finish()
    }
}

impl SlashCommand {
    pub fn new(name: impl Into<String>, description: impl Into<String>, handler: SlashCommandHandler) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            usage: None,
            aliases: Vec::new(),
            source: CommandSource::Builtin,
            handler,
        }
    }

    pub fn with_usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = Some(usage.into());
        self
    }

    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }

    pub fn from_extension(
        name: impl Into<String>,
        description: impl Into<String>,
        extension_name: impl Into<String>,
        handler: SlashCommandHandler,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            usage: None,
            aliases: Vec::new(),
            source: CommandSource::Extension(extension_name.into()),
            handler,
        }
    }

    /// 检查名称是否匹配（包括别名）
    pub fn matches(&self, name: &str) -> bool {
        if self.name.eq_ignore_ascii_case(name) {
            return true;
        }
        self.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    }
}

/// 扩展工具包装器 - 带来源和权限信息
pub struct ExtensionToolWrapper {
    inner: Arc<dyn AgentTool>,
    extension_name: String,
    requires_permission: AtomicBool,
}

impl ExtensionToolWrapper {
    pub fn new(inner: Arc<dyn AgentTool>, extension_name: String) -> Self {
        Self {
            inner,
            extension_name,
            requires_permission: AtomicBool::new(true), // 默认需要权限
        }
    }

    pub fn extension_name(&self) -> &str {
        &self.extension_name
    }

    pub fn set_requires_permission(&self, requires: bool) {
        self.requires_permission.store(requires, Ordering::Relaxed);
    }

    pub fn requires_permission(&self) -> bool {
        self.requires_permission.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl AgentTool for ExtensionToolWrapper {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters(&self) -> serde_json::Value {
        self.inner.parameters()
    }

    fn prepare_arguments(&self, args: serde_json::Value) -> serde_json::Value {
        self.inner.prepare_arguments(args)
    }

    async fn execute(
        &self,
        tool_call_id: &str,
        params: serde_json::Value,
        cancel: tokio_util::sync::CancellationToken,
        on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
    ) -> anyhow::Result<AgentToolResult> {
        self.inner.execute(tool_call_id, params, cancel, on_update).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pi_agent::types::{AgentToolResult, AgentEvent};
    use std::path::PathBuf;

    // ==================== EventResult Tests ====================
    
    #[test]
    fn test_event_result_default() {
        let result = EventResult::default();
        assert!(matches!(result, EventResult::Continue));
    }

    #[test]
    fn test_event_result_variants() {
        let continue_result = EventResult::Continue;
        let block_result = EventResult::Block("reason".to_string());
        let modified_result = EventResult::Modified(serde_json::json!({"key": "value"}));
        
        assert!(matches!(continue_result, EventResult::Continue));
        assert!(matches!(block_result, EventResult::Block(_)));
        assert!(matches!(modified_result, EventResult::Modified(_)));
        
        if let EventResult::Block(reason) = block_result {
            assert_eq!(reason, "reason");
        }
        if let EventResult::Modified(value) = modified_result {
            assert_eq!(value["key"], "value");
        }
    }

    // ==================== CommandSource Tests ====================

    #[test]
    fn test_command_source_variants() {
        let builtin = CommandSource::Builtin;
        let extension = CommandSource::Extension("test-extension".to_string());
        
        assert!(matches!(builtin, CommandSource::Builtin));
        assert!(matches!(extension, CommandSource::Extension(_)));
        
        if let CommandSource::Extension(name) = extension {
            assert_eq!(name, "test-extension");
        }
    }

    // ==================== CommandArgs Tests ====================

    #[test]
    fn test_command_args_new() {
        let args = CommandArgs::new("hello world test");
        
        assert_eq!(args.raw, "hello world test");
        assert_eq!(args.parts, vec!["hello", "world", "test"]);
    }

    #[test]
    fn test_command_args_new_empty() {
        let args = CommandArgs::new("");
        
        assert_eq!(args.raw, "");
        assert!(args.parts.is_empty());
        assert!(args.is_empty());
    }

    #[test]
    fn test_command_args_first() {
        let args = CommandArgs::new("first second third");
        
        assert_eq!(args.first(), Some("first"));
        
        let empty_args = CommandArgs::new("");
        assert_eq!(empty_args.first(), None);
    }

    #[test]
    fn test_command_args_is_empty() {
        let args = CommandArgs::new("  ");
        assert!(args.is_empty());
        
        let args_with_content = CommandArgs::new("  content  ");
        assert!(!args_with_content.is_empty());
    }

    // ==================== CommandResult Tests ====================

    #[test]
    fn test_command_result_new() {
        let result = CommandResult::new("test message");
        
        assert_eq!(result.message, "test message");
        assert!(result.should_render);
    }

    #[test]
    fn test_command_result_silent() {
        let result = CommandResult::silent();
        
        assert_eq!(result.message, "");
        assert!(!result.should_render);
    }

    // ==================== SlashCommand Tests ====================

    fn create_test_handler() -> SlashCommandHandler {
        Arc::new(|_args: CommandArgs| {
            Box::pin(async move { Ok(CommandResult::new("test")) })
        })
    }

    #[test]
    fn test_slash_command_new() {
        let handler = create_test_handler();
        let cmd = SlashCommand::new("test-cmd", "Test description", handler);
        
        assert_eq!(cmd.name, "test-cmd");
        assert_eq!(cmd.description, "Test description");
        assert!(cmd.usage.is_none());
        assert!(cmd.aliases.is_empty());
        assert!(matches!(cmd.source, CommandSource::Builtin));
    }

    #[test]
    fn test_slash_command_with_usage() {
        let handler = create_test_handler();
        let cmd = SlashCommand::new("test-cmd", "desc", handler)
            .with_usage("/test-cmd [options]");
        
        assert_eq!(cmd.usage, Some("/test-cmd [options]".to_string()));
    }

    #[test]
    fn test_slash_command_with_aliases() {
        let handler = create_test_handler();
        let cmd = SlashCommand::new("test-cmd", "desc", handler)
            .with_aliases(vec!["tc".to_string(), "t".to_string()]);
        
        assert_eq!(cmd.aliases, vec!["tc", "t"]);
    }

    #[test]
    fn test_slash_command_from_extension() {
        let handler = create_test_handler();
        let cmd = SlashCommand::from_extension(
            "test-cmd",
            "desc",
            "my-extension",
            handler
        );
        
        assert_eq!(cmd.name, "test-cmd");
        assert!(matches!(cmd.source, CommandSource::Extension(_)));
        
        if let CommandSource::Extension(name) = &cmd.source {
            assert_eq!(name, "my-extension");
        }
    }

    #[test]
    fn test_slash_command_matches() {
        let handler = create_test_handler();
        let cmd = SlashCommand::new("test-cmd", "desc", handler)
            .with_aliases(vec!["tc".to_string(), "t".to_string()]);

        // Match by name
        assert!(cmd.matches("test-cmd"));
        assert!(cmd.matches("TEST-CMD")); // case insensitive
        
        // Match by alias
        assert!(cmd.matches("tc"));
        assert!(cmd.matches("TC")); // case insensitive
        assert!(cmd.matches("t"));
        
        // No match
        assert!(!cmd.matches("other"));
        assert!(!cmd.matches("test"));
    }

    #[test]
    fn test_slash_command_clone() {
        let handler = create_test_handler();
        let cmd = SlashCommand::new("test-cmd", "desc", handler)
            .with_usage("/test-cmd")
            .with_aliases(vec!["t".to_string()]);
        
        let cloned = cmd.clone();
        
        assert_eq!(cloned.name, cmd.name);
        assert_eq!(cloned.description, cmd.description);
        assert_eq!(cloned.usage, cmd.usage);
        assert_eq!(cloned.aliases, cmd.aliases);
    }

    // ==================== ExtensionToolWrapper Tests ====================

    struct MockTool {
        name: String,
    }

    #[async_trait]
    impl AgentTool for MockTool {
        fn name(&self) -> &str { &self.name }
        fn label(&self) -> &str { "Mock Tool" }
        fn description(&self) -> &str { "A mock tool for testing" }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({ "type": "object" })
        }
        
        async fn execute(
            &self,
            _tool_call_id: &str,
            _params: serde_json::Value,
            _cancel: tokio_util::sync::CancellationToken,
            _on_update: Option<Box<dyn Fn(AgentToolResult) + Send + Sync>>,
        ) -> anyhow::Result<AgentToolResult> {
            Ok(AgentToolResult {
                content: vec![],
                details: serde_json::Value::Null,
            })
        }
    }

    #[test]
    fn test_extension_tool_wrapper_new() {
        let mock_tool = Arc::new(MockTool { name: "mock".to_string() });
        let wrapper = ExtensionToolWrapper::new(mock_tool, "test-extension".to_string());
        
        assert_eq!(wrapper.name(), "mock");
        assert_eq!(wrapper.label(), "Mock Tool");
        assert_eq!(wrapper.description(), "A mock tool for testing");
        assert_eq!(wrapper.extension_name(), "test-extension");
        assert!(wrapper.requires_permission()); // default is true
    }

    #[test]
    fn test_extension_tool_wrapper_permission() {
        let mock_tool = Arc::new(MockTool { name: "mock".to_string() });
        let wrapper = ExtensionToolWrapper::new(mock_tool, "ext".to_string());
        
        assert!(wrapper.requires_permission());
        
        wrapper.set_requires_permission(false);
        assert!(!wrapper.requires_permission());
        
        wrapper.set_requires_permission(true);
        assert!(wrapper.requires_permission());
    }

    #[tokio::test]
    async fn test_extension_tool_wrapper_execute() {
        let mock_tool = Arc::new(MockTool { name: "mock".to_string() });
        let wrapper = ExtensionToolWrapper::new(mock_tool, "ext".to_string());
        
        let cancel = tokio_util::sync::CancellationToken::new();
        let result = wrapper.execute("call-id", serde_json::json!({}), cancel, None).await;
        
        assert!(result.is_ok());
    }

    // ==================== ExtensionManifest Tests ====================

    #[test]
    fn test_extension_manifest() {
        let manifest = ExtensionManifest {
            name: "test-ext".to_string(),
            version: "1.0.0".to_string(),
            description: "Test extension".to_string(),
            author: "test".to_string(),
            entry_point: PathBuf::from("test.rs"),
        };
        
        assert_eq!(manifest.name, "test-ext");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "Test extension");
        assert_eq!(manifest.author, "test");
        assert_eq!(manifest.entry_point, PathBuf::from("test.rs"));
    }

    #[test]
    fn test_extension_manifest_serialization() {
        let manifest = ExtensionManifest {
            name: "test-ext".to_string(),
            version: "1.0.0".to_string(),
            description: "Test extension".to_string(),
            author: "test".to_string(),
            entry_point: PathBuf::from("test.rs"),
        };
        
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: ExtensionManifest = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.name, manifest.name);
        assert_eq!(parsed.version, manifest.version);
    }
}
