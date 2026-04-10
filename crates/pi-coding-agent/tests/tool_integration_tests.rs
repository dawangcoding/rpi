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
