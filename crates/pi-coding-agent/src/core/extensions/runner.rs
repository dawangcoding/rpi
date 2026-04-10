use super::types::{EventResult, Extension, SlashCommand, ExtensionToolWrapper};
use super::api::ExtensionContext;
use pi_agent::types::{AgentTool, AgentEvent};
use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Result;

/// 扩展管理器 - 管理所有已加载扩展的生命周期
pub struct ExtensionManager {
    extensions: Vec<Box<dyn Extension>>,
    activated: bool,
    /// 扩展动态注册的工具 (extension_name -> tools)
    dynamic_tools: HashMap<String, Vec<Arc<dyn AgentTool>>>,
}

impl ExtensionManager {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            activated: false,
            dynamic_tools: HashMap::new(),
        }
    }
    
    /// 注册一个扩展
    pub fn register(&mut self, extension: Box<dyn Extension>) {
        tracing::info!("Registered extension: {}", extension.manifest().name);
        self.extensions.push(extension);
    }
    
    /// 激活所有已注册的扩展
    pub async fn activate_all(&mut self, ctx: &ExtensionContext) -> Result<()> {
        for ext in &mut self.extensions {
            let name = ext.manifest().name.clone();
            match ext.activate(ctx).await {
                Ok(()) => tracing::info!("Activated extension: {}", name),
                Err(e) => tracing::error!("Failed to activate extension {}: {}", name, e),
            }
        }
        self.activated = true;
        Ok(())
    }
    
    /// 停用所有扩展
    pub async fn deactivate_all(&mut self) -> Result<()> {
        for ext in &mut self.extensions {
            let name = ext.manifest().name.clone();
            match ext.deactivate().await {
                Ok(()) => tracing::debug!("Deactivated extension: {}", name),
                Err(e) => tracing::warn!("Failed to deactivate extension {}: {}", name, e),
            }
        }
        self.activated = false;
        Ok(())
    }
    
    /// 收集所有扩展注册的工具
    pub fn get_all_tools(&self) -> Vec<Arc<dyn AgentTool>> {
        let mut tools = Vec::new();
        // 扩展通过 registered_tools() 返回的工具
        for ext in &self.extensions {
            tools.extend(ext.registered_tools());
        }
        // 动态注册的工具
        for ext_tools in self.dynamic_tools.values() {
            tools.extend(ext_tools.iter().cloned());
        }
        tools
    }

    /// 动态注册工具
    pub fn register_tool(&mut self, extension_name: &str, tool: Arc<dyn AgentTool>) {
        let wrapped = Arc::new(ExtensionToolWrapper::new(tool, extension_name.to_string()));
        self.dynamic_tools
            .entry(extension_name.to_string())
            .or_default()
            .push(wrapped);
        tracing::info!(
            "Extension {} registered dynamic tool: {}",
            extension_name,
            self.dynamic_tools.get(extension_name).unwrap().last().unwrap().name()
        );
    }

    /// 取消注册工具
    pub fn unregister_tool(&mut self, extension_name: &str, tool_name: &str) {
        if let Some(tools) = self.dynamic_tools.get_mut(extension_name) {
            tools.retain(|t| t.name() != tool_name);
            tracing::info!("Extension {} unregistered tool: {}", extension_name, tool_name);
        }
    }

    /// 获取指定扩展的工具列表
    pub fn get_extension_tools(&self, extension_name: &str) -> Vec<Arc<dyn AgentTool>> {
        self.dynamic_tools.get(extension_name).cloned().unwrap_or_default()
    }

    /// 获取工具的来源扩展名称
    pub fn get_tool_source(&self, tool_name: &str) -> Option<String> {
        for (ext_name, tools) in &self.dynamic_tools {
            if tools.iter().any(|t| t.name() == tool_name) {
                return Some(ext_name.clone());
            }
        }
        None
    }
    
    /// 收集所有扩展注册的 slash 命令
    pub fn get_all_commands(&self) -> Vec<SlashCommand> {
        let mut commands = Vec::new();
        for ext in &self.extensions {
            commands.extend(ext.registered_commands());
        }
        commands
    }
    
    /// 向所有扩展分发事件
    pub async fn dispatch_event(&self, event: &AgentEvent) -> Vec<EventResult> {
        let mut results = Vec::new();
        for ext in &self.extensions {
            match ext.on_event(event).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!("Extension {} event handler error: {}", ext.manifest().name, e);
                    results.push(EventResult::Continue);
                }
            }
        }
        results
    }
    
    /// 获取已注册扩展数量
    pub fn extension_count(&self) -> usize {
        self.extensions.len()
    }
    
    /// 获取扩展列表信息
    pub fn list_extensions(&self) -> Vec<&super::types::ExtensionManifest> {
        self.extensions.iter().map(|e| e.manifest()).collect()
    }
    
    /// 是否已激活
    pub fn is_activated(&self) -> bool {
        self.activated
    }
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::core::extensions::types::{ExtensionManifest, Extension, EventResult, CommandArgs, CommandResult, SlashCommand, CommandSource};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::pin::Pin;
    use std::future::Future;
    use std::path::PathBuf;

    // ==================== Mock Extension ====================

    struct MockExtension {
        manifest: ExtensionManifest,
        activated: Arc<AtomicBool>,
    }

    impl MockExtension {
        fn new(name: &str) -> Self {
            Self {
                manifest: ExtensionManifest {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Mock extension for testing".to_string(),
                    author: "test".to_string(),
                    entry_point: PathBuf::new(),
                },
                activated: Arc::new(AtomicBool::new(false)),
            }
        }
        
        fn is_activated(&self) -> bool {
            self.activated.load(Ordering::Relaxed)
        }
    }

    #[async_trait::async_trait]
    impl Extension for MockExtension {
        fn manifest(&self) -> &ExtensionManifest {
            &self.manifest
        }
        
        async fn activate(&mut self, _ctx: &super::super::api::ExtensionContext) -> anyhow::Result<()> {
            self.activated.store(true, Ordering::Relaxed);
            Ok(())
        }
        
        async fn deactivate(&mut self) -> anyhow::Result<()> {
            self.activated.store(false, Ordering::Relaxed);
            Ok(())
        }
        
        fn registered_tools(&self) -> Vec<Arc<dyn AgentTool>> {
            vec![]
        }
        
        fn registered_commands(&self) -> Vec<SlashCommand> {
            vec![]
        }
        
        async fn on_event(&self, _event: &AgentEvent) -> anyhow::Result<EventResult> {
            Ok(EventResult::Continue)
        }
    }

    // ==================== Mock Tool ====================

    struct MockTool {
        name: String,
    }

    impl MockTool {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    #[async_trait::async_trait]
    impl AgentTool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn label(&self) -> &str {
            &self.name
        }
        
        fn description(&self) -> &str {
            "Mock tool for testing"
        }
        
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        
        async fn execute(
            &self,
            _tool_call_id: &str,
            _params: serde_json::Value,
            _cancel: tokio_util::sync::CancellationToken,
            _on_update: Option<Box<dyn Fn(pi_agent::types::AgentToolResult) + Send + Sync>>,
        ) -> anyhow::Result<pi_agent::types::AgentToolResult> {
            Ok(pi_agent::types::AgentToolResult {
                content: vec![],
                details: serde_json::Value::Null,
            })
        }
    }

    // ==================== ExtensionManager Tests ====================

    #[test]
    fn test_extension_manager_new() {
        let manager = ExtensionManager::new();
        
        assert_eq!(manager.extension_count(), 0);
        assert!(!manager.is_activated());
    }

    #[test]
    fn test_extension_manager_register() {
        let mut manager = ExtensionManager::new();
        
        manager.register(Box::new(MockExtension::new("ext1")));
        assert_eq!(manager.extension_count(), 1);
        
        manager.register(Box::new(MockExtension::new("ext2")));
        assert_eq!(manager.extension_count(), 2);
    }

    #[test]
    fn test_extension_manager_default() {
        let manager = ExtensionManager::default();
        assert_eq!(manager.extension_count(), 0);
    }

    #[tokio::test]
    async fn test_extension_manager_activate_all() {
        let mut manager = ExtensionManager::new();
        let ext1 = MockExtension::new("ext1");
        let activated1 = ext1.activated.clone();
        manager.register(Box::new(ext1));
        
        let ext2 = MockExtension::new("ext2");
        let activated2 = ext2.activated.clone();
        manager.register(Box::new(ext2));
        
        assert!(!activated1.load(Ordering::Relaxed));
        assert!(!activated2.load(Ordering::Relaxed));
        
        let config = AppConfig::default();
        let ctx = super::super::api::ExtensionContext::new(
            PathBuf::from("."),
            config,
            "test-session".to_string(),
            "test-ext",
        );
        
        let result = manager.activate_all(&ctx).await;
        assert!(result.is_ok());
        assert!(manager.is_activated());
    }

    #[tokio::test]
    async fn test_extension_manager_deactivate_all() {
        let mut manager = ExtensionManager::new();
        let ext = MockExtension::new("ext1");
        let activated = ext.activated.clone();
        manager.register(Box::new(ext));
        
        let config = AppConfig::default();
        let ctx = super::super::api::ExtensionContext::new(
            PathBuf::from("."),
            config,
            "test-session".to_string(),
            "test-ext",
        );
        
        manager.activate_all(&ctx).await.unwrap();
        assert!(activated.load(Ordering::Relaxed));
        
        let result = manager.deactivate_all().await;
        assert!(result.is_ok());
        assert!(!manager.is_activated());
    }

    #[tokio::test]
    async fn test_extension_manager_dispatch_event() {
        let mut manager = ExtensionManager::new();
        manager.register(Box::new(MockExtension::new("ext1")));
        manager.register(Box::new(MockExtension::new("ext2")));
        
        let event = AgentEvent::AgentStart;
        let results = manager.dispatch_event(&event).await;
        
        assert_eq!(results.len(), 2);
        for result in results {
            assert!(matches!(result, EventResult::Continue));
        }
    }

    #[test]
    fn test_extension_manager_register_tool() {
        let mut manager = ExtensionManager::new();
        let tool = Arc::new(MockTool::new("test-tool"));
        
        manager.register_tool("test-ext", tool);
        
        let tools = manager.get_extension_tools("test-ext");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "test-tool");
    }

    #[test]
    fn test_extension_manager_unregister_tool() {
        let mut manager = ExtensionManager::new();
        let tool = Arc::new(MockTool::new("test-tool"));
        
        manager.register_tool("test-ext", tool);
        assert_eq!(manager.get_extension_tools("test-ext").len(), 1);
        
        manager.unregister_tool("test-ext", "test-tool");
        assert_eq!(manager.get_extension_tools("test-ext").len(), 0);
    }

    #[test]
    fn test_extension_manager_get_extension_tools() {
        let mut manager = ExtensionManager::new();
        
        // Non-existent extension returns empty
        assert!(manager.get_extension_tools("nonexistent").is_empty());
        
        // Add tools
        manager.register_tool("ext1", Arc::new(MockTool::new("tool1")));
        manager.register_tool("ext1", Arc::new(MockTool::new("tool2")));
        manager.register_tool("ext2", Arc::new(MockTool::new("tool3")));
        
        assert_eq!(manager.get_extension_tools("ext1").len(), 2);
        assert_eq!(manager.get_extension_tools("ext2").len(), 1);
    }

    #[test]
    fn test_extension_manager_get_tool_source() {
        let mut manager = ExtensionManager::new();
        manager.register_tool("ext1", Arc::new(MockTool::new("tool1")));
        manager.register_tool("ext2", Arc::new(MockTool::new("tool2")));
        
        assert_eq!(manager.get_tool_source("tool1"), Some("ext1".to_string()));
        assert_eq!(manager.get_tool_source("tool2"), Some("ext2".to_string()));
        assert_eq!(manager.get_tool_source("nonexistent"), None);
    }

    #[test]
    fn test_extension_manager_get_all_tools() {
        let mut manager = ExtensionManager::new();
        
        // Static tools from extensions
        // (MockExtension returns empty registered_tools)
        manager.register(Box::new(MockExtension::new("ext1")));
        
        // Dynamic tools
        manager.register_tool("ext1", Arc::new(MockTool::new("dynamic-tool")));
        
        let all_tools = manager.get_all_tools();
        assert_eq!(all_tools.len(), 1);
        assert_eq!(all_tools[0].name(), "dynamic-tool");
    }

    #[test]
    fn test_extension_manager_list_extensions() {
        let mut manager = ExtensionManager::new();
        manager.register(Box::new(MockExtension::new("ext1")));
        manager.register(Box::new(MockExtension::new("ext2")));
        
        let manifests = manager.list_extensions();
        assert_eq!(manifests.len(), 2);
        
        let names: Vec<&str> = manifests.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"ext1"));
        assert!(names.contains(&"ext2"));
    }
}
