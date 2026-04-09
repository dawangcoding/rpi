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
}

impl Markdown {
    /// 创建新的 Markdown 组件
    pub fn new() -> Self {
        Self {
            content: String::new(),
            rendered_lines: Vec::new(),
            needs_render: true,
            wrap_width: None,
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
        self.render_markdown(width)
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
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
}
