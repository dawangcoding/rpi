//! 模型注册表
//!
//! 管理内置的 LLM 模型定义和查询

use std::collections::HashMap;
use std::sync::OnceLock;
use crate::types::*;

/// 模型成本（每百万 token 的美元价格）
/// 
/// 定义模型的定价信息
#[derive(Debug, Clone, Copy)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

impl Default for ModelCost {
    fn default() -> Self {
        Self {
            input: 0.0,
            output: 0.0,
            cache_read: None,
            cache_write: None,
        }
    }
}

impl From<ModelCost> for crate::types::ModelCost {
    fn from(cost: ModelCost) -> Self {
        Self {
            input: cost.input,
            output: cost.output,
            cache_read: cost.cache_read,
            cache_write: cost.cache_write,
        }
    }
}

/// 注册内置模型
fn builtin_models() -> Vec<Model> {
    vec![
        // ==================== Anthropic ====================
        Model {
            id: "claude-sonnet-4-20250514".to_string(),
            name: "Claude Sonnet 4".to_string(),
            api: Api::Anthropic,
            provider: Provider::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            }.into(),
            context_window: 200000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "claude-3-5-sonnet-20241022".to_string(),
            name: "Claude 3.5 Sonnet".to_string(),
            api: Api::Anthropic,
            provider: Provider::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            }.into(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },
        Model {
            id: "claude-opus-4-20250514".to_string(),
            name: "Claude Opus 4".to_string(),
            api: Api::Anthropic,
            provider: Provider::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 15.0,
                output: 75.0,
                cache_read: Some(1.5),
                cache_write: Some(18.75),
            }.into(),
            context_window: 200000,
            max_tokens: 32000,
            headers: None,
            compat: None,
        },
        Model {
            id: "claude-3-7-sonnet-20250219".to_string(),
            name: "Claude 3.7 Sonnet".to_string(),
            api: Api::Anthropic,
            provider: Provider::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            }.into(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },
        
        // ==================== OpenAI ====================
        Model {
            id: "gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
            api: Api::OpenAiChatCompletions,
            provider: Provider::Openai,
            base_url: "https://api.openai.com/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 2.5,
                output: 10.0,
                cache_read: Some(1.25),
                cache_write: Some(2.5),
            }.into(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "gpt-4o-mini".to_string(),
            name: "GPT-4o Mini".to_string(),
            api: Api::OpenAiChatCompletions,
            provider: Provider::Openai,
            base_url: "https://api.openai.com/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.15,
                output: 0.6,
                cache_read: Some(0.075),
                cache_write: Some(0.15),
            }.into(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "o3-mini".to_string(),
            name: "o3-mini".to_string(),
            api: Api::OpenAiChatCompletions,
            provider: Provider::Openai,
            base_url: "https://api.openai.com/v1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 1.1,
                output: 4.4,
                cache_read: Some(0.55),
                cache_write: Some(1.1),
            }.into(),
            context_window: 200000,
            max_tokens: 100000,
            headers: None,
            compat: None,
        },
        Model {
            id: "o1".to_string(),
            name: "o1".to_string(),
            api: Api::OpenAiChatCompletions,
            provider: Provider::Openai,
            base_url: "https://api.openai.com/v1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 15.0,
                output: 60.0,
                cache_read: Some(7.5),
                cache_write: Some(15.0),
            }.into(),
            context_window: 200000,
            max_tokens: 100000,
            headers: None,
            compat: None,
        },
        
        // ==================== Google ====================
        Model {
            id: "gemini-2.5-pro-preview-05-06".to_string(),
            name: "Gemini 2.5 Pro".to_string(),
            api: Api::Google,
            provider: Provider::Google,
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 1.25,
                output: 10.0,
                cache_read: Some(0.315),
                cache_write: Some(1.25),
            }.into(),
            context_window: 1048576,
            max_tokens: 65536,
            headers: None,
            compat: None,
        },
        Model {
            id: "gemini-2.5-flash-preview-04-17".to_string(),
            name: "Gemini 2.5 Flash".to_string(),
            api: Api::Google,
            provider: Provider::Google,
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.15,
                output: 0.6,
                cache_read: Some(0.0375),
                cache_write: Some(0.15),
            }.into(),
            context_window: 1048576,
            max_tokens: 65536,
            headers: None,
            compat: None,
        },
        Model {
            id: "gemini-2.0-flash".to_string(),
            name: "Gemini 2.0 Flash".to_string(),
            api: Api::Google,
            provider: Provider::Google,
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.1,
                output: 0.4,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 1048576,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },

        // ==================== Mistral ====================
        Model {
            id: "mistral-large-latest".to_string(),
            name: "Mistral Large".to_string(),
            api: Api::Mistral,
            provider: Provider::Mistral,
            base_url: "https://api.mistral.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 2.0,
                output: 6.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        },
        Model {
            id: "mistral-small-latest".to_string(),
            name: "Mistral Small".to_string(),
            api: Api::Mistral,
            provider: Provider::Mistral,
            base_url: "https://api.mistral.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.1,
                output: 0.3,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 128000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        },
        Model {
            id: "codestral-latest".to_string(),
            name: "Codestral".to_string(),
            api: Api::Mistral,
            provider: Provider::Mistral,
            base_url: "https://api.mistral.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost {
                input: 0.3,
                output: 0.9,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 256000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        },

        // ==================== Amazon Bedrock ====================
        Model {
            id: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            name: "Claude 3.5 Sonnet (Bedrock)".to_string(),
            api: Api::AmazonBedrock,
            provider: Provider::AmazonBedrock,
            base_url: "bedrock://us-east-1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            }.into(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },
        Model {
            id: "anthropic.claude-3-opus-20240229-v1:0".to_string(),
            name: "Claude 3 Opus (Bedrock)".to_string(),
            api: Api::AmazonBedrock,
            provider: Provider::AmazonBedrock,
            base_url: "bedrock://us-east-1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 15.0,
                output: 75.0,
                cache_read: Some(1.5),
                cache_write: Some(18.75),
            }.into(),
            context_window: 200000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        },
        Model {
            id: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            name: "Claude 3 Haiku (Bedrock)".to_string(),
            api: Api::AmazonBedrock,
            provider: Provider::AmazonBedrock,
            base_url: "bedrock://us-east-1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.25,
                output: 1.25,
                cache_read: Some(0.03),
                cache_write: Some(0.3),
            }.into(),
            context_window: 200000,
            max_tokens: 4096,
            headers: None,
            compat: None,
        },
        Model {
            id: "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
            name: "Claude 3.5 Haiku (Bedrock)".to_string(),
            api: Api::AmazonBedrock,
            provider: Provider::AmazonBedrock,
            base_url: "bedrock://us-east-1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.8,
                output: 4.0,
                cache_read: Some(0.08),
                cache_write: Some(1.0),
            }.into(),
            context_window: 200000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },

        // ==================== Azure OpenAI ====================
        Model {
            id: "azure/gpt-4o".to_string(),
            name: "GPT-4o (Azure)".to_string(),
            api: Api::AzureOpenAiResponses,
            provider: Provider::AzureOpenAiResponses,
            base_url: "".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 2.5,
                output: 10.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "azure/gpt-4o-mini".to_string(),
            name: "GPT-4o Mini (Azure)".to_string(),
            api: Api::AzureOpenAiResponses,
            provider: Provider::AzureOpenAiResponses,
            base_url: "".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.15,
                output: 0.6,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "azure/o3-mini".to_string(),
            name: "o3-mini (Azure)".to_string(),
            api: Api::AzureOpenAiResponses,
            provider: Provider::AzureOpenAiResponses,
            base_url: "".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 1.1,
                output: 4.4,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 200000,
            max_tokens: 100000,
            headers: None,
            compat: None,
        },
        Model {
            id: "azure/o1".to_string(),
            name: "o1 (Azure)".to_string(),
            api: Api::AzureOpenAiResponses,
            provider: Provider::AzureOpenAiResponses,
            base_url: "".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 15.0,
                output: 60.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 200000,
            max_tokens: 100000,
            headers: None,
            compat: None,
        },

        // ==================== xAI ====================
        Model {
            id: "grok-3".to_string(),
            name: "Grok 3".to_string(),
            api: Api::Xai,
            provider: Provider::Xai,
            base_url: "https://api.x.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 131072,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "grok-3-mini".to_string(),
            name: "Grok 3 Mini".to_string(),
            api: Api::Xai,
            provider: Provider::Xai,
            base_url: "https://api.x.ai/v1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text],
            cost: ModelCost {
                input: 0.3,
                output: 0.5,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 131072,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "grok-2-vision-1212".to_string(),
            name: "Grok 2 Vision".to_string(),
            api: Api::Xai,
            provider: Provider::Xai,
            base_url: "https://api.x.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 2.0,
                output: 10.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 32768,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },

        // ==================== OpenRouter ====================
        Model {
            id: "openrouter/auto".to_string(),
            name: "OpenRouter Auto".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost::default().into(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "openrouter/anthropic/claude-sonnet-4-20250514".to_string(),
            name: "Claude Sonnet 4 (OpenRouter)".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: Some(0.3),
                cache_write: Some(3.75),
            }.into(),
            context_window: 200000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "openrouter/google/gemini-2.5-flash-preview-04-17".to_string(),
            name: "Gemini 2.5 Flash (OpenRouter)".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.15,
                output: 0.6,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 1048576,
            max_tokens: 65536,
            headers: None,
            compat: None,
        },
        Model {
            id: "openrouter/openai/gpt-4o".to_string(),
            name: "GPT-4o (OpenRouter)".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 2.5,
                output: 10.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 128000,
            max_tokens: 16384,
            headers: None,
            compat: None,
        },
        Model {
            id: "openrouter/meta-llama/llama-4-maverick".to_string(),
            name: "Llama 4 Maverick (OpenRouter)".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text, InputModality::Image],
            cost: ModelCost {
                input: 0.25,
                output: 1.0,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 1048576,
            max_tokens: 65536,
            headers: None,
            compat: None,
        },
        Model {
            id: "openrouter/deepseek/deepseek-r1".to_string(),
            name: "DeepSeek R1 (OpenRouter)".to_string(),
            api: Api::Openrouter,
            provider: Provider::Openrouter,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            reasoning: true,
            input: vec![InputModality::Text],
            cost: ModelCost {
                input: 0.55,
                output: 2.19,
                cache_read: None,
                cache_write: None,
            }.into(),
            context_window: 163840,
            max_tokens: 65536,
            headers: None,
            compat: None,
        },

        // ==================== Groq ====================
        Model {
            id: "llama-3.3-70b-versatile".to_string(),
            name: "Llama 3.3 70B Versatile".to_string(),
            api: Api::Groq,
            provider: Provider::Groq,
            base_url: "https://api.groq.com/openai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost { input: 0.59, output: 0.79, cache_read: None, cache_write: None }.into(),
            context_window: 128000,
            max_tokens: 32768,
            headers: None,
            compat: None,
        },
        Model {
            id: "llama-3.1-8b-instant".to_string(),
            name: "Llama 3.1 8B Instant".to_string(),
            api: Api::Groq,
            provider: Provider::Groq,
            base_url: "https://api.groq.com/openai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost { input: 0.05, output: 0.08, cache_read: None, cache_write: None }.into(),
            context_window: 131072,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },
        Model {
            id: "mixtral-8x7b-32768".to_string(),
            name: "Mixtral 8x7B".to_string(),
            api: Api::Groq,
            provider: Provider::Groq,
            base_url: "https://api.groq.com/openai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost { input: 0.24, output: 0.24, cache_read: None, cache_write: None }.into(),
            context_window: 32768,
            max_tokens: 32768,
            headers: None,
            compat: None,
        },

        // ==================== Cerebras ====================
        Model {
            id: "llama3.1-8b".to_string(),
            name: "Llama 3.1 8B (Cerebras)".to_string(),
            api: Api::Cerebras,
            provider: Provider::Cerebras,
            base_url: "https://api.cerebras.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost { input: 0.10, output: 0.10, cache_read: None, cache_write: None }.into(),
            context_window: 128000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },
        Model {
            id: "llama3.1-70b".to_string(),
            name: "Llama 3.1 70B (Cerebras)".to_string(),
            api: Api::Cerebras,
            provider: Provider::Cerebras,
            base_url: "https://api.cerebras.ai/v1".to_string(),
            reasoning: false,
            input: vec![InputModality::Text],
            cost: ModelCost { input: 0.60, output: 0.60, cache_read: None, cache_write: None }.into(),
            context_window: 128000,
            max_tokens: 8192,
            headers: None,
            compat: None,
        },
    ]
}

