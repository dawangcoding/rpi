//! 多行文本编辑器组件
//! 支持光标移动、文本编辑、撤销重做、自动完成等功能

use crate::autocomplete::{AutocompleteProvider, AutocompleteSuggestions};
use crate::kill_ring::{KillRing, PushOptions};
use crate::tui::{Component, Focusable};
use crate::utils::CURSOR_MARKER;
use crate::undo_stack::UndoStack;
use crate::utils::{visible_width, wrap_text_with_ansi};

/// 编辑器状态快照（用于撤销）
#[derive(Debug, Clone)]
struct EditorSnapshot {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
}

/// 选择范围
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

impl Selection {
    /// 创建新的选择范围
    pub fn new(start_row: usize, start_col: usize, end_row: usize, end_col: usize) -> Self {
        Self {
            start_row,
            start_col,
            end_row,
            end_col,
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.start_row == self.end_row && self.start_col == self.end_col
    }

    /// 规范化选择（确保起点 <= 终点）
    pub fn normalized(&self) -> Self {
        if self.start_row < self.end_row
            || (self.start_row == self.end_row && self.start_col <= self.end_col)
        {
            *self
        } else {
            Self {
                start_row: self.end_row,
                start_col: self.end_col,
                end_row: self.start_row,
                end_col: self.start_col,
            }
        }
    }
}

/// 编辑器配置
#[derive(Debug, Clone)]
pub struct EditorConfig {
    /// 占位符文本
    pub placeholder: Option<String>,
    /// 最大行数
    pub max_lines: Option<usize>,
    /// 只读模式
    pub read_only: bool,
    /// 显示行号
    pub line_numbers: bool,
    /// 自动换行
    pub wrap: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            placeholder: None,
            max_lines: None,
            read_only: false,
            line_numbers: false,
            wrap: true,
        }
    }
}

/// 多行文本编辑器
pub struct Editor {
    // 文本状态
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,

    // 选择
    selection: Option<Selection>,

    // 视口
    scroll_row: usize,
    scroll_col: usize,

    // 撤销/重做
    undo_stack: UndoStack<EditorSnapshot>,

    // 剪贴板
    kill_ring: KillRing,

    // 自动完成
    autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
    autocomplete_suggestions: Option<AutocompleteSuggestions>,
    autocomplete_index: usize,

    // 配置
    config: EditorConfig,

    // 状态标志
    focused: bool,
    needs_render: bool,

    // 渲染缓存
    last_width: u16,

    // 最后操作类型（用于撤销分组）
    last_action: LastAction,

