use std::collections::HashMap;
use std::collections::HashSet;
use anyhow::Result;
use super::types::{Skill, SkillCategory};

/// 技能注册表
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: HashMap::new() }
    }
    
    /// 注册一个技能
    pub fn register(&mut self, skill: Skill) -> Result<()> {
        if self.skills.contains_key(&skill.id) {
            anyhow::bail!("Skill '{}' already registered", skill.id);
        }
        self.skills.insert(skill.id.clone(), skill);
        Ok(())
    }
    
    /// 注销一个技能
    pub fn unregister(&mut self, id: &str) -> Option<Skill> {
        self.skills.remove(id)
    }
    
    /// 获取技能
    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id)
    }
    
    /// 获取所有技能
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }
    
    /// 按分类筛选
    pub fn list_by_category(&self, category: &SkillCategory) -> Vec<&Skill> {
        self.skills.values()
            .filter(|s| &s.category == category)
            .collect()
    }
    
    /// 搜索技能（名称或描述匹配）
    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let query_lower = query.to_lowercase();
        self.skills.values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                || s.description.to_lowercase().contains(&query_lower)
                || s.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
    
    /// 技能数量
    pub fn count(&self) -> usize {
        self.skills.len()
    }
    
    /// 获取所有分类
    pub fn categories(&self) -> Vec<SkillCategory> {
        let cats: HashSet<SkillCategory> = self.skills.values()
            .map(|s| s.category.clone())
            .collect();
        let mut result: Vec<_> = cats.into_iter().collect();
        result.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
        result
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{SkillParameter, ParameterType};

    fn create_test_skill(id: &str, name: &str, category: SkillCategory) -> Skill {
        Skill {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("Description for {}", name),
            prompt_template: "Template {code}".to_string(),
            parameters: vec![],
            category,
            tags: vec!["test".to_string()],
            builtin: false,
        }
    }

    #[test]
    fn test_new_registry_is_empty() {
        let registry = SkillRegistry::new();
        assert_eq!(registry.count(), 0);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_register_skill() {
        let mut registry = SkillRegistry::new();
        let skill = create_test_skill("test-skill", "Test Skill", SkillCategory::Custom);
        
        registry.register(skill).unwrap();
        assert_eq!(registry.count(), 1);
        assert!(registry.get("test-skill").is_some());
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mut registry = SkillRegistry::new();
        let skill1 = create_test_skill("test-skill", "Test 1", SkillCategory::Custom);
        let skill2 = create_test_skill("test-skill", "Test 2", SkillCategory::Custom);
        
        registry.register(skill1).unwrap();
        let result = registry.register(skill2);
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn test_unregister_skill() {
        let mut registry = SkillRegistry::new();
        let skill = create_test_skill("test-skill", "Test", SkillCategory::Custom);
        
        registry.register(skill).unwrap();
        assert_eq!(registry.count(), 1);
        
        let removed = registry.unregister("test-skill");
        assert!(removed.is_some());
        assert_eq!(registry.count(), 0);
        assert!(registry.get("test-skill").is_none());
    }

    #[test]
    fn test_unregister_nonexistent() {
        let mut registry = SkillRegistry::new();
        let result = registry.unregister("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_skill() {
        let mut registry = SkillRegistry::new();
        let skill = create_test_skill("my-skill", "My Skill", SkillCategory::CodeReview);
        
        registry.register(skill).unwrap();
        
        let retrieved = registry.get("my-skill").unwrap();
        assert_eq!(retrieved.name, "My Skill");
        assert_eq!(retrieved.category, SkillCategory::CodeReview);
    }

    #[test]
    fn test_list_skills() {
        let mut registry = SkillRegistry::new();
        registry.register(create_test_skill("s1", "Skill 1", SkillCategory::Custom)).unwrap();
        registry.register(create_test_skill("s2", "Skill 2", SkillCategory::Custom)).unwrap();
        registry.register(create_test_skill("s3", "Skill 3", SkillCategory::Custom)).unwrap();
        
        let list = registry.list();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_list_by_category() {
        let mut registry = SkillRegistry::new();
        registry.register(create_test_skill("s1", "Skill 1", SkillCategory::CodeReview)).unwrap();
        registry.register(create_test_skill("s2", "Skill 2", SkillCategory::Refactoring)).unwrap();
        registry.register(create_test_skill("s3", "Skill 3", SkillCategory::CodeReview)).unwrap();
        
        let code_review_skills = registry.list_by_category(&SkillCategory::CodeReview);
        assert_eq!(code_review_skills.len(), 2);
        
        let refactoring_skills = registry.list_by_category(&SkillCategory::Refactoring);
        assert_eq!(refactoring_skills.len(), 1);
        
        let testing_skills = registry.list_by_category(&SkillCategory::Testing);
        assert_eq!(testing_skills.len(), 0);
    }

    #[test]
    fn test_search_by_name() {
        let mut registry = SkillRegistry::new();
        registry.register(create_test_skill("code-review", "Code Review", SkillCategory::CodeReview)).unwrap();
        registry.register(create_test_skill("refactor", "Refactoring Helper", SkillCategory::Refactoring)).unwrap();
        registry.register(create_test_skill("bug-fix", "Bug Fixer", SkillCategory::Debugging)).unwrap();
        
        let results = registry.search("code");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "code-review");
    }

    #[test]
    fn test_search_by_description() {
        let mut registry = SkillRegistry::new();
        let mut skill = create_test_skill("s1", "Skill", SkillCategory::Custom);
        skill.description = "Analyzes performance bottlenecks".to_string();
        registry.register(skill).unwrap();
        
        let results = registry.search("performance");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_tag() {
        let mut registry = SkillRegistry::new();
        let mut skill = create_test_skill("s1", "Skill", SkillCategory::Custom);
        skill.tags = vec!["security".to_string(), "audit".to_string()];
        registry.register(skill).unwrap();
        
        let results = registry.search("security");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut registry = SkillRegistry::new();
        registry.register(create_test_skill("code-review", "Code Review", SkillCategory::CodeReview)).unwrap();
        
        let results = registry.search("CODE");
        assert_eq!(results.len(), 1);
        
        let results = registry.search("Review");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_no_match() {
        let mut registry = SkillRegistry::new();
        registry.register(create_test_skill("s1", "Skill", SkillCategory::Custom)).unwrap();
        
        let results = registry.search("nonexistent");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_categories() {
        let mut registry = SkillRegistry::new();
        registry.register(create_test_skill("s1", "S1", SkillCategory::CodeReview)).unwrap();
        registry.register(create_test_skill("s2", "S2", SkillCategory::Refactoring)).unwrap();
        registry.register(create_test_skill("s3", "S3", SkillCategory::CodeReview)).unwrap();
        
        let categories = registry.categories();
        assert_eq!(categories.len(), 2);
        assert!(categories.contains(&SkillCategory::CodeReview));
        assert!(categories.contains(&SkillCategory::Refactoring));
    }

    #[test]
    fn test_default() {
        let registry = SkillRegistry::default();
        assert_eq!(registry.count(), 0);
    }
}