// 全局模型注册表
static MODEL_REGISTRY: OnceLock<HashMap<String, Model>> = OnceLock::new();

/// 初始化模型注册表
fn init_model_registry() -> HashMap<String, Model> {
    let models = builtin_models();
    let mut registry = HashMap::with_capacity(models.len());
    
    for model in models {
        registry.insert(model.id.clone(), model);
    }
    
    registry
}

/// 获取模型注册表
fn get_registry() -> &'static HashMap<String, Model> {
    MODEL_REGISTRY.get_or_init(init_model_registry)
}

/// 通过模型 ID 获取模型
/// 
/// 从模型注册表中查找指定 ID 的模型
pub fn get_model(id: &str) -> Option<Model> {
    get_registry().get(id).cloned()
}

/// 获取所有模型
/// 
/// 返回注册表中所有可用的模型
pub fn get_models() -> Vec<Model> {
    get_registry().values().cloned().collect()
}

/// 根据 provider 获取模型
/// 
/// 筛选指定提供商的模型列表
pub fn get_models_by_provider(provider: &Provider) -> Vec<Model> {
    get_registry()
        .values()
        .filter(|m| &m.provider == provider)
        .cloned()
        .collect()
}

/// 根据 API 类型获取模型
/// 
/// 筛选指定 API 类型的模型列表
pub fn get_models_by_api(api: &Api) -> Vec<Model> {
    get_registry()
        .values()
        .filter(|m| &m.api == api)
        .cloned()
        .collect()
}

