//! 会话持久化管理模块
//!
//! 负责会话的保存、加载、列表和删除

use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use pi_agent::types::AgentMessage;
use crate::config::AppConfig;

/// 会话元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
    pub model: String,
}

/// 保存的会话数据
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedSession {
    pub metadata: SessionMetadata,
    pub messages: Vec<AgentMessage>,
}

/// 会话管理器
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new(config: &AppConfig) -> anyhow::Result<Self> {
        let sessions_dir = config.sessions_dir();
        std::fs::create_dir_all(&sessions_dir)?;
        Ok(Self { sessions_dir })
    }
    
    /// 从指定目录创建会话管理器
    pub fn with_dir(sessions_dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&sessions_dir)?;
        Ok(Self { sessions_dir })
    }
    
    /// 保存会话
    pub async fn save_session(
        &self,
        session_id: &str,
        messages: &[AgentMessage],
    ) -> anyhow::Result<PathBuf> {
        let path = self.session_path(session_id);
        
        let title = extract_title(messages);
        let model = extract_model(messages);
        
        let session = SavedSession {
            metadata: SessionMetadata {
                id: session_id.to_string(),
                title,
                created_at: chrono::Utc::now().timestamp_millis(),
                updated_at: chrono::Utc::now().timestamp_millis(),
                message_count: messages.len(),
                model,
            },
            messages: messages.to_vec(),
        };
        
        let json = serde_json::to_string_pretty(&session)?;
        tokio::fs::write(&path, json).await?;
        
        Ok(path)
    }
    
    /// 加载会话
    pub async fn load_session(&self, session_id: &str) -> anyhow::Result<SavedSession> {
        let path = self.session_path(session_id);
        let json = tokio::fs::read_to_string(&path).await?;
        Ok(serde_json::from_str(&json)?)
    }
    
    /// 从指定路径加载会话
    pub async fn load_session_from_path(path: &Path) -> anyhow::Result<SavedSession> {
        let json = tokio::fs::read_to_string(path).await?;
        Ok(serde_json::from_str(&json)?)
    }
    
    /// 列出所有会话
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionMetadata>> {
        let mut sessions = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.sessions_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok(session) = serde_json::from_str::<SavedSession>(&content) {
                        sessions.push(session.metadata);
                    }
                }
            }
        }
        
        // 按更新时间排序（最新的在前）
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(sessions)
    }
    
    /// 删除会话
    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        let path = self.session_path(session_id);
        tokio::fs::remove_file(&path).await?;
        Ok(())
    }
    
    /// 检查会话是否存在
    pub fn session_exists(&self, session_id: &str) -> bool {
        self.session_path(session_id).exists()
    }
    
    /// 获取会话文件路径
    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", session_id))
    }
    
    /// 获取会话目录
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }
    
    /// 生成新的会话 ID
    pub fn generate_session_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
    
    /// 查找最近的会话
    pub async fn find_most_recent(&self) -> anyhow::Result<Option<SessionMetadata>> {
        let sessions = self.list_sessions().await?;
        Ok(sessions.into_iter().next())
    }
}

/// 从消息中提取标题（取第一条用户消息的前 100 字符）
fn extract_title(messages: &[AgentMessage]) -> Option<String> {
    for msg in messages {
        if let AgentMessage::Llm(pi_ai::types::Message::User(user_msg)) = msg {
            let content = match &user_msg.content {
                pi_ai::types::UserContent::Text(text) => text.clone(),
                pi_ai::types::UserContent::Blocks(blocks) => {
                    blocks.iter()
                        .filter_map(|block| {
                            if let pi_ai::types::ContentBlock::Text(text) = block {
                                Some(text.text.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };
            
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                let title = if trimmed.len() > 100 {
                    format!("{}...", &trimmed[..100])
                } else {
                    trimmed.to_string()
                };
                return Some(title);
            }
        }
    }
    None
}

/// 从消息中提取模型信息
fn extract_model(messages: &[AgentMessage]) -> String {
    for msg in messages.iter().rev() {
        if let AgentMessage::Llm(pi_ai::types::Message::Assistant(assistant)) = msg {
            return format!("{:?}/{}", assistant.provider, assistant.model);
        }
    }
    String::new()
}

/// 会话过滤器
#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub model: Option<String>,
    pub before: Option<i64>,
    pub after: Option<i64>,
}

impl SessionManager {
    /// 列出会话（带过滤）
    pub async fn list_sessions_filtered(&self, filter: &SessionFilter) -> anyhow::Result<Vec<SessionMetadata>> {
        let all_sessions = self.list_sessions().await?;
        
        let filtered: Vec<SessionMetadata> = all_sessions
            .into_iter()
            .filter(|s| {
                // 模型过滤
                if let Some(ref model) = filter.model {
                    if !s.model.contains(model) {
                        return false;
                    }
                }
                
                // 时间范围过滤
                if let Some(before) = filter.before {
                    if s.updated_at >= before {
                        return false;
                    }
                }
                
                if let Some(after) = filter.after {
                    if s.updated_at <= after {
                        return false;
                    }
                }
                
                true
            })
            .collect();
        
        Ok(filtered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_title() {
        let messages = vec![
            AgentMessage::user("Hello, this is a test message that is quite long and should be truncated properly"),
        ];
        
        let title = extract_title(&messages);
        assert!(title.is_some());
        assert!(title.unwrap().len() <= 103); // 100 + "..."
    }
    
    #[test]
    fn test_extract_title_short() {
        let messages = vec![
            AgentMessage::user("Short"),
        ];
        
        let title = extract_title(&messages);
        assert_eq!(title, Some("Short".to_string()));
    }
    
    #[test]
    fn test_extract_title_empty() {
        let messages: Vec<AgentMessage> = vec![];
        let title = extract_title(&messages);
        assert!(title.is_none());
    }
}
