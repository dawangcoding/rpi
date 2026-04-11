use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// 技能参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub description: String,
    pub param_type: ParameterType,
    pub required: bool,
    pub default: Option<String>,
}

/// 参数类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Enum(Vec<String>),
}

/// 技能分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum SkillCategory {
    CodeReview,
    Refactoring,
    Documentation,
    Debugging,
    Performance,
    Testing,
    Security,
    Custom,
}

/// 技能定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt_template: String,
    pub parameters: Vec<SkillParameter>,
    pub category: SkillCategory,
    pub tags: Vec<String>,
    #[serde(default)]
    pub builtin: bool,
}

impl Skill {
    /// 使用参数值渲染提示词模板
    pub fn render_prompt(&self, params: &HashMap<String, String>) -> String {
        let mut result = self.prompt_template.clone();
        for (key, value) in params {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_parameter_serialization() {
        let param = SkillParameter {
            name: "language".to_string(),
            description: "Programming language".to_string(),
            param_type: ParameterType::String,
            required: true,
            default: None,
        };
        
        let json = serde_json::to_string(&param).unwrap();
        assert!(json.contains("\"name\":\"language\""));
        
        let deserialized: SkillParameter = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "language");
    }

    #[test]
    fn test_parameter_type_enum() {
        let enum_type = ParameterType::Enum(vec!["rust".to_string(), "python".to_string()]);
        let json = serde_json::to_string(&enum_type).unwrap();
        assert!(json.contains("enum"));
        
        let deserialized: ParameterType = serde_json::from_str(&json).unwrap();
        if let ParameterType::Enum(variants) = deserialized {
            assert_eq!(variants.len(), 2);
        } else {
            panic!("Expected Enum type");
        }
    }

    #[test]
    fn test_skill_category_kebab_case() {
        let category = SkillCategory::CodeReview;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"code-review\"");
        
        let deserialized: SkillCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SkillCategory::CodeReview);
    }

    #[test]
    fn test_skill_render_prompt() {
        let skill = Skill {
            id: "test".to_string(),
            name: "Test Skill".to_string(),
            description: "A test skill".to_string(),
            prompt_template: "Review this {language} code:\n{code}".to_string(),
            parameters: vec![],
            category: SkillCategory::CodeReview,
            tags: vec![],
            builtin: false,
        };
        
        let mut params = HashMap::new();
        params.insert("language".to_string(), "Rust".to_string());
        params.insert("code".to_string(), "fn main() {}".to_string());
        
        let result = skill.render_prompt(&params);
        assert_eq!(result, "Review this Rust code:\nfn main() {}");
    }

    #[test]
    fn test_skill_render_prompt_missing_param() {
        let skill = Skill {
            id: "test".to_string(),
            name: "Test Skill".to_string(),
            description: "A test skill".to_string(),
            prompt_template: "Review: {code} by {author}".to_string(),
            parameters: vec![],
            category: SkillCategory::CodeReview,
            tags: vec![],
            builtin: false,
        };
        
        let mut params = HashMap::new();
        params.insert("code".to_string(), "fn main() {}".to_string());
        // author not provided
        
        let result = skill.render_prompt(&params);
        // Missing params should remain as template
        assert_eq!(result, "Review: fn main() {} by {author}");
    }

    #[test]
    fn test_skill_serialization() {
        let skill = Skill {
            id: "code-review".to_string(),
            name: "Code Review".to_string(),
            description: "Review code quality".to_string(),
            prompt_template: "Review: {code}".to_string(),
            parameters: vec![SkillParameter {
                name: "code".to_string(),
                description: "Code to review".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            }],
            category: SkillCategory::CodeReview,
            tags: vec!["quality".to_string()],
            builtin: true,
        };
        
        let json = serde_json::to_string(&skill).unwrap();
        assert!(json.contains("\"builtin\":true"));
        
        let deserialized: Skill = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "code-review");
        assert!(deserialized.builtin);
    }
}
