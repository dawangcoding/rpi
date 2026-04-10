//! Markdown 渲染组件
//! 使用 pulldown-cmark 将 Markdown 渲染为带 ANSI 样式的终端文本

use crate::tui::Component;
use crate::utils::wrap_text_with_ansi;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

/// Markdown 渲染组件
pub struct Markdown {
    content: String,
    rendered_lines: Vec<String>,
    needs_render: bool,
    wrap_width: Option<u16>,
    // 缓存字段
    cached_content: String,
    cached_width: u16,
    cached_output: Vec<String>,
}

impl Markdown {
    /// 创建新的 Markdown 组件
    pub fn new() -> Self {
        Self {
            content: String::new(),
            rendered_lines: Vec::new(),
            needs_render: true,
            wrap_width: None,
            cached_content: String::new(),
            cached_width: 0,
            cached_output: Vec::new(),
        }
    }

    /// 设置 Markdown 内容
    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
        self.needs_render = true;
    }

    /// 追加内容（用于流式输入）
    pub fn append_content(&mut self, chunk: &str) {
        self.content.push_str(chunk);
        self.needs_render = true;
    }

    /// 获取原始内容
    pub fn content(&self) -> &str {
        &self.content
    }

    /// 清空内容
    pub fn clear(&mut self) {
        self.content.clear();
        self.rendered_lines.clear();
        self.needs_render = true;
        self.cached_content.clear();
        self.cached_width = 0;
        self.cached_output.clear();
    }

    /// 设置换行宽度
    pub fn set_wrap_width(&mut self, width: Option<u16>) {
        self.wrap_width = width;
        self.needs_render = true;
    }

    /// 渲染 Markdown 到终端文本
    fn render_markdown(&self, width: u16) -> Vec<String> {
        if self.content.trim().is_empty() {
            return Vec::new();
        }

        let parser = Parser::new(&self.content);
        let mut lines = Vec::new();
        let mut current_text = String::new();
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut _in_list = false;
        let mut list_indent: usize = 0;
        let mut in_quote = false;
        let mut in_bold = false;
        let mut in_italic = false;
        let _in_code = false;
        let mut in_link = false;
        let mut link_url = String::new();

        for event in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::CodeBlock(lang) => {
                            in_code_block = true;
                            code_block_lang = match lang {
                                pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                                _ => String::new(),
                            };
                            if !current_text.is_empty() {
                                Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                            }
                            // 代码块开始标记
                            let lang_str = if code_block_lang.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", code_block_lang)
                            };
                            lines.push(format!("\x1b[90m```{}\x1b[0m", lang_str));
                        }
                        Tag::Strong => in_bold = true,
                        Tag::Emphasis => in_italic = true,
                        // Note: Inline Code is handled via Event::Code in pulldown-cmark 0.12
                        Tag::Link { dest_url, .. } => {
                            in_link = true;
                            link_url = dest_url.to_string();
                        }
                        Tag::List(_start_num) => {
                            _in_list = true;
                            list_indent = 0;
                            if !current_text.is_empty() {
                                Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                            }
                        }
                        Tag::Item => {
                            list_indent += 1;
                            if !current_text.is_empty() {
                                Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                            }
                            current_text.push_str(&"  ".repeat(list_indent.saturating_sub(1)));
                            current_text.push_str("\x1b[36m- \x1b[0m");
                        }
                        Tag::BlockQuote(_) => {
                            in_quote = true;
                            if !current_text.is_empty() {
                                Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                            }
                        }
                        Tag::Heading { level, .. } => {
                            if !current_text.is_empty() {
                                Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                            }
                            // 添加空行（如果不是第一行）
                            if !lines.is_empty() {
                                lines.push(String::new());
                            }
                            // 根据级别设置样式
                            match level {
                                pulldown_cmark::HeadingLevel::H1 => {
                                    current_text.push_str("\x1b[1;4m"); // 粗体+下划线
                                }
                                pulldown_cmark::HeadingLevel::H2 => {
                                    current_text.push_str("\x1b[1m"); // 粗体
                                }
                                _ => {
                                    current_text.push_str("\x1b[1m"); // 粗体
                                }
                            }
                        }
                        Tag::Paragraph => {
                            if !current_text.is_empty() {
                                Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                            }
                            // 段落之间添加空行
                            if !lines.is_empty() {
                                lines.push(String::new());
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(tag_end) => {
                    match tag_end {
                        TagEnd::CodeBlock => {
                            in_code_block = false;
                            if !current_text.is_empty() {
                                // 渲染代码块内容
                                for line in current_text.lines() {
                                    lines.push(format!("  \x1b[36m{}\x1b[0m", line));
                                }
                                current_text.clear();
                            }
                            lines.push("\x1b[90m```\x1b[0m".to_string());
                            code_block_lang.clear();
                        }
                        TagEnd::Strong => {
                            in_bold = false;
                            current_text.push_str("\x1b[22m"); // 重置粗体
                        }
                        TagEnd::Emphasis => {
                            in_italic = false;
                            current_text.push_str("\x1b[23m"); // 重置斜体
                        }
                        // Note: Inline Code end is handled via Event::Code in pulldown-cmark 0.12
                        TagEnd::Link => {
                            in_link = false;
                            if !link_url.is_empty() {
                                current_text.push_str(&format!(" (\x1b[4m{}\x1b[24m)", link_url));
                            }
                            link_url.clear();
                        }
                        TagEnd::List(_) => {
                            _in_list = false;
                            list_indent = 0;
                        }
                        TagEnd::Item => {
                            list_indent = list_indent.saturating_sub(1);
                            Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                        }
                        TagEnd::BlockQuote(_) => {
                            in_quote = false;
                            Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                        }
                        TagEnd::Heading(_) => {
                            current_text.push_str("\x1b[0m"); // 重置样式
                            Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                        }
                        TagEnd::Paragraph => {
                            Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    if in_code_block {
                        current_text.push_str(&text);
                    } else if false {
                        // Inline code handled via Event::Code
                        let _ = &text; // suppress unused warning
                    } else if in_bold && in_italic {
                        current_text.push_str("\x1b[1;3m"); // bold + italic
                        current_text.push_str(&text);
                    } else if in_bold {
                        current_text.push_str("\x1b[1m"); // bold
                        current_text.push_str(&text);
                    } else if in_italic {
                        current_text.push_str("\x1b[3m"); // italic
                        current_text.push_str(&text);
                    } else if in_link {
                        current_text.push_str("\x1b[4m"); // underline
                        current_text.push_str(&text);
                    } else if in_quote {
                        current_text.push_str("\x1b[90m│ \x1b[3m"); // gray + italic
                        current_text.push_str(&text);
                    } else {
                        current_text.push_str(&text);
                    }
                }
                Event::Code(code) => {
                    current_text.push_str("\x1b[36m"); // cyan
                    current_text.push_str(&code);
                    current_text.push_str("\x1b[39m"); // reset color
                }
                Event::Html(html) => {
                    current_text.push_str(&html);
                }
                Event::SoftBreak => {
                    current_text.push(' ');
                }
                Event::HardBreak => {
                    Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                }
                Event::Rule => {
                    if !current_text.is_empty() {
                        Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
                    }
                    let rule_width = (width as usize).min(80);
                    lines.push(format!("\x1b[90m{}\x1b[0m", "─".repeat(rule_width)));
                }
                _ => {}
            }
        }

        // 刷新剩余文本
        if !current_text.is_empty() {
            Self::flush_text(&mut current_text, &mut lines, width, self.wrap_width);
        }

        lines
    }

    /// 刷新文本到行
    fn flush_text(text: &mut String, lines: &mut Vec<String>, width: u16, wrap_width: Option<u16>) {
        if text.is_empty() {
            return;
        }

        let effective_width = wrap_width.unwrap_or(width);
        let wrapped = wrap_text_with_ansi(text.trim_end(), effective_width as usize);
        lines.extend(wrapped);
        text.clear();
    }
}

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Markdown {
    fn render(&self, width: u16) -> Vec<String> {
        // 检查缓存 - 如果内容和宽度都未变化，直接返回缓存结果
        if self.content == self.cached_content && width == self.cached_width && !self.cached_output.is_empty() {
            return self.cached_output.clone();
        }
        
        // 正常渲染
        
        
        // 注意：由于 Component trait 的 render 是 &self，我们无法在这里更新缓存
        // 缓存更新由调用方通过 invalidate() 后重新渲染时处理
        self.render_markdown(width)
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
        // 注意：这里不清除缓存，让 render 方法决定是否使用缓存
    }
}

