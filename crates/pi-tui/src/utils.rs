//! 工具函数模块
//! 提供字符串宽度计算、ANSI 处理、文本换行等功能

use std::collections::HashMap;
use unicode_width::UnicodeWidthChar;

/// 光标标记 (IME 支持) - APC 序列，零宽度
pub const CURSOR_MARKER: &str = "\x1b_pi:c\x07";

/// ANSI 转义序列的最大缓存大小
const WIDTH_CACHE_SIZE: usize = 512;

thread_local! {
    static WIDTH_CACHE: std::cell::RefCell<HashMap<String, usize>> = 
        std::cell::RefCell::new(HashMap::new());
}

/// 检查字符是否为可打印 ASCII
fn is_printable_ascii(s: &str) -> bool {
    s.bytes().all(|b| b >= 0x20 && b <= 0x7e)
}

/// 提取 ANSI 转义序列
/// 
/// 支持:
/// - CSI 序列: ESC [ ... m/G/K/H/J
/// - OSC 序列: ESC ] ... BEL 或 ESC ] ... ST
/// - APC 序列: ESC _ ... BEL 或 ESC _ ... ST
pub fn extract_ansi_code(s: &str, pos: usize) -> Option<(String, usize)> {
    if pos >= s.len() {
        return None;
    }
    
    let bytes = s.as_bytes();
    if bytes[pos] != 0x1b {
        return None;
    }
    
    let next = pos + 1;
    if next >= s.len() {
        return None;
    }
    
    match bytes[next] {
        // CSI 序列: ESC [ ... m/G/K/H/J
        b'[' => {
            let mut j = next + 1;
            while j < s.len() {
                let c = bytes[j];
                // CSI 序列结束字符: @-~
                if c >= 0x40 && c <= 0x7e {
                    let code = s[pos..=j].to_string();
                    return Some((code, j + 1 - pos));
                }
                j += 1;
            }
            None
        }
        // OSC 序列: ESC ] ... BEL 或 ESC ] ... ESC \
        b']' => {
            let mut j = next + 1;
            while j < s.len() {
                if bytes[j] == 0x07 {
                    // BEL 结束
                    let code = s[pos..=j].to_string();
                    return Some((code, j + 1 - pos));
                }
                if bytes[j] == 0x1b && j + 1 < s.len() && bytes[j + 1] == b'\\' {
                    // ST (ESC \) 结束
                    let code = s[pos..=j + 1].to_string();
                    return Some((code, j + 2 - pos));
                }
                j += 1;
            }
            None
        }
        // APC 序列: ESC _ ... BEL 或 ESC _ ... ESC \
        b'_' => {
            let mut j = next + 1;
            while j < s.len() {
                if bytes[j] == 0x07 {
                    // BEL 结束
                    let code = s[pos..=j].to_string();
                    return Some((code, j + 1 - pos));
                }
                if bytes[j] == 0x1b && j + 1 < s.len() && bytes[j + 1] == b'\\' {
                    // ST (ESC \) 结束
                    let code = s[pos..=j + 1].to_string();
                    return Some((code, j + 2 - pos));
                }
                j += 1;
            }
            None
        }
        _ => None,
    }
}

/// 计算单个字符的显示宽度
fn char_width(c: char) -> usize {
    // 处理特殊字符
    let cp = c as u32;
    
    // Regional indicator symbols (国旗) 通常显示为 2 宽度
    if cp >= 0x1f1e6 && cp <= 0x1f1ff {
        return 2;
    }
    
    // 使用 unicode-width crate
    c.width().unwrap_or(0)
}

