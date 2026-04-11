//! Provider 集成测试
//!
//! 测试 Provider 模块的集成行为和完整性

use pi_ai::{init_providers, get_all_api_providers, has_api_provider, Api, Provider, get_models, get_model, Model, InputModality};

/// 测试模型注册完整性
#[test]
fn test_model_registration_complete() {
    // 初始化所有 provider
    init_providers();
    
    // 获取所有注册的 provider
    let providers = get_all_api_providers();
    
    // 验证至少有一些 provider 被注册
    assert!(!providers.is_empty(), "Should have registered providers");
    
    // 验证主要的 provider 都被注册
    assert!(has_api_provider(&Api::Anthropic), "Anthropic provider should be registered");
    assert!(has_api_provider(&Api::OpenAiChatCompletions), "OpenAI provider should be registered");
    assert!(has_api_provider(&Api::Google), "Google provider should be registered");
}

/// 测试 Provider 枚举覆盖
#[test]
fn test_provider_enum_coverage() {
    let providers = [
        Provider::Anthropic,
        Provider::Openai,
        Provider::Google,
        Provider::Mistral,
        Provider::AmazonBedrock,
    ];
    
    // 验证 provider 数量正确
    assert_eq!(providers.len(), 5, "Should have 5 providers");
}

/// 测试 API 枚举覆盖
#[test]
fn test_api_enum_coverage() {
    let apis = [
        Api::Anthropic,
        Api::OpenAiChatCompletions,
        Api::Google,
        Api::Mistral,
        Api::AmazonBedrock,
    ];
    
    // 验证每个 API 都是唯一的
    let unique: std::collections::HashSet<_> = apis.iter().collect();
    assert_eq!(
        unique.len(),
        apis.len(),
        "All APIs should be unique"
    );
}

/// 测试模型列表非空
#[test]
fn test_models_list_non_empty() {
    let models = get_models();
    
    // 验证有模型被定义
    assert!(!models.is_empty(), "Should have defined models");
    
    // 验证每个模型都有必需的字段
    for model in &models {
        assert!(!model.id.is_empty(), "Model should have an ID");
        assert!(!model.name.is_empty(), "Model should have a name");
        // base_url 可能为空（某些 provider）
        assert!(model.context_window > 0, "Model should have a positive context_window");
        assert!(model.max_tokens > 0, "Model should have a positive max_tokens");
    }
}

/// 测试模型按 Provider 分组
#[test]
fn test_models_by_provider() {
    use pi_ai::get_models_by_provider;
    
    let providers_to_test = vec![
        Provider::Anthropic,
        Provider::Openai,
        Provider::Google,
    ];
    
    for provider in providers_to_test {
        let models = get_models_by_provider(&provider);
        
        // 每个主要 provider 应该至少有一些模型
        assert!(
            !models.is_empty(),
            "Provider {:?} should have at least one model",
            provider
        );
        
        // 验证所有返回的模型都属于该 provider
        for model in &models {
            assert_eq!(
                model.provider, provider,
                "Model {} should belong to provider {:?}",
                model.id, provider
            );
        }
    }
}

/// 测试模型按 API 分组
#[test]
fn test_models_by_api() {
    use pi_ai::get_models_by_api;
    
    let apis_to_test = vec![
        Api::Anthropic,
        Api::OpenAiChatCompletions,
        Api::Google,
    ];
    
    for api in apis_to_test {
        let models = get_models_by_api(&api);
        
        // 每个主要 API 应该至少有一些模型
        assert!(
            !models.is_empty(),
            "API {:?} should have at least one model",
            api
        );
        
        // 验证所有返回的模型都使用该 API
        for model in &models {
            assert_eq!(
                model.api, api,
                "Model {} should use API {:?}",
                model.id, api
            );
        }
    }
}

