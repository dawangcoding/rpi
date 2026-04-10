use super::types::{Extension, SlashCommand};
use super::api::ExtensionContext;
use pi_agent::types::{AgentTool, AgentEvent};
use std::sync::Arc;
use anyhow::Result;

/// 扩展管理器 - 管理所有已加载扩展的生命周期
pub struct ExtensionManager {
    extensions: Vec<Box<dyn Extension>>,
    activated: bool,
}

impl ExtensionManager {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            activated: false,
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
        for ext in &self.extensions {
            tools.extend(ext.registered_tools());
        }
        tools
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
    pub fn dispatch_event(&self, event: &AgentEvent) {
        for ext in &self.extensions {
            if let Err(e) = ext.on_event(event) {
                tracing::warn!("Extension {} event handler error: {}", ext.manifest().name, e);
            }
        }
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