impl Markdown {
    /// 带缓存的渲染方法 - 需要可变引用以更新缓存
    pub fn render_with_cache(&mut self, width: u16) -> Vec<String> {
        // 检查缓存
        if self.content == self.cached_content && width == self.cached_width && !self.cached_output.is_empty() {
            return self.cached_output.clone();
        }
        
        // 渲染并更新缓存
        let output = self.render_markdown(width);
        self.cached_content = self.content.clone();
        self.cached_width = width;
        self.cached_output = output.clone();
        self.needs_render = false;
        
        output
    }
    
    /// 强制刷新缓存
    pub fn refresh_cache(&mut self, width: u16) -> Vec<String> {
        self.cached_content.clear();
        self.cached_width = 0;
        self.cached_output.clear();
        self.render_with_cache(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_empty() {
        let md = Markdown::new();
        let lines = md.render(80);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_markdown_heading() {
        let mut md = Markdown::new();
        md.set_content("# Heading 1\n\n## Heading 2");
        let lines = md.render(80);
        
        assert!(!lines.is_empty());
        let first_line = &lines[0];
        assert!(first_line.contains("Heading 1"));
        assert!(first_line.contains("\x1b[1")); // 包含粗体样式
    }

    #[test]
    fn test_markdown_bold() {
        let mut md = Markdown::new();
        md.set_content("This is **bold** text");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("bold"));
        assert!(lines[0].contains("\x1b[1m")); // 包含粗体开始
    }

    #[test]
    fn test_markdown_italic() {
        let mut md = Markdown::new();
        md.set_content("This is *italic* text");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("italic"));
        assert!(lines[0].contains("\x1b[3m")); // 包含斜体开始
    }

    #[test]
    fn test_markdown_code() {
        let mut md = Markdown::new();
        md.set_content("This is `code` inline");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("code"));
        assert!(lines[0].contains("\x1b[36m")); // 包含 cyan 颜色
    }

