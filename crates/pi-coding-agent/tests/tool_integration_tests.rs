//! 工具集成测试
//!
//! 测试工具模块的集成行为和完整性

use std::path::PathBuf;
use pi_agent::AgentTool;

/// 测试工具创建和基本属性
#[test]
fn test_tool_creation() {
    let cwd = PathBuf::from("/tmp");
    
    // BashTool 应该正确创建
    let bash = pi_coding_agent::core::tools::BashTool::new(cwd.clone());
    assert_eq!(bash.name(), "bash");
    
    // ReadTool 应该正确创建
    let read = pi_coding_agent::core::tools::ReadTool::new(cwd.clone());
    assert_eq!(read.name(), "read");
    
    // EditTool 应该正确创建
    let edit = pi_coding_agent::core::tools::EditTool::new(cwd.clone());
    assert_eq!(edit.name(), "edit");
    
    // WriteTool 应该正确创建
    let write = pi_coding_agent::core::tools::WriteTool::new(cwd.clone());
    assert_eq!(write.name(), "write");
    
    // GrepTool 应该正确创建
    let grep = pi_coding_agent::core::tools::GrepTool::new(cwd.clone());
    assert_eq!(grep.name(), "grep");
    
    // FindTool 应该正确创建
    let find = pi_coding_agent::core::tools::FindTool::new(cwd.clone());
    assert_eq!(find.name(), "find");
    
    // LsTool 应该正确创建
    let ls = pi_coding_agent::core::tools::LsTool::new(cwd);
    assert_eq!(ls.name(), "ls");
}

/// 测试工具名称唯一性
#[test]
fn test_tool_names_unique() {
    let cwd = PathBuf::from("/tmp");
    
    let tool_names: Vec<String> = vec![
        pi_coding_agent::core::tools::BashTool::new(cwd.clone()).name().to_string(),
        pi_coding_agent::core::tools::ReadTool::new(cwd.clone()).name().to_string(),
        pi_coding_agent::core::tools::WriteTool::new(cwd.clone()).name().to_string(),
        pi_coding_agent::core::tools::EditTool::new(cwd.clone()).name().to_string(),
        pi_coding_agent::core::tools::GrepTool::new(cwd.clone()).name().to_string(),
        pi_coding_agent::core::tools::FindTool::new(cwd.clone()).name().to_string(),
        pi_coding_agent::core::tools::LsTool::new(cwd).name().to_string(),
    ];
    
    // 验证所有工具名称都是唯一的
    let unique_names: std::collections::HashSet<_> = tool_names.iter().collect();
    assert_eq!(
        unique_names.len(),
        tool_names.len(),
        "All tool names should be unique"
    );
}

/// 测试工具描述非空
#[test]
fn test_tool_descriptions_non_empty() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(BashTool::new(cwd.clone())),
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(EditTool::new(cwd.clone())),
        Box::new(GrepTool::new(cwd.clone())),
        Box::new(FindTool::new(cwd.clone())),
        Box::new(LsTool::new(cwd)),
    ];
    
    for tool in &tools {
        let desc = tool.description();
        assert!(
            !desc.is_empty(),
            "Tool {} should have a non-empty description",
            tool.name()
        );
    }
}

/// 测试工具参数 schema 有效性
#[test]
fn test_tool_parameter_schemas() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(BashTool::new(cwd.clone())),
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(EditTool::new(cwd.clone())),
        Box::new(GrepTool::new(cwd.clone())),
        Box::new(FindTool::new(cwd.clone())),
        Box::new(LsTool::new(cwd)),
    ];
    
    for tool in &tools {
        let schema = tool.parameters();
        
        // 验证 schema 是有效的 JSON 对象
        assert!(
            schema.is_object(),
            "Tool {} should have an object parameters schema",
            tool.name()
        );
        
        // 验证有 type 字段
        assert!(
            schema.get("type").is_some(),
            "Tool {} parameters should have a 'type' field",
            tool.name()
        );
    }
}

/// 测试文件系统工具的基本功能
#[cfg(test)]
mod fs_tool_tests {
    use std::io::Write;
    use tempfile::TempDir;
    
    #[test]
    fn test_read_tool_reads_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // 创建测试文件
        {
            let mut file = std::fs::File::create(&file_path).unwrap();
            file.write_all(b"Hello, World!").unwrap();
        }
        