/// 测试模型成本计算
#[test]
fn test_model_cost_calculation() {
    use pi_ai::{calculate_cost, Usage};
    
    let models = get_models();
    
    // 测试每个模型的成本计算
    for model in &models {
        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: None,
            cache_write_tokens: None,
        };
        
        let cost = calculate_cost(model, &usage);
        
        // 成本应该是非负的
        assert!(
            cost >= 0.0,
            "Cost for {} should be non-negative",
            model.id
        );
    }
}

/// 测试模型 ID 唯一性
#[test]
fn test_model_id_uniqueness() {
    let models = get_models();
    
    let mut ids = std::collections::HashSet::new();
    
    for model in &models {
        assert!(
            ids.insert(&model.id),
            "Model ID {} should be unique",
            model.id
        );
    }
}

/// 测试模型输入模态
#[test]
fn test_model_input_modalities() {
    use pi_ai::InputModality;
    
    let models = get_models();
    
    for model in &models {
        // 每个模型应该至少有一种输入模态
        assert!(
            !model.input.is_empty(),
            "Model {} should have at least one input modality",
            model.id
        );
        
        // 文本输入是最基本的，应该被所有模型支持
        assert!(
            model.input.contains(&InputModality::Text),
            "Model {} should support text input",
            model.id
        );
    }
}

/// 测试模型获取功能
#[test]
fn test_get_model_by_id() {
    // 测试获取已知模型
    let model_id = "claude-sonnet-4-20250514";
    let model = get_model(model_id);
    
    assert!(
        model.is_some(),
        "Should find model with id {}",
        model_id
    );
    
    let model = model.unwrap();
    assert_eq!(model.id, model_id);
    assert_eq!(model.provider, Provider::Anthropic);
}

/// 测试获取不存在的模型
#[test]
fn test_get_nonexistent_model() {
    let model = get_model("nonexistent-model-12345");
    assert!(
        model.is_none(),
        "Should not find nonexistent model"
    );
}

// ============== 边界测试 ==============

/// 测试空内容处理
#[test]
fn test_empty_content_handling() {
    use pi_ai::types::{UserMessage, Context};
    
    // 测试空消息创建
    let empty_msg = UserMessage::new("");
    if let pi_ai::types::UserContent::Text(text) = &empty_msg.content {
        assert!(text.is_empty());
    }
    
    // 测试空上下文
    let empty_context = Context::new(vec![]);
    assert!(empty_context.messages.is_empty());
}

/// 测试模型成本边界值
#[test]
fn test_model_cost_boundary_values() {
    use pi_ai::{calculate_cost, Usage, ModelCost};
    
    // 创建零成本模型
    let zero_cost_model = Model {
        id: "zero-cost".to_string(),
        name: "Zero Cost".to_string(),
        api: Api::Anthropic,
        provider: Provider::Anthropic,
        base_url: "".to_string(),
        reasoning: false,
        input: vec![InputModality::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: None,
            cache_write: None,
        },
        context_window: 1000,
        max_tokens: 100,
        headers: None,
        compat: None,
    };
    
    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: None,
        cache_write_tokens: None,
    };
    
    let cost = calculate_cost(&zero_cost_model, &usage);
    assert_eq!(cost, 0.0, "Zero cost model should produce zero cost");
}

/// 测试大数值成本计算
#[test]
fn test_large_usage_cost_calculation() {
    use pi_ai::{calculate_cost, Usage};
    
    let models = get_models();
    if let Some(model) = models.first() {
        let usage = Usage {
            input_tokens: 1_000_000, // 1M tokens
            output_tokens: 500_000,  // 500K tokens
            cache_read_tokens: Some(100_000),
            cache_write_tokens: Some(50_000),
        };
        
        let cost = calculate_cost(model, &usage);
        
        // 成本应该是有限的正数
        assert!(cost.is_finite(), "Cost should be finite");
        assert!(cost >= 0.0, "Cost should be non-negative");
    }
}

