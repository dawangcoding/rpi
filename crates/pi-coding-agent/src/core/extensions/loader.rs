use super::types::ExtensionManifest;
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

impl Default for ExtensionLoader {
    fn default() -> Self {
        Self::new()
    }
}