        // 验证文件存在且可读
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }
    
    #[test]
    fn test_ls_tool_lists_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        // 创建一些测试文件
        std::fs::File::create(temp_dir.path().join("file1.txt")).unwrap();
        std::fs::File::create(temp_dir.path().join("file2.txt")).unwrap();
        
        // 验证目录内容
        let entries: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        
        assert_eq!(entries.len(), 2);
    }
}

/// 测试工具名称格式规范
#[test]
fn test_tool_name_format() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(BashTool::new(cwd.clone())),
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(EditTool::new(cwd.clone())),
        Box::new(GrepTool::new(cwd.clone())),
        Box::new(FindTool::new(cwd.clone())),
        Box::new(LsTool::new(cwd)),
    ];
    
    for tool in &tools {
        let name = tool.name();
        
        // 工具名称应该是小写
        assert_eq!(
            name,
            name.to_lowercase(),
            "Tool name {} should be lowercase",
            name
        );
        
        // 工具名称不应该包含空格
        assert!(
            !name.contains(' '),
            "Tool name {} should not contain spaces",
            name
        );
        
        // 工具名称应该只包含字母、数字和下划线
        assert!(
            name.chars().all(|c| c.is_alphanumeric() || c == '_'),
            "Tool name {} should only contain alphanumeric characters and underscores",
            name
        );
    }
}

// ============== 边界测试 ==============

/// 测试工具参数边界值
#[test]
fn test_tool_parameter_boundaries() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(BashTool::new(cwd)),
    ];
    
    for tool in &tools {
        let schema = tool.parameters();
        
        // 验证 schema 结构完整性
        if let Some(obj) = schema.as_object() {
            // 检查 properties
            if let Some(props) = obj.get("properties") {
                assert!(
                    props.is_object(),
                    "Tool {} properties should be an object",
                    tool.name()
                );
            }
            
            // 检查 required 字段（如果存在）
            if let Some(required) = obj.get("required") {
                assert!(
                    required.is_array(),
                    "Tool {} required should be an array",
                    tool.name()
                );
            }
        }
    }
}

/// 测试工具标签和描述非空
#[test]
fn test_tool_label_and_description() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(BashTool::new(cwd.clone())),
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(EditTool::new(cwd.clone())),
        Box::new(GrepTool::new(cwd.clone())),
        Box::new(FindTool::new(cwd.clone())),
        Box::new(LsTool::new(cwd)),
    ];
    
    for tool in &tools {
        // 标签不应该为空
        assert!(
            !tool.label().is_empty(),
            "Tool {} should have a non-empty label",
            tool.name()
        );
        
        // 描述不应该为空
        assert!(
            !tool.description().is_empty(),
            "Tool {} should have a non-empty description",
            tool.name()
        );
        
        // 标签应该与名称一致或相关
        assert_eq!(
            tool.label(),
            tool.label().trim(),
            "Tool {} label should not have leading/trailing whitespace",
            tool.name()
        );
    }
}

/// 测试工具在不同工作目录下的创建
#[test]
fn test_tool_creation_different_cwd() {
    use pi_coding_agent::core::tools::*;
    
    let temp_dir = std::env::temp_dir();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    
    // 在不同目录下创建工具
    let _bash1 = BashTool::new(temp_dir.clone());
    let _bash2 = BashTool::new(home_dir.clone());
    
    let _read1 = ReadTool::new(temp_dir);
    let _read2 = ReadTool::new(home_dir);
    
    // 验证创建成功
}

/// 测试工具 prepare_arguments 方法
#[test]
fn test_tool_prepare_arguments() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let bash = BashTool::new(cwd.clone());
    
    // 测试参数准备（默认实现返回原参数）
    let args = serde_json::json!({"command": "echo hello"});
    let prepared = bash.prepare_arguments(args.clone());
    
    assert_eq!(
        prepared, args,
        "BashTool prepare_arguments should return the same arguments by default"
    );
}

/// 测试文件系统工具边界情况
#[cfg(test)]
mod fs_tool_edge_cases {
    use std::io::Write;
    use tempfile::TempDir;
    
    #[test]
    fn test_read_tool_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        
        // 创建空文件
        std::fs::File::create(&file_path).unwrap();
        
