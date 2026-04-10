use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub name: String,
    pub authorize_url: String,
    pub token_url: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub use_pkce: bool,
}

/// 获取内置 OAuth 提供商配置
pub fn get_oauth_provider(name: &str) -> Option<OAuthProviderConfig> {
    match name {
        "anthropic" => Some(OAuthProviderConfig {
            name: "anthropic".to_string(),
            authorize_url: "https://console.anthropic.com/oauth/authorize".to_string(),
            token_url: "https://console.anthropic.com/oauth/token".to_string(),
            client_id: "pi-coding-agent".to_string(),
            scopes: vec!["user:inference".to_string()],
            use_pkce: true,
        }),
        "github-copilot" => Some(OAuthProviderConfig {
            name: "github-copilot".to_string(),
            authorize_url: "https://github.com/login/device/code".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            client_id: "Iv1.b507a08c87ecfe98".to_string(),
            scopes: vec!["copilot".to_string()],
            use_pkce: false,
        }),
        _ => None,
    }
}

/// 列出所有支持的 OAuth 提供商
pub fn list_oauth_providers() -> Vec<&'static str> {
    vec!["anthropic", "github-copilot"]
}