/// 计算成本（返回美元）
/// 
/// 根据 token 使用量计算 API 调用成本
pub fn calculate_cost(model: &Model, usage: &Usage) -> f64 {
    let input_cost = (model.cost.input / 1_000_000.0) * usage.input_tokens as f64;
    let output_cost = (model.cost.output / 1_000_000.0) * usage.output_tokens as f64;
    
    let cache_read_cost = usage.cache_read_tokens.map(|tokens| {
        model.cost.cache_read.unwrap_or(0.0) / 1_000_000.0 * tokens as f64
    }).unwrap_or(0.0);
    
    let cache_write_cost = usage.cache_write_tokens.map(|tokens| {
        model.cost.cache_write.unwrap_or(0.0) / 1_000_000.0 * tokens as f64
    }).unwrap_or(0.0);
    
    input_cost + output_cost + cache_read_cost + cache_write_cost
}

/// 检查模型是否支持 xhigh thinking level
/// 
/// 判断模型是否支持最高级别的思考模式
pub fn supports_xhigh(model: &Model) -> bool {
    let id = &model.id;
    id.contains("gpt-5.2") || id.contains("gpt-5.3") || id.contains("gpt-5.4")
        || id.contains("opus-4-6") || id.contains("opus-4.6")
}

/// 检查两个模型是否相等
/// 
/// 比较两个模型的 ID 和提供商
pub fn models_are_equal(a: Option<&Model>, b: Option<&Model>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.id == b.id && a.provider == b.provider,
        _ => false,
    }
}

