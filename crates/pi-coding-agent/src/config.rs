//! 配置管理模块
//!
//! 管理应用配置，包括 API keys、默认模型、会话目录等

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::permissions::ToolPermissionConfig;

/// 应用配置
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

    /// 工具权限配置
    #[serde(default)]
    pub permissions: Option<ToolPermissionConfig>,
}

/// 自定义模型配置
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

impl AppConfig {
    /// 加载配置（从 ~/.pi/config.yaml）
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path();

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)?;
        let config: AppConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置
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
}

/// 获取项目目录
pub fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("com", "pi", "pi")
}

/// 确保目录存在
pub fn ensure_dir(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}
