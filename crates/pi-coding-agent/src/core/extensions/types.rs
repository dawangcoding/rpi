use async_trait::async_trait;
use pi_agent::types::{AgentTool, AgentEvent};
use std::sync::Arc;
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
    
    /// 处理 Agent 事件
    fn on_event(&self, event: &AgentEvent) -> anyhow::Result<()> {
        let _ = event;
        Ok(())
    }
}

/// Slash 命令定义
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub handler: SlashCommandHandler,
}

/// 命令处理器类型
pub type SlashCommandHandler = Box<
    dyn Fn(&str) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>> + Send + Sync
>;

impl std::fmt::Debug for SlashCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlashCommand")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}
