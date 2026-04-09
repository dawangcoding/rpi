//! SSE (Server-Sent Events) 解析器
//!
//! 用于解析 LLM 流式响应的 SSE 格式数据

/// SSE 事件
#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    /// 事件类型
    pub event: Option<String>,
    /// 事件数据
    pub data: String,
    /// 事件 ID
    pub id: Option<String>,
}

impl SseEvent {
    /// 创建新的 SSE 事件
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            event: None,
            data: data.into(),
            id: None,
        }
    }
    
    /// 设置事件类型
    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }
    
    /// 设置事件 ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// SSE 行解析器（增量式）
///
/// 用于处理可能分块到达的 SSE 数据流
#[derive(Debug, Clone)]
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    /// 创建新的 SSE 解析器
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
    
    /// 输入原始数据，返回解析出的事件
    ///
    /// 可以多次调用 feed 方法，解析器会保持状态并处理跨块的数据
    pub fn feed(&mut self, chunk: &str) -> Vec<SseEvent> {
        self.buffer.push_str(chunk);
        self.parse_events()
    }
    
    /// 解析缓冲区中的事件
    fn parse_events(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();
        let mut current_event = SseEvent::new("");
        let mut data_lines: Vec<String> = Vec::new();
        
        // 按行分割缓冲区
        let mut lines: Vec<&str> = self.buffer.lines().collect();
        
        // 检查最后一行是否完整（以换行符结尾）
        let ends_with_newline = self.buffer.ends_with('\n');
        let incomplete_line = if !ends_with_newline && !lines.is_empty() {
            lines.pop()
        } else {
            None
        };
        
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            
            if line.is_empty() {
                // 空行表示事件结束
                if !data_lines.is_empty() {
                    current_event.data = data_lines.join("\n");
                    events.push(current_event);
                }
                // 重置状态
                current_event = SseEvent::new("");
                data_lines.clear();
            } else if let Some(stripped) = line.strip_prefix("data: ") {
                // data: 行
                data_lines.push(stripped.to_string());
            } else if let Some(stripped) = line.strip_prefix("event: ") {
                // event: 行
                current_event.event = Some(stripped.to_string());
            } else if let Some(stripped) = line.strip_prefix("id: ") {
                // id: 行
                current_event.id = Some(stripped.to_string());
            } else if line.starts_with(':') {
                // 注释行，忽略
            } else {
                // 其他行，可能是没有前缀的 data
                data_lines.push(line.to_string());
            }
            
            i += 1;
        }
        
        // 处理未完成的最后一行
        if let Some(incomplete) = incomplete_line {
            self.buffer = incomplete.to_string();
        } else {
            self.buffer.clear();
        }
        
        // 如果缓冲区以空行结尾，处理最后一个事件
        if ends_with_newline && !data_lines.is_empty() {
            current_event.data = data_lines.join("\n");
            events.push(current_event);
        }
        
        events
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
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析单个 SSE 数据行
///
/// 返回 (field_name, value) 或 None（如果是空行或注释）
pub fn parse_sse_line(line: &str) -> Option<(&str, &str)> {
    if line.is_empty() || line.starts_with(':') {
        return None;
    }
    
    if let Some(pos) = line.find(':') {
        let field = &line[..pos];
        let value = if line.len() > pos + 1 && line.as_bytes().get(pos + 1) == Some(&b' ') {
            &line[pos + 2..]
        } else {
            &line[pos + 1..]
        };
        Some((field, value))
    } else {
        // 没有冒号，整行作为值
        Some(("data", line))
    }
}

/// 将 JSON 数据解析为 SSE 事件流
///
/// 用于处理 OpenAI/Anthropic 等 API 的流式响应
pub fn parse_json_stream_events(data: &str) -> Vec<serde_json::Value> {
    data.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            if line.starts_with("data: ") {
                let json_str = &line[6..];
                if json_str == "[DONE]" {
                    return None;
                }
                serde_json::from_str(json_str).ok()
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sse_parser_basic() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: hello\n\n");
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event, None);
    }
    
    #[test]
    fn test_sse_parser_with_event() {
        let mut parser = SseParser::new();
        let events = parser.feed("event: message\ndata: hello world\n\n");
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, Some("message".to_string()));
        assert_eq!(events[0].data, "hello world");
    }
    
    #[test]
    fn test_sse_parser_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: line1\ndata: line2\ndata: line3\n\n");
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2\nline3");
    }
    
    #[test]
    fn test_sse_parser_multiple_events() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: event1\n\ndata: event2\n\n");
        
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "event1");
        assert_eq!(events[1].data, "event2");
    }
    
    #[test]
    fn test_sse_parser_chunked() {
        let mut parser = SseParser::new();
        
        let events1 = parser.feed("data: hel");
        assert_eq!(events1.len(), 0);
        
        let events2 = parser.feed("lo\n\ndata: wor");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello");
        
        let events3 = parser.feed("ld\n\n");
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0].data, "world");
    }
    
    #[test]
    fn test_sse_parser_with_id() {
        let mut parser = SseParser::new();
        let events = parser.feed("id: 123\nevent: update\ndata: content\n\n");
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("123".to_string()));
        assert_eq!(events[0].event, Some("update".to_string()));
        assert_eq!(events[0].data, "content");
    }
    
    #[test]
    fn test_parse_json_stream_events() {
        let data = r#"data: {"key": "value"}
data: [DONE]
data: {"another": "object"}"#;
        
        let events = parse_json_stream_events(data);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["key"], "value");
        assert_eq!(events[1]["another"], "object");
    }
}