/// 计算字符串的可见宽度（考虑 ANSI 转义和东亚字符）
/// 
/// - 跳过 ANSI 转义序列
/// - 正确处理东亚全角字符
/// - 处理 CURSOR_MARKER（不计入宽度）
/// - Tab 字符计为 3 个空格
pub fn visible_width(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    
    // 快速路径: 纯 ASCII 可打印字符
    if is_printable_ascii(s) {
        return s.len();
    }
    
    // 检查缓存
    let cached = WIDTH_CACHE.with(|cache| cache.borrow().get(s).copied());
    if let Some(width) = cached {
        return width;
    }
    
    // 处理 tab 和 ANSI 序列
    let mut clean = s.to_string();
    
    // 替换 tab 为 3 个空格
    if clean.contains('\t') {
        clean = clean.replace('\t', "   ");
    }
    
    // 移除 CURSOR_MARKER
    clean = clean.replace(CURSOR_MARKER, "");
    
    // 移除 ANSI 序列
    if clean.contains('\x1b') {
        let mut stripped = String::new();
        let mut i = 0;
        while i < clean.len() {
            if let Some((_code, len)) = extract_ansi_code(&clean, i) {
                i += len;
            } else {
                stripped.push(clean.chars().nth(i).unwrap());
                i += 1;
            }
        }
        clean = stripped;
    }
    
    // 计算宽度
    let mut width = 0;
    for c in clean.chars() {
        width += char_width(c);
    }
    
    // 缓存结果
    WIDTH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= WIDTH_CACHE_SIZE {
            // 简单的 LRU: 清空缓存
            cache.clear();
        }
        cache.insert(s.to_string(), width);
    });
    
    width
}

/// 去除 ANSI 转义序列
pub fn strip_ansi(s: &str) -> String {
    if !s.contains('\x1b') {
        return s.to_string();
    }
    
    let mut result = String::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((_, len)) = extract_ansi_code(s, i) {
            i += len;
        } else {
            result.push(s.chars().nth(i).unwrap());
            i += 1;
        }
    }
    result
}

/// 将字符串截断到指定可见宽度
/// 
/// # Arguments
/// * `s` - 输入字符串（可能包含 ANSI 代码）
/// * `max_width` - 最大可见宽度
/// * `ellipsis` - 截断时添加的省略号（默认 "..."）
/// 
/// # Returns
/// 截断后的字符串
pub fn truncate_to_width(s: &str, max_width: usize) -> String {
    truncate_to_width_with_ellipsis(s, max_width, "...")
}

/// 将字符串截断到指定可见宽度，可自定义省略号
pub fn truncate_to_width_with_ellipsis(s: &str, max_width: usize, ellipsis: &str) -> String {
    if max_width == 0 {
        return String::new();
    }
    
    let current_width = visible_width(s);
    if current_width <= max_width {
        return s.to_string();
    }
    
    let ellipsis_width = visible_width(ellipsis);
    let target_width = if ellipsis_width >= max_width {
        max_width
    } else {
        max_width - ellipsis_width
    };
    
    let mut result = String::new();
    let mut width = 0;
    let mut i = 0;
    let mut pending_ansi = String::new();
    
    while i < s.len() {
        // 检查 ANSI 序列
        if let Some((code, len)) = extract_ansi_code(s, i) {
            pending_ansi.push_str(&code);
            i += len;
            continue;
        }
        
        // 处理 tab
        if s.as_bytes()[i] == b'\t' {
            let tab_width = 3;
            if width + tab_width > target_width {
                break;
            }
            if !pending_ansi.is_empty() {
                result.push_str(&pending_ansi);
                pending_ansi.clear();
            }
            result.push('\t');
            width += tab_width;
            i += 1;
            continue;
        }
        
        // 获取字符
        let c = s.chars().nth(i).unwrap();
        let w = char_width(c);
        
        if width + w > target_width {
            break;
        }
        
        if !pending_ansi.is_empty() {
            result.push_str(&pending_ansi);
            pending_ansi.clear();
        }
        result.push(c);
        width += w;
        i += c.len_utf8();
    }
    
    // 添加省略号
    if ellipsis_width > 0 && ellipsis_width <= max_width && width < max_width {
        result.push_str(ellipsis);
    }
    
    // 添加重置代码
    result.push_str("\x1b[0m");
    
    result
}

/// 填充到指定宽度
/// 
/// 如果字符串宽度小于指定宽度，右侧填充空格
pub fn pad_to_width(s: &str, width: usize) -> String {
    let current_width = visible_width(s);
    if current_width >= width {
        return s.to_string();
    }
    
    let padding = width - current_width;
    format!("{}{}", s, " ".repeat(padding))
}

