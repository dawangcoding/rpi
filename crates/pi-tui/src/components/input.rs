//! 单行输入框组件
//! 支持自动完成、光标移动和基本编辑功能

use crate::autocomplete::{AutocompleteProvider, AutocompleteSuggestions};
use crate::tui::{Component, Focusable};
use crate::utils::CURSOR_MARKER;
use crate::utils::visible_width;

/// 单行输入框
pub struct Input {
    /// 输入文本
    text: String,
    /// 光标位置（字符索引）
    cursor: usize,
    /// 占位符文本
    placeholder: Option<String>,
    /// 是否聚焦
    focused: bool,
    /// 是否需要重新渲染
    needs_render: bool,
    /// 自动完成提供者
    autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
    /// 自动完成建议
    autocomplete_suggestions: Option<AutocompleteSuggestions>,
    /// 当前选中的建议索引
    autocomplete_index: usize,
    /// 水平滚动偏移
    scroll_offset: usize,
    /// 最大长度限制
    max_length: Option<usize>,
    /// 密码模式
    password_mode: bool,
}

impl Input {
    /// 创建新的输入框
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            placeholder: None,
            focused: false,
            needs_render: true,
            autocomplete_provider: None,
            autocomplete_suggestions: None,
            autocomplete_index: 0,
            scroll_offset: 0,
            max_length: None,
            password_mode: false,
        }
    }

    /// 创建带占位符的输入框
    pub fn with_placeholder(placeholder: &str) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            placeholder: Some(placeholder.to_string()),
            focused: false,
            needs_render: true,
            autocomplete_provider: None,
            autocomplete_suggestions: None,
            autocomplete_index: 0,
            scroll_offset: 0,
            max_length: None,
            password_mode: false,
        }
    }

    /// 创建密码输入框
    pub fn password() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            placeholder: None,
            focused: false,
            needs_render: true,
            autocomplete_provider: None,
            autocomplete_suggestions: None,
            autocomplete_index: 0,
            scroll_offset: 0,
            max_length: None,
            password_mode: true,
        }
    }

    // === 文本访问 ===

    /// 获取当前文本
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 设置文本
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.text.len(); // 将光标移动到文本末尾
        self.scroll_offset = 0;
        self.needs_render = true;
        self.dismiss_autocomplete();
    }

    /// 清空输入
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.scroll_offset = 0;
        self.needs_render = true;
        self.dismiss_autocomplete();
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 获取文本长度
    pub fn len(&self) -> usize {
        self.text.len()
    }

    // === 编辑操作 ===

    /// 插入字符
    pub fn insert_char(&mut self, ch: char) {
        // 检查最大长度限制
        if let Some(max) = self.max_length {
            if self.text.len() >= max {
                return;
            }
        }

        // 插入字符
        let char_len = ch.len_utf8();
        if self.cursor > self.text.len() {
            self.cursor = self.text.len();
        }
        
        self.text.insert(self.cursor, ch);
        self.cursor += char_len;
        
        self.needs_render = true;
        self.update_autocomplete();
    }

    /// 插入文本（在当前光标位置）
    pub fn insert_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_char(ch);
        }
    }

    /// 删除光标前的字符（Backspace）
    pub fn delete_char_before(&mut self) {
        if self.cursor > 0 {
            // 找到前一个字符的起始位置
            let char_idx = self.text[..self.cursor].char_indices().nth(
                self.text[..self.cursor].chars().count().saturating_sub(1)
            );
            
            if let Some((idx, _ch)) = char_idx {
                self.text.remove(idx);
                self.cursor = idx;
            }
            
            self.needs_render = true;
            self.update_autocomplete();
        }
    }

    /// 删除光标后的字符（Delete）
    pub fn delete_char_after(&mut self) {
        if self.cursor < self.text.len() {
            let char_idx = self.text[self.cursor..].char_indices().nth(1);
            
            if let Some((idx, _ch)) = char_idx {
                self.text.drain(self.cursor..self.cursor + idx);
            } else {
                self.text.truncate(self.cursor);
            }
            
            self.needs_render = true;
            self.update_autocomplete();
        }
    }

    /// 删除到行首
    pub fn delete_to_line_start(&mut self) {
        if self.cursor > 0 {
            self.text.drain(0..self.cursor);
            self.cursor = 0;
            self.scroll_offset = 0;
            self.needs_render = true;
            self.update_autocomplete();
        }
    }

    /// 删除到行尾
    pub fn delete_to_line_end(&mut self) {
        if self.cursor < self.text.len() {
            self.text.truncate(self.cursor);
            self.needs_render = true;
            self.update_autocomplete();
        }
    }

    /// 删除前一个单词
    pub fn delete_word_before(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let old_cursor = self.cursor;
        self.move_word_left();
        let delete_from = self.cursor;
        self.cursor = old_cursor;

        self.text.drain(delete_from..self.cursor);
        self.cursor = delete_from;
        
        self.needs_render = true;
        self.update_autocomplete();
    }

    /// 删除后一个单词
    pub fn delete_word_after(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }

        let old_cursor = self.cursor;
        self.move_word_right();
        let delete_to = self.cursor;
        self.cursor = old_cursor;

        self.text.drain(self.cursor..delete_to);
        
        self.needs_render = true;
        self.update_autocomplete();
    }

    // === 光标移动 ===

    /// 向左移动
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            let char_idx = self.text[..self.cursor].char_indices().nth(
                self.text[..self.cursor].chars().count().saturating_sub(1)
            );
            
            if let Some((idx, _)) = char_idx {
                self.cursor = idx;
            }
            self.needs_render = true;
        }
    }

    /// 向右移动
    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            let char_idx = self.text[self.cursor..].char_indices().nth(1);
            
            if let Some((idx, _)) = char_idx {
                self.cursor += idx;
            } else {
                self.cursor = self.text.len();
            }
            self.needs_render = true;
        }
    }

    /// 移动到行首
    pub fn move_home(&mut self) {
        self.cursor = 0;
        self.scroll_offset = 0;
        self.needs_render = true;
    }

    /// 移动到行尾
    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
        self.needs_render = true;
    }

    /// 向左移动一个单词
    pub fn move_word_left(&mut self) {
        let text_before = &self.text[..self.cursor];

        // 跳过尾部空白
        let mut new_cursor = self.cursor;
        for (idx, ch) in text_before.char_indices().rev() {
            if !ch.is_whitespace() {
                new_cursor = idx + ch.len_utf8();
                break;
            }
            new_cursor = idx;
        }

        // 跳过单词
        let mut found_word = false;
        for (idx, ch) in self.text[..new_cursor].char_indices().rev() {
            if ch.is_alphanumeric() {
                found_word = true;
            } else if found_word {
                new_cursor = idx + ch.len_utf8();
                break;
            }
            new_cursor = idx;
        }

        self.cursor = new_cursor;
        self.needs_render = true;
    }

    /// 向右移动一个单词
    pub fn move_word_right(&mut self) {
        let text_after = &self.text[self.cursor..];

        // 跳过前导空白
        let mut new_cursor = self.cursor;
        for (idx, ch) in text_after.char_indices() {
            if !ch.is_whitespace() {
                break;
            }
            new_cursor = self.cursor + idx + ch.len_utf8();
        }

        // 跳过单词
        let mut found_word = false;
        for (idx, ch) in self.text[new_cursor..].char_indices() {
            if ch.is_alphanumeric() {
                found_word = true;
            } else if found_word {
                break;
            }
            new_cursor = self.cursor + idx + ch.len_utf8();
        }

        self.cursor = new_cursor;
        self.needs_render = true;
    }

    // === 自动完成 ===

    /// 设置自动完成提供者
    pub fn set_autocomplete_provider(&mut self, provider: Box<dyn AutocompleteProvider>) {
        self.autocomplete_provider = Some(provider);
    }

    /// 触发自动完成
    pub fn trigger_autocomplete(&mut self) {
        self.update_autocomplete();
    }

    /// 接受当前建议
    pub fn accept_autocomplete(&mut self) {
        if let Some(suggestions) = &self.autocomplete_suggestions {
            if self.autocomplete_index < suggestions.items.len() {
                let item = &suggestions.items[self.autocomplete_index];
                let insert_text = item.get_insert_text();
                
                // 替换前缀
                let prefix_len = suggestions.prefix.len();
                let start = self.cursor.saturating_sub(prefix_len);
                self.text.replace_range(start..self.cursor, insert_text);
                self.cursor = start + insert_text.len();

                self.dismiss_autocomplete();
                self.needs_render = true;
            }
        }
    }

    /// 关闭自动完成
    pub fn dismiss_autocomplete(&mut self) {
        self.autocomplete_suggestions = None;
        self.autocomplete_index = 0;
        self.needs_render = true;
    }

    /// 下一个建议
    pub fn next_autocomplete(&mut self) {
        if let Some(suggestions) = &self.autocomplete_suggestions {
            if !suggestions.items.is_empty() {
                self.autocomplete_index = (self.autocomplete_index + 1) % suggestions.items.len();
                self.needs_render = true;
            }
        }
    }

    /// 上一个建议
    pub fn prev_autocomplete(&mut self) {
        if let Some(suggestions) = &self.autocomplete_suggestions {
            if !suggestions.items.is_empty() {
                self.autocomplete_index = if self.autocomplete_index == 0 {
                    suggestions.items.len() - 1
                } else {
                    self.autocomplete_index - 1
                };
                self.needs_render = true;
            }
        }
    }

    /// 检查是否正在显示自动完成
    pub fn is_showing_autocomplete(&self) -> bool {
        self.autocomplete_suggestions.is_some()
    }

    /// 更新自动完成建议
    fn update_autocomplete(&mut self) {
        if let Some(provider) = &self.autocomplete_provider {
            if let Some(suggestions) = provider.provide(&self.text, self.cursor) {
                if !suggestions.is_empty() {
                    self.autocomplete_suggestions = Some(suggestions);
                    self.autocomplete_index = 0;
                    self.needs_render = true;
                    return;
                }
            }
        }
        
        self.autocomplete_suggestions = None;
    }

    // === 配置 ===

    /// 设置占位符
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = Some(placeholder.into());
        self.needs_render = true;
    }

    /// 设置最大长度
    pub fn set_max_length(&mut self, max: Option<usize>) {
        self.max_length = max;
        if let Some(max) = max {
            if self.text.len() > max {
                self.text.truncate(max);
                self.cursor = self.cursor.min(max);
                self.needs_render = true;
            }
        }
    }

    /// 设置密码模式
    pub fn set_password_mode(&mut self, enabled: bool) {
        self.password_mode = enabled;
        self.needs_render = true;
    }

    // === 内部辅助 ===

    /// 计算可见文本范围
    fn calculate_visible_range(&self, available_width: usize) -> (usize, usize) {
        let text_width = if self.password_mode {
            self.text.chars().count() // 密码模式每个字符显示为 *
        } else {
            visible_width(&self.text)
        };

        if text_width <= available_width {
            return (0, self.text.len());
        }

        // 计算光标在显示宽度中的位置
        let cursor_display_pos = if self.password_mode {
            self.text[..self.cursor].chars().count()
        } else {
            visible_width(&self.text[..self.cursor.min(self.text.len())])
        };

        // 计算滚动偏移，使光标保持可见
        let half_width = available_width / 2;
        let scroll = cursor_display_pos.saturating_sub(half_width);

        // 将显示宽度转换回字符索引
        let mut current_width = 0;
        let mut start_idx = 0;
        for (idx, ch) in self.text.char_indices() {
            let char_width = if self.password_mode { 1 } else { visible_width(&ch.to_string()) };
            if current_width >= scroll {
                start_idx = idx;
                break;
            }
            current_width += char_width;
        }

        // 计算结束索引
        current_width = 0;
        let mut end_idx = self.text.len();
        for (idx, ch) in self.text[start_idx..].char_indices() {
            let char_width = if self.password_mode { 1 } else { visible_width(&ch.to_string()) };
            if current_width + char_width > available_width {
                end_idx = start_idx + idx;
                break;
            }
            current_width += char_width;
        }

        (start_idx, end_idx)
    }

    /// 获取用于显示的文本
    fn get_display_text(&self) -> String {
        if self.password_mode {
            "*".repeat(self.text.chars().count())
        } else {
            self.text.clone()
        }
    }

    /// 渲染自动完成弹出菜单
    fn render_autocomplete(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        
        if let Some(suggestions) = &self.autocomplete_suggestions {
            if suggestions.items.is_empty() {
                return lines;
            }

            let max_visible = 5.min(suggestions.items.len());
            let start = (self.autocomplete_index / max_visible) * max_visible;
            let end = (start + max_visible).min(suggestions.items.len());

            // 顶部边框
            lines.push("─".repeat(width).to_string());

            for i in start..end {
                let item = &suggestions.items[i];
                let is_selected = i == self.autocomplete_index;
                
                let label = if item.label.len() > width.saturating_sub(4) {
                    format!("{}...", &item.label[..width.saturating_sub(7)])
                } else {
                    item.label.clone()
                };

                let line = if is_selected {
                    format!("  \x1b[7m {:width$}\x1b[0m  ", label, width = width.saturating_sub(4))
                } else {
                    format!("   {:width$}   ", label, width = width.saturating_sub(4))
                };

                lines.push(line);
            }

            // 底部边框
            lines.push("─".repeat(width).to_string());
        }

        lines
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Input {
    fn render(&self, width: u16) -> Vec<String> {
        let width = width as usize;
        let prompt = "> ";
        let available_width = width.saturating_sub(prompt.len());

        if available_width == 0 {
            return vec![prompt.to_string()];
        }

        let display_text = self.get_display_text();

        // 计算可见范围
        let (start_idx, end_idx) = self.calculate_visible_range(available_width);
        let visible_text = &display_text[start_idx..end_idx.min(display_text.len())];

        // 计算光标在可见文本中的位置
        let cursor_in_visible = if self.password_mode {
            self.text[..self.cursor].chars().count().saturating_sub(start_idx)
        } else {
            visible_width(&self.text[start_idx..self.cursor.min(self.text.len())])
        };

        // 构建显示行
        let mut line = prompt.to_string();

        if self.focused {
            // 在光标位置插入光标标记
            let before = &visible_text[..cursor_in_visible.min(visible_text.len())];
            let after = if cursor_in_visible < visible_text.len() {
                let char_idx = visible_text.char_indices().nth(
                    visible_text[..cursor_in_visible].chars().count()
                );
                if let Some((idx, ch)) = char_idx {
                    format!("\x1b[7m{}\x1b[0m{}", ch, &visible_text[idx + ch.len_utf8()..])
                } else {
                    "\x1b[7m \x1b[0m".to_string()
                }
            } else {
                "\x1b[7m \x1b[0m".to_string()
            };

            line.push_str(before);
            line.push_str(CURSOR_MARKER);
            line.push_str(&after);
        } else {
            line.push_str(visible_text);
        }

        // 填充到可用宽度
        let current_width = visible_width(&line);
        if current_width < width {
            line.push_str(&" ".repeat(width - current_width));
        }

        let mut lines = vec![line];

        // 添加自动完成弹出菜单
        if self.autocomplete_suggestions.is_some() {
            let autocomplete_lines = self.render_autocomplete(available_width);
            lines.extend(autocomplete_lines);
        }

        lines
    }

    fn handle_input(&mut self, data: &str) -> bool {
        match data {
            // 字符输入
            _ if data.len() == 1 && data.as_bytes()[0] >= 32 && data.as_bytes()[0] < 127 => {
                self.insert_char(data.chars().next().unwrap());
                true
            }
            // 回车
            "\r" | "\n" | "\r\n" => {
                // 提交操作，由调用者处理
                false
            }
            // 退格
            "\x7f" | "\x08" => {
                self.delete_char_before();
                true
            }
            // Delete
            "\x1b[3~" => {
                self.delete_char_after();
                true
            }
            // 方向键
            "\x1b[D" => {
                // Left
                if self.autocomplete_suggestions.is_some() {
                    self.dismiss_autocomplete();
                }
                self.move_left();
                true
            }
            "\x1b[C" => {
                // Right
                if self.autocomplete_suggestions.is_some() {
                    self.dismiss_autocomplete();
                }
                self.move_right();
                true
            }
            "\x1b[A" => {
                // Up
                if self.autocomplete_suggestions.is_some() {
                    self.prev_autocomplete();
                }
                true
            }
            "\x1b[B" => {
                // Down
                if self.autocomplete_suggestions.is_some() {
                    self.next_autocomplete();
                }
                true
            }
            // Home
            "\x1b[H" | "\x1b[1~" | "\x1bOH" => {
                self.move_home();
                true
            }
            // End
            "\x1b[F" | "\x1b[4~" | "\x1bOF" => {
                self.move_end();
                true
            }
            // Ctrl+A (行首)
            "\x01" => {
                self.move_home();
                true
            }
            // Ctrl+E (行尾)
            "\x05" => {
                self.move_end();
                true
            }
            // Ctrl+U (删除到行首)
            "\x15" => {
                self.delete_to_line_start();
                true
            }
            // Ctrl+K (删除到行尾)
            "\x0b" => {
                self.delete_to_line_end();
                true
            }
            // Ctrl+W (删除前一个单词)
            "\x17" => {
                self.delete_word_before();
                true
            }
            // Tab (接受自动完成或触发)
            "\t" => {
                if self.autocomplete_suggestions.is_some() {
                    self.accept_autocomplete();
                } else {
                    self.trigger_autocomplete();
                }
                true
            }
            // Escape (取消自动完成)
            "\x1b" | "\x1b\x1b" => {
                if self.autocomplete_suggestions.is_some() {
                    self.dismiss_autocomplete();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
    }
}

impl Focusable for Input {
    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.needs_render = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_new() {
        let input = Input::new();
        assert!(input.is_empty());
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_input_with_placeholder() {
        let input = Input::with_placeholder("Enter text...");
        assert_eq!(input.placeholder, Some("Enter text...".to_string()));
    }

    #[test]
    fn test_input_insert() {
        let mut input = Input::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.text(), "hi");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn test_input_delete() {
        let mut input = Input::new();
        input.insert_text("hello");
        input.delete_char_before();
        assert_eq!(input.text(), "hell");
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn test_input_movement() {
        let mut input = Input::new();
        input.insert_text("hello");
        input.move_left();
        assert_eq!(input.cursor, 4);
        input.move_home();
        assert_eq!(input.cursor, 0);
        input.move_end();
        assert_eq!(input.cursor, 5);
    }

    #[test]
    fn test_input_word_movement() {
        let mut input = Input::new();
        input.insert_text("hello world");
        input.move_word_left();
        assert_eq!(input.cursor, 6); // 在 "world" 前
        input.move_word_right();
        assert_eq!(input.cursor, 11); // 到末尾
    }

    #[test]
    fn test_input_clear() {
        let mut input = Input::new();
        input.insert_text("hello");
        input.clear();
        assert!(input.is_empty());
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn test_input_set_text() {
        let mut input = Input::new();
        input.set_text("hello world");
        assert_eq!(input.text(), "hello world");
        assert_eq!(input.cursor, 11);
    }

    #[test]
    fn test_input_max_length() {
        let mut input = Input::new();
        input.set_max_length(Some(5));
        input.insert_text("hello world");
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn test_input_password_mode() {
        let input = Input::password();
        assert!(input.password_mode);
    }

    #[test]
    fn test_input_unicode() {
        let mut input = Input::new();
        input.insert_text("你好世界");
        assert_eq!(input.text(), "你好世界");
        input.move_left();
        assert!(input.cursor < 12); // UTF-8 编码长度
    }
}