    // 粘贴标记计数器
    paste_counter: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastAction {
    None,
    TypeWord,
    Kill,
    Yank,
}

impl Editor {
    /// 创建新的编辑器
    pub fn new(config: EditorConfig) -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            selection: None,
            scroll_row: 0,
            scroll_col: 0,
            undo_stack: UndoStack::new(100),
            kill_ring: KillRing::new(50),
            autocomplete_provider: None,
            autocomplete_suggestions: None,
            autocomplete_index: 0,
            config,
            focused: false,
            needs_render: true,
            last_width: 80,
            last_action: LastAction::None,
            paste_counter: 0,
        }
    }

    // === 文本操作 ===

    /// 获取所有文本
    pub fn get_text(&self) -> String {
        self.lines.join("\n")
    }

    /// 设置文本
    pub fn set_text(&mut self, text: &str) {
        self.save_snapshot();
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(|s| s.to_string()).collect()
        };
        self.cursor_row = self.lines.len().saturating_sub(1);
        self.cursor_col = self.lines.last().map(|s| s.len()).unwrap_or(0);
        self.ensure_cursor_valid();
        self.needs_render = true;
        self.last_action = LastAction::None;
    }

    /// 插入字符
    pub fn insert_char(&mut self, ch: char) {
        if self.config.read_only {
            return;
        }

        // 撤销分组：连续的单词字符合并为一个撤销操作
        if ch.is_whitespace() || self.last_action != LastAction::TypeWord {
            self.save_snapshot();
        }
        self.last_action = LastAction::TypeWord;

        // 如果有选择，先删除选择内容
        if self.selection.is_some() {
            self.delete_selection();
        }

        let line = &mut self.lines[self.cursor_row];
        if self.cursor_col > line.len() {
            self.cursor_col = line.len();
        }
        line.insert(self.cursor_col, ch);
        self.cursor_col += ch.len_utf8();

        self.ensure_cursor_valid();
        self.needs_render = true;
        self.update_autocomplete();
    }

    /// 插入文本
    pub fn insert_text(&mut self, text: &str) {
        if self.config.read_only || text.is_empty() {
            return;
        }

        self.save_snapshot();
        self.last_action = LastAction::None;

        // 如果有选择，先删除选择内容
        if self.selection.is_some() {
            self.delete_selection();
        }

        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return;
        }

        let current_line = &mut self.lines[self.cursor_row];
        let before = current_line[..self.cursor_col].to_string();
        let after = current_line[self.cursor_col..].to_string();

        if lines.len() == 1 {
            // 单行插入
            current_line.clear();
            current_line.push_str(&before);
            current_line.push_str(lines[0]);
            current_line.push_str(&after);
            self.cursor_col = before.len() + lines[0].len();
        } else {
            // 多行插入
            let mut new_lines = Vec::new();

            // 第一行
            let mut first = before.clone();
            first.push_str(lines[0]);
            new_lines.push(first);

            // 中间行
            for &line in &lines[1..lines.len() - 1] {
                new_lines.push(line.to_string());
            }

            // 最后一行
            let mut last = lines.last().unwrap().to_string();
            last.push_str(&after);
            new_lines.push(last);

            // 替换当前行
            self.lines.splice(self.cursor_row..=self.cursor_row, new_lines);

            self.cursor_row += lines.len() - 1;
            self.cursor_col = lines.last().unwrap().len();
        }

        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    /// 删除光标前的字符（Backspace）
    pub fn delete_char_before(&mut self) {
        if self.config.read_only {
            return;
        }

        // 如果有选择，删除选择内容
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }

        if self.cursor_col > 0 {
            self.save_snapshot();
            self.last_action = LastAction::None;

            let line = &mut self.lines[self.cursor_row];
            let char_idx = line.char_indices().nth(
                line[..self.cursor_col].chars().count().saturating_sub(1)
            );
            
            if let Some((idx, _ch)) = char_idx {
                line.remove(idx);
                self.cursor_col = idx;
            }
        } else if self.cursor_row > 0 {
            // 合并到上一行
            self.save_snapshot();
            self.last_action = LastAction::None;

            let current_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_line = &mut self.lines[self.cursor_row];
            self.cursor_col = prev_line.len();
            prev_line.push_str(&current_line);
        }

        self.ensure_cursor_valid();
        self.needs_render = true;
        self.update_autocomplete();
    }

    /// 删除光标后的字符（Delete）
    pub fn delete_char_after(&mut self) {
        if self.config.read_only {
            return;
        }

        // 如果有选择，删除选择内容
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }

        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            self.save_snapshot();
            self.last_action = LastAction::None;

            let line = &mut self.lines[self.cursor_row];
            let char_idx = line[self.cursor_col..].char_indices().nth(1);
            
            if let Some((idx, _)) = char_idx {
                line.drain(self.cursor_col..self.cursor_col + idx);
            } else {
                line.truncate(self.cursor_col);
            }
        } else if self.cursor_row < self.lines.len() - 1 {
            // 合并下一行
            self.save_snapshot();
            self.last_action = LastAction::None;

            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
        }

        self.ensure_cursor_valid();
        self.needs_render = true;
        self.update_autocomplete();
    }

    /// 删除整行
    pub fn delete_line(&mut self) {
        if self.config.read_only {
            return;
        }

        self.save_snapshot();
        self.last_action = LastAction::None;

        if self.lines.len() > 1 {
            self.lines.remove(self.cursor_row);
            if self.cursor_row >= self.lines.len() {
                self.cursor_row = self.lines.len() - 1;
            }
        } else {
            self.lines[0].clear();
        }
        self.cursor_col = 0;

        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    /// 插入新行（Enter）
    pub fn new_line(&mut self) {
        if self.config.read_only {
            return;
        }

        self.save_snapshot();
        self.last_action = LastAction::None;

        // 检查最大行数限制
        if let Some(max_lines) = self.config.max_lines {
            if self.lines.len() >= max_lines {
                return;
            }
        }

        let line = &mut self.lines[self.cursor_row];
        let after = line.split_off(self.cursor_col);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, after);
        self.cursor_col = 0;

        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    // === 光标移动 ===

    /// 向左移动
    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            let char_idx = line[..self.cursor_col].char_indices().nth(
                line[..self.cursor_col].chars().count().saturating_sub(1)
            );
            if let Some((idx, _)) = char_idx {
                self.cursor_col = idx;
            }
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 向右移动
    pub fn move_right(&mut self) {
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            let char_idx = line[self.cursor_col..].char_indices().nth(1);
            if let Some((idx, _)) = char_idx {
                self.cursor_col += idx;
            } else {
                self.cursor_col = line.len();
            }
        } else if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 向上移动
    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.ensure_cursor_valid();
        }
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 向下移动
    pub fn move_down(&mut self) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.ensure_cursor_valid();
        }
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 移动到行首
    pub fn move_home(&mut self) {
        self.cursor_col = 0;
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 移动到行尾
    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].len();
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 向左移动一个单词
    pub fn move_word_left(&mut self) {
        let line = &self.lines[self.cursor_row];
        let text_before = &line[..self.cursor_col];

        // 跳过尾部空白
        let mut new_col = self.cursor_col;
        for (idx, ch) in text_before.char_indices().rev() {
            if !ch.is_whitespace() {
                new_col = idx + ch.len_utf8();
                break;
            }
            new_col = idx;
        }

        // 跳过单词
        let mut found_word = false;
        for (idx, ch) in text_before[..new_col].char_indices().rev() {
            if ch.is_alphanumeric() {
                found_word = true;
            } else if found_word {
                new_col = idx + ch.len_utf8();
                break;
            }
            new_col = idx;
        }

        self.cursor_col = new_col;
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 向右移动一个单词
    pub fn move_word_right(&mut self) {
        let line = &self.lines[self.cursor_row];
        let text_after = &line[self.cursor_col..];

        // 跳过前导空白
        let mut new_col = self.cursor_col;
        for (idx, ch) in text_after.char_indices() {
            if !ch.is_whitespace() {
                break;
            }
            new_col = self.cursor_col + idx + ch.len_utf8();
        }

        // 跳过单词
        let mut found_word = false;
        for (idx, ch) in line[new_col..].char_indices() {
            if ch.is_alphanumeric() {
                found_word = true;
            } else if found_word {
                break;
            }
            new_col = self.cursor_col + idx + ch.len_utf8();
        }

        self.cursor_col = new_col;
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 移动到文档开头
    pub fn move_to_start(&mut self) {
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    /// 移动到文档结尾
    pub fn move_to_end(&mut self) {
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_row].len();
        self.selection = None;
        self.last_action = LastAction::None;
        self.needs_render = true;
    }

    // === 选择 ===

    /// 全选
    pub fn select_all(&mut self) {
        let last_row = self.lines.len() - 1;
        self.selection = Some(Selection::new(
            0,
            0,
            last_row,
            self.lines[last_row].len(),
        ));
        self.needs_render = true;
    }

    /// 获取选中的文本
    pub fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let sel = sel.normalized();

        if sel.is_empty() {
            return None;
        }

        let mut result = Vec::new();
        for row in sel.start_row..=sel.end_row {
            let line = &self.lines[row];
            let start = if row == sel.start_row { sel.start_col } else { 0 };
            let end = if row == sel.end_row { sel.end_col } else { line.len() };
            result.push(&line[start..end]);
        }

        Some(result.join("\n"))
    }

    /// 删除选择的内容
    pub fn delete_selection(&mut self) -> bool {
        let sel = match self.selection {
            Some(s) => s.normalized(),
            None => return false,
        };

        if sel.is_empty() {
            self.selection = None;
            return false;
        }

        self.save_snapshot();
        self.last_action = LastAction::None;

        if sel.start_row == sel.end_row {
            // 单行选择
            let line = &mut self.lines[sel.start_row];
            line.drain(sel.start_col..sel.end_col);
            self.cursor_row = sel.start_row;
            self.cursor_col = sel.start_col;
        } else {
            // 多行选择
            let first_line = &self.lines[sel.start_row][..sel.start_col].to_string();
            let last_line = &self.lines[sel.end_row][sel.end_col..].to_string();

            let mut new_line = first_line.clone();
            new_line.push_str(last_line);

            self.lines.splice(sel.start_row..=sel.end_row, vec![new_line]);
            self.cursor_row = sel.start_row;
            self.cursor_col = sel.start_col;
        }

        self.selection = None;
        self.ensure_cursor_valid();
        self.needs_render = true;
        true
    }

    // === 撤销/重做 ===

    /// 撤销
    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.undo() {
            self.lines = snapshot.lines.clone();
            self.cursor_row = snapshot.cursor_row;
            self.cursor_col = snapshot.cursor_col;
            self.selection = None;
            self.last_action = LastAction::None;
            self.needs_render = true;
        }
    }

    /// 重做
    pub fn redo(&mut self) {
        if let Some(snapshot) = self.undo_stack.redo() {
            self.lines = snapshot.lines.clone();
            self.cursor_row = snapshot.cursor_row;
            self.cursor_col = snapshot.cursor_col;
            self.selection = None;
            self.last_action = LastAction::None;
            self.needs_render = true;
        }
    }

    /// 保存当前状态快照
    fn save_snapshot(&mut self) {
        self.undo_stack.push(EditorSnapshot {
            lines: self.lines.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        });
    }

    // === Kill ring ===

    /// 删除到行尾（Ctrl+K）
    pub fn kill_line(&mut self) {
        if self.config.read_only {
            return;
        }

        self.save_snapshot();

        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            let killed = line[self.cursor_col..].to_string();
            self.lines[self.cursor_row].truncate(self.cursor_col);
            
            self.kill_ring.push(
                killed,
                PushOptions::new(false, self.last_action == LastAction::Kill),
            );
            self.last_action = LastAction::Kill;
        } else if self.cursor_row < self.lines.len() - 1 {
            // 删除换行符
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
            
            self.kill_ring.push(
                "\n".to_string(),
                PushOptions::new(false, self.last_action == LastAction::Kill),
            );
            self.last_action = LastAction::Kill;
        }

        self.needs_render = true;
    }

    /// 粘贴（Ctrl+Y）
    pub fn yank(&mut self) {
        if self.config.read_only {
            return;
        }

        let text = self.kill_ring.yank().map(|s| s.to_string());
        if let Some(ref text) = text {
            self.save_snapshot();
            self.insert_text_internal(&text);
            self.last_action = LastAction::Yank;
            self.needs_render = true;
        }
    }

    /// 循环粘贴（Alt+Y）
    pub fn yank_pop(&mut self) {
        if self.config.read_only || self.last_action != LastAction::Yank {
            return;
        }

        let text = self.kill_ring.yank_pop().map(|s| s.to_string());
        if let Some(ref text) = text {
            // 简单实现：撤销上一次的 yank 然后插入新的
            // 实际应该记录 yank 的位置和长度来精确替换
            self.insert_text_internal(&text);
            self.needs_render = true;
        }
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

    /// 接受当前自动完成建议
    pub fn accept_autocomplete(&mut self) {
        let insert_text = self.autocomplete_suggestions.as_ref().and_then(|suggestions| {
            if self.autocomplete_index < suggestions.items.len() {
                let item = &suggestions.items[self.autocomplete_index];
                Some((item.get_insert_text().to_string(), suggestions.prefix.len()))
            } else {
                None
            }
        });

        if let Some((insert_text, prefix_len)) = insert_text {
            self.save_snapshot();
            self.last_action = LastAction::None;

            // 替换前缀
            let start = self.cursor_col.saturating_sub(prefix_len);
            let line = &mut self.lines[self.cursor_row];
            line.replace_range(start..self.cursor_col, &insert_text);
            self.cursor_col = start + insert_text.len();

            self.dismiss_autocomplete();
            self.needs_render = true;
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

    /// 更新自动完成建议
    fn update_autocomplete(&mut self) {
        if let Some(provider) = &self.autocomplete_provider {
            let cursor_pos = self.lines[..self.cursor_row]
                .iter()
                .map(|l| l.len() + 1) // +1 for newline
                .sum::<usize>()
                + self.cursor_col;
            
            let input = self.get_text();
            
            if let Some(suggestions) = provider.provide(&input, cursor_pos) {
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

    // === 粘贴 ===

    /// 粘贴文本
    pub fn paste(&mut self, text: &str) {
        if self.config.read_only {
            return;
        }

        self.save_snapshot();
        self.last_action = LastAction::None;

        // 处理大粘贴（> 10 行或 > 1000 字符）
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() > 10 || text.len() > 1000 {
            self.paste_counter += 1;
            let marker = if lines.len() > 10 {
                format!("[paste #{} +{} lines]", self.paste_counter, lines.len())
            } else {
                format!("[paste #{} {} chars]", self.paste_counter, text.len())
            };
            self.insert_text_internal(&marker);
        } else {
            // 清理粘贴文本
            let cleaned = text.replace('\t', "    ");
            self.insert_text_internal(&cleaned);
        }

        self.needs_render = true;
    }

    // === 查询 ===

    /// 获取光标位置 (row, col)
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// 获取行数
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// 检查是否有修改（相对于空状态）
    pub fn is_modified(&self) -> bool {
        !self.is_empty() || self.undo_stack.can_undo()
    }

    // === 内部辅助 ===

    /// 确保光标位置有效
    fn ensure_cursor_valid(&mut self) {
        if self.cursor_row >= self.lines.len() {
            self.cursor_row = self.lines.len() - 1;
        }
        
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// 确保光标可见
    fn ensure_visible(&mut self, height: u16) {
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + height as usize {
            self.scroll_row = self.cursor_row.saturating_sub(height as usize - 1);
        }
    }

    /// 获取当前行
    fn current_line(&self) -> &str {
        &self.lines[self.cursor_row]
    }

    /// 获取当前行的可变引用
    fn current_line_mut(&mut self) -> &mut String {
        &mut self.lines[self.cursor_row]
    }

    /// 内部插入文本（不保存快照）
    fn insert_text_internal(&mut self, text: &str) {
        let lines: Vec<&str> = text.lines().collect();
        
        if lines.is_empty() {
            return;
        }

        let current_line = &mut self.lines[self.cursor_row];
        let before = current_line[..self.cursor_col].to_string();
        let after = current_line[self.cursor_col..].to_string();

        if lines.len() == 1 {
            current_line.clear();
            current_line.push_str(&before);
            current_line.push_str(lines[0]);
            current_line.push_str(&after);
            self.cursor_col = before.len() + lines[0].len();
        } else {
            let mut new_lines = Vec::new();
            
            let mut first = before.clone();
            first.push_str(lines[0]);
            new_lines.push(first);
            
            for &line in &lines[1..lines.len() - 1] {
                new_lines.push(line.to_string());
            }
            
            let mut last = lines.last().unwrap().to_string();
            last.push_str(&after);
            new_lines.push(last);

            self.lines.splice(self.cursor_row..=self.cursor_row, new_lines);
            self.cursor_row += lines.len() - 1;
            self.cursor_col = lines.last().unwrap().len();
        }

        self.ensure_cursor_valid();
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
            lines.push(format!("{}", "─".repeat(width)));

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
            lines.push(format!("{}", "─".repeat(width)));
        }

        lines
    }
}

impl Component for Editor {
    fn render(&self, width: u16) -> Vec<String> {
        let mut lines = Vec::new();
        let content_width = if self.config.line_numbers {
            width.saturating_sub(4) as usize
        } else {
            width as usize
        };

        // 计算可见区域
        let visible_height = 10; // 默认可见高度
        let start_row = self.scroll_row;
        let end_row = (self.scroll_row + visible_height).min(self.lines.len());

        // 渲染每一行
        for row in start_row..end_row {
            let line = &self.lines[row];
            let is_current_line = row == self.cursor_row;

            // 行号
            let line_num = if self.config.line_numbers {
                format!("{:3} ", row + 1)
            } else {
                String::new()
            };

            // 处理自动换行
            let wrapped = if self.config.wrap {
                wrap_text_with_ansi(line, content_width)
            } else {
                vec![line.clone()]
            };

            for (wrap_idx, wrap_line) in wrapped.iter().enumerate() {
                let mut display_line = line_num.clone();

                // 如果是当前行且光标在此换行段
                if is_current_line {
                    let cursor_in_this_wrap = if self.config.wrap {
                        let chars_before: usize = wrapped[..wrap_idx]
                            .iter()
                            .map(|s| visible_width(s))
                            .sum();
                        self.cursor_col >= chars_before
                            && self.cursor_col < chars_before + visible_width(wrap_line)
                    } else {
                        wrap_idx == 0
                    };

                    if cursor_in_this_wrap || (wrap_idx == wrapped.len() - 1 && self.cursor_col >= wrap_line.len()) {
                        // 计算光标在换行段中的位置
                        let col_in_wrap = if wrap_idx == 0 {
                            self.cursor_col
                        } else {
                            let chars_before: usize = wrapped[..wrap_idx]
                                .iter()
                                .map(|s| s.chars().count())
                                .sum();
                            self.cursor_col - chars_before
                        };

                        // 插入光标标记
                        let before = &wrap_line[..col_in_wrap.min(wrap_line.len())];
                        let after = if col_in_wrap < wrap_line.len() {
                            let char_idx = wrap_line.char_indices().nth(
                                wrap_line[..col_in_wrap].chars().count()
                            );
                            if let Some((idx, ch)) = char_idx {
                                format!("\x1b[7m{}\x1b[0m{}", ch, &wrap_line[idx + ch.len_utf8()..])
                            } else {
                                format!("\x1b[7m \x1b[0m")
                            }
                        } else {
                            format!("\x1b[7m \x1b[0m")
                        };

                        display_line.push_str(before);
                        display_line.push_str(CURSOR_MARKER);
                        display_line.push_str(&after);
                    } else {
                        display_line.push_str(wrap_line);
                    }
                } else {
                    display_line.push_str(wrap_line);
                }

                lines.push(display_line);
            }
        }

        // 显示占位符
        if self.is_empty() && self.config.placeholder.is_some() && !self.focused {
            let placeholder = self.config.placeholder.as_ref().unwrap();
            let line = format!("\x1b[90m{}\x1b[0m", placeholder);
            lines.push(line);
        }

        // 添加自动完成弹出菜单
        if self.autocomplete_suggestions.is_some() {
            let autocomplete_lines = self.render_autocomplete(content_width);
            lines.extend(autocomplete_lines);
        }

        lines
    }

    fn handle_input(&mut self, data: &str) -> bool {
        // 基本按键处理
        match data {
            // 字符输入
            _ if data.len() == 1 && data.as_bytes()[0] >= 32 && data.as_bytes()[0] < 127 => {
                self.insert_char(data.chars().next().unwrap());
                true
            }
            // 回车
            "\r" | "\n" | "\r\n" => {
                self.new_line();
                true
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
                self.move_left();
                true
            }
            "\x1b[C" => {
                // Right
                self.move_right();
                true
            }
            "\x1b[A" => {
                // Up
                if self.autocomplete_suggestions.is_some() {
                    self.prev_autocomplete();
                } else {
                    self.move_up();
                }
                true
            }
            "\x1b[B" => {
                // Down
                if self.autocomplete_suggestions.is_some() {
                    self.next_autocomplete();
                } else {
                    self.move_down();
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
            // Ctrl+A (全选)
            "\x01" => {
                self.select_all();
                true
            }
            // Ctrl+K (删除到行尾)
            "\x0b" => {
                self.kill_line();
                true
            }
            // Ctrl+Y (粘贴)
            "\x19" => {
                self.yank();
                true
            }
            // Ctrl+Z (撤销)
            "\x1a" => {
                self.undo();
                true
            }
            // Tab (接受自动完成)
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

impl Focusable for Editor {
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
    fn test_editor_new() {
        let editor = Editor::new(EditorConfig::default());
        assert!(editor.is_empty());
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.cursor_position(), (0, 0));
    }

    #[test]
    fn test_editor_insert() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_char('h');
        editor.insert_char('i');
        assert_eq!(editor.get_text(), "hi");
    }

    #[test]
    fn test_editor_new_line() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_text("hello");
        editor.new_line();
        editor.insert_text("world");
        assert_eq!(editor.get_text(), "hello\nworld");
    }

    #[test]
    fn test_editor_delete() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_text("hello");
        editor.delete_char_before();
        assert_eq!(editor.get_text(), "hell");
    }

    #[test]
    fn test_editor_movement() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_text("hello");
        editor.move_left();
        assert_eq!(editor.cursor_position(), (0, 4));
        editor.move_home();
        assert_eq!(editor.cursor_position(), (0, 0));
        editor.move_end();
        assert_eq!(editor.cursor_position(), (0, 5));
    }

    #[test]
    fn test_editor_undo() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_text("hello");
        editor.undo();
        assert!(editor.is_empty());
    }

    #[test]
    fn test_editor_selection() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_text("hello world");
        editor.select_all();
        assert_eq!(editor.get_selected_text(), Some("hello world".to_string()));
    }

    #[test]
    fn test_editor_kill_ring() {
        let mut editor = Editor::new(EditorConfig::default());
        editor.insert_text("hello world");
        editor.move_home();
        editor.move_word_right();
        editor.kill_line();
        assert_eq!(editor.get_text(), "hello ");
        editor.move_end();
        editor.yank();
        assert_eq!(editor.get_text(), "hello world");
    }
}