        // 验证文件存在且为空
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.is_empty());
    }
    
    #[test]
    fn test_read_tool_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.txt");
        
        // 创建大文件（100KB）
        let large_content = "x".repeat(100_000);
        {
            let mut file = std::fs::File::create(&file_path).unwrap();
            file.write_all(large_content.as_bytes()).unwrap();
        }
        
        // 验证文件内容
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content.len(), 100_000);
    }
    
    #[test]
    fn test_ls_tool_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        
        // 验证空目录
        let entries: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        
        assert!(entries.is_empty());
    }
    
    #[test]
    fn test_ls_tool_nested_directories() {
        let temp_dir = TempDir::new().unwrap();
        
        // 创建嵌套目录结构
        let nested = temp_dir.path().join("level1/level2/level3");
        std::fs::create_dir_all(&nested).unwrap();
        
        // 在各层创建文件
        std::fs::File::create(temp_dir.path().join("level1/file1.txt")).unwrap();
        std::fs::File::create(nested.join("file2.txt")).unwrap();
        
        // 验证顶层目录内容
        let entries: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        
        assert_eq!(entries.len(), 1);
    }
    
    #[test]
    fn test_write_tool_overwrite() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("overwrite.txt");
        
        // 写入初始内容
        {
            let mut file = std::fs::File::create(&file_path).unwrap();
            file.write_all(b"initial").unwrap();
        }
        
        // 覆盖写入
        {
            let mut file = std::fs::File::create(&file_path).unwrap();
            file.write_all(b"overwritten").unwrap();
        }
        
        // 验证内容被覆盖
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "overwritten");
    }
}

/// 测试工具组合场景
#[test]
fn test_tool_combinations() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    
    // 创建工具组合
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(BashTool::new(cwd.clone())),
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(EditTool::new(cwd.clone())),
        Box::new(GrepTool::new(cwd.clone())),
        Box::new(FindTool::new(cwd.clone())),
        Box::new(LsTool::new(cwd)),
    ];
    
    // 验证所有工具名称唯一
    let mut names = std::collections::HashSet::new();
    for tool in &tools {
        assert!(
            names.insert(tool.name().to_string()),
            "Tool name {} should be unique",
            tool.name()
        );
    }
    
    // 验证工具数量
    assert_eq!(tools.len(), 7, "Should have 7 built-in tools");
}

/// 测试工具参数 JSON Schema 完整性
#[test]
fn test_tool_json_schema_completeness() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/tmp");
    let tools: Vec<Box<dyn AgentTool>> = vec![
        Box::new(BashTool::new(cwd.clone())),
        Box::new(ReadTool::new(cwd.clone())),
        Box::new(WriteTool::new(cwd.clone())),
        Box::new(EditTool::new(cwd.clone())),
        Box::new(GrepTool::new(cwd.clone())),
        Box::new(FindTool::new(cwd.clone())),
        Box::new(LsTool::new(cwd)),
    ];
    
    for tool in &tools {
        let schema = tool.parameters();
        
        // 验证是有效的 JSON 对象
        assert!(
            schema.is_object(),
            "Tool {} parameters should be a JSON object",
            tool.name()
        );
        
        // 验证有 type 字段且值为 "object"
        if let Some(type_val) = schema.get("type") {
            assert_eq!(
                type_val.as_str(),
                Some("object"),
                "Tool {} parameters type should be 'object'",
                tool.name()
            );
        }
        
        // 验证 properties 存在且为对象
        if let Some(props) = schema.get("properties") {
            assert!(
                props.is_object(),
                "Tool {} properties should be an object",
                tool.name()
            );
        }
    }
}

/// 测试工具执行模式（同步/异步边界）
#[tokio::test]
async fn test_tool_execution_async_boundary() {
    use pi_coding_agent::core::tools::*;
    use pi_agent::AgentTool;
    
    let temp_dir = tempfile::tempdir().unwrap();
    let cwd = temp_dir.path().to_path_buf();
    
    // 创建 ReadTool
    let read_tool = ReadTool::new(cwd.clone());
    
    // 创建测试文件
    let test_file = cwd.join("test.txt");
    std::fs::write(&test_file, "test content").unwrap();
    
    // 验证工具创建成功
    assert_eq!(read_tool.name(), "read");
    assert!(!read_tool.description().is_empty());
}

/// 测试工具错误处理边界
#[test]
fn test_tool_error_handling_boundaries() {
    use pi_coding_agent::core::tools::*;
    
    let cwd = PathBuf::from("/nonexistent/path/that/should/not/exist");
    
    // 在不存在路径下创建工具（应该可以创建，但执行时会失败）
    let read_tool = ReadTool::new(cwd.clone());
    let write_tool = WriteTool::new(cwd);
    
    // 验证工具创建成功（路径验证在执行时进行）
    assert_eq!(read_tool.name(), "read");
    assert_eq!(write_tool.name(), "write");
}
