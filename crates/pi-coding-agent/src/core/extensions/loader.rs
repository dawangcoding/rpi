use super::types::{ExtensionManifest, Extension};
use crate::config::AppConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

/// 扩展加载器 - 从文件系统扫描和加载扩展
pub struct ExtensionLoader {
    extensions_dir: PathBuf,
}

impl ExtensionLoader {
    pub fn new() -> Self {
        let extensions_dir = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".pi").join("extensions"))
            .unwrap_or_else(|| PathBuf::from(".pi/extensions"));
        
        Self { extensions_dir }
    }
    
    #[allow(dead_code)]
    pub fn with_dir(extensions_dir: PathBuf) -> Self {
        Self { extensions_dir }
    }
    
    /// 扫描扩展目录，返回所有找到的扩展 manifest
    pub fn scan_extensions(&self) -> Vec<ExtensionManifest> {
        let mut manifests = Vec::new();
        
        if !self.extensions_dir.exists() {
            tracing::debug!("Extensions directory does not exist: {:?}", self.extensions_dir);
            return manifests;
        }
        
        let entries = match std::fs::read_dir(&self.extensions_dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read extensions directory: {}", e);
                return manifests;
            }
        };
        
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    match self.load_manifest(&manifest_path) {
                        Ok(manifest) => {
                            tracing::info!("Found extension: {} v{}", manifest.name, manifest.version);
                            manifests.push(manifest);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load manifest {:?}: {}", manifest_path, e);
                        }
                    }
                }
            }
        }
        
        manifests
    }
    
    fn load_manifest(&self, path: &std::path::Path) -> Result<ExtensionManifest> {
        let content = std::fs::read_to_string(path)?;
        let manifest: ExtensionManifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }
    
    /// 获取扩展目录路径
    pub fn extensions_dir(&self) -> &PathBuf {
        &self.extensions_dir
    }
}

impl ExtensionLoader {
    /// 从注册表和配置加载扩展
    pub fn load_extensions(&self, registry: &ExtensionRegistry, config: &AppConfig) -> Vec<Box<dyn Extension>> {
        registry.load_enabled(config)
    }
}

impl Default for ExtensionLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// 扩展工厂 trait - 用于编译时链接
pub trait ExtensionFactory: Send + Sync {
    /// 工厂名称（对应扩展名称）
    fn name(&self) -> &str;
    /// 创建扩展实例
    fn create(&self) -> Box<dyn Extension>;
    /// 扩展描述
    #[allow(dead_code)]
    fn description(&self) -> &str { "" }
}

