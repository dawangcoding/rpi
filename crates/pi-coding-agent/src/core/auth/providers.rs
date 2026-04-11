use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub name: String,
    pub authorize_url: String,
    pub token_url: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub use_pkce: bool,
    /// Provider 特有的额外授权 URL 参数
    pub extra_auth_params: Vec<(String, String)>,
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
            extra_auth_params: vec![],
        }),
        "github-copilot" => Some(OAuthProviderConfig {
            name: "github-copilot".to_string(),
            authorize_url: "https://github.com/login/device/code".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            client_id: "Iv1.b507a08c87ecfe98".to_string(),
            scopes: vec!["copilot".to_string()],
            use_pkce: false,
            extra_auth_params: vec![],
        }),
        "openai" => Some(OAuthProviderConfig {
            name: "openai".to_string(),
            authorize_url: "https://auth.openai.com/authorize".to_string(),
            token_url: "https://auth.openai.com/oauth/token".to_string(),
            client_id: "app_live_rlRRsAMIvfOyyPxU1gzM4SZQ".to_string(),
            scopes: vec!["openai.public".to_string()],
            use_pkce: true,
            extra_auth_params: vec![
                ("audience".to_string(), "https://api.openai.com/v1".to_string()),
            ],
        }),
        "google" => Some(OAuthProviderConfig {
            name: "google".to_string(),
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            client_id: "764086051850-6qr4p6gpi6hn506pt8ejuq83di341hur.apps.googleusercontent.com".to_string(),
            scopes: vec![
                "https://www.googleapis.com/auth/generative-language".to_string(),
            ],
            use_pkce: true,
            extra_auth_params: vec![
                ("access_type".to_string(), "offline".to_string()),
                ("prompt".to_string(), "consent".to_string()),
            ],
        }),
        "azure-openai" => Some(OAuthProviderConfig {
            name: "azure-openai".to_string(),
            authorize_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
            client_id: "pi-coding-agent-azure".to_string(),
            scopes: vec!["https://cognitiveservices.azure.com/.default".to_string()],
            use_pkce: true,
            extra_auth_params: vec![
                ("response_mode".to_string(), "query".to_string()),
            ],
        }),
        "mistral" => Some(OAuthProviderConfig {
            name: "mistral".to_string(),
            authorize_url: "https://auth.mistral.ai/oauth/authorize".to_string(),
            token_url: "https://auth.mistral.ai/oauth/token".to_string(),
            client_id: "pi-coding-agent-mistral".to_string(),
            scopes: vec!["api".to_string()],
            use_pkce: true,
            extra_auth_params: vec![],
        }),
        "huggingface" => Some(OAuthProviderConfig {
            name: "huggingface".to_string(),
            authorize_url: "https://huggingface.co/oauth/authorize".to_string(),
            token_url: "https://huggingface.co/oauth/token".to_string(),
            client_id: "pi-coding-agent-hf".to_string(),
            scopes: vec!["inference-api".to_string()],
            use_pkce: true,
            extra_auth_params: vec![],
        }),
        "openrouter" => Some(OAuthProviderConfig {
            name: "openrouter".to_string(),
            authorize_url: "https://openrouter.ai/auth".to_string(),
            token_url: "https://openrouter.ai/api/v1/auth/keys".to_string(),
            client_id: "pi-coding-agent-or".to_string(),
            scopes: vec![],
            use_pkce: false,
            extra_auth_params: vec![],
        }),
        _ => None,
    }
}

/// 列出所有支持的 OAuth 提供商
pub fn list_oauth_providers() -> Vec<&'static str> {
    vec!["anthropic", "github-copilot", "openai", "google", "azure-openai", "mistral", "huggingface", "openrouter"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_anthropic_provider() {
        let provider = get_oauth_provider("anthropic");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "anthropic");
        assert!(p.use_pkce);
        assert!(!p.scopes.is_empty());
        assert!(p.authorize_url.starts_with("https://"));
        assert!(p.token_url.starts_with("https://"));
    }

    #[test]
    fn test_get_openai_provider() {
        let provider = get_oauth_provider("openai");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "openai");
        assert!(p.use_pkce);
    }

    #[test]
    fn test_get_google_provider() {
        let provider = get_oauth_provider("google");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "google");
        assert!(p.use_pkce);
        // Google 应该有 extra_auth_params
        assert!(!p.extra_auth_params.is_empty());
        // 验证 access_type=offline
        assert!(p.extra_auth_params.iter().any(|(k, v)| k == "access_type" && v == "offline"));
    }

    #[test]
    fn test_get_github_copilot_provider() {
        let provider = get_oauth_provider("github-copilot");
        assert!(provider.is_some());
    }

    #[test]
    fn test_get_azure_openai_provider() {
        let provider = get_oauth_provider("azure-openai");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "azure-openai");
        assert!(p.use_pkce);
        assert!(!p.scopes.is_empty());
        // Azure 应该有 extra_auth_params (response_mode)
        assert!(!p.extra_auth_params.is_empty());
        assert!(p.extra_auth_params.iter().any(|(k, v)| k == "response_mode" && v == "query"));
    }

    #[test]
    fn test_get_mistral_provider() {
        let provider = get_oauth_provider("mistral");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "mistral");
        assert!(p.use_pkce);
        assert!(!p.scopes.is_empty());
    }

    #[test]
    fn test_get_huggingface_provider() {
        let provider = get_oauth_provider("huggingface");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "huggingface");
        assert!(p.use_pkce);
        assert!(!p.scopes.is_empty());
    }

    #[test]
    fn test_get_openrouter_provider() {
        let provider = get_oauth_provider("openrouter");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.name, "openrouter");
        // OpenRouter 不使用 PKCE
        assert!(!p.use_pkce);
        // OpenRouter 没有 scopes
        assert!(p.scopes.is_empty());
    }

    #[test]
    fn test_get_unknown_provider() {
        let provider = get_oauth_provider("nonexistent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_list_oauth_providers() {
        let providers = list_oauth_providers();
        assert!(providers.len() >= 8);
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"google"));
        assert!(providers.contains(&"azure-openai"));
        assert!(providers.contains(&"mistral"));
        assert!(providers.contains(&"huggingface"));
        assert!(providers.contains(&"openrouter"));
    }

    #[test]
    fn test_provider_urls_are_valid_https() {
        for name in list_oauth_providers() {
            let provider = get_oauth_provider(name).unwrap();
            assert!(
                provider.authorize_url.starts_with("https://"),
                "Provider {} authorize_url should be HTTPS",
                name
            );
            assert!(
                provider.token_url.starts_with("https://"),
                "Provider {} token_url should be HTTPS",
                name
            );
        }
    }
}
