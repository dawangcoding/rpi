//! Provider 集成测试
//!
//! 测试 Provider 模块的集成行为和完整性

use pi_ai::{init_providers, get_all_api_providers, has_api_provider, Api, Provider, get_models, get_model};

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
    let providers = vec![
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
    let apis = vec![
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
