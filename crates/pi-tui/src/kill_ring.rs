//! Emacs 风格剪贴板环 (Kill Ring)
//! 支持 kill/yank 操作，可累积连续删除的文本

/// 推送选项
#[derive(Debug, Clone, Copy)]
pub struct PushOptions {
    /// 如果是累积，前置（向后删除）或后置（向前删除）
    pub prepend: bool,
    /// 是否累积到最近条目
    pub accumulate: bool,
}

impl PushOptions {
    /// 创建新的推送选项
    pub fn new(prepend: bool, accumulate: bool) -> Self {
        Self { prepend, accumulate }
    }
}

impl Default for PushOptions {
    fn default() -> Self {
        Self {
            prepend: false,
            accumulate: false,
        }
    }
}

/// Emacs 风格剪贴板环
/// 
/// 用于存储删除的文本，支持 yank (粘贴) 和 yank-pop (循环粘贴)
#[derive(Debug, Clone)]
pub struct KillRing {
    ring: Vec<String>,
    index: usize,
    max_size: usize,
}

impl KillRing {
    /// 创建新的剪贴板环
    /// 
    /// # Arguments
    /// * `max_size` - 最大存储条目数
    pub fn new(max_size: usize) -> Self {
        Self {
            ring: Vec::new(),
            index: 0,
            max_size: max_size.max(1),
        }
    }

    /// 添加文本到剪贴板环
    /// 
    /// # Arguments
    /// * `text` - 要添加的文本
    /// * `opts` - 推送选项
    /// 
    /// 如果 `accumulate` 为 true，会合并到最近条目
    pub fn push(&mut self, text: String, opts: PushOptions) {
        if text.is_empty() {
            return;
        }

        if opts.accumulate && !self.ring.is_empty() {
            // 累积到最近条目
            let last = self.ring.pop().unwrap();
            let merged = if opts.prepend {
                text + &last
            } else {
                last + &text
            };
            self.ring.push(merged);
        } else {
            // 添加新条目
            self.ring.push(text);
            
            // 限制大小
            if self.ring.len() > self.max_size {
                self.ring.remove(0);
            }
        }
        
        // 重置索引到末尾（最新条目）
        self.index = self.ring.len().saturating_sub(1);
    }

    /// 获取当前条目（用于 yank）
    /// 
    /// 不移除条目，仅返回引用
    pub fn yank(&self) -> Option<&str> {
        self.ring.get(self.index).map(|s| s.as_str())
    }

    /// 循环到下一个条目（用于 yank-pop）
    /// 
    /// 返回新的当前条目
    pub fn yank_pop(&mut self) -> Option<&str> {
        if self.ring.len() <= 1 {
            return self.yank();
        }
        
        // 移动到上一个条目（循环）
        if self.index == 0 {
            self.index = self.ring.len() - 1;
        } else {
            self.index -= 1;
        }
        
        self.yank()
    }

    /// 检查剪贴板环是否为空
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// 获取条目数量
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// 清空剪贴板环
    pub fn clear(&mut self) {
        self.ring.clear();
        self.index = 0;
    }

    /// 获取所有条目（用于调试）
    pub fn entries(&self) -> &[String] {
        &self.ring
    }
}

impl Default for KillRing {
    fn default() -> Self {
        Self::new(50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kill_ring_basic() {
        let mut ring = KillRing::new(10);
        
        assert!(ring.is_empty());
        assert_eq!(ring.yank(), None);

        ring.push("hello".to_string(), PushOptions::default());
        assert!(!ring.is_empty());
        assert_eq!(ring.yank(), Some("hello"));

        ring.push("world".to_string(), PushOptions::default());
        assert_eq!(ring.yank(), Some("world")); // 最新条目
    }

    #[test]
    fn test_kill_ring_accumulate() {
        let mut ring = KillRing::new(10);
        
        // 第一次 push
        ring.push("hello ".to_string(), PushOptions::default());
        assert_eq!(ring.yank(), Some("hello "));

        // 累积（向后删除 = append）
        ring.push("world".to_string(), PushOptions::new(false, true));
        assert_eq!(ring.yank(), Some("hello world"));
        assert_eq!(ring.len(), 1);

        // 新的 push（不累积）
        ring.push("!".to_string(), PushOptions::default());
        assert_eq!(ring.len(), 2);
    }

    #[test]
    fn test_kill_ring_prepend() {
        let mut ring = KillRing::new(10);
        
        ring.push("world".to_string(), PushOptions::default());
        
        // 向前删除 = prepend
        ring.push("hello ".to_string(), PushOptions::new(true, true));
        assert_eq!(ring.yank(), Some("hello world"));
    }

    #[test]
    fn test_kill_ring_yank_pop() {
        let mut ring = KillRing::new(10);
        
        ring.push("first".to_string(), PushOptions::default());
        ring.push("second".to_string(), PushOptions::default());
        ring.push("third".to_string(), PushOptions::default());

        // 当前是 third
        assert_eq!(ring.yank(), Some("third"));
        
        // yank-pop 循环
        assert_eq!(ring.yank_pop(), Some("second"));
        assert_eq!(ring.yank_pop(), Some("first"));
        assert_eq!(ring.yank_pop(), Some("third")); // 循环回最后
        assert_eq!(ring.yank_pop(), Some("second"));
    }

    #[test]
    fn test_kill_ring_empty_pop() {
        let mut ring = KillRing::new(10);
        
        assert_eq!(ring.yank_pop(), None);
        
        ring.push("only".to_string(), PushOptions::default());
        assert_eq!(ring.yank_pop(), Some("only")); // 只有一个条目
    }

    #[test]
    fn test_kill_ring_max_size() {
        let mut ring = KillRing::new(3);
        
        ring.push("a".to_string(), PushOptions::default());
        ring.push("b".to_string(), PushOptions::default());
        ring.push("c".to_string(), PushOptions::default());
        ring.push("d".to_string(), PushOptions::default()); // 应该移除 a

        assert_eq!(ring.len(), 3);
        
        // 检查条目
        let entries = ring.entries();
        assert_eq!(entries[0], "b");
        assert_eq!(entries[1], "c");
        assert_eq!(entries[2], "d");
    }

    #[test]
    fn test_kill_ring_clear() {
        let mut ring = KillRing::new(10);
        
        ring.push("test".to_string(), PushOptions::default());
        ring.clear();
        
        assert!(ring.is_empty());
        assert_eq!(ring.yank(), None);
    }

    #[test]
    fn test_empty_text_not_pushed() {
        let mut ring = KillRing::new(10);
        
        ring.push("".to_string(), PushOptions::default());
        assert!(ring.is_empty());
    }
}
