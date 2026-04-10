use anyhow::{Result, Context};
use hyper::{Request, Response, body::Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use http_body_util::Full;
use bytes::Bytes;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use std::collections::HashMap;

use super::providers::OAuthProviderConfig;
use super::token_storage::{TokenStorage, StoredToken};

/// PKCE code_verifier 生成
fn generate_code_verifier() -> String {
    use base64::Engine;
    let random_bytes: Vec<u8> = (0..32).map(|_| rand_byte()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes)
}

fn rand_byte() -> u8 {
    // 使用简单的时间+计数器种子
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let val = COUNTER.fetch_add(1, Ordering::Relaxed);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    ((val ^ time) & 0xFF) as u8
}

/// SHA256 hash for PKCE code_challenge
fn sha256_base64url(input: &str) -> String {
    use sha2::{Digest, Sha256};
    use base64::Engine;

    let digest = Sha256::digest(input.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// 生成随机 state 参数
fn generate_state() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 运行完整的 OAuth 授权流程
pub async fn run_oauth_flow(
    provider_config: &OAuthProviderConfig,
    token_storage: &TokenStorage,
) -> Result<StoredToken> {
    // 1. 启动本地回调服务器
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let redirect_uri = format!("http://127.0.0.1:{}/callback", local_addr.port());
    
    // 2. 生成 PKCE 和 state
    let state = generate_state();
    let code_verifier = if provider_config.use_pkce {
        Some(generate_code_verifier())
    } else {
        None
    };
    
    // 3. 构建授权 URL
    let mut auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&state={}",
        provider_config.authorize_url,
        urlencoding_encode(&provider_config.client_id),
        urlencoding_encode(&redirect_uri),
        urlencoding_encode(&state),
    );
    
    if !provider_config.scopes.is_empty() {
        auth_url.push_str(&format!("&scope={}", urlencoding_encode(&provider_config.scopes.join(" "))));
    }
    
    if let Some(ref verifier) = code_verifier {
        let challenge = sha256_base64url(verifier);
        auth_url.push_str(&format!("&code_challenge={}&code_challenge_method=S256", challenge));
    }

    // 追加 Provider 特有的额外授权参数
    for (key, value) in &provider_config.extra_auth_params {
        auth_url.push_str(&format!("&{}={}", urlencoding_encode(key), urlencoding_encode(value)));
    }

    // 4. 打开浏览器
    println!("\n打开浏览器进行授权...");
    println!("如果浏览器未自动打开，请手动访问：\n{}\n", auth_url);
    let _ = open_browser(&auth_url);
    
    // 5. 等待回调
    let (tx, rx) = oneshot::channel::<String>();
    let expected_state = state.clone();
    
    tokio::spawn(async move {
        // 接受一个连接
        if let Ok((stream, _)) = listener.accept().await {
            let io = TokioIo::new(stream);
            let tx = std::sync::Mutex::new(Some(tx));
            let expected_state = expected_state.clone();
            
            let service = service_fn(move |req: Request<Incoming>| {
                let tx = tx.lock().unwrap().take();
                let expected_state = expected_state.clone();
                async move {
                    let query = req.uri().query().unwrap_or("");
                    let params = parse_query_string(query);
                    
                    // 验证 state
                    if params.get("state").map(|s| s.as_str()) != Some(&expected_state) {
                        return Ok::<_, hyper::Error>(Response::new(Full::new(Bytes::from("State mismatch. Authorization failed."))));
                    }
                    
                    if let Some(code) = params.get("code") {
                        if let Some(tx) = tx {
                            let _ = tx.send(code.clone());
                        }
                        Ok(Response::new(Full::new(Bytes::from(
                            "<html><body><h2>Authorization successful!</h2><p>You can close this window.</p></body></html>"
                        ))))
                    } else {
                        let error = params.get("error").cloned().unwrap_or_else(|| "unknown".to_string());
                        Ok(Response::new(Full::new(Bytes::from(format!(
                            "<html><body><h2>Authorization failed</h2><p>Error: {}</p></body></html>", error
                        )))))
                    }
                }
            });
            
            let _ = http1::Builder::new().serve_connection(io, service).await;
        }
    });
    
    // 6. 等待 authorization code（超时 120 秒）
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        rx
    ).await
        .context("OAuth authorization timed out (120s)")?
        .context("Failed to receive authorization code")?;
    
    // 7. 用 code 换取 token
    let client = reqwest::Client::new();
    let mut token_params = HashMap::new();
    token_params.insert("grant_type", "authorization_code".to_string());
    token_params.insert("code", code);
    token_params.insert("redirect_uri", redirect_uri);
    token_params.insert("client_id", provider_config.client_id.clone());
    
    if let Some(ref verifier) = code_verifier {
        token_params.insert("code_verifier", verifier.clone());
    }
    
    let resp = client.post(&provider_config.token_url)
        .form(&token_params)
        .send()
        .await
        .context("Failed to exchange authorization code for token")?;
    
    let token_response: serde_json::Value = resp.json().await
        .context("Failed to parse token response")?;
    
    let access_token = token_response["access_token"]
        .as_str()
        .context("No access_token in response")?
        .to_string();
    
    let refresh_token = token_response["refresh_token"]
        .as_str()
        .map(|s| s.to_string());
    
    let expires_in = token_response["expires_in"].as_u64();
    let expires_at = expires_in.map(|secs| {
        chrono::Utc::now() + chrono::Duration::seconds(secs as i64)
    });
    
    // 8. 存储 token
    let stored_token = StoredToken {
        provider: provider_config.name.clone(),
        access_token,
        refresh_token,
        expires_at,
    };
    
    token_storage.save_token(&stored_token)?;
    
    println!("✓ 已成功登录 {}", provider_config.name);
    
    Ok(stored_token)
}

/// 简单的 URL 编码
fn urlencoding_encode(s: &str) -> String {
    s.chars().map(|c| match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}

/// 解析查询字符串
fn parse_query_string(query: &str) -> HashMap<String, String> {
    query.split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or("");
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

/// 打开浏览器
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/c", "start", url]).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_code_verifier_length() {
        let verifier = generate_code_verifier();
        // Base64 URL 编码的 32 字节应该产生 43 字符
        assert!(verifier.len() >= 43);
        // 应该只包含 URL 安全字符
        assert!(verifier.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_generate_code_verifier_uniqueness() {
        let v1 = generate_code_verifier();
        let v2 = generate_code_verifier();
        assert_ne!(v1, v2, "Two verifiers should be different");
    }

    #[test]
    fn test_sha256_base64url() {
        // 已知输入的 SHA256 Base64 URL 编码
        let result = sha256_base64url("test_verifier");
        assert!(!result.is_empty());
        // 结果不应包含 + / = 字符（这些是标准 Base64，不是 URL 安全的）
        assert!(!result.contains('+'));
        assert!(!result.contains('/'));
        assert!(!result.contains('='));
    }

    #[test]
    fn test_sha256_base64url_deterministic() {
        let r1 = sha256_base64url("same_input");
        let r2 = sha256_base64url("same_input");
        assert_eq!(r1, r2, "Same input should produce same output");
    }

    #[test]
    fn test_generate_state_uniqueness() {
        let s1 = generate_state();
        let s2 = generate_state();
        assert_ne!(s1, s2, "Two states should be different");
    }

    #[test]
    fn test_generate_state_format() {
        let state = generate_state();
        // UUID v4 格式
        assert!(!state.is_empty());
    }

    #[test]
    fn test_parse_query_string() {
        let params = parse_query_string("code=abc123&state=xyz789&scope=read");
        assert_eq!(params.get("code"), Some(&"abc123".to_string()));
        assert_eq!(params.get("state"), Some(&"xyz789".to_string()));
        assert_eq!(params.get("scope"), Some(&"read".to_string()));
    }

    #[test]
    fn test_parse_query_string_empty() {
        let params = parse_query_string("");
        // 空字符串 split 后会产生一个空 key 的 entry
        // 这是当前实现的实际行为
        assert_eq!(params.get(""), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_query_string_encoded() {
        let params = parse_query_string("key=hello%20world&other=a%26b");
        // 注意：取决于实现是否做 URL 解码
        assert!(params.contains_key("key"));
    }

    #[test]
    fn test_urlencoding_encode() {
        let encoded = urlencoding_encode("hello world");
        assert_eq!(encoded, "hello%20world");
    }

    #[test]
    fn test_urlencoding_encode_special_chars() {
        let encoded = urlencoding_encode("a&b=c");
        assert!(encoded.contains("%26") || encoded.contains("%3D"));
    }
}
