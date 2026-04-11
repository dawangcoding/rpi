use super::types::*;

/// 加载所有内置技能
pub fn builtin_skills() -> Vec<Skill> {
    vec![
        code_review_skill(),
        refactoring_skill(),
        doc_generation_skill(),
        bug_analysis_skill(),
        performance_optimization_skill(),
    ]
}

/// 代码审查技能
fn code_review_skill() -> Skill {
    Skill {
        id: "code-review".to_string(),
        name: "代码审查".to_string(),
        description: "分析代码质量、可维护性、安全性，提供改进建议".to_string(),
        prompt_template: r#"请对以下代码进行全面的代码审查：

```
{code}
```

请从以下几个方面进行分析：

1. **代码质量**：命名规范、代码结构、可读性
2. **潜在问题**：边界条件、错误处理、资源泄漏
3. **安全性**：输入验证、敏感信息处理、注入风险
4. **可维护性**：代码复杂度、重复代码、模块化程度
5. **改进建议**：具体的优化和重构建议

请提供详细的审查报告和改进建议。"#.to_string(),
        parameters: vec![
            SkillParameter {
                name: "code".to_string(),
                description: "需要审查的代码片段".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
        ],
        category: SkillCategory::CodeReview,
        tags: vec!["review".to_string(), "quality".to_string(), "security".to_string()],
        builtin: true,
    }
}

/// 重构建议技能
fn refactoring_skill() -> Skill {
    Skill {
        id: "refactoring".to_string(),
        name: "重构建议".to_string(),
        description: "分析代码结构，提出改进方案和重构建议".to_string(),
        prompt_template: r#"请分析以下代码并提供重构建议：

```
{code}
```

重构目标：{goal}

请提供以下内容：

1. **当前问题分析**
   - 代码异味识别
   - 设计模式缺失
   - 架构问题

2. **重构方案**
   - 具体的重构步骤
   - 应用到的设计模式
   - 重构前后对比示例

3. **实施建议**
   - 重构优先级
   - 风险评估
   - 测试策略

请确保重构建议具有可操作性。"#.to_string(),
        parameters: vec![
            SkillParameter {
                name: "code".to_string(),
                description: "需要重构的代码片段".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            SkillParameter {
                name: "goal".to_string(),
                description: "重构目标（如：提高可读性、降低复杂度、增强扩展性）".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some("提高代码质量和可维护性".to_string()),
            },
        ],
        category: SkillCategory::Refactoring,
        tags: vec!["refactor".to_string(), "design-pattern".to_string(), "clean-code".to_string()],
        builtin: true,
    }
}

/// 文档生成技能
fn doc_generation_skill() -> Skill {
    Skill {
        id: "doc-generation".to_string(),
        name: "文档生成".to_string(),
        description: "为代码生成文档注释和 README 文档".to_string(),
        prompt_template: r#"请为以下代码生成文档：

```
{code}
```

文档类型：{doc_type}
目标受众：{audience}

请生成以下文档：

1. **代码注释**
   - 函数/方法文档注释
   - 复杂逻辑说明
   - 参数和返回值说明

2. **模块文档**
   - 模块概述
   - 使用示例
   - 注意事项

3. **API 文档**（如适用）
   - 端点描述
   - 请求/响应格式
   - 错误码说明

请确保文档清晰、准确、易于理解。"#.to_string(),
        parameters: vec![
            SkillParameter {
                name: "code".to_string(),
                description: "需要生成文档的代码片段".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            SkillParameter {
                name: "doc_type".to_string(),
                description: "文档类型".to_string(),
                param_type: ParameterType::Enum(vec![
                    "inline-comments".to_string(),
                    "module-docs".to_string(),
                    "readme".to_string(),
                    "api-docs".to_string(),
                ]),
                required: false,
                default: Some("inline-comments".to_string()),
            },
            SkillParameter {
                name: "audience".to_string(),
                description: "目标受众".to_string(),
                param_type: ParameterType::Enum(vec![
                    "developers".to_string(),
                    "users".to_string(),
                    "maintainers".to_string(),
                ]),
                required: false,
                default: Some("developers".to_string()),
            },
        ],
        category: SkillCategory::Documentation,
        tags: vec!["documentation".to_string(), "comments".to_string(), "readme".to_string()],
        builtin: true,
    }
}

/// Bug 分析技能
fn bug_analysis_skill() -> Skill {
    Skill {
        id: "bug-analysis".to_string(),
        name: "Bug 分析".to_string(),
        description: "系统分析潜在缺陷、边界条件和错误处理问题".to_string(),
        prompt_template: r#"请分析以下代码中的潜在 Bug：

```
{code}
```

问题描述：{problem}

请从以下几个方面进行分析：

1. **边界条件分析**
   - 空值/None 处理
   - 空集合处理
   - 数值溢出/下溢
   - 索引越界

2. **错误处理审查**
   - 错误是否被正确传播
   - 错误是否被正确恢复
   - 错误信息是否清晰

3. **并发问题**（如适用）
   - 竞态条件
   - 死锁风险
   - 数据竞争

4. **资源管理**
   - 内存泄漏
   - 文件句柄泄漏
   - 连接泄漏

5. **逻辑错误**
   - 条件判断错误
   - 循环终止条件
   - 状态转换错误

请提供详细的问题列表和修复建议。"#.to_string(),
        parameters: vec![
            SkillParameter {
                name: "code".to_string(),
                description: "需要分析的代码片段".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            SkillParameter {
                name: "problem".to_string(),
                description: "已知的问题描述或错误信息".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some("未知问题，需要全面分析".to_string()),
            },
        ],
        category: SkillCategory::Debugging,
        tags: vec!["bug".to_string(), "debugging".to_string(), "error-handling".to_string()],
        builtin: true,
    }
}

/// 性能优化技能
fn performance_optimization_skill() -> Skill {
    Skill {
        id: "performance-optimization".to_string(),
        name: "性能优化".to_string(),
        description: "分析性能瓶颈并提出优化建议".to_string(),
        prompt_template: r#"请分析以下代码的性能并提出优化建议：

```
{code}
```

性能问题：{issue}
性能目标：{target}

请进行以下分析：

1. **性能瓶颈识别**
   - 算法复杂度分析
   - I/O 操作热点
   - 内存分配模式
   - 循环优化机会

2. **优化建议**
   - 算法优化
   - 数据结构选择
   - 缓存策略
   - 并行化机会

3. **代码级优化**
   - 减少不必要的计算
   - 避免重复操作
   - 内联优化
   - 内存预分配

4. **权衡分析**
   - 时间 vs 空间权衡
   - 可读性 vs 性能权衡
   - 优化收益评估

请提供具体的优化代码示例。"#.to_string(),
        parameters: vec![
            SkillParameter {
                name: "code".to_string(),
                description: "需要优化的代码片段".to_string(),
                param_type: ParameterType::String,
                required: true,
                default: None,
            },
            SkillParameter {
                name: "issue".to_string(),
                description: "已知的性能问题".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some("需要全面性能分析".to_string()),
            },
            SkillParameter {
                name: "target".to_string(),
                description: "性能优化目标".to_string(),
                param_type: ParameterType::String,
                required: false,
                default: Some("提高执行效率，降低资源消耗".to_string()),
            },
        ],
        category: SkillCategory::Performance,
        tags: vec!["performance".to_string(), "optimization".to_string(), "profiling".to_string()],
        builtin: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_count() {
        let skills = builtin_skills();
        assert_eq!(skills.len(), 5);
    }

    #[test]
    fn test_all_builtin_skills_marked_as_builtin() {
        let skills = builtin_skills();
        for skill in &skills {
            assert!(skill.builtin, "Skill '{}' should be marked as builtin", skill.id);
        }
    }

    #[test]
    fn test_skill_ids_are_unique() {
        let skills = builtin_skills();
        let ids: std::collections::HashSet<_> = skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), skills.len(), "All skill IDs should be unique");
    }

    #[test]
    fn test_code_review_skill() {
        let skill = code_review_skill();
        assert_eq!(skill.id, "code-review");
        assert_eq!(skill.category, SkillCategory::CodeReview);
        assert!(!skill.parameters.is_empty());
        assert!(skill.prompt_template.contains("{code}"));
    }

    #[test]
    fn test_refactoring_skill() {
        let skill = refactoring_skill();
        assert_eq!(skill.id, "refactoring");
        assert_eq!(skill.category, SkillCategory::Refactoring);
        assert!(skill.parameters.iter().any(|p| p.name == "goal"));
    }

    #[test]
    fn test_doc_generation_skill() {
        let skill = doc_generation_skill();
        assert_eq!(skill.id, "doc-generation");
        assert_eq!(skill.category, SkillCategory::Documentation);
        
        // 检查 doc_type 参数是枚举类型
        let doc_type_param = skill.parameters.iter()
            .find(|p| p.name == "doc_type")
            .expect("doc_type parameter should exist");
        if let ParameterType::Enum(variants) = &doc_type_param.param_type {
            assert!(variants.contains(&"readme".to_string()));
        } else {
            panic!("doc_type should be Enum type");
        }
    }

    #[test]
    fn test_bug_analysis_skill() {
        let skill = bug_analysis_skill();
        assert_eq!(skill.id, "bug-analysis");
        assert_eq!(skill.category, SkillCategory::Debugging);
        assert!(!skill.tags.is_empty());
    }

    #[test]
    fn test_performance_optimization_skill() {
        let skill = performance_optimization_skill();
        assert_eq!(skill.id, "performance-optimization");
        assert_eq!(skill.category, SkillCategory::Performance);
        assert!(skill.parameters.len() >= 2); // code and at least one optional
    }

    #[test]
    fn test_skill_render_prompt() {
        let skill = code_review_skill();
        let mut params = std::collections::HashMap::new();
        params.insert("code".to_string(), "fn main() {}".to_string());
        
        let rendered = skill.render_prompt(&params);
        assert!(rendered.contains("fn main() {}"));
        assert!(!rendered.contains("{code}"));
    }
}
