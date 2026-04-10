//! 文本组件
//! 提供纯文本显示和截断文本功能

use crate::tui::Component;
use crate::utils::{truncate_to_width_with_ellipsis, wrap_text_with_ansi};

/// 纯文本组件
pub struct Text {
    content: String,
    style: Option<String>,
    wrap: bool,
    needs_render: bool,
}

impl Text {
    /// 创建新的文本组件
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            style: None,
            wrap: true,
            needs_render: true,
        }
    }

    /// 创建带样式的文本组件
    pub fn styled(content: &str, style: &str) -> Self {
        Self {
            content: content.to_string(),
            style: Some(style.to_string()),
            wrap: true,
            needs_render: true,
        }
    }

    /// 设置内容
    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
        self.needs_render = true;
    }

    /// 获取内容
    pub fn content(&self) -> &str {
        &self.content
    }

    /// 设置是否自动换行
    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
        self.needs_render = true;
    }

    /// 设置样式
    pub fn set_style(&mut self, style: Option<String>) {
        self.style = style;
        self.needs_render = true;
    }

    /// 应用样式到文本
    fn apply_style(&self, text: &str) -> String {
        match &self.style {
            Some(style) => format!("{}{}\x1b[0m", style, text),
            None => text.to_string(),
        }
    }
}

impl Component for Text {
    fn render(&self, width: u16) -> Vec<String> {
        if self.content.is_empty() {
            return vec![String::new()];
        }

        let content = self.content.replace('\t', "   ");
        
        if self.wrap {
            let lines = wrap_text_with_ansi(&content, width as usize);
            lines.into_iter()
                .map(|line| self.apply_style(&line))
                .collect()
        } else {
            // 不换行，按原样返回
            content.lines()
                .map(|line| self.apply_style(line))
                .collect()
        }
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
    }
}

/// 截断文本组件（单行，超出截断并加省略号）
pub struct TruncatedText {
    content: String,
    style: Option<String>,
    ellipsis: String,
    needs_render: bool,
}

impl TruncatedText {
    /// 创建新的截断文本组件
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
            style: None,
            ellipsis: "...".to_string(),
            needs_render: true,
        }
    }

    /// 创建带样式的截断文本组件
    pub fn styled(content: &str, style: &str) -> Self {
        Self {
            content: content.to_string(),
            style: Some(style.to_string()),
            ellipsis: "...".to_string(),
            needs_render: true,
        }
    }

    /// 设置内容
    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
        self.needs_render = true;
    }

    /// 获取内容
    pub fn content(&self) -> &str {
        &self.content
    }

    /// 设置省略号
    pub fn set_ellipsis(&mut self, ellipsis: &str) {
        self.ellipsis = ellipsis.to_string();
        self.needs_render = true;
    }

    /// 设置样式
    pub fn set_style(&mut self, style: Option<String>) {
        self.style = style;
        self.needs_render = true;
    }

    /// 应用样式到文本
    fn apply_style(&self, text: &str) -> String {
        match &self.style {
            Some(style) => format!("{}{}\x1b[0m", style, text),
            None => text.to_string(),
        }
    }
}

impl Component for TruncatedText {
    fn render(&self, width: u16) -> Vec<String> {
        // 只取第一行（去除换行符）
        let first_line = self.content.lines().next().unwrap_or("");
        
        // 截断到指定宽度
        let truncated = truncate_to_width_with_ellipsis(
            first_line, 
            width as usize, 
            &self.ellipsis
        );
        
        vec![self.apply_style(&truncated)]
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::visible_width;

    #[test]
    fn test_text_new() {
        let text = Text::new("Hello World");
        let lines = text.render(80);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Hello World");
    }

    #[test]
    fn test_text_wrapping() {
        let text = Text::new("Hello World this is a long text");
        let lines = text.render(10);
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_text_no_wrap() {
        let mut text = Text::new("Hello World this is a long text");
        text.set_wrap(false);
        let lines = text.render(10);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_text_styled() {
        let text = Text::styled("Hello", "\x1b[31m");
        let lines = text.render(80);
        assert!(lines[0].starts_with("\x1b[31m"));
        assert!(lines[0].ends_with("\x1b[0m"));
    }

    #[test]
    fn test_text_multiline() {
        let text = Text::new("Line 1\nLine 2\nLine 3");
        let lines = text.render(80);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_truncated_text() {
        let text = TruncatedText::new("This is a very long text that should be truncated");
        let lines = text.render(20);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("..."));
        assert!(visible_width(&lines[0]) <= 20);
    }

    #[test]
    fn test_truncated_text_short() {
        let text = TruncatedText::new("Short");
        let lines = text.render(80);
        assert_eq!(lines[0].trim(), "Short");
    }

    #[test]
    fn test_truncated_text_multiline() {
        // 截断文本组件应该只显示第一行
        let text = TruncatedText::new("Line 1\nLine 2\nLine 3");
        let lines = text.render(80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Line 1"));
        assert!(!lines[0].contains("Line 2"));
    }

    #[test]
    fn test_truncated_text_custom_ellipsis() {
        let mut text = TruncatedText::new("This is a very long text");
        text.set_ellipsis("..");
        let lines = text.render(15);
        assert!(lines[0].contains(".."));
    }
}