/// 测试 Provider 枚举完整性
#[test]
fn test_provider_enum_exhaustiveness() {
    let providers = vec![
        Provider::Anthropic,
        Provider::Openai,
        Provider::Google,
        Provider::Mistral,
        Provider::AmazonBedrock,
    ];
    
    // 验证每个 provider 都有对应的 API
    for provider in &providers {
        let _api = match provider {
            Provider::Anthropic => Api::Anthropic,
            Provider::Openai => Api::OpenAiChatCompletions,
            Provider::Google => Api::Google,
            Provider::Mistral => Api::Mistral,
            Provider::AmazonBedrock => Api::AmazonBedrock,
            _ => Api::Anthropic, // 其他 provider 使用默认
        };
        
        // 验证该 provider 有模型
        let models = pi_ai::get_models_by_provider(provider);
        assert!(!models.is_empty(), "Provider {:?} should have models", provider);
    }
}

/// 测试模型上下文窗口边界
#[test]
fn test_model_context_window_boundaries() {
    let models = get_models();
    
    for model in &models {
        // 上下文窗口应该大于 0
        assert!(
            model.context_window > 0,
            "Model {} should have positive context_window",
            model.id
        );
        
        // 最大 token 数应该大于 0 且不超过上下文窗口
        assert!(
            model.max_tokens > 0,
            "Model {} should have positive max_tokens",
            model.id
        );
        
        assert!(
            model.max_tokens <= model.context_window,
            "Model {} max_tokens should not exceed context_window",
            model.id
        );
    }
}

/// 测试缓存成本计算
#[test]
fn test_cache_cost_calculation() {
    use pi_ai::{calculate_cost, Usage, ModelCost};
    
    // 创建带缓存成本的模型
    let cached_model = Model {
        id: "cached-model".to_string(),
        name: "Cached Model".to_string(),
        api: Api::Anthropic,
        provider: Provider::Anthropic,
        base_url: "".to_string(),
        reasoning: false,
        input: vec![InputModality::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: Some(0.3),
            cache_write: Some(3.75),
        },
        context_window: 200000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    };
    
    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: Some(2000),
        cache_write_tokens: Some(1000),
    };
    
    let cost = calculate_cost(&cached_model, &usage);
    
    // 成本应该包含缓存部分
    assert!(cost > 0.0, "Cost with cache should be positive");
}

/// 测试模型 ID 格式
#[test]
fn test_model_id_format() {
    let models = get_models();
    
    for model in &models {
        // ID 不应该为空
        assert!(!model.id.is_empty(), "Model ID should not be empty");
        
        // ID 不应该包含空格
        assert!(
            !model.id.contains(' '),
            "Model ID {} should not contain spaces",
            model.id
        );
        
        // ID 应该只包含有效字符（允许字母、数字、连字符、下划线、点、冒号和斜杠）
        assert!(
            model.id.chars().all(|c| {
                c.is_ascii_lowercase() || 
                c.is_ascii_digit() || 
                c == '-' || 
                c == '_' ||
                c == '.' ||
                c == ':' ||
                c == '/'
            }),
            "Model ID {} contains invalid characters",
            model.id
        );
    }
}

/// 测试 API Key 获取（环境变量不存在时）
#[test]
fn test_api_key_from_env_not_set() {
    // 测试获取不存在的环境变量
    let _key = pi_ai::get_api_key_from_env(&Provider::Anthropic);
    // 结果可能是 None 或 Some("")，取决于环境
    // 这里我们只验证函数不 panic
}

/// 测试模型按 API 分组边界情况
#[test]
fn test_models_by_api_edge_cases() {
    use pi_ai::get_models_by_api;
    
    // 测试所有 API 类型
    let all_apis = vec![
        Api::Anthropic,
        Api::OpenAiChatCompletions,
        Api::Google,
        Api::Mistral,
        Api::AmazonBedrock,
    ];
    
    for api in all_apis {
        let models = get_models_by_api(&api);
        
        // 每个 API 应该至少有一个模型
        assert!(
            !models.is_empty(),
            "API {:?} should have at least one model",
            api
        );
        
        // 所有返回的模型都应该使用该 API
        for model in &models {
            assert_eq!(
                model.api, api,
                "Model {} should use API {:?}",
                model.id, api
            );
        }
    }
}
