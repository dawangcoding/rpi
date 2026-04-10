//! 增量 JSON 解析器
//!
//! 用于流式解析可能不完整的 JSON 数据

/// 尝试解析可能不完整的 JSON 字符串
///
/// 用于流式解析工具调用的参数。如果解析失败，返回 None。
///
/// # Examples
///
/// ```
/// use pi_ai::utils::json_parse::parse_partial_json;
///
/// // 完整 JSON
/// let result = parse_partial_json(r#"{"key": "value"}"#);
/// assert!(result.is_some());
///
/// // 不完整 JSON（缺少闭合括号）
/// let result = parse_partial_json(r#"{"key": "val""#);
/// // 可能返回部分解析结果
/// ```
pub fn parse_partial_json(input: &str) -> Option<serde_json::Value> {
    if input.trim().is_empty() {
        return None;
    }
    
    // 首先尝试标准解析（最快）
    if let Ok(value) = serde_json::from_str(input) {
        return Some(value);
    }
    
    // 尝试修复常见的 JSON 不完整问题
    let fixed = try_fix_json(input);
    
    // 再次尝试解析
    if let Ok(value) = serde_json::from_str(&fixed) {
        return Some(value);
    }
    
    // 尝试提取有效的 JSON 对象
    extract_valid_json(input)
}

/// 尝试修复不完整的 JSON
fn try_fix_json(input: &str) -> String {
    let trimmed_input = input.trim();
    let mut result = trimmed_input.to_string();
    
    // 统计括号
    let open_braces = result.chars().filter(|&c| c == '{').count();
    let close_braces = result.chars().filter(|&c| c == '}').count();
    let open_brackets = result.chars().filter(|&c| c == '[').count();
    let close_brackets = result.chars().filter(|&c| c == ']').count();
    let open_quotes = result.chars().filter(|&c| c == '"').count();
    
    // 补全字符串引号（如果数量为奇数）
    if open_quotes % 2 == 1 {
        result.push('"');
    }
    
    // 补全花括号
    for _ in 0..(open_braces.saturating_sub(close_braces)) {
        result.push('}');
    }
    
    // 补全方括号
    for _ in 0..(open_brackets.saturating_sub(close_brackets)) {
        result.push(']');
    }
    
    // 处理对象内部的尾部逗号（如 {"a": 1, "b": 2,}）
    // 使用正则式思路：找到所有 ,} 和 ,] 并替换为 } 和 ]
    result = result.replace(",}", "}").replace(",]", "]");
    
    result
}