/// 按宽度换行（保留 ANSI 转义序列）
/// 
/// # Arguments
/// * `s` - 输入字符串
/// * `width` - 每行最大宽度
/// 
/// # Returns
/// 换行后的字符串数组
pub fn wrap_text_with_ansi(s: &str, width: usize) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    
    // 按换行符分割处理
    let lines: Vec<&str> = s.split('\n').collect();
    let mut result = Vec::new();
    
    for line in lines {
        if line.is_empty() {
            result.push(String::new());
            continue;
        }
        
        let wrapped = wrap_single_line(line, width);
        result.extend(wrapped);
    }
    
    if result.is_empty() {
        result.push(String::new());
    }
    
    result
}

/// 单行换行处理
fn wrap_single_line(line: &str, width: usize) -> Vec<String> {
    let visible_len = visible_width(line);
    if visible_len <= width {
        return vec![line.to_string()];
    }
    
    let mut wrapped = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    let mut pending_ansi = String::new();
    let mut i = 0;
    
    while i < line.len() {
        // 检查 ANSI 序列
        if let Some((code, len)) = extract_ansi_code(line, i) {
            pending_ansi.push_str(&code);
            i += len;
            continue;
        }
        
        // 处理 tab
        if line.as_bytes()[i] == b'\t' {
            let tab_width = 3;
            if current_width + tab_width > width {
                // 换行
                wrapped.push(current_line);
                current_line = pending_ansi.clone();
                current_width = 0;
            }
            current_line.push('\t');
            current_width += tab_width;
            i += 1;
            continue;
        }
        
        // 获取字符
        let c = line.chars().nth(i).unwrap();
        let w = char_width(c);
        
        // 检查是否需要换行
        if current_width + w > width {
            wrapped.push(current_line);
            current_line = pending_ansi.clone();
            current_width = 0;
        }
        
        if !pending_ansi.is_empty() {
            current_line.push_str(&pending_ansi);
            pending_ansi.clear();
        }
        current_line.push(c);
        current_width += w;
        i += c.len_utf8();
    }
    
    if !current_line.is_empty() {
        wrapped.push(current_line);
    }
    
    wrapped
}

/// 从指定列位置切片字符串
/// 
/// # Arguments
/// * `line` - 输入字符串
/// * `start_col` - 起始列（0-indexed）
/// * `length` - 要提取的宽度
/// * `strict` - 如果为 true，排除边界处会超出范围的宽字符
/// 
/// # Returns
/// 切片后的字符串和实际宽度
pub fn slice_with_width(line: &str, start_col: usize, length: usize, strict: bool) -> (String, usize) {
    if length == 0 {
        return (String::new(), 0);
    }
    
    let end_col = start_col + length;
    let mut result = String::new();
    let mut result_width = 0;
    let mut current_col = 0;
    let mut i = 0;
    let mut pending_ansi = String::new();
    
    while i < line.len() {
        // 检查 ANSI 序列
        if let Some((code, len)) = extract_ansi_code(line, i) {
            if current_col >= start_col && current_col < end_col {
                result.push_str(&code);
            } else if current_col < start_col {
                pending_ansi.push_str(&code);
            }
            i += len;
            continue;
        }
        
        // 获取字符
        let c = line.chars().nth(i).unwrap();
        let w = char_width(c);
        
        let in_range = current_col >= start_col && current_col < end_col;
        let fits = !strict || current_col + w <= end_col;
        
        if in_range && fits {
            if !pending_ansi.is_empty() {
                result.push_str(&pending_ansi);
                pending_ansi.clear();
            }
            result.push(c);
            result_width += w;
        }
        
        current_col += w;
        if current_col >= end_col {
            break;
        }
        
        i += c.len_utf8();
    }
    
    (result, result_width)
}

/// 从指定列位置切片字符串（简化版）
pub fn slice_by_column(line: &str, start_col: usize, length: usize, strict: bool) -> String {
    slice_with_width(line, start_col, length, strict).0
}