/// 从环境变量获取 API Key
/// 
/// 根据提供商从环境变量读取 API 密钥
pub fn get_api_key_from_env(provider: &Provider) -> Option<String> {
    use std::env;
    
    match provider {
        Provider::Anthropic => {
            env::var("ANTHROPIC_OAUTH_TOKEN")
                .ok()
                .or_else(|| env::var("ANTHROPIC_API_KEY").ok())
        }
        Provider::Openai => env::var("OPENAI_API_KEY").ok(),
        Provider::Google | Provider::GoogleGeminiCli | Provider::GoogleAntigravity => {
            env::var("GOOGLE_API_KEY")
                .ok()
                .or_else(|| env::var("GEMINI_API_KEY").ok())
        }
        Provider::GoogleVertex => {
            env::var("GOOGLE_CLOUD_API_KEY").ok()
        }
        Provider::Groq => env::var("GROQ_API_KEY").ok(),
        Provider::Cerebras => env::var("CEREBRAS_API_KEY").ok(),
        Provider::Xai => env::var("XAI_API_KEY").ok(),
        Provider::Openrouter => env::var("OPENROUTER_API_KEY").ok(),
        Provider::VercelAiGateway => env::var("AI_GATEWAY_API_KEY").ok(),
        Provider::Mistral => env::var("MISTRAL_API_KEY").ok(),
        Provider::Minimax => env::var("MINIMAX_API_KEY").ok(),
        Provider::MinimaxCn => env::var("MINIMAX_CN_API_KEY").ok(),
        Provider::Huggingface => env::var("HF_TOKEN").ok(),
        Provider::Opencode | Provider::OpencodeGo => env::var("OPENCODE_API_KEY").ok(),
        Provider::KimiCoding => env::var("KIMI_API_KEY").ok(),
        Provider::AzureOpenAiResponses => env::var("AZURE_OPENAI_API_KEY").ok(),
        Provider::OpenAiCodex => env::var("OPENAI_CODEX_API_KEY").ok(),
        Provider::GithubCopilot => {
            env::var("COPILOT_GITHUB_TOKEN")
                .ok()
                .or_else(|| env::var("GH_TOKEN").ok())
                .or_else(|| env::var("GITHUB_TOKEN").ok())
        }
        Provider::Zai => env::var("ZAI_API_KEY").ok(),
        Provider::AmazonBedrock => {
            // Amazon Bedrock 使用 AWS 凭证
            if env::var("AWS_PROFILE").is_ok()
                || (env::var("AWS_ACCESS_KEY_ID").is_ok() && env::var("AWS_SECRET_ACCESS_KEY").is_ok())
                || env::var("AWS_BEARER_TOKEN_BEDROCK").is_ok()
                || env::var("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI").is_ok()
                || env::var("AWS_CONTAINER_CREDENTIALS_FULL_URI").is_ok()
                || env::var("AWS_WEB_IDENTITY_TOKEN_FILE").is_ok()
            {
                Some("<authenticated>".to_string())
            } else {
                None
            }
        }
        Provider::Other(_) => None,
    }
}