/// 尝试从字符串中提取有效的 JSON 对象
fn extract_valid_json(input: &str) -> Option<serde_json::Value> {
    // 尝试找到完整的 JSON 对象
    let trimmed = input.trim();
    
    // 如果是对象，尝试找到匹配的闭合括号
    if trimmed.starts_with('{') {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;
        
        for (i, c) in trimmed.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }
            
            if c == '\\' && in_string {
                escape_next = true;
                continue;
            }
            
            if c == '"' {
                in_string = !in_string;
                continue;
            }
            
            if !in_string {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            // 找到了完整的对象
                            if let Ok(value) = serde_json::from_str(&trimmed[..=i]) {
                                return Some(value);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    // 如果是数组，尝试找到匹配的闭合括号
    if trimmed.starts_with('[') {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;
        
        for (i, c) in trimmed.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }
            
            if c == '\\' && in_string {
                escape_next = true;
                continue;
            }
            
            if c == '"' {
                in_string = !in_string;
                continue;
            }
            
            if !in_string {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            if let Ok(value) = serde_json::from_str(&trimmed[..=i]) {
                                return Some(value);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    
    None
}

/// 增量 JSON 追踪器
///
/// 用于累积接收到的 JSON 片段，并在可能时解析
#[derive(Debug, Clone)]
pub struct IncrementalJsonParser {
    buffer: String,
}

impl IncrementalJsonParser {
    /// 创建新的增量 JSON 解析器
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
    
    /// 追加新的数据片段
    pub fn append(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }
    
    /// 尝试解析当前缓冲区中的 JSON
    ///
    /// 如果解析成功，返回解析后的值；否则返回 None
    pub fn try_parse(&self) -> Option<serde_json::Value> {
        parse_partial_json(&self.buffer)
    }
    
    /// 获取当前缓冲区内容
    pub fn buffer(&self) -> &str {
        &self.buffer
    }
    
    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
    
    /// 检查缓冲区是否为空
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
    
    /// 获取缓冲区长度
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    
    /// 尝试解析并清空缓冲区
    ///
    /// 如果解析成功，返回解析后的值并清空缓冲区；
    /// 如果解析失败，返回 None 并保留缓冲区内容
    pub fn try_parse_and_clear(&mut self) -> Option<serde_json::Value> {
        if let Some(value) = parse_partial_json(&self.buffer) {
            self.buffer.clear();
            Some(value)
        } else {
            None
        }
    }
}

impl Default for IncrementalJsonParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 流式 JSON 解析器
///
/// 用于处理可能跨多个数据块到达的 JSON 数据
#[derive(Debug, Clone)]
pub struct StreamingJsonParser {
    buffer: String,
}

impl StreamingJsonParser {
    /// 创建新的流式 JSON 解析器
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
    
    /// 处理新的数据块，返回解析出的所有完整 JSON 对象
    pub fn feed(&mut self, chunk: &str) -> Vec<serde_json::Value> {
        self.buffer.push_str(chunk);
        let mut results = Vec::new();
        
        // 尝试解析缓冲区中的完整 JSON 对象
        // 注意：这里使用严格的解析，不尝试修复不完整的 JSON
        loop {
            // 尝试找到完整的 JSON 对象
            if let Some((value, consumed)) = Self::try_extract_json(&self.buffer) {
                results.push(value);
                // 移除已解析的部分
                self.buffer.drain(..consumed);
            } else {
                break;
            }
        }
        
        results
    }
    
    /// 尝试从缓冲区开头提取一个完整的 JSON 对象
    /// 返回解析后的值和消耗的字符数
    fn try_extract_json(buffer: &str) -> Option<(serde_json::Value, usize)> {
        let trimmed = buffer.trim_start();
        let leading_ws = buffer.len() - trimmed.len();
        
        // 尝试不同长度的前缀
        for end in 1..=trimmed.len() {
            let candidate = &trimmed[..end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) {
                // 成功解析，返回结果和消耗的总字符数
                return Some((value, leading_ws + end));
            }
        }
        
        None
    }
    
    /// 获取当前缓冲区
    pub fn buffer(&self) -> &str {
        &self.buffer
    }
    
    /// 清空缓冲区
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for StreamingJsonParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_partial_json_complete() {
        let input = r#"{"key": "value", "number": 42}"#;
        let result = parse_partial_json(input);
        
        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value["key"], "value");
        assert_eq!(value["number"], 42);
    }
    
    #[test]
    fn test_parse_partial_json_incomplete() {
        // 缺少闭合括号
        let input = r#"{"key": "value", "number": 42"#;
        let result = parse_partial_json(input);
        
        // 应该能够修复并解析
        assert!(result.is_some());
    }
    
    #[test]
    fn test_parse_partial_json_nested() {
        let input = r#"{"outer": {"inner": "value"}}"#;
        let result = parse_partial_json(input);
        
        assert!(result.is_some());
        let value = result.unwrap();
        assert_eq!(value["outer"]["inner"], "value");
    }
    
    #[test]
    fn test_incremental_parser() {
        let mut parser = IncrementalJsonParser::new();
        
        parser.append(r#"{"key": "#);
        assert!(parser.try_parse().is_none());
        
        parser.append(r#""value"}"#);
        let result = parser.try_parse();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["key"], "value");
    }
    
    #[test]
    fn test_streaming_parser() {
        let mut parser = StreamingJsonParser::new();
        
        // 第一个对象
        let results1 = parser.feed(r#"{"a": 1}"#);
        assert_eq!(results1.len(), 1);
        assert_eq!(results1[0]["a"], 1);
        
        // 第二个对象（分块到达）
        let results2 = parser.feed(r#"{"b": 2"#);
        assert_eq!(results2.len(), 0); // 不完整
        
        let results3 = parser.feed(r#"}"#);
        assert_eq!(results3.len(), 1);
        assert_eq!(results3[0]["b"], 2);
    }
    
    #[test]
    fn test_empty_input() {
        assert!(parse_partial_json("").is_none());
        assert!(parse_partial_json("   ").is_none());
    }
    
    #[test]
    fn test_trailing_comma() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let result = parse_partial_json(input);
        
        // 应该能够处理尾部逗号
        assert!(result.is_some());
    }
}
