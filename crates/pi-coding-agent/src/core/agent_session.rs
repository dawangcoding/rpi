//! Agent 会话管理核心
//!
//! 负责 Agent 会话的生命周期管理、事件处理和统计

use std::sync::Arc;
use tokio::sync::RwLock;
use pi_agent::agent::Agent;
use pi_agent::types::*;
use pi_ai::types::*;
use tokio_util::sync::CancellationToken;

use super::system_prompt::*;
use super::tools;
use super::session_manager::SessionManager;
use crate::config::AppConfig;

/// 会话统计
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionStats {
    pub session_id: String,
    pub session_file: Option<String>,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_calls: usize,
    pub tool_results: usize,
    pub total_messages: usize,
    pub tokens: TokenStats,
    pub cost: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TokenStats {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total: u64,
}

/// 会话配置
pub struct AgentSessionConfig {
    pub model: Model,
    pub thinking_level: ThinkingLevel,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub context_files: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub no_bash: bool,
    pub no_edit: bool,
    pub app_config: AppConfig,
    pub session_id: Option<String>,
}

/// Agent 会话
pub struct AgentSession {
    agent: Agent,
    config: AgentSessionConfig,
    stats: Arc<RwLock<SessionStats>>,
    session_manager: Option<SessionManager>,
}

impl AgentSession {
    /// 创建新会话
    pub async fn new(config: AgentSessionConfig) -> anyhow::Result<Self> {
        let session_id = config.session_id.clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        
        // 构建工具列表
        let mut tool_list: Vec<Arc<dyn AgentTool>> = vec![
            Arc::new(tools::ReadTool::new(config.cwd.clone())),
            Arc::new(tools::WriteTool::new(config.cwd.clone())),
            Arc::new(tools::GrepTool::new(config.cwd.clone())),
            Arc::new(tools::FindTool::new(config.cwd.clone())),
            Arc::new(tools::LsTool::new(config.cwd.clone())),
        ];
        
        if !config.no_bash {
            tool_list.push(Arc::new(tools::BashTool::new(config.cwd.clone())));
        }
        if !config.no_edit {
            tool_list.push(Arc::new(tools::EditTool::new(config.cwd.clone())));
        }
        
        // 构建系统提示词
        let context_files = load_all_context_files(&config.context_files, &config.cwd);
        let system_prompt = build_system_prompt(&BuildSystemPromptOptions {
            custom_prompt: config.system_prompt.clone(),
            append_system_prompt: config.append_system_prompt.clone(),
            tools: tool_list.clone(),
            guidelines: vec![],
            context_files,
            cwd: config.cwd.clone(),
        });
        
        // 获取 API key
        let provider_str = format!("{:?}", config.model.provider).to_lowercase();
        let api_key = config.app_config.get_api_key(&provider_str);
        
        // 创建 Agent
        let mut agent_options = pi_agent::agent::AgentOptions {
            model: Some(config.model.clone()),
            system_prompt: Some(system_prompt),
            tools: tool_list,
            thinking_level: config.thinking_level.clone(),
            tool_execution: ToolExecutionMode::Parallel,
            session_id: Some(session_id.clone()),
            ..Default::default()
        };
        
        // 设置 API key 回调
        if let Some(key) = api_key {
            agent_options.get_api_key = Some(Arc::new(move |_provider: &str| Some(key.clone())));
        }
        
        let agent = Agent::new(agent_options);
        
        let stats = Arc::new(RwLock::new(SessionStats {
            session_id: session_id.clone(),
            ..Default::default()
        }));
        
        // 设置事件监听器用于统计
        let stats_clone = stats.clone();
        agent.subscribe(Arc::new(move |event: AgentEvent, _cancel: CancellationToken| {
            let stats = stats_clone.clone();
            tokio::spawn(async move {
                let mut s = stats.write().await;
                match &event {
                    AgentEvent::MessageEnd { message } => {
                        match message {
                            AgentMessage::Llm(Message::User(_)) => s.user_messages += 1,
                            AgentMessage::Llm(Message::Assistant(msg)) => {
                                s.assistant_messages += 1;
                                s.tokens.input += msg.usage.input_tokens;
                                s.tokens.output += msg.usage.output_tokens;
                                if let Some(cr) = msg.usage.cache_read_tokens {
                                    s.tokens.cache_read += cr;
                                }
                                if let Some(cw) = msg.usage.cache_write_tokens {
                                    s.tokens.cache_write += cw;
                                }
                                s.tokens.total = s.tokens.input + s.tokens.output;
                            },
                            AgentMessage::Llm(Message::ToolResult(_)) => s.tool_results += 1,
                        }
                        s.total_messages += 1;
                    },
                    AgentEvent::ToolExecutionEnd { .. } => {
                        s.tool_calls += 1;
                    },
                    _ => {}
                }
            });
        }));
        
        // 会话管理器
        let session_manager = SessionManager::new(&config.app_config)?;
        
        Ok(Self {
            agent,
            config,
            stats,
            session_manager: Some(session_manager),
        })
    }
    
    /// 发送 prompt
    pub async fn prompt(&self, message: AgentMessage) -> anyhow::Result<()> {
        self.agent.prompt(message).await
    }
    
    /// 发送文本 prompt
    pub async fn prompt_text(&self, text: &str) -> anyhow::Result<()> {
        self.agent.prompt_text(text).await
    }
    
    /// 获取 Agent 引用
    pub fn agent(&self) -> &Agent { 
        &self.agent 
    }
    
    /// 获取统计
    pub async fn stats(&self) -> SessionStats {
        self.stats.read().await.clone()
    }
    
    /// 中止
    pub async fn abort(&self) { 
        self.agent.abort().await; 
    }
    
    /// 等待空闲
    pub async fn wait_for_idle(&self) { 
        self.agent.wait_for_idle().await; 
    }
    
    /// 保存会话
    pub async fn save(&self) -> anyhow::Result<()> {
        if let Some(mgr) = &self.session_manager {
            let state = self.agent.state().await;
            mgr.save_session(&self.stats.read().await.session_id, &state.messages).await?;
        }
        Ok(())
    }
    
    /// 获取会话 ID
    pub fn session_id(&self) -> String {
        self.stats.blocking_read().session_id.clone()
    }
    
    /// 获取配置引用
    pub fn config(&self) -> &AgentSessionConfig {
        &self.config
    }
    
    /// 更新会话统计中的会话文件路径
    pub async fn set_session_file(&self, file: Option<String>) {
        self.stats.write().await.session_file = file;
    }
}

/// 创建 AgentSession 的 Builder 模式
pub struct AgentSessionBuilder {
    model: Option<Model>,
    thinking_level: ThinkingLevel,
    system_prompt: Option<String>,
    append_system_prompt: Option<String>,
    context_files: Vec<String>,
    cwd: std::path::PathBuf,
    no_bash: bool,
    no_edit: bool,
    app_config: Option<AppConfig>,
    session_id: Option<String>,
}

impl Default for AgentSessionBuilder {
    fn default() -> Self {
        Self {
            model: None,
            thinking_level: ThinkingLevel::Off,
            system_prompt: None,
            append_system_prompt: None,
            context_files: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            no_bash: false,
            no_edit: false,
            app_config: None,
            session_id: None,
        }
    }
}

impl AgentSessionBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn model(mut self, model: Model) -> Self {
        self.model = Some(model);
        self
    }
    
    pub fn thinking_level(mut self, level: ThinkingLevel) -> Self {
        self.thinking_level = level;
        self
    }
    
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }
    
    pub fn append_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.append_system_prompt = Some(prompt.into());
        self
    }
    
    pub fn context_files(mut self, files: Vec<String>) -> Self {
        self.context_files = files;
        self
    }
    
    pub fn cwd(mut self, cwd: std::path::PathBuf) -> Self {
        self.cwd = cwd;
        self
    }
    
    pub fn no_bash(mut self, no_bash: bool) -> Self {
        self.no_bash = no_bash;
        self
    }
    
    pub fn no_edit(mut self, no_edit: bool) -> Self {
        self.no_edit = no_edit;
        self
    }
    
    pub fn app_config(mut self, config: AppConfig) -> Self {
        self.app_config = Some(config);
        self
    }
    
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }
    
    pub async fn build(self) -> anyhow::Result<AgentSession> {
        let app_config = self.app_config.unwrap_or_else(|| AppConfig::load().unwrap_or_default());
        
        let model = self.model.unwrap_or_else(|| {
            // 使用默认模型
            pi_ai::models::get_model("claude-sonnet-4-20250514")
                .unwrap_or_else(|| pi_ai::types::Model {
                    id: "claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4".to_string(),
                    api: pi_ai::types::Api::Anthropic,
                    provider: pi_ai::types::Provider::Anthropic,
                    base_url: "https://api.anthropic.com".to_string(),
                    reasoning: true,
                    input: vec![pi_ai::types::InputModality::Text, pi_ai::types::InputModality::Image],
                    cost: pi_ai::types::ModelCost {
                        input: 3.0,
                        output: 15.0,
                        cache_read: Some(0.3),
                        cache_write: Some(3.75),
                    },
                    context_window: 200000,
                    max_tokens: 16384,
                    headers: None,
                    compat: None,
                })
        });
        
        let config = AgentSessionConfig {
            model,
            thinking_level: self.thinking_level,
            system_prompt: self.system_prompt,
            append_system_prompt: self.append_system_prompt,
            context_files: self.context_files,
            cwd: self.cwd,
            no_bash: self.no_bash,
            no_edit: self.no_edit,
            app_config,
            session_id: self.session_id,
        };
        
        AgentSession::new(config).await
    }
}
