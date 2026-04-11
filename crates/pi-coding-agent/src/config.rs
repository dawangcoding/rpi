//! 配置管理模块
//!
//! 管理应用配置，包括 API keys、默认模型、会话目录等

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::permissions::ToolPermissionConfig;

/// 配置文件格式
#[derive(Debug, Clone, Copy)]
enum ConfigFormat {
    Yaml,
    Json,
    Toml,
}

/// 应用配置
/// 
/// 管理应用的全局配置，包括 API keys、默认模型、会话目录等
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// 默认模型 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,

    /// 默认 thinking level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thinking: Option<String>,

    /// API Keys (provider -> key)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub api_keys: HashMap<String, String>,

    /// 自定义模型定义
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_models: Vec<CustomModelConfig>,

    /// 默认系统提示词追加
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_system_prompt: Option<String>,

    /// Shell 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// 会话目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions_dir: Option<String>,

    /// 快捷键配置文件路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keybindings_path: Option<String>,

    /// 工具权限配置
    #[serde(default)]
    pub permissions: Option<ToolPermissionConfig>,

    /// 扩展配置
    #[serde(default)]
    pub extensions: Option<ExtensionsConfig>,
}

/// 自定义模型配置
/// 
/// 用户自定义的模型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModelConfig {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub api: String,
    pub provider: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
}

/// 扩展配置
/// 
/// 管理扩展的启用/禁用和设置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionsConfig {
    /// 启用的扩展列表（为空表示加载所有可用扩展）
    #[serde(default)]
    pub enabled: Vec<String>,
    /// 禁用的扩展列表
    #[serde(default)]
    pub disabled: Vec<String>,
    /// 扩展特定设置
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
}

impl AppConfig {
    /// 加载 .env 文件（从 ~/.pi/.env）
    ///
    /// 不覆盖已存在的环境变量
    pub fn load_env_file() {
        let config_dir = Self::config_dir();
        let env_path = config_dir.join(".env");
        if env_path.exists() {
            // dotenvy::from_path 默认行为是不覆盖已有环境变量
            if let Err(e) = dotenvy::from_path(&env_path) {
                eprintln!("Warning: Failed to load .env file: {}", e);
            }
        }
    }

    /// 解析配置文件内容
    fn parse_config(content: &str, format: ConfigFormat, path: &Path) -> anyhow::Result<Self> {
        match format {
            ConfigFormat::Yaml => serde_yaml::from_str(content)
                .map_err(|e| anyhow::anyhow!("YAML config error in {}: {}", path.display(), e)),
            ConfigFormat::Json => serde_json::from_str(content)
                .map_err(|e| anyhow::anyhow!("JSON config error in {}: {}", path.display(), e)),
            ConfigFormat::Toml => toml::from_str(content)
                .map_err(|e| anyhow::anyhow!("TOML config error in {}: {}", path.display(), e)),
        }
    }

    /// 加载配置（支持多格式自动检测）
    ///
    /// 按优先级搜索配置文件：config.yaml > config.yml > config.json > config.toml
    /// 同时会加载 ~/.pi/.env 文件中的环境变量
    pub fn load() -> anyhow::Result<Self> {
        // 先加载 .env 文件
        Self::load_env_file();

        let config_dir = Self::config_dir();

        // 按优先级搜索配置文件
        let config_candidates = [
            ("config.yaml", ConfigFormat::Yaml),
            ("config.yml", ConfigFormat::Yaml),
            ("config.json", ConfigFormat::Json),
            ("config.toml", ConfigFormat::Toml),
        ];

        for (filename, format) in &config_candidates {
            let path = config_dir.join(filename);
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                return Self::parse_config(&content, *format, &path);
            }
        }