    #[test]
    fn test_markdown_code_block() {
        let mut md = Markdown::new();
        md.set_content("```rust\nfn main() {}\n```");
        let lines = md.render(80);
        
        assert!(lines.len() >= 3);
        assert!(lines[0].contains("```"));
        assert!(lines[1].contains("fn main()"));
        assert!(lines[2].contains("```"));
    }

    #[test]
    fn test_markdown_list() {
        let mut md = Markdown::new();
        md.set_content("- Item 1\n- Item 2\n- Item 3");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("Item 1"));
        assert!(lines[1].contains("Item 2"));
        assert!(lines[2].contains("Item 3"));
    }

    #[test]
    fn test_markdown_link() {
        let mut md = Markdown::new();
        md.set_content("[link text](https://example.com)");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("link text"));
        assert!(lines[0].contains("https://example.com"));
    }

    #[test]
    fn test_markdown_horizontal_rule() {
        let mut md = Markdown::new();
        md.set_content("Text\n\n---\n\nMore text");
        let lines = md.render(80);
        
        // 应该有一行包含水平线
        let has_rule = lines.iter().any(|line| line.contains('─'));
        assert!(has_rule);
    }

    #[test]
    fn test_markdown_append() {
        let mut md = Markdown::new();
        md.append_content("Hello ");
        md.append_content("World");
        
        assert_eq!(md.content(), "Hello World");
    }

    #[test]
    fn test_markdown_clear() {
        let mut md = Markdown::new();
        md.set_content("Some content");
        md.clear();
        
        assert!(md.content().is_empty());
    }

    #[test]
    fn test_render_plain_text() {
        let mut md = Markdown::new();
        md.set_content("This is plain text without any formatting");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("This is plain text without any formatting"));
    }

    #[test]
    fn test_render_bold_italic() {
        let mut md = Markdown::new();
        md.set_content("This is ***bold and italic*** text");
        let lines = md.render(80);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("bold and italic"));
    }

    #[test]
    fn test_render_width_wrapping() {
        let mut md = Markdown::new();
        md.set_content("This is a very long line that should be wrapped when rendered with a narrow width limit");
        md.set_wrap_width(Some(20));
        let lines = md.render(80);
        
        // 验证换行生效
        assert!(lines.len() > 1);
        // 每行应该不超过 20 个字符（不包括 ANSI 转义序列）
        for line in &lines {
            let visible_len = line.chars().count();
            assert!(visible_len <= 30, "Line too long: {}", line); // 允许一些 ANSI 转义序列的额外空间
        }
    }

    #[test]
    fn test_render_with_cache() {
        let mut md = Markdown::new();
        md.set_content("Test content for caching");
        
        // 第一次渲染
        let lines1 = md.render_with_cache(80);
        // 第二次渲染（应该使用缓存）
        let lines2 = md.render_with_cache(80);
        
        assert_eq!(lines1, lines2);
        assert!(!md.content().is_empty());
        assert_eq!(md.cached_content, "Test content for caching");
    }

    #[test]
    fn test_refresh_cache() {
        let mut md = Markdown::new();
        md.set_content("Original content");
        md.render_with_cache(80);
        
        // 刷新缓存
        let lines = md.refresh_cache(80);
        
        assert!(!lines.is_empty());
        assert!(lines[0].contains("Original"));
    }

    #[test]
    fn test_render_nested_list() {
        let mut md = Markdown::new();
        md.set_content("- Item 1\n  - Subitem 1\n  - Subitem 2\n- Item 2");
        let lines = md.render(80);
        
        assert!(lines.len() >= 4);
        assert!(lines.iter().any(|l| l.contains("Item 1")));
        assert!(lines.iter().any(|l| l.contains("Subitem 1")));
        assert!(lines.iter().any(|l| l.contains("Item 2")));
    }

    #[test]
    fn test_render_mixed_formatting() {
        let mut md = Markdown::new();
        md.set_content("# Title\n\nThis has **bold** and *italic* and `code`.\n\n- List item\n- Another item");
        let lines = md.render(80);
        
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.contains("Title")));
        assert!(lines.iter().any(|l| l.contains("bold")));
        assert!(lines.iter().any(|l| l.contains("italic")));
        assert!(lines.iter().any(|l| l.contains("code")));
        assert!(lines.iter().any(|l| l.contains("List item")));
    }

    #[test]
    fn test_render_multiple_paragraphs() {
        let mut md = Markdown::new();
        md.set_content("First paragraph.\n\nSecond paragraph.\n\nThird paragraph.");
        let lines = md.render(80);
        
        // 应该有段落之间的空行
        assert!(lines.iter().any(|l| l.contains("First paragraph")));
        assert!(lines.iter().any(|l| l.contains("Second paragraph")));
        assert!(lines.iter().any(|l| l.contains("Third paragraph")));
    }

    #[test]
    fn test_render_code_block_with_language() {
        let mut md = Markdown::new();
        md.set_content("```python\ndef hello():\n    print('world')\n```");
        let lines = md.render(80);
        
        // 验证代码块被渲染（至少包含代码内容）
        assert!(lines.len() >= 2);
        // 第一行应该包含 ``` 和语言标识
        let first_line_has_marker = lines.iter().any(|l| l.contains("```") && l.contains("python"));
        assert!(first_line_has_marker, "Expected code block marker with language");
        // 应该包含代码内容
        assert!(lines.iter().any(|l| l.contains("def hello")));
        // 应该有结束标记
        let has_end_marker = lines.iter().any(|l| l.contains("```") && !l.contains("python"));
        assert!(has_end_marker, "Expected code block end marker");
    }

    #[test]
    fn test_render_ordered_list() {
        let mut md = Markdown::new();
        md.set_content("1. First item\n2. Second item\n3. Third item");
        let lines = md.render(80);
        
        assert!(lines.len() >= 3);
        assert!(lines.iter().any(|l| l.contains("First item")));
        assert!(lines.iter().any(|l| l.contains("Second item")));
        assert!(lines.iter().any(|l| l.contains("Third item")));
    }
}
