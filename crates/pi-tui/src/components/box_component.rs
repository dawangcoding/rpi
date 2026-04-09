//! 边框容器组件
//! 提供带边框的容器，可以包裹其他组件

use crate::tui::Component;
use crate::utils::{pad_to_width, visible_width};

/// 边框样式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Single,   // ┌─┐│└─┘
    Double,   // ╔═╗║╚═╝
    Rounded,  // ╭─╮│╰─╯
    Heavy,    // ┏━┓┃┗━┛
}

impl BorderStyle {
    /// 获取边框字符
    fn chars(&self) -> BorderChars {
        match self {
            BorderStyle::None => BorderChars {
                top_left: ' ',
                top_right: ' ',
                bottom_left: ' ',
                bottom_right: ' ',
                horizontal: ' ',
                vertical: ' ',
            },
            BorderStyle::Single => BorderChars {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
            },
            BorderStyle::Double => BorderChars {
                top_left: '╔',
                top_right: '╗',
                bottom_left: '╚',
                bottom_right: '╝',
                horizontal: '═',
                vertical: '║',
            },
            BorderStyle::Rounded => BorderChars {
                top_left: '╭',
                top_right: '╮',
                bottom_left: '╰',
                bottom_right: '╯',
                horizontal: '─',
                vertical: '│',
            },
            BorderStyle::Heavy => BorderChars {
                top_left: '┏',
                top_right: '┓',
                bottom_left: '┗',
                bottom_right: '┛',
                horizontal: '━',
                vertical: '┃',
            },
        }
    }
}

/// 边框字符
struct BorderChars {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
}

/// 边框容器组件
pub struct BoxComponent {
    child: Option<Box<dyn Component>>,
    title: Option<String>,
    border_style: BorderStyle,
    padding: u16,
    needs_render: bool,
}

impl BoxComponent {
    /// 创建新的边框容器
    pub fn new(border_style: BorderStyle) -> Self {
        Self {
            child: None,
            title: None,
            border_style,
            padding: 0,
            needs_render: true,
        }
    }

    /// 创建无边框容器
    pub fn none() -> Self {
        Self::new(BorderStyle::None)
    }

    /// 创建单线边框容器
    pub fn single() -> Self {
        Self::new(BorderStyle::Single)
    }

    /// 创建双线边框容器
    pub fn double() -> Self {
        Self::new(BorderStyle::Double)
    }

    /// 创建圆角边框容器
    pub fn rounded() -> Self {
        Self::new(BorderStyle::Rounded)
    }

    /// 创建粗线边框容器
    pub fn heavy() -> Self {
        Self::new(BorderStyle::Heavy)
    }

    /// 设置标题（链式调用）
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self.needs_render = true;
        self
    }

    /// 设置内边距（链式调用）
    pub fn with_padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self.needs_render = true;
        self
    }

    /// 设置子组件
    pub fn set_child(&mut self, child: Box<dyn Component>) {
        self.child = Some(child);
        self.needs_render = true;
    }

    /// 移除子组件
    pub fn remove_child(&mut self) -> Option<Box<dyn Component>> {
        self.needs_render = true;
        self.child.take()
    }

    /// 设置标题
    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
        self.needs_render = true;
    }

    /// 获取标题
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// 设置内边距
    pub fn set_padding(&mut self, padding: u16) {
        self.padding = padding;
        self.needs_render = true;
    }

    /// 获取内边距
    pub fn padding(&self) -> u16 {
        self.padding
    }

    /// 设置边框样式
    pub fn set_border_style(&mut self, style: BorderStyle) {
        self.border_style = style;
        self.needs_render = true;
    }

    /// 获取边框样式
    pub fn border_style(&self) -> BorderStyle {
        self.border_style
    }

    /// 渲染顶部边框
    fn render_top_border(&self, width: u16, chars: &BorderChars) -> String {
        let content_width = width.saturating_sub(2) as usize;
        
        if let Some(title) = &self.title {
            let title_len = visible_width(title);
            let available_width = content_width.saturating_sub(title_len + 2); // 2 for spaces around title
            
            if available_width > 0 {
                let left_width = available_width / 2;
                let right_width = available_width - left_width;
                format!(
                    "{}{} {} {}{}",
                    chars.top_left,
                    chars.horizontal.to_string().repeat(left_width),
                    title,
                    chars.horizontal.to_string().repeat(right_width),
                    chars.top_right
                )
            } else {
                // Title too long, just render border
                format!(
                    "{}{}{}",
                    chars.top_left,
                    chars.horizontal.to_string().repeat(content_width),
                    chars.top_right
                )
            }
        } else {
            format!(
                "{}{}{}",
                chars.top_left,
                chars.horizontal.to_string().repeat(content_width),
                chars.top_right
            )
        }
    }

    /// 渲染底部边框
    fn render_bottom_border(&self, width: u16, chars: &BorderChars) -> String {
        let content_width = width.saturating_sub(2) as usize;
        format!(
            "{}{}{}",
            chars.bottom_left,
            chars.horizontal.to_string().repeat(content_width),
            chars.bottom_right
        )
    }

    /// 渲染内容行（带左右边框）
    fn render_content_line(&self, line: &str, width: u16, chars: &BorderChars) -> String {
        let content_width = width.saturating_sub(2) as usize;
        let padded = pad_to_width(line, content_width);
        format!("{}{}{}", chars.vertical, padded, chars.vertical)
    }

    /// 渲染空行（带左右边框）
    fn render_empty_line(&self, width: u16, chars: &BorderChars) -> String {
        let content_width = width.saturating_sub(2) as usize;
        format!(
            "{}{}{}",
            chars.vertical,
            " ".repeat(content_width),
            chars.vertical
        )
    }
}