/// 提取线段（用于覆盖层合成）
/// 
/// 在一次遍历中提取 "before" 和 "after" 段
/// 用于覆盖层合成时需要内容前后部分的情况
pub fn extract_segments(
    line: &str,
    before_end: usize,
    after_start: usize,
    after_len: usize,
    strict_after: bool,
) -> (String, usize, String, usize) {
    let after_end = after_start + after_len;
    
    let mut before = String::new();
    let mut before_width = 0;
    let mut after = String::new();
    let mut after_width = 0;
    let mut current_col = 0;
    let mut i = 0;
    let mut pending_ansi_before = String::new();
    let mut after_started = false;
    
    // 跟踪样式状态
    let mut active_codes = String::new();
    
    while i < line.len() {
        // 检查 ANSI 序列
        if let Some((code, len)) = extract_ansi_code(line, i) {
            // 跟踪 SGR 代码
            if code.starts_with("\x1b[") && code.ends_with('m') {
                active_codes.push_str(&code);
            }
            
            // 包含 ANSI 代码到各自的段
            if current_col < before_end {
                pending_ansi_before.push_str(&code);
            } else if current_col >= after_start && current_col < after_end && after_started {
                after.push_str(&code);
            }
            
            i += len;
            continue;
        }
        
        // 获取字符
        let c = line.chars().nth(i).unwrap();
        let w = char_width(c);
        
        if current_col < before_end {
            if !pending_ansi_before.is_empty() {
                before.push_str(&pending_ansi_before);
                pending_ansi_before.clear();
            }
            before.push(c);
            before_width += w;
        } else if current_col >= after_start && current_col < after_end {
            let fits = !strict_after || current_col + w <= after_end;
            if fits {
                // 在第一个 "after" 字符时，预置继承的样式
                if !after_started {
                    after.push_str(&active_codes);
                    after_started = true;
                }
                after.push(c);
                after_width += w;
            }
        }
        
        current_col += w;
        
        // 提前退出
        let exit_point = if after_len == 0 { before_end } else { after_end };
        if current_col >= exit_point {
            break;
        }
        
        i += c.len_utf8();
    }
    
    (before, before_width, after, after_width)
}

/// 清除宽度缓存
pub fn clear_width_cache() {
    WIDTH_CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_visible_width_ascii() {
        assert_eq!(visible_width("hello"), 5);
        assert_eq!(visible_width(""), 0);
    }
    
    #[test]
    fn test_visible_width_cjk() {
        assert_eq!(visible_width("你好"), 4); // 每个 CJK 字符通常占 2 宽度
        assert_eq!(visible_width("中a文"), 5);
    }
    
    #[test]
    fn test_visible_width_ansi() {
        assert_eq!(visible_width("\x1b[31mred\x1b[0m"), 3);
        assert_eq!(visible_width("\x1b[1;31mbold red\x1b[0m"), 8);
    }
    
    #[test]
    fn test_visible_width_cursor_marker() {
        assert_eq!(visible_width(CURSOR_MARKER), 0);
        assert_eq!(visible_width(&format!("text{}more", CURSOR_MARKER)), 8);
    }
    
    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;31mbold\x1b[0m"), "bold");
        assert_eq!(strip_ansi("no ansi"), "no ansi");
    }
    
    #[test]
    fn test_truncate_to_width() {
        assert_eq!(truncate_to_width("hello world", 5), "he...");
        assert_eq!(truncate_to_width("hi", 5), "hi");
    }
    
    #[test]
    fn test_pad_to_width() {
        assert_eq!(pad_to_width("hi", 5), "hi   ");
        assert_eq!(pad_to_width("hello", 3), "hello");
    }
    
    #[test]
    fn test_wrap_text() {
        let result = wrap_text_with_ansi("hello world", 5);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "hello");
    }
    
    #[test]
    fn test_extract_ansi_code() {
        let s = "\x1b[31mred";
        assert_eq!(extract_ansi_code(s, 0), Some(("\x1b[31m".to_string(), 5)));
        assert_eq!(extract_ansi_code(s, 1), None);
    }
}
