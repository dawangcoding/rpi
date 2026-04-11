//! Vim 命令处理
//!
//! 实现 Vim 模式下的移动、编辑、搜索等命令

use super::Editor;
use super::vim::{VimMode, VimCommand, RegisterContent};

/// Vim 输入处理结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimAction {
    /// 已处理，无需外部操作
    Handled,
    /// 需要提交内容（:w 或 :wq）
    Submit,
    /// 需要取消（:q）
    Cancel,
    /// 未处理
    NotHandled,
}

impl Editor {
    /// 处理 Vim 模式下的输入
    pub fn handle_vim_input(&mut self, data: &str) -> bool {
        let vim = match self.vim_state.as_ref() {
            Some(v) => v,
            None => return false,
        };

        let mode = vim.mode;
        let result = match mode {
            VimMode::Normal => self.handle_vim_normal(data),
            VimMode::Insert => self.handle_vim_insert(data),
            VimMode::Visual | VimMode::VisualLine => self.handle_vim_visual(data),
            VimMode::Command => self.handle_vim_command(data),
            VimMode::Search => self.handle_vim_search(data),
        };

        matches!(result, VimAction::Handled | VimAction::Submit | VimAction::Cancel)
    }

    /// Normal 模式处理
    fn handle_vim_normal(&mut self, data: &str) -> VimAction {
        // 检查是否在等待替换字符（r 命令）
        if let Some(ref vim) = self.vim_state {
            if vim.waiting_for_char {
                return self.handle_vim_replace_char(data);
            }
        }

        // 尝试编辑命令（x, r, dd, yy, p, u, . 等）
        // 这会先处理 pending_keys 中的多键命令
        if let Some(action) = self.handle_vim_edit_command(data) {
            return action;
        }

        // 处理 gg（移动到文件开头）
        if let Some(ref mut vim) = self.vim_state {
            if vim.pending_keys == "g" && data == "g" {
                vim.clear_pending();
                self.move_to_start();
                self.needs_render = true;
                return VimAction::Handled;
            }
            // g 开始的组合
            if data == "g" {
                vim.pending_keys.push('g');
                return VimAction::Handled;
            }
        }

        match data {
            // 模式切换
            "i" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Insert);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            "a" => {
                // 光标后进入 Insert
                self.move_right_no_wrap();
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Insert);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            "o" => {
                // 下方新建行进入 Insert
                self.move_end();
                self.new_line();
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Insert);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            "O" => {
                // 上方新建行进入 Insert
                self.move_home();
                self.new_line();
                self.move_up();
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Insert);
                }
                self.needs_render = true;
                VimAction::Handled
            }

            // 移动命令
            "h" => { self.vim_move_left(); VimAction::Handled }
            "j" => { self.move_down(); self.vim_clamp_cursor(); VimAction::Handled }
            "k" => { self.move_up(); self.vim_clamp_cursor(); VimAction::Handled }
            "l" => { self.vim_move_right(); VimAction::Handled }
            "w" => { self.vim_move_word_forward(); VimAction::Handled }
            "b" => { self.move_word_left(); self.vim_clamp_cursor(); VimAction::Handled }
            "e" => { self.vim_move_word_end(); VimAction::Handled }
            "0" => { self.move_home(); VimAction::Handled }
            "$" => { self.vim_move_to_line_end(); VimAction::Handled }
            "G" => { self.move_to_end(); self.vim_clamp_cursor(); VimAction::Handled }
            // Ctrl+U (半屏上滚)
            "\x15" => { self.vim_scroll_half_page_up(); VimAction::Handled }
            // Ctrl+D (半屏下滚)
            "\x04" => { self.vim_scroll_half_page_down(); VimAction::Handled }

            // 方向键也应该工作
            "\x1b[D" => { self.vim_move_left(); VimAction::Handled }
            "\x1b[B" => { self.move_down(); self.vim_clamp_cursor(); VimAction::Handled }
            "\x1b[A" => { self.move_up(); self.vim_clamp_cursor(); VimAction::Handled }
            "\x1b[C" => { self.vim_move_right(); VimAction::Handled }

            // 进入 Visual 模式
            "v" => {
                // 进入字符 Visual 模式
                if let Some(ref mut vim) = self.vim_state {
                    vim.visual_start = Some((self.cursor_row, self.cursor_col));
                    vim.switch_mode(VimMode::Visual);
                }
                // 设置初始选择
                self.selection = Some(super::Selection::new(
                    self.cursor_row, self.cursor_col,
                    self.cursor_row, self.cursor_col,
                ));
                self.needs_render = true;
                VimAction::Handled
            }
            "V" => {
                // 进入行 Visual 模式
                if let Some(ref mut vim) = self.vim_state {
                    vim.visual_start = Some((self.cursor_row, 0));
                    vim.switch_mode(VimMode::VisualLine);
                }
                // 选择整行
                let line_len = self.lines[self.cursor_row].len();
                self.selection = Some(super::Selection::new(
                    self.cursor_row, 0,
                    self.cursor_row, line_len,
                ));
                self.needs_render = true;
                VimAction::Handled
            }

            // Command 模式入口 (:)
            ":" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Command);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            // Search 模式入口 (/)
            "/" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Search);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            // 搜索跳转 - 下一个匹配
            "n" => {
                self.vim_search_next();
                VimAction::Handled
            }
            // 搜索跳转 - 上一个匹配
            "N" => {
                self.vim_search_prev();
                VimAction::Handled
            }
            // 其他命令将在 Task 4/5 中实现
            _ => VimAction::NotHandled,
        }
    }

    /// Insert 模式处理
    fn handle_vim_insert(&mut self, data: &str) -> VimAction {
        match data {
            // Escape 返回 Normal
            "\x1b" | "\x1b\x1b" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Normal);
                }
                // Normal 模式下光标不应超过行尾
                self.vim_clamp_cursor();
                self.needs_render = true;
                VimAction::Handled
            }
            // Insert 模式下的按键大部分委托给原有 Emacs 逻辑
            _ => {
                // 记录输入到 insert_text_buffer 用于 . 重复
                if let Some(ref mut vim) = self.vim_state {
                    if data.len() == 1 && data.as_bytes()[0] >= 32 {
                        vim.insert_text_buffer.push_str(data);
                    }
                }
                // 委托给默认输入处理
                if self.handle_emacs_input(data) {
                    VimAction::Handled
                } else {
                    VimAction::NotHandled
                }
            }
        }
    }

    /// Visual 模式处理
    fn handle_vim_visual(&mut self, data: &str) -> VimAction {
        let is_line_mode = self.vim_state.as_ref()
            .map(|v| v.mode == VimMode::VisualLine)
            .unwrap_or(false);

        match data {
            // Escape 取消选择
            "\x1b" | "\x1b\x1b" => {
                self.selection = None;
                if let Some(ref mut vim) = self.vim_state {
                    vim.visual_start = None;
                    vim.switch_mode(VimMode::Normal);
                }
                self.needs_render = true;
                VimAction::Handled
            }

            // 移动命令（扩展选择）
            "h" => { self.vim_visual_move_left(); VimAction::Handled }
            "j" => { self.vim_visual_move_down(is_line_mode); VimAction::Handled }
            "k" => { self.vim_visual_move_up(is_line_mode); VimAction::Handled }
            "l" => { self.vim_visual_move_right(); VimAction::Handled }
            "w" => { self.vim_move_word_forward(); self.vim_update_visual_selection(is_line_mode); VimAction::Handled }
            "b" => { self.move_word_left(); self.vim_update_visual_selection(is_line_mode); VimAction::Handled }
            "e" => { self.vim_move_word_end(); self.vim_update_visual_selection(is_line_mode); VimAction::Handled }
            "0" => { self.move_home(); self.vim_update_visual_selection(is_line_mode); VimAction::Handled }
            "$" => { self.vim_move_to_line_end(); self.vim_update_visual_selection(is_line_mode); VimAction::Handled }
            "G" => { self.move_to_end(); self.vim_update_visual_selection(is_line_mode); VimAction::Handled }
            "g" => {
                if let Some(ref mut vim) = self.vim_state {
                    if vim.pending_keys == "g" {
                        vim.clear_pending();
                        self.move_to_start();
                        self.vim_update_visual_selection(is_line_mode);
                        return VimAction::Handled;
                    }
                    vim.pending_keys.push('g');
                }
                VimAction::Handled
            }

            // 方向键
            "\x1b[D" => { self.vim_visual_move_left(); VimAction::Handled }
            "\x1b[B" => { self.vim_visual_move_down(is_line_mode); VimAction::Handled }
            "\x1b[A" => { self.vim_visual_move_up(is_line_mode); VimAction::Handled }
            "\x1b[C" => { self.vim_visual_move_right(); VimAction::Handled }

            // 操作命令
            "d" => {
                // 删除选择内容
                self.vim_visual_delete();
                VimAction::Handled
            }
            "y" => {
                // 复制选择内容
                self.vim_visual_yank();
                VimAction::Handled
            }
            ">" => {
                // 缩进选择行
                self.vim_visual_indent();
                VimAction::Handled
            }
            "<" => {
                // 反缩进选择行
                self.vim_visual_outdent();
                VimAction::Handled
            }

            // 切换 Visual 类型
            "v" => {
                if is_line_mode {
                    // 从 VisualLine 切换到 Visual
                    if let Some(ref mut vim) = self.vim_state {
                        vim.mode = VimMode::Visual;
                    }
                    self.vim_update_visual_selection(false);
                } else {
                    // 在 Visual 模式再按 v 退出
                    self.selection = None;
                    if let Some(ref mut vim) = self.vim_state {
                        vim.visual_start = None;
                        vim.switch_mode(VimMode::Normal);
                    }
                }
                self.needs_render = true;
                VimAction::Handled
            }
            "V" => {
                if !is_line_mode {
                    // 从 Visual 切换到 VisualLine
                    if let Some(ref mut vim) = self.vim_state {
                        vim.mode = VimMode::VisualLine;
                    }
                    self.vim_update_visual_selection(true);
                } else {
                    // 在 VisualLine 模式再按 V 退出
                    self.selection = None;
                    if let Some(ref mut vim) = self.vim_state {
                        vim.visual_start = None;
                        vim.switch_mode(VimMode::Normal);
                    }
                }
                self.needs_render = true;
                VimAction::Handled
            }

            _ => VimAction::NotHandled,
        }
    }

    /// Command 模式处理（: 命令行）
    fn handle_vim_command(&mut self, data: &str) -> VimAction {
        match data {
            // Escape 取消
            "\x1b" | "\x1b\x1b" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.switch_mode(VimMode::Normal);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            // Enter 执行命令
            "\r" | "\n" | "\r\n" => {
                let cmd = if let Some(ref vim) = self.vim_state {
                    vim.command_line.trim().to_string()
                } else {
                    String::new()
                };

                let result = match cmd.as_str() {
                    "w" => {
                        if let Some(ref mut vim) = self.vim_state {
                            vim.status_message = "Written".to_string();
                            vim.switch_mode(VimMode::Normal);
                        }
                        VimAction::Submit
                    }
                    "q" | "q!" => {
                        if let Some(ref mut vim) = self.vim_state {
                            vim.switch_mode(VimMode::Normal);
                        }
                        VimAction::Cancel
                    }
                    "wq" | "x" => {
                        if let Some(ref mut vim) = self.vim_state {
                            vim.switch_mode(VimMode::Normal);
                        }
                        VimAction::Submit
                    }
                    _ => {
                        if let Some(ref mut vim) = self.vim_state {
                            vim.status_message = format!("E492: Not an editor command: {}", cmd);
                            vim.switch_mode(VimMode::Normal);
                        }
                        VimAction::Handled
                    }
                };

                self.needs_render = true;
                result
            }
            // Backspace 删除字符
            "\x7f" | "\x08" => {
                if let Some(ref mut vim) = self.vim_state {
                    if vim.command_line.is_empty() {
                        // 命令行为空时 Backspace 退出命令模式
                        vim.switch_mode(VimMode::Normal);
                    } else {
                        vim.command_line.pop();
                    }
                }
                self.needs_render = true;
                VimAction::Handled
            }
            // 普通字符输入
            _ if data.len() == 1 && data.as_bytes()[0] >= 32 && data.as_bytes()[0] < 127 => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.command_line.push_str(data);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            _ => VimAction::Handled,
        }
    }

    /// Search 模式处理（/ 搜索输入）
    fn handle_vim_search(&mut self, data: &str) -> VimAction {
        match data {
            // Escape 取消搜索
            "\x1b" | "\x1b\x1b" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.search_input.clear();
                    vim.switch_mode(VimMode::Normal);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            // Enter 执行搜索
            "\r" | "\n" | "\r\n" => {
                let pattern = if let Some(ref vim) = self.vim_state {
                    if vim.search_input.is_empty() {
                        // 空搜索使用上次的模式
                        vim.search_pattern.clone()
                    } else {
                        Some(vim.search_input.clone())
                    }
                } else {
                    None
                };

                if let Some(pattern) = pattern {
                    // 执行搜索
                    self.vim_execute_search(&pattern);

                    if let Some(ref mut vim) = self.vim_state {
                        vim.search_pattern = Some(pattern);
                        vim.search_input.clear();
                        vim.switch_mode(VimMode::Normal);
                    }
                } else {
                    if let Some(ref mut vim) = self.vim_state {
                        vim.switch_mode(VimMode::Normal);
                    }
                }

                self.needs_render = true;
                VimAction::Handled
            }
            // Backspace
            "\x7f" | "\x08" => {
                if let Some(ref mut vim) = self.vim_state {
                    if vim.search_input.is_empty() {
                        vim.switch_mode(VimMode::Normal);
                    } else {
                        vim.search_input.pop();
                    }
                }
                self.needs_render = true;
                VimAction::Handled
            }
            // 普通字符
            _ if data.len() == 1 && data.as_bytes()[0] >= 32 && data.as_bytes()[0] < 127 => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.search_input.push_str(data);
                }
                self.needs_render = true;
                VimAction::Handled
            }
            _ => VimAction::Handled,
        }
    }

    /// r 命令的替换字符处理
    fn handle_vim_replace_char(&mut self, data: &str) -> VimAction {
        if let Some(ref mut vim) = self.vim_state {
            vim.waiting_for_char = false;
        }

        if data.len() == 1 {
            let ch = data.chars().next().unwrap();
            let should_replace = {
                let line = &self.lines[self.cursor_row];
                !line.is_empty() && self.cursor_col < line.len()
            };
            if should_replace {
                self.save_snapshot();
                let line = &self.lines[self.cursor_row];
                let old_ch = line[self.cursor_col..].chars().next().unwrap();
                let line = &mut self.lines[self.cursor_row];
                line.replace_range(
                    self.cursor_col..self.cursor_col + old_ch.len_utf8(),
                    &ch.to_string()
                );

                if let Some(ref mut vim) = self.vim_state {
                    vim.last_command = Some(VimCommand::ReplaceChar(ch));
                }
                self.needs_render = true;
            }
        }
        VimAction::Handled
    }

    /// 处理 Normal 模式下的编辑命令
    /// 返回 Some(VimAction) 如果命令被处理，None 如果不是编辑命令
    fn handle_vim_edit_command(&mut self, data: &str) -> Option<VimAction> {
        // 检查 pending_keys 处理多键命令（dd, yy）
        if let Some(ref vim) = self.vim_state {
            if !vim.pending_keys.is_empty() {
                let pending = vim.pending_keys.clone();

                // gg 由 handle_vim_normal 处理，这里跳过
                if pending == "g" {
                    return None;
                }

                // dd - 删除当前行
                if pending == "d" && data == "d" {
                    if let Some(ref mut vim) = self.vim_state {
                        vim.clear_pending();
                        let line_content = self.lines[self.cursor_row].clone();
                        vim.register = Some(RegisterContent::Lines(line_content));
                        vim.last_command = Some(VimCommand::DeleteLine);
                    }
                    self.delete_line();
                    self.vim_clamp_cursor();
                    return Some(VimAction::Handled);
                }

                // yy - 复制当前行
                if pending == "y" && data == "y" {
                    if let Some(ref mut vim) = self.vim_state {
                        vim.clear_pending();
                        let line_content = self.lines[self.cursor_row].clone();
                        vim.register = Some(RegisterContent::Lines(line_content));
                        vim.last_command = Some(VimCommand::YankLine);
                        vim.status_message = "1 line yanked".to_string();
                    }
                    self.needs_render = true;
                    return Some(VimAction::Handled);
                }

                // 其他未知的 pending 组合，清空
                if let Some(ref mut vim) = self.vim_state {
                    vim.clear_pending();
                }
                return None;
            }
        }

        match data {
            // x - 删除光标处字符
            "x" => {
                let should_delete = {
                    let line = &self.lines[self.cursor_row];
                    !line.is_empty() && self.cursor_col < line.len()
                };
                if should_delete {
                    // 获取要删除的字符
                    let line = &self.lines[self.cursor_row];
                    let ch = line[self.cursor_col..].chars().next().unwrap();
                    let deleted = ch.to_string();

                    // 存入 register
                    if let Some(ref mut vim) = self.vim_state {
                        vim.register = Some(RegisterContent::Chars(deleted));
                        vim.last_command = Some(VimCommand::DeleteChar);
                    }

                    // 执行删除
                    self.delete_char_after();
                    self.vim_clamp_cursor();
                }
                Some(VimAction::Handled)
            }

            // r - 替换字符（设置等待状态）
            "r" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.waiting_for_char = true;
                }
                Some(VimAction::Handled)
            }

            // d - 可能是 dd 的开始
            "d" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.pending_keys.push('d');
                }
                Some(VimAction::Handled)
            }

            // y - 可能是 yy 的开始
            "y" => {
                if let Some(ref mut vim) = self.vim_state {
                    vim.pending_keys.push('y');
                }
                Some(VimAction::Handled)
            }

            // u - 撤销
            "u" => {
                self.undo();
                Some(VimAction::Handled)
            }

            // Ctrl+R - 重做
            "\x12" => {
                self.redo();
                Some(VimAction::Handled)
            }

            // p - 粘贴到光标后/下方
            "p" => {
                self.handle_vim_paste(false)
            }

            // P - 粘贴到光标前/上方
            "P" => {
                self.handle_vim_paste(true)
            }

            // . - 重复上次命令
            "." => {
                self.handle_vim_repeat_last_command();
                Some(VimAction::Handled)
            }

            // 不是编辑命令
            _ => None,
        }
    }

    /// 处理 Vim 粘贴命令
    /// before: true 表示粘贴到光标前/上方（P），false 表示后/下方（p）
    fn handle_vim_paste(&mut self, before: bool) -> Option<VimAction> {
        if let Some(ref vim) = self.vim_state {
            if let Some(ref content) = vim.register {
                let content = content.clone();
                match content {
                    RegisterContent::Lines(text) => {
                        self.save_snapshot();
                        // 按行拆分，确保每行不含换行符
                        let new_lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
                        if new_lines.is_empty() {
                            return Some(VimAction::Handled);
                        }

                        if before {
                            // P: 在当前行上方插入
                            let insert_row = self.cursor_row;
                            self.lines.splice(insert_row..insert_row, new_lines);
                            self.cursor_row = insert_row;
                        } else {
                            // p: 在当前行下方插入
                            let insert_row = self.cursor_row + 1;
                            self.lines.splice(insert_row..insert_row, new_lines);
                            self.cursor_row = insert_row;
                        }
                        self.cursor_col = 0;
                        // 跳到非空白字符
                        let line = &self.lines[self.cursor_row];
                        for (idx, ch) in line.char_indices() {
                            if !ch.is_whitespace() {
                                self.cursor_col = idx;
                                break;
                            }
                        }
                        self.needs_render = true;
                    }
                    RegisterContent::Chars(text) => {
                        self.save_snapshot();
                        if before {
                            // P: 在光标前插入
                            self.insert_text_internal(&text);
                            // 光标位置已在插入文本后，需要调整
                            if self.cursor_col > 0 {
                                self.cursor_col -= 1;
                            }
                        } else {
                            // p: 在光标后插入
                            let line = &self.lines[self.cursor_row];
                            if !line.is_empty() && self.cursor_col < line.len() {
                                let ch = line[self.cursor_col..].chars().next().unwrap();
                                self.cursor_col += ch.len_utf8();
                            }
                            self.insert_text_internal(&text);
                            // 光标移到粘贴文本的最后一个字符
                            if self.cursor_col > 0 {
                                self.cursor_col -= 1;
                            }
                        }
                        self.needs_render = true;
                    }
                }

                if let Some(ref mut vim) = self.vim_state {
                    if before {
                        vim.last_command = Some(VimCommand::PasteBefore);
                    } else {
                        vim.last_command = Some(VimCommand::PasteAfter);
                    }
                }
            }
        }
        Some(VimAction::Handled)
    }

    /// 处理 . 命令（重复上次操作）
    fn handle_vim_repeat_last_command(&mut self) {
        if let Some(ref vim) = self.vim_state {
            if let Some(cmd) = vim.last_command.clone() {
                match cmd {
                    VimCommand::DeleteLine => {
                        let line_content = self.lines[self.cursor_row].clone();
                        self.delete_line();
                        self.vim_clamp_cursor();
                        if let Some(ref mut vim) = self.vim_state {
                            vim.register = Some(RegisterContent::Lines(line_content));
                        }
                    }
                    VimCommand::DeleteChar => {
                        let should_delete = {
                            let line = &self.lines[self.cursor_row];
                            !line.is_empty() && self.cursor_col < line.len()
                        };
                        if should_delete {
                            let line = &self.lines[self.cursor_row];
                            let ch = line[self.cursor_col..].chars().next().unwrap();
                            let deleted = ch.to_string();
                            self.delete_char_after();
                            self.vim_clamp_cursor();
                            if let Some(ref mut vim) = self.vim_state {
                                vim.register = Some(RegisterContent::Chars(deleted));
                            }
                        }
                    }
                    VimCommand::ReplaceChar(ch) => {
                        // 重复替换
                        let should_replace = {
                            let line = &self.lines[self.cursor_row];
                            !line.is_empty() && self.cursor_col < line.len()
                        };
                        if should_replace {
                            self.save_snapshot();
                            let old_ch_len = {
                                let line = &self.lines[self.cursor_row];
                                let old_ch = line[self.cursor_col..].chars().next().unwrap();
                                old_ch.len_utf8()
                            };
                            let line = &mut self.lines[self.cursor_row];
                            line.replace_range(
                                self.cursor_col..self.cursor_col + old_ch_len,
                                &ch.to_string()
                            );
                            self.needs_render = true;
                        }
                    }
                    VimCommand::InsertText(text) => {
                        self.save_snapshot();
                        self.insert_text_internal(&text);
                        self.needs_render = true;
                    }
                    VimCommand::PasteAfter => {
                        self.handle_vim_paste(false);
                    }
                    VimCommand::PasteBefore => {
                        self.handle_vim_paste(true);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Vim 模式下光标不应超出行尾（Normal 模式光标最多在最后一个字符上）
    fn vim_clamp_cursor(&mut self) {
        let line_len = self.lines[self.cursor_row].len();
        if line_len > 0 && self.cursor_col >= line_len {
            // 找到最后一个字符的起始位置
            let last_char_start = self.lines[self.cursor_row]
                .char_indices()
                .last()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.cursor_col = last_char_start;
        }
    }

    /// 向右移动（不跨行，用于 Vim 'a' 命令）
    fn move_right_no_wrap(&mut self) {
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            let char_idx = line[self.cursor_col..].char_indices().nth(1);
            if let Some((idx, _)) = char_idx {
                self.cursor_col += idx;
            } else {
                self.cursor_col = line.len();
            }
        }
        self.needs_render = true;
    }

    /// Vim 左移（不跨行）
    fn vim_move_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            // 找到前一个字符的起始位置
            if let Some((idx, _)) = line[..self.cursor_col].char_indices().last() {
                // 需要找到这个字符的起始位置
                self.cursor_col = idx;
            }
        }
        self.selection = None;
        self.needs_render = true;
    }

    /// Vim 右移（不跨行，Normal 模式不超过最后一个字符）
    fn vim_move_right(&mut self) {
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            if let Some((idx, _)) = line[self.cursor_col..].char_indices().nth(1) {
                let new_col = self.cursor_col + idx;
                // Normal 模式下不超过最后一个字符
                let last_char_start = line.char_indices().last().map(|(i, _)| i).unwrap_or(0);
                self.cursor_col = new_col.min(last_char_start);
            }
        }
        self.selection = None;
        self.needs_render = true;
    }

    /// Vim w 命令：移动到下一个词首
    fn vim_move_word_forward(&mut self) {
        let line = &self.lines[self.cursor_row];
        let len = line.len();
        
        if self.cursor_col >= len {
            // 如果在行尾，移到下一行首
            if self.cursor_row < self.lines.len() - 1 {
                self.cursor_row += 1;
                self.cursor_col = 0;
                // 跳过空白找到词首
                let next_line = &self.lines[self.cursor_row];
                for (idx, ch) in next_line.char_indices() {
                    if !ch.is_whitespace() {
                        self.cursor_col = idx;
                        break;
                    }
                }
            }
            self.needs_render = true;
            return;
        }

        let chars: Vec<(usize, char)> = line[self.cursor_col..].char_indices().collect();
        if chars.is_empty() {
            return;
        }

        let first_char = chars[0].1;
        let mut pos = 0;

        // 跳过当前 word（同类字符）
        if first_char.is_alphanumeric() || first_char == '_' {
            // 跳过 word 字符
            for &(idx, ch) in &chars {
                if !(ch.is_alphanumeric() || ch == '_') {
                    pos = idx;
                    break;
                }
                pos = idx + ch.len_utf8();
            }
        } else if !first_char.is_whitespace() {
            // 跳过标点/符号字符
            for &(idx, ch) in &chars {
                if ch.is_alphanumeric() || ch == '_' || ch.is_whitespace() {
                    pos = idx;
                    break;
                }
                pos = idx + ch.len_utf8();
            }
        }

        // 跳过空白
        let remaining = &line[self.cursor_col + pos..];
        for (idx, ch) in remaining.char_indices() {
            if !ch.is_whitespace() {
                self.cursor_col = self.cursor_col + pos + idx;
                self.needs_render = true;
                return;
            }
        }

        // 如果到行尾都是空白，移到下一行
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.cursor_col = 0;
            let next_line = &self.lines[self.cursor_row];
            for (idx, ch) in next_line.char_indices() {
                if !ch.is_whitespace() {
                    self.cursor_col = idx;
                    break;
                }
            }
        } else {
            self.cursor_col = len.saturating_sub(1);
        }
        self.needs_render = true;
    }

    /// Vim e 命令：移动到词尾
    fn vim_move_word_end(&mut self) {
        let line = &self.lines[self.cursor_row];
        let len = line.len();
        
        // 先前进一个字符
        let start_col = if self.cursor_col < len {
            let next = line[self.cursor_col..].char_indices().nth(1);
            match next {
                Some((idx, _)) => self.cursor_col + idx,
                None => {
                    // 当前行最后一个字符，移到下一行
                    if self.cursor_row < self.lines.len() - 1 {
                        self.cursor_row += 1;
                        self.cursor_col = 0;
                        let next_line = &self.lines[self.cursor_row];
                        // 跳过空白
                        let mut col = 0;
                        for (idx, ch) in next_line.char_indices() {
                            if !ch.is_whitespace() {
                                col = idx;
                                break;
                            }
                        }
                        // 找词尾
                        self.find_word_end_from(self.cursor_row, col);
                        self.needs_render = true;
                        return;
                    }
                    self.needs_render = true;
                    return;
                }
            }
        } else {
            if self.cursor_row < self.lines.len() - 1 {
                self.cursor_row += 1;
                0
            } else {
                return;
            }
        };

        // 跳过空白
        let line = &self.lines[self.cursor_row];
        let mut col = start_col;
        for (idx, ch) in line[start_col..].char_indices() {
            if !ch.is_whitespace() {
                col = start_col + idx;
                break;
            }
            col = start_col + idx + ch.len_utf8();
        }

        self.find_word_end_from(self.cursor_row, col);
        self.needs_render = true;
    }

    /// 从指定位置查找词尾
    fn find_word_end_from(&mut self, row: usize, col: usize) {
        let line = &self.lines[row];
        if col >= line.len() {
            self.cursor_col = line.len().saturating_sub(1);
            return;
        }

        let chars: Vec<(usize, char)> = line[col..].char_indices().collect();
        if chars.is_empty() {
            return;
        }

        let first_char = chars[0].1;
        let mut last_pos = col;

        if first_char.is_alphanumeric() || first_char == '_' {
            for &(idx, ch) in &chars {
                if !(ch.is_alphanumeric() || ch == '_') {
                    break;
                }
                last_pos = col + idx;
            }
        } else if !first_char.is_whitespace() {
            for &(idx, ch) in &chars {
                if ch.is_alphanumeric() || ch == '_' || ch.is_whitespace() {
                    break;
                }
                last_pos = col + idx;
            }
        }

        self.cursor_col = last_pos;
    }

    /// Vim $ 命令：移动到行尾最后一个字符
    fn vim_move_to_line_end(&mut self) {
        let line = &self.lines[self.cursor_row];
        if line.is_empty() {
            self.cursor_col = 0;
        } else {
            // 移到最后一个字符的位置（不是行尾之后）
            self.cursor_col = line.char_indices().last().map(|(idx, _)| idx).unwrap_or(0);
        }
        self.selection = None;
        self.needs_render = true;
    }

    /// Vim Ctrl+U：半屏向上滚动
    fn vim_scroll_half_page_up(&mut self) {
        let half_page = 5; // 默认半屏高度
        for _ in 0..half_page {
            if self.cursor_row > 0 {
                self.cursor_row -= 1;
            } else {
                break;
            }
        }
        self.vim_clamp_cursor();
        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    /// Vim Ctrl+D：半屏向下滚动
    fn vim_scroll_half_page_down(&mut self) {
        let half_page = 5; // 默认半屏高度
        let max_row = self.lines.len().saturating_sub(1);
        for _ in 0..half_page {
            if self.cursor_row < max_row {
                self.cursor_row += 1;
            } else {
                break;
            }
        }
        self.vim_clamp_cursor();
        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    /// Visual 模式下移动并更新选择
    fn vim_visual_move_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            if let Some((idx, _)) = line[..self.cursor_col].char_indices().last() {
                self.cursor_col = idx;
            }
        }
        let is_line_mode = self.vim_state.as_ref()
            .map(|v| v.mode == VimMode::VisualLine)
            .unwrap_or(false);
        self.vim_update_visual_selection(is_line_mode);
    }

    fn vim_visual_move_right(&mut self) {
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            if let Some((idx, _)) = line[self.cursor_col..].char_indices().nth(1) {
                self.cursor_col += idx;
            } else {
                self.cursor_col = line.len();
            }
        }
        let is_line_mode = self.vim_state.as_ref()
            .map(|v| v.mode == VimMode::VisualLine)
            .unwrap_or(false);
        self.vim_update_visual_selection(is_line_mode);
    }

    fn vim_visual_move_down(&mut self, is_line_mode: bool) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.ensure_cursor_valid();
        }
        self.vim_update_visual_selection(is_line_mode);
    }

    fn vim_visual_move_up(&mut self, is_line_mode: bool) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.ensure_cursor_valid();
        }
        self.vim_update_visual_selection(is_line_mode);
    }

    /// 根据 visual_start 和当前光标更新选择区域
    fn vim_update_visual_selection(&mut self, is_line_mode: bool) {
        let visual_start = if let Some(ref vim) = self.vim_state {
            vim.visual_start
        } else {
            return;
        };

        if let Some((start_row, start_col)) = visual_start {
            if is_line_mode {
                // 行模式：选择从起始行到当前行的所有内容
                let (sel_start_row, sel_end_row) = if start_row <= self.cursor_row {
                    (start_row, self.cursor_row)
                } else {
                    (self.cursor_row, start_row)
                };
                let end_col = self.lines[sel_end_row].len();
                self.selection = Some(super::Selection::new(sel_start_row, 0, sel_end_row, end_col));
            } else {
                // 字符模式：从 visual_start 到当前光标
                // 需要考虑当前光标位置包含当前字符
                let mut end_col = self.cursor_col;
                if self.cursor_row == start_row && self.cursor_col >= start_col {
                    // 向右选择，包含当前字符
                    let line = &self.lines[self.cursor_row];
                    if end_col < line.len() {
                        let ch = line[end_col..].chars().next().unwrap();
                        end_col += ch.len_utf8();
                    }
                } else if self.cursor_row > start_row || (self.cursor_row == start_row && self.cursor_col > start_col) {
                    let line = &self.lines[self.cursor_row];
                    if end_col < line.len() {
                        let ch = line[end_col..].chars().next().unwrap();
                        end_col += ch.len_utf8();
                    }
                }
                self.selection = Some(super::Selection::new(start_row, start_col, self.cursor_row, end_col));
            }
        }
        self.needs_render = true;
    }

    /// Visual 模式删除选择内容
    fn vim_visual_delete(&mut self) {
        use super::vim::RegisterContent;

        let is_line_mode = self.vim_state.as_ref()
            .map(|v| v.mode == VimMode::VisualLine)
            .unwrap_or(false);

        // 获取选中文本
        let selected = self.get_selected_text();

        if let Some(text) = selected {
            // 存入 register
            if let Some(ref mut vim) = self.vim_state {
                if is_line_mode {
                    vim.register = Some(RegisterContent::Lines(text));
                } else {
                    vim.register = Some(RegisterContent::Chars(text));
                }
            }
        }

        // 删除选择
        if is_line_mode {
            // 行模式删除：删除整行
            if let Some(sel) = self.selection {
                let sel = sel.normalized();
                self.save_snapshot();
                let start_row = sel.start_row;
                let end_row = sel.end_row;

                if end_row < self.lines.len() - 1 || start_row > 0 {
                    // 可以安全删除行
                    for _ in start_row..=end_row {
                        if self.lines.len() > 1 {
                            self.lines.remove(start_row);
                        } else {
                            self.lines[0].clear();
                        }
                    }
                    if self.cursor_row >= self.lines.len() {
                        self.cursor_row = self.lines.len() - 1;
                    }
                    self.cursor_col = 0;
                } else {
                    // 只有这些行
                    self.lines = vec![String::new()];
                    self.cursor_row = 0;
                    self.cursor_col = 0;
                }
                self.needs_render = true;
            }
        } else {
            self.delete_selection();
        }

        // 退出 Visual 模式
        self.selection = None;
        if let Some(ref mut vim) = self.vim_state {
            vim.visual_start = None;
            vim.switch_mode(VimMode::Normal);
        }
        self.vim_clamp_cursor();
        self.needs_render = true;
    }

    /// Visual 模式复制选择内容
    fn vim_visual_yank(&mut self) {
        use super::vim::RegisterContent;

        let is_line_mode = self.vim_state.as_ref()
            .map(|v| v.mode == VimMode::VisualLine)
            .unwrap_or(false);

        // 获取选中文本
        let selected = self.get_selected_text();

        if let Some(text) = selected {
            let lines_count = text.lines().count();
            if let Some(ref mut vim) = self.vim_state {
                if is_line_mode {
                    vim.register = Some(RegisterContent::Lines(text));
                    vim.status_message = format!("{} lines yanked", lines_count);
                } else {
                    vim.register = Some(RegisterContent::Chars(text));
                    vim.status_message = "yanked".to_string();
                }
            }
        }

        // 退出 Visual 模式，光标回到选择起点
        if let Some(ref vim) = self.vim_state {
            if let Some((row, col)) = vim.visual_start {
                let sel = self.selection.map(|s| s.normalized());
                if let Some(s) = sel {
                    self.cursor_row = s.start_row;
                    self.cursor_col = s.start_col;
                } else {
                    self.cursor_row = row;
                    self.cursor_col = col;
                }
            }
        }

        self.selection = None;
        if let Some(ref mut vim) = self.vim_state {
            vim.visual_start = None;
            vim.switch_mode(VimMode::Normal);
        }
        self.needs_render = true;
    }

    /// Visual 模式缩进
    fn vim_visual_indent(&mut self) {
        if let Some(sel) = self.selection {
            let sel = sel.normalized();
            self.save_snapshot();

            for row in sel.start_row..=sel.end_row {
                self.lines[row].insert_str(0, "    "); // 4 空格缩进
            }

            if let Some(ref mut vim) = self.vim_state {
                vim.last_command = Some(super::vim::VimCommand::Indent);
            }
        }

        // 退出 Visual 模式
        self.selection = None;
        if let Some(ref mut vim) = self.vim_state {
            vim.visual_start = None;
            vim.switch_mode(VimMode::Normal);
        }
        self.needs_render = true;
    }

    /// Visual 模式反缩进
    fn vim_visual_outdent(&mut self) {
        if let Some(sel) = self.selection {
            let sel = sel.normalized();
            self.save_snapshot();

            for row in sel.start_row..=sel.end_row {
                let line = &self.lines[row];
                let spaces = line.chars().take_while(|c| *c == ' ').count().min(4);
                if spaces > 0 {
                    self.lines[row] = self.lines[row][spaces..].to_string();
                }
            }

            if let Some(ref mut vim) = self.vim_state {
                vim.last_command = Some(super::vim::VimCommand::Outdent);
            }
        }

        // 退出 Visual 模式
        self.selection = None;
        if let Some(ref mut vim) = self.vim_state {
            vim.visual_start = None;
            vim.switch_mode(VimMode::Normal);
        }
        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    // ========== 搜索相关方法 ==========

    /// 执行搜索并跳转到第一个匹配
    fn vim_execute_search(&mut self, pattern: &str) {
        if pattern.is_empty() {
            return;
        }

        // 构建匹配列表
        let mut matches = Vec::new();
        for (row, line) in self.lines.iter().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(pattern) {
                matches.push((row, start + pos));
                start += pos + pattern.len();
                if start >= line.len() {
                    break;
                }
            }
        }

        if matches.is_empty() {
            if let Some(ref mut vim) = self.vim_state {
                vim.status_message = format!("Pattern not found: {}", pattern);
                vim.search_matches.clear();
                vim.search_match_index = 0;
            }
            return;
        }

        // 找到当前光标位置的第一个匹配（从当前位置开始，包含当前位置）
        let current_pos = (self.cursor_row, self.cursor_col);
        let mut found_index = 0;
        for (i, &(row, col)) in matches.iter().enumerate() {
            if (row, col) >= current_pos {
                found_index = i;
                break;
            }
            // 如果所有匹配都在光标之前，wrap around 到第一个
            found_index = 0;
        }

        // 跳转到匹配位置
        let (target_row, target_col) = matches[found_index];
        self.cursor_row = target_row;
        self.cursor_col = target_col;

        if let Some(ref mut vim) = self.vim_state {
            vim.search_matches = matches;
            vim.search_match_index = found_index;
            let total = vim.search_matches.len();
            vim.status_message = format!("[{}/{}]", found_index + 1, total);
        }

        self.ensure_cursor_valid();
        self.needs_render = true;
    }

    /// 跳转到下一个搜索匹配
    fn vim_search_next(&mut self) {
        // 检查是否有搜索模式或匹配
        let (has_matches, next_index, needs_research) = if let Some(ref vim) = self.vim_state {
            if vim.search_matches.is_empty() {
                // 如果没有匹配但有 pattern，需要重新搜索
                if vim.search_pattern.is_some() {
                    (false, 0, true)
                } else {
                    return;
                }
            } else {
                let next = (vim.search_match_index + 1) % vim.search_matches.len();
                (true, next, false)
            }
        } else {
            return;
        };

        // 需要重新执行搜索
        if needs_research {
            if let Some(ref vim) = self.vim_state {
                if let Some(ref p) = vim.search_pattern {
                    let pattern = p.clone();
                    self.vim_execute_search(&pattern);
                }
            }
            return;
        }

        if has_matches {
            if let Some(ref mut vim) = self.vim_state {
                vim.search_match_index = next_index;
                let (row, col) = vim.search_matches[next_index];
                self.cursor_row = row;
                self.cursor_col = col;
                let total = vim.search_matches.len();
                vim.status_message = format!("[{}/{}]", next_index + 1, total);
            }
            self.ensure_cursor_valid();
            self.needs_render = true;
        }
    }

    /// 跳转到上一个搜索匹配
    fn vim_search_prev(&mut self) {
        if let Some(ref mut vim) = self.vim_state {
            if vim.search_matches.is_empty() {
                return;
            }
            let prev = if vim.search_match_index == 0 {
                vim.search_matches.len() - 1
            } else {
                vim.search_match_index - 1
            };
            vim.search_match_index = prev;
            let (row, col) = vim.search_matches[prev];
            self.cursor_row = row;
            self.cursor_col = col;
            let total = vim.search_matches.len();
            vim.status_message = format!("[{}/{}]", prev + 1, total);
        }
        self.ensure_cursor_valid();
        self.needs_render = true;
    }
}
