//! API Provider 注册系统
//!
//! 提供全局的 Provider 注册表，用于管理不同 LLM 提供商的实现

use std::collections::HashMap;
use std::sync::{Arc, RwLock, OnceLock};
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::types::*;

/// API Provider trait - 所有 LLM 提供商实现此 trait
#[async_trait]
pub trait ApiProvider: Send + Sync {
    /// 返回此 provider 支持的 API 类型
    fn api(&self) -> Api;
    
    /// 流式调用 LLM
    async fn stream(
        &self,
        context: &Context,
        model: &Model,
        options: &StreamOptions,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<AssistantMessageEvent>> + Send>>>;
}

/// API Provider 注册表
pub struct ApiRegistry {
    providers: HashMap<Api, Arc<dyn ApiProvider>>,
}

impl ApiRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
    
    /// 注册一个 provider
    pub fn register(&mut self, provider: Arc<dyn ApiProvider>) {
        let api = provider.api();
        self.providers.insert(api, provider);
    }
    
    /// 获取指定 API 类型的 provider
    pub fn get(&self, api: &Api) -> Option<Arc<dyn ApiProvider>> {
        self.providers.get(api).cloned()
    }
    
    /// 检查是否已注册指定 API 类型的 provider
    pub fn has(&self, api: &Api) -> bool {
        self.providers.contains_key(api)
    }
    
    /// 获取所有已注册的 provider
    pub fn get_all(&self) -> Vec<Arc<dyn ApiProvider>> {
        self.providers.values().cloned().collect()
    }
    
    /// 清除所有注册的 provider
    pub fn clear(&mut self) {
        self.providers.clear();
    }
}

impl Default for ApiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// 全局注册表
static GLOBAL_REGISTRY: OnceLock<RwLock<ApiRegistry>> = OnceLock::new();

/// 获取全局注册表
fn get_global_registry() -> &'static RwLock<ApiRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| RwLock::new(ApiRegistry::new()))
}

/// 注册一个 API provider 到全局注册表
pub fn register_api_provider(provider: Arc<dyn ApiProvider>) {
    let registry = get_global_registry();
    if let Ok(mut reg) = registry.write() {
        reg.register(provider);
    }
}

/// 从全局注册表获取指定 API 类型的 provider
pub fn get_api_provider(api: &Api) -> Option<Arc<dyn ApiProvider>> {
    let registry = get_global_registry();
    registry.read().ok().and_then(|reg| reg.get(api))
}

/// 检查全局注册表是否包含指定 API 类型的 provider
pub fn has_api_provider(api: &Api) -> bool {
    let registry = get_global_registry();
    registry.read().ok().map(|reg| reg.has(api)).unwrap_or(false)
}

/// 获取全局注册表中所有 provider
pub fn get_all_api_providers() -> Vec<Arc<dyn ApiProvider>> {
    let registry = get_global_registry();
    registry.read().ok().map(|reg| reg.get_all()).unwrap_or_default()
}

/// 清除全局注册表中的所有 provider
pub fn clear_api_providers() {
    let registry = get_global_registry();
    if let Ok(mut reg) = registry.write() {
        reg.clear();
    }
}

/// 解析 API provider，如果未找到则返回错误
pub fn resolve_api_provider(api: &Api) -> anyhow::Result<Arc<dyn ApiProvider>> {
    get_api_provider(api)
        .ok_or_else(|| anyhow::anyhow!("No API provider registered for api: {:?}", api))
}