/// 获取指定 provider 的 API key 环境变量名称
/// 
/// 返回提供商对应的环境变量名
pub fn get_api_key_env_var(provider: &Provider) -> Option<&'static str> {
    match provider {
        Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
        Provider::Openai => Some("OPENAI_API_KEY"),
        Provider::Google | Provider::GoogleGeminiCli | Provider::GoogleAntigravity => Some("GEMINI_API_KEY"),
        Provider::GoogleVertex => Some("GOOGLE_CLOUD_API_KEY"),
        Provider::Groq => Some("GROQ_API_KEY"),
        Provider::Cerebras => Some("CEREBRAS_API_KEY"),
        Provider::Xai => Some("XAI_API_KEY"),
        Provider::Openrouter => Some("OPENROUTER_API_KEY"),
        Provider::VercelAiGateway => Some("AI_GATEWAY_API_KEY"),
        Provider::Mistral => Some("MISTRAL_API_KEY"),
        Provider::Minimax => Some("MINIMAX_API_KEY"),
        Provider::MinimaxCn => Some("MINIMAX_CN_API_KEY"),
        Provider::Huggingface => Some("HF_TOKEN"),
        Provider::Opencode | Provider::OpencodeGo => Some("OPENCODE_API_KEY"),
        Provider::KimiCoding => Some("KIMI_API_KEY"),
        Provider::AzureOpenAiResponses => Some("AZURE_OPENAI_API_KEY"),
        Provider::OpenAiCodex => Some("OPENAI_CODEX_API_KEY"),
        Provider::GithubCopilot => Some("GITHUB_TOKEN"),
        Provider::Zai => Some("ZAI_API_KEY"),
        Provider::AmazonBedrock => Some("AWS_ACCESS_KEY_ID"),
        Provider::Other(_) => None,
    }
}