        Ok(Self::default())
    }

    /// 保存配置
    #[allow(dead_code)] // 预留方法供未来使用
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();

        // 确保配置目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_yaml::to_string(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// 获取配置文件路径
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.yaml")
    }

    /// 获取配置目录
    pub fn config_dir() -> PathBuf {
        if let Ok(env_dir) = std::env::var("PI_CODING_AGENT_DIR") {
            if env_dir == "~" {
                return dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".pi");
            }
            if let Some(stripped) = env_dir.strip_prefix("~/") {
                return dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(stripped);
            }
            return PathBuf::from(env_dir);
        }

        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pi")
    }

    /// 获取会话目录
    pub fn sessions_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.sessions_dir {
            PathBuf::from(dir)
        } else {
            Self::config_dir().join("sessions")
        }
    }

    /// 获取快捷键配置文件路径
    pub fn keybindings_path(&self) -> PathBuf {
        if let Some(ref path) = self.keybindings_path {
            PathBuf::from(path)
        } else {
            Self::config_dir().join("keybindings.toml")
        }
    }

    /// 加载并应用快捷键配置
    pub fn load_keybindings(&self) -> anyhow::Result<()> {
        let path = self.keybindings_path();
        if path.exists() {
            let config = pi_tui::keybindings::KeybindingsConfig::load_from_file(&path)?;
            pi_tui::keybindings::apply_keybindings_config(&config)?;
            tracing::info!("Loaded keybindings config from {}", path.display());
        }
        Ok(())
    }

    /// 获取 API Key (先查 OAuth token，再查配置，最后查环境变量)
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        // 1. 先检查 OAuth token 存储
        let token_storage = crate::core::auth::TokenStorage::new();
        if let Some(token) = token_storage.get_valid_token(provider) {
            return Some(token);
        }

        // 2. 检查配置
        if let Some(key) = self.api_keys.get(provider) {
            return Some(key.clone());
        }

        // 3. 检查环境变量
        Self::get_api_key_from_env(provider)
    }

    /// 异步获取 API Key，支持自动 token 刷新
    /// 
    /// 与 get_api_key 的区别：此方法会在 token 即将过期时自动尝试刷新
    #[allow(dead_code)] // 预留方法供未来使用
    pub async fn get_api_key_async(&self, provider: &str) -> Option<String> {
        // 1. 先检查 OAuth token 存储（带自动刷新）
        let token_storage = crate::core::auth::TokenStorage::new();
        if let Some(token) = token_storage.get_valid_token_or_refresh(provider).await {
            return Some(token);
        }

        // 2. 检查配置
        if let Some(key) = self.api_keys.get(provider) {
            return Some(key.clone());
        }

        // 3. 检查环境变量
        Self::get_api_key_from_env(provider)
    }

    /// 从环境变量获取 API Key
    fn get_api_key_from_env(provider: &str) -> Option<String> {
        match provider {
            "anthropic" => std::env::var("ANTHROPIC_OAUTH_TOKEN")
                .ok()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok()),
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "google" | "google-gemini-cli" | "google-antigravity" => std::env::var("GOOGLE_API_KEY")
                .ok()
                .or_else(|| std::env::var("GEMINI_API_KEY").ok()),
            "google-vertex" => std::env::var("GOOGLE_CLOUD_API_KEY").ok(),
            "groq" => std::env::var("GROQ_API_KEY").ok(),
            "cerebras" => std::env::var("CEREBRAS_API_KEY").ok(),
            "xai" => std::env::var("XAI_API_KEY").ok(),
            "openrouter" => std::env::var("OPENROUTER_API_KEY").ok(),
            "vercel-ai-gateway" => std::env::var("AI_GATEWAY_API_KEY").ok(),
            "mistral" => std::env::var("MISTRAL_API_KEY").ok(),
            "minimax" => std::env::var("MINIMAX_API_KEY").ok(),
            "minimax-cn" => std::env::var("MINIMAX_CN_API_KEY").ok(),
            "huggingface" => std::env::var("HF_TOKEN").ok(),
            "opencode" | "opencode-go" => std::env::var("OPENCODE_API_KEY").ok(),
            "kimi-coding" => std::env::var("KIMI_API_KEY").ok(),
            "azure-openai-responses" => std::env::var("AZURE_OPENAI_API_KEY").ok(),
            "openai-codex" => std::env::var("OPENAI_CODEX_API_KEY").ok(),
            "github-copilot" => std::env::var("COPILOT_GITHUB_TOKEN")
                .ok()
                .or_else(|| std::env::var("GH_TOKEN").ok())
                .or_else(|| std::env::var("GITHUB_TOKEN").ok()),
            "zai" => std::env::var("ZAI_API_KEY").ok(),
            "amazon-bedrock" => {
                // Amazon Bedrock 使用 AWS 凭证
                if std::env::var("AWS_PROFILE").is_ok()
                    || (std::env::var("AWS_ACCESS_KEY_ID").is_ok()
                        && std::env::var("AWS_SECRET_ACCESS_KEY").is_ok())
                    || std::env::var("AWS_BEARER_TOKEN_BEDROCK").is_ok()
                {
                    Some("<authenticated>".to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 获取扩展配置
    pub fn extensions_config(&self) -> Option<&ExtensionsConfig> {
        self.extensions.as_ref()
    }
}

/// 获取项目目录
#[allow(dead_code)] // 预留函数供未来使用
pub fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("com", "pi", "pi")
}

/// 确保目录存在
#[allow(dead_code)] // 预留函数供未来使用
pub fn ensure_dir(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}
