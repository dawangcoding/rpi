//! 间距组件
//! 用于在布局中创建空白行

use crate::tui::Component;

/// 间距组件 - 渲染指定数量的空行
pub struct Spacer {
    lines: usize,
}

impl Spacer {
    /// 创建新的间距组件
    pub fn new(lines: usize) -> Self {
        Self { lines }
    }

    /// 设置行数
    pub fn set_lines(&mut self, lines: usize) {
        self.lines = lines;
    }

    /// 获取行数
    pub fn lines(&self) -> usize {
        self.lines
    }
}

impl Component for Spacer {
    fn render(&self, _width: u16) -> Vec<String> {
        vec![String::new(); self.lines]
    }

    fn invalidate(&mut self) {
        // 无缓存需要清除
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spacer_render() {
        let spacer = Spacer::new(3);
        let lines = spacer.render(80);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "");
    }

    #[test]
    fn test_spacer_set_lines() {
        let mut spacer = Spacer::new(1);
        spacer.set_lines(5);
        assert_eq!(spacer.lines(), 5);
        let lines = spacer.render(80);
        assert_eq!(lines.len(), 5);
    }
}
