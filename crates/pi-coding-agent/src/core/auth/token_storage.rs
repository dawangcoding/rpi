use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl StoredToken {
    /// 检查 token 是否已过期
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() >= expires_at,
            None => false, // 无过期时间视为未过期
        }
    }
    
    /// 检查 token 是否即将过期（5 分钟内）
    pub fn is_expiring_soon(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() + chrono::Duration::minutes(5) >= expires_at,
            None => false,
        }
    }
}

/// Token 持久化存储
pub struct TokenStorage {
    storage_path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TokenStore {
    tokens: HashMap<String, StoredToken>,
}

impl TokenStorage {
    pub fn new() -> Self {
        let storage_path = directories::BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".pi").join("auth").join("tokens.json"))
            .unwrap_or_else(|| PathBuf::from(".pi/auth/tokens.json"));
        Self { storage_path }
    }
    
    pub fn with_path(storage_path: PathBuf) -> Self {
        Self { storage_path }
    }
    
    /// 保存 token
    pub fn save_token(&self, token: &StoredToken) -> Result<()> {
        let mut store = self.load_store().unwrap_or_default();
        store.tokens.insert(token.provider.clone(), token.clone());
        self.save_store(&store)
    }
    
    /// 获取指定 provider 的 token
    pub fn get_token(&self, provider: &str) -> Option<StoredToken> {
        let store = self.load_store().ok()?;
        store.tokens.get(provider).cloned()
    }
    
    /// 获取有效的 access token（未过期的）
    pub fn get_valid_token(&self, provider: &str) -> Option<String> {
        let token = self.get_token(provider)?;
        if token.is_expired() {
            None
        } else {
            Some(token.access_token.clone())
        }
    }
    
    /// 删除指定 provider 的 token
    pub fn remove_token(&self, provider: &str) -> Result<()> {
        let mut store = self.load_store().unwrap_or_default();
        store.tokens.remove(provider);
        self.save_store(&store)
    }
    
    /// 列出所有已存储的 provider
    pub fn list_providers(&self) -> Vec<String> {
        self.load_store()
            .map(|store| store.tokens.keys().cloned().collect())
            .unwrap_or_default()
    }
    
    /// 刷新 token（使用 refresh_token 获取新的 access_token）
    pub async fn refresh_token(&self, provider: &str, token_url: &str, client_id: &str) -> Result<StoredToken> {
        let stored = self.get_token(provider)
            .context("No stored token found")?;
        
        let refresh_token = stored.refresh_token
            .context("No refresh token available")?;
        
        let client = reqwest::Client::new();
        let resp = client.post(token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh_token),
                ("client_id", client_id),
            ])
            .send()
            .await
            .context("Failed to refresh token")?;
        
        let token_response: serde_json::Value = resp.json().await
            .context("Failed to parse refresh response")?;
        
        let new_token = StoredToken {
            provider: provider.to_string(),
            access_token: token_response["access_token"]
                .as_str()
                .context("No access_token in refresh response")?
                .to_string(),
            refresh_token: token_response["refresh_token"]
                .as_str()
                .map(|s| s.to_string())
                .or(Some(refresh_token)),
            expires_at: token_response["expires_in"]
                .as_u64()
                .map(|secs| Utc::now() + chrono::Duration::seconds(secs as i64)),
        };
        
        self.save_token(&new_token)?;
        Ok(new_token)
    }
    
    fn load_store(&self) -> Result<TokenStore> {
        let content = std::fs::read_to_string(&self.storage_path)
            .context("Failed to read token store")?;
        let store: TokenStore = serde_json::from_str(&content)
            .context("Failed to parse token store")?;
        Ok(store)
    }
    
    fn save_store(&self, store: &TokenStore) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(store)?;
        std::fs::write(&self.storage_path, content)?;
        
        // 设置文件权限为 600（仅所有者读写）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.storage_path, std::fs::Permissions::from_mode(0o600))?;
        }
        
        Ok(())
    }
}

impl Default for TokenStorage {
    fn default() -> Self {
        Self::new()
    }
}
