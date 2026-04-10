//! 会话持久化管理模块
//!
//! 负责会话的保存、加载、列表和删除

use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use pi_agent::types::AgentMessage;
use crate::config::AppConfig;

/// 压缩记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRecord {
    /// 压缩时间戳
    pub compacted_at: i64,
    /// 被替换的消息范围 (start, end)
    pub removed_message_range: (usize, usize),
    /// 摘要 token 数
    pub summary_tokens: usize,
    /// 原始 token 数
    pub original_tokens: usize,
}

/// 会话元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
    pub model: String,
    #[serde(default)]
    pub parent_session_id: Option<String>,  // 父会话 ID
    #[serde(default)]
    pub fork_at_index: Option<usize>,       // fork 消息索引
}

/// 保存的会话数据
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedSession {
    pub metadata: SessionMetadata,
    pub messages: Vec<AgentMessage>,
    #[serde(default)]
    pub compaction_history: Vec<CompactionRecord>,
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
        self.save_session_with_compaction(session_id, messages, &[]).await
    }

    /// 保存会话（带压缩历史）
    pub async fn save_session_with_compaction(
        &self,
        session_id: &str,
        messages: &[AgentMessage],
        compaction_history: &[CompactionRecord],
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
                parent_session_id: None,
                fork_at_index: None,
            },
            messages: messages.to_vec(),
            compaction_history: compaction_history.to_vec(),
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
            if path.extension().is_some_and(|e| e == "json") {
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
    
    /// Fork 会话
    /// 
    /// 从指定会话的某个消息位置创建分支，生成新的会话
    /// 
    /// # Arguments
    /// * `session_id` - 原会话 ID
    /// * `fork_at_message_index` - fork 的消息索引，None 表示保留全部消息
    /// 
    /// # Returns
    /// 返回新会话的 ID
    pub async fn fork_session(
        &self,
        session_id: &str,
        fork_at_message_index: Option<usize>,
    ) -> anyhow::Result<String> {
        // 加载原会话
        let saved_session = self.load_session(session_id).await?;
        
        // 截断消息到指定索引（如果指定了索引）
        let messages: Vec<AgentMessage> = if let Some(index) = fork_at_message_index {
            saved_session.messages.into_iter().take(index).collect()
        } else {
            saved_session.messages
        };
        
        // 生成新会话 ID
        let new_session_id = Self::generate_session_id();
        
        // 创建新 metadata
        let title = extract_title(&messages);
        let model = extract_model(&messages);
        let now = chrono::Utc::now().timestamp_millis();
        
        let new_session = SavedSession {
            metadata: SessionMetadata {
                id: new_session_id.clone(),
                title,
                created_at: now,
                updated_at: now,
                message_count: messages.len(),
                model,
                parent_session_id: Some(session_id.to_string()),
                fork_at_index: fork_at_message_index,
            },
            messages,
            compaction_history: Vec::new(), // Fork 的会话不继承压缩历史
        };
        
        // 保存新会话文件
        let path = self.session_path(&new_session_id);
        let json = serde_json::to_string_pretty(&new_session)?;
        tokio::fs::write(&path, json).await?;
        
        Ok(new_session_id)
    }
    
    /// 列出指定会话的所有 fork 子会话
    /// 
    /// # Arguments
    /// * `session_id` - 父会话 ID
    /// 
    /// # Returns
    /// 返回所有 parent_session_id 等于给定 session_id 的会话元信息列表
    pub async fn list_forks(&self, session_id: &str) -> anyhow::Result<Vec<SessionMetadata>> {
        let all_sessions = self.list_sessions().await?;
        
        let forks: Vec<SessionMetadata> = all_sessions
            .into_iter()
            .filter(|s| s.parent_session_id.as_ref() == Some(&session_id.to_string()))
            .collect();
        
        Ok(forks)
    }
    
    /// 获取会话的完整分支树
    /// 
    /// 向上追溯到根会话，向下列出所有分支
    /// 
    /// # Arguments
    /// * `session_id` - 起始会话 ID
    /// 
    /// # Returns
    /// 返回包含该会话及其所有后代的会话元信息列表
    pub async fn get_session_tree(&self, session_id: &str) -> anyhow::Result<Vec<SessionMetadata>> {
        let mut tree = Vec::new();
        let mut to_process = vec![session_id.to_string()];
        let mut processed = std::collections::HashSet::new();
        
        while let Some(current_id) = to_process.pop() {
            if processed.contains(&current_id) {
                continue;
            }
            processed.insert(current_id.clone());
            
            // 尝试加载当前会话
            if let Ok(saved_session) = self.load_session(&current_id).await {
                tree.push(saved_session.metadata);
                
                // 查找该会话的所有子会话
                let children = self.list_forks(&current_id).await?;
                for child in children {
                    if !processed.contains(&child.id) {
                        to_process.push(child.id);
                    }
                }
            }
        }
        
        Ok(tree)
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
    use tempfile::TempDir;

    fn create_test_manager() -> (SessionManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let manager = SessionManager::with_dir(dir.path().to_path_buf()).unwrap();
        (manager, dir)
    }

    fn create_assistant_message(_text: &str) -> AgentMessage {
        AgentMessage::Llm(pi_ai::types::Message::Assistant(pi_ai::types::AssistantMessage::new(
            pi_ai::types::Api::Anthropic,
            pi_ai::types::Provider::Anthropic,
            "claude-3"
        )))
    }

    #[tokio::test]
    async fn test_create_session() {
        let (_manager, _dir) = create_test_manager();
        let session_id = SessionManager::generate_session_id();
        
        // 验证生成的会话 ID 是有效的 UUID
        assert!(!session_id.is_empty());
        assert!(uuid::Uuid::parse_str(&session_id).is_ok());
    }

    #[tokio::test]
    async fn test_save_and_load_session() {
        let (manager, _dir) = create_test_manager();
        let session_id = "test-session-123";
        
        let messages = vec![
            AgentMessage::user("Hello, world!"),
            create_assistant_message("Hi there!"),
        ];
        
        // 保存会话
        let path = manager.save_session(session_id, &messages).await.unwrap();
        assert!(path.exists());
        
        // 加载会话
        let loaded = manager.load_session(session_id).await.unwrap();
        assert_eq!(loaded.metadata.id, session_id);
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.metadata.message_count, 2);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (manager, _dir) = create_test_manager();
        
        // 创建多个会话
        for i in 0..3 {
            let session_id = format!("session-{}", i);
            let messages = vec![AgentMessage::user(&format!("Message {}", i))];
            manager.save_session(&session_id, &messages).await.unwrap();
            // 小延迟确保更新时间不同
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        // 列出会话
        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
        
        // 验证按更新时间排序（最新的在前）
        for i in 0..sessions.len() - 1 {
            assert!(sessions[i].updated_at >= sessions[i + 1].updated_at);
        }
    }

    #[tokio::test]
    async fn test_fork_session() {
        let (manager, _dir) = create_test_manager();
        let parent_id = "parent-session";
        
        let messages = vec![
            AgentMessage::user("Message 1"),
            create_assistant_message("Response 1"),
            AgentMessage::user("Message 2"),
            create_assistant_message("Response 2"),
        ];
        
        // 保存父会话
        manager.save_session(parent_id, &messages).await.unwrap();
        
        // Fork 会话（保留前 2 条消息）
        let forked_id = manager.fork_session(parent_id, Some(2)).await.unwrap();
        
        // 验证 Fork 的会话
        let forked = manager.load_session(&forked_id).await.unwrap();
        assert_eq!(forked.metadata.parent_session_id, Some(parent_id.to_string()));
        assert_eq!(forked.metadata.fork_at_index, Some(2));
        assert_eq!(forked.messages.len(), 2);
        assert!(forked.compaction_history.is_empty()); // Fork 不继承压缩历史
    }

    #[tokio::test]
    async fn test_fork_session_full() {
        let (manager, _dir) = create_test_manager();
        let parent_id = "parent-session-full";
        
        let messages = vec![
            AgentMessage::user("Message 1"),
            create_assistant_message("Response 1"),
        ];
        
        manager.save_session(parent_id, &messages).await.unwrap();
        
        // Fork 会话（保留全部消息）
        let forked_id = manager.fork_session(parent_id, None).await.unwrap();
        
        let forked = manager.load_session(&forked_id).await.unwrap();
        assert_eq!(forked.messages.len(), 2);
        assert_eq!(forked.metadata.fork_at_index, None);
    }

    #[tokio::test]
    async fn test_delete_session() {
        let (manager, _dir) = create_test_manager();
        let session_id = "to-delete";
        
        let messages = vec![AgentMessage::user("Test")];
        manager.save_session(session_id, &messages).await.unwrap();
        
        // 验证会话存在
        assert!(manager.session_exists(session_id));
        
        // 删除会话
        manager.delete_session(session_id).await.unwrap();
        
        // 验证会话已删除
        assert!(!manager.session_exists(session_id));
    }

    #[tokio::test]
    async fn test_list_forks() {
        let (manager, _dir) = create_test_manager();
        let parent_id = "parent-for-forks";
        
        let messages = vec![AgentMessage::user("Parent")];
        manager.save_session(parent_id, &messages).await.unwrap();
        
        // 创建两个 Fork
        let fork1 = manager.fork_session(parent_id, Some(1)).await.unwrap();
        let fork2 = manager.fork_session(parent_id, Some(1)).await.unwrap();
        
        // 列出 Forks
        let forks = manager.list_forks(parent_id).await.unwrap();
        assert_eq!(forks.len(), 2);
        
        let fork_ids: Vec<_> = forks.iter().map(|f| &f.id).collect();
        assert!(fork_ids.contains(&&fork1));
        assert!(fork_ids.contains(&&fork2));
    }

    #[tokio::test]
    async fn test_get_session_tree() {
        let (manager, _dir) = create_test_manager();
        let root_id = "root-session";
        
        let messages = vec![AgentMessage::user("Root")];
        manager.save_session(root_id, &messages).await.unwrap();
        
        // 创建 Fork 链
        let child1 = manager.fork_session(root_id, None).await.unwrap();
        let grandchild = manager.fork_session(&child1, None).await.unwrap();
        
        // 获取树
        let tree = manager.get_session_tree(root_id).await.unwrap();
        assert_eq!(tree.len(), 3);
        
        let ids: Vec<_> = tree.iter().map(|s| s.id.clone()).collect();
        assert!(ids.contains(&root_id.to_string()));
        assert!(ids.contains(&child1));
        assert!(ids.contains(&grandchild));
    }

    #[tokio::test]
    async fn test_find_most_recent() {
        let (manager, _dir) = create_test_manager();
        
        // 创建会话
        let session_id = "recent-session";
        let messages = vec![AgentMessage::user("Recent")];
        manager.save_session(session_id, &messages).await.unwrap();
        
        // 查找最近的会话
        let recent = manager.find_most_recent().await.unwrap();
        assert!(recent.is_some());
        assert_eq!(recent.unwrap().id, session_id);
    }

    #[tokio::test]
    async fn test_list_sessions_filtered() {
        let (manager, _dir) = create_test_manager();
        
        // 创建不同模型的会话
        let messages1 = vec![
            AgentMessage::user("Test"),
            AgentMessage::Llm(pi_ai::types::Message::Assistant(pi_ai::types::AssistantMessage::new(
                pi_ai::types::Api::Anthropic,
                pi_ai::types::Provider::Anthropic,
                "claude-3"
            ))),
        ];
        
        manager.save_session("session-1", &messages1).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let messages2 = vec![AgentMessage::user("Test 2")];
        manager.save_session("session-2", &messages2).await.unwrap();
        
        // 按模型过滤
        let filter = SessionFilter {
            model: Some("claude".to_string()),
            before: None,
            after: None,
        };
        let filtered = manager.list_sessions_filtered(&filter).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "session-1");
    }

    #[tokio::test]
    async fn test_save_session_with_compaction() {
        let (manager, _dir) = create_test_manager();
        let session_id = "compacted-session";
        
        let messages = vec![
            AgentMessage::user("Message 1"),
            AgentMessage::Llm(pi_ai::types::Message::Assistant(pi_ai::types::AssistantMessage::new(
                pi_ai::types::Api::Anthropic,
                pi_ai::types::Provider::Anthropic,
                "claude-3"
            ))),
        ];
        
        let compaction_history = vec![
            CompactionRecord {
                compacted_at: chrono::Utc::now().timestamp_millis(),
                removed_message_range: (0, 2),
                summary_tokens: 50,
                original_tokens: 200,
            },
        ];
        
        let path = manager.save_session_with_compaction(session_id, &messages, &compaction_history).await.unwrap();
        assert!(path.exists());
        
        let loaded = manager.load_session(session_id).await.unwrap();
        assert_eq!(loaded.compaction_history.len(), 1);
        assert_eq!(loaded.compaction_history[0].original_tokens, 200);
    }

    #[tokio::test]
    async fn test_load_session_from_path() {
        let (manager, _dir) = create_test_manager();
        let session_id = "path-test";
        
        let messages = vec![AgentMessage::user("Test")];
        let path = manager.save_session(session_id, &messages).await.unwrap();
        
        // 从路径加载
        let loaded = SessionManager::load_session_from_path(&path).await.unwrap();
        assert_eq!(loaded.metadata.id, session_id);
    }
    
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