/// 扩展注册表 - 管理可用的扩展工厂
pub struct ExtensionRegistry {
    factories: HashMap<String, Box<dyn ExtensionFactory>>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }
    
    /// 注册一个扩展工厂
    pub fn register_factory(&mut self, factory: Box<dyn ExtensionFactory>) {
        let name = factory.name().to_string();
        tracing::info!("Registered extension factory: {}", name);
        self.factories.insert(name, factory);
    }
    
    /// 列出所有可用的扩展名称
    #[allow(dead_code)]
    pub fn available_extensions(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }
    
    /// 从配置加载启用的扩展
    pub fn load_enabled(&self, config: &AppConfig) -> Vec<Box<dyn Extension>> {
        let mut extensions = Vec::new();
        
        let enabled = config.extensions_config()
            .map(|c| c.enabled.clone())
            .unwrap_or_default();
        let disabled = config.extensions_config()
            .map(|c| c.disabled.clone())
            .unwrap_or_default();
        
        // 如果 enabled 为空，默认加载所有非 disabled 的扩展
        if enabled.is_empty() {
            for (name, factory) in &self.factories {
                if !disabled.contains(name) {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| factory.create())) {
                        Ok(ext) => {
                            tracing::info!("Loaded extension: {}", name);
                            extensions.push(ext);
                        }
                        Err(_) => {
                            tracing::error!("Failed to create extension: {} (panic)", name);
                        }
                    }
                }
            }
        } else {
            for name in &enabled {
                if disabled.contains(name) {
                    continue;
                }
                if let Some(factory) = self.factories.get(name) {
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| factory.create())) {
                        Ok(ext) => {
                            tracing::info!("Loaded extension: {}", name);
                            extensions.push(ext);
                        }
                        Err(_) => {
                            tracing::error!("Failed to create extension: {} (panic)", name);
                        }
                    }
                } else {
                    tracing::warn!("Extension factory not found: {}", name);
                }
            }
        }
        
        extensions
    }
    
    /// 获取注册的工厂数量
    #[allow(dead_code)]
    pub fn factory_count(&self) -> usize {
        self.factories.len()
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ExtensionsConfig};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::future::Future;

    // ==================== Mock Extension ====================

    struct MockExtension {
        manifest: ExtensionManifest,
    }

    impl MockExtension {
        fn new(name: &str) -> Self {
            Self {
                manifest: ExtensionManifest {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Mock extension".to_string(),
                    author: "test".to_string(),
                    entry_point: PathBuf::new(),
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl super::super::types::Extension for MockExtension {
        fn manifest(&self) -> &ExtensionManifest {
            &self.manifest
        }
        
        async fn activate(&mut self, _ctx: &super::super::api::ExtensionContext) -> anyhow::Result<()> {
            Ok(())
        }
        
        async fn deactivate(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        
        fn registered_tools(&self) -> Vec<Arc<dyn pi_agent::types::AgentTool>> {
            vec![]
        }
        
        fn registered_commands(&self) -> Vec<super::super::types::SlashCommand> {
            vec![]
        }
    }

    // ==================== Mock Factory ====================

    struct MockFactory {
        name: String,
    }

    impl MockFactory {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    impl ExtensionFactory for MockFactory {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn create(&self) -> Box<dyn super::super::types::Extension> {
            Box::new(MockExtension::new(&self.name))
        }
        
        fn description(&self) -> &str {
            "Mock factory for testing"
        }
    }

    // ==================== ExtensionRegistry Tests ====================

    #[test]
    fn test_extension_registry_new() {
        let registry = ExtensionRegistry::new();
        assert_eq!(registry.factory_count(), 0);
    }

    #[test]
    fn test_extension_registry_default() {
        let registry = ExtensionRegistry::default();
        assert_eq!(registry.factory_count(), 0);
    }

    #[test]
    fn test_extension_registry_register_factory() {
        let mut registry = ExtensionRegistry::new();
        
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        assert_eq!(registry.factory_count(), 1);
        
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        assert_eq!(registry.factory_count(), 2);
        
        // Same name should overwrite
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        assert_eq!(registry.factory_count(), 2);
    }

    #[test]
    fn test_extension_registry_available_extensions() {
        let mut registry = ExtensionRegistry::new();
        
        let available = registry.available_extensions();
        assert!(available.is_empty());
        
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        
        let available = registry.available_extensions();
        assert_eq!(available.len(), 2);
        assert!(available.contains(&"ext1"));
        assert!(available.contains(&"ext2"));
    }

    #[test]
    fn test_extension_registry_factory_count() {
        let mut registry = ExtensionRegistry::new();
        
        assert_eq!(registry.factory_count(), 0);
        
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        assert_eq!(registry.factory_count(), 1);
        
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        assert_eq!(registry.factory_count(), 2);
    }

    // ==================== ExtensionRegistry load_enabled Tests ====================

    fn create_config_with_extensions(enabled: Vec<String>, disabled: Vec<String>) -> AppConfig {
        let mut config = AppConfig::default();
        config.extensions = Some(ExtensionsConfig {
            enabled,
            disabled,
            settings: std::collections::HashMap::new(),
        });
        config
    }

    #[test]
    fn test_load_enabled_empty_enabled_loads_all() {
        let mut registry = ExtensionRegistry::new();
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        registry.register_factory(Box::new(MockFactory::new("ext3")));
        
        // Empty enabled list -> load all
        let config = create_config_with_extensions(vec![], vec![]);
        let extensions = registry.load_enabled(&config);
        
        assert_eq!(extensions.len(), 3);
    }

    #[test]
    fn test_load_enabled_specific_enabled() {
        let mut registry = ExtensionRegistry::new();
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        registry.register_factory(Box::new(MockFactory::new("ext3")));
        
        // Only load ext1 and ext2
        let config = create_config_with_extensions(
            vec!["ext1".to_string(), "ext2".to_string()],
            vec![]
        );
        let extensions = registry.load_enabled(&config);
        
        assert_eq!(extensions.len(), 2);
    }

    #[test]
    fn test_load_enabled_disabled_excludes() {
        let mut registry = ExtensionRegistry::new();
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        registry.register_factory(Box::new(MockFactory::new("ext3")));
        
        // Empty enabled -> load all, but exclude ext3 via disabled
        let config = create_config_with_extensions(
            vec![],
            vec!["ext3".to_string()]
        );
        let extensions = registry.load_enabled(&config);
        
        assert_eq!(extensions.len(), 2);
    }

    #[test]
    fn test_load_enabled_enabled_and_disabled_conflict() {
        let mut registry = ExtensionRegistry::new();
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        registry.register_factory(Box::new(MockFactory::new("ext2")));
        registry.register_factory(Box::new(MockFactory::new("ext3")));
        
        // ext2 is both enabled and disabled -> disabled wins
        let config = create_config_with_extensions(
            vec!["ext1".to_string(), "ext2".to_string()],
            vec!["ext2".to_string()]
        );
        let extensions = registry.load_enabled(&config);
        
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].manifest().name, "ext1");
    }

    #[test]
    fn test_load_enabled_nonexistent_enabled() {
        let mut registry = ExtensionRegistry::new();
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        
        // Request non-existent extension
        let config = create_config_with_extensions(
            vec!["ext1".to_string(), "nonexistent".to_string()],
            vec![]
        );
        let extensions = registry.load_enabled(&config);
        
        // Only ext1 should be loaded
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].manifest().name, "ext1");
    }

    // ==================== ExtensionLoader Tests ====================

    #[test]
    fn test_extension_loader_new() {
        let loader = ExtensionLoader::new();
        // Default path should be ~/.pi/extensions
        assert!(loader.extensions_dir().to_string_lossy().contains("extensions"));
    }

    #[test]
    fn test_extension_loader_with_dir() {
        let custom_path = PathBuf::from("/custom/extensions");
        let loader = ExtensionLoader::with_dir(custom_path.clone());
        
        assert_eq!(*loader.extensions_dir(), custom_path);
    }

    #[test]
    fn test_extension_loader_scan_extensions_nonexistent_dir() {
        let loader = ExtensionLoader::with_dir(PathBuf::from("/nonexistent/path"));
        let manifests = loader.scan_extensions();
        
        assert!(manifests.is_empty());
    }

    #[test]
    fn test_extension_loader_default() {
        let loader = ExtensionLoader::default();
        assert!(loader.extensions_dir().to_string_lossy().contains("extensions"));
    }

    #[test]
    fn test_extension_loader_load_extensions() {
        let loader = ExtensionLoader::new();
        let mut registry = ExtensionRegistry::new();
        registry.register_factory(Box::new(MockFactory::new("ext1")));
        
        let config = AppConfig::default();
        let extensions = loader.load_extensions(&registry, &config);
        
        assert_eq!(extensions.len(), 1);
    }
}
