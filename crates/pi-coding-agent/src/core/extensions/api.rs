use std::path::PathBuf;
use crate::config::AppConfig;

/// 提供给扩展使用的 API 上下文
#[derive(Debug, Clone)]
pub struct ExtensionContext {
    /// 当前工作目录
    pub cwd: PathBuf,
    /// 应用配置（只读副本）
    pub config: AppConfig,
    /// 当前会话 ID
    pub session_id: String,
    /// 扩展数据目录（~/.pi/extensions/<name>/data/）
    pub data_dir: PathBuf,
}

impl ExtensionContext {
    pub fn new(cwd: PathBuf, config: AppConfig, session_id: String, extension_name: &str) -> Self {
        let data_dir = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".pi").join("extensions").join(extension_name).join("data"))
            .unwrap_or_else(|| PathBuf::from(".pi/extensions").join(extension_name).join("data"));
        
        Self { cwd, config, session_id, data_dir }
    }
}