impl Component for BoxComponent {
    fn render(&self, width: u16) -> Vec<String> {
        if width < 2 {
            return vec![String::new()];
        }

        let chars = self.border_style.chars();
        let mut lines = Vec::new();

        // 顶部边框
        if self.border_style != BorderStyle::None {
            lines.push(self.render_top_border(width, &chars));
        }

        // 上内边距
        for _ in 0..self.padding {
            if self.border_style != BorderStyle::None {
                lines.push(self.render_empty_line(width, &chars));
            } else {
                lines.push(String::new());
            }
        }

        // 子组件内容
        let content_width = if self.border_style == BorderStyle::None {
            width
        } else {
            width.saturating_sub(2 + self.padding * 2)
        };

        if let Some(child) = &self.child {
            let child_lines = child.render(content_width);
            for line in child_lines {
                if self.border_style != BorderStyle::None {
                    // 添加左右内边距
                    let padded = format!(
                        "{}{}{}",
                        " ".repeat(self.padding as usize),
                        line,
                        " ".repeat(self.padding as usize)
                    );
                    lines.push(self.render_content_line(&padded, width, &chars));
                } else {
                    lines.push(line);
                }
            }
        }

        // 下内边距
        for _ in 0..self.padding {
            if self.border_style != BorderStyle::None {
                lines.push(self.render_empty_line(width, &chars));
            } else {
                lines.push(String::new());
            }
        }

        // 底部边框
        if self.border_style != BorderStyle::None {
            lines.push(self.render_bottom_border(width, &chars));
        }

        lines
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
        if let Some(child) = &mut self.child {
            child.invalidate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockComponent {
        lines: Vec<String>,
    }

    impl Component for MockComponent {
        fn render(&self, _width: u16) -> Vec<String> {
            self.lines.clone()
        }

        fn invalidate(&mut self) {}
    }

    #[test]
    fn test_box_single() {
        let mut box_comp = BoxComponent::single();
        box_comp.set_child(Box::new(MockComponent {
            lines: vec!["Hello".to_string()],
        }));
        
        let lines = box_comp.render(10);
        assert!(lines[0].starts_with('┌'));
        assert!(lines[0].ends_with('┐'));
        assert!(lines[1].contains("Hello"));
        assert!(lines[2].starts_with('└'));
        assert!(lines[2].ends_with('┘'));
    }

    #[test]
    fn test_box_with_title() {
        let box_comp = BoxComponent::single()
            .with_title("Test");
        
        let lines = box_comp.render(20);
        assert!(lines[0].contains("Test"));
    }

    #[test]
    fn test_box_with_padding() {
        let mut box_comp = BoxComponent::single()
            .with_padding(1);
        box_comp.set_child(Box::new(MockComponent {
            lines: vec!["Content".to_string()],
        }));
        
        let lines = box_comp.render(15);
        // Should have: top border, padding, content, padding, bottom border
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_box_none() {
        let mut box_comp = BoxComponent::none();
        box_comp.set_child(Box::new(MockComponent {
            lines: vec!["Hello".to_string()],
        }));
        
        let lines = box_comp.render(10);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Hello");
    }

    #[test]
    fn test_box_various_styles() {
        let styles = vec![
            (BoxComponent::double(), '╔', '╗'),
            (BoxComponent::rounded(), '╭', '╮'),
            (BoxComponent::heavy(), '┏', '┓'),
        ];

        for (box_comp, expected_left, expected_right) in styles {
            let lines = box_comp.render(10);
            assert!(lines[0].starts_with(expected_left));
            assert!(lines[0].ends_with(expected_right));
        }
    }
}
