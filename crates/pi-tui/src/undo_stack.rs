//! 撤销/重做栈
//! 提供通用的撤销/重做功能，支持状态快照存储

/// 撤销/重做栈
/// 
/// 存储状态快照，支持撤销(undo)和重做(redo)操作
#[derive(Debug, Clone)]
pub struct UndoStack<T: Clone> {
    stack: Vec<T>,
    index: usize,  // 当前位置（指向下一个可撤销的状态）
    max_size: usize,
}

impl<T: Clone> UndoStack<T> {
    /// 创建新的撤销栈
    /// 
    /// # Arguments
    /// * `max_size` - 最大存储状态数
    pub fn new(max_size: usize) -> Self {
        Self {
            stack: Vec::new(),
            index: 0,
            max_size: max_size.max(1),
        }
    }

    /// 推送新状态到栈中
    /// 
    /// 如果在重做历史中间推送新状态，会丢弃后续的重做历史
    pub fn push(&mut self, state: T) {
        // 如果在重做历史中间，丢弃后续状态
        if self.index < self.stack.len() {
            self.stack.truncate(self.index);
        }

        self.stack.push(state);
        self.index += 1;

        // 限制栈大小
        if self.stack.len() > self.max_size {
            self.stack.remove(0);
            self.index -= 1;
        }
    }

    /// 撤销操作，返回上一个状态
    /// 
    /// 如果没有可撤销的状态，返回 None
    pub fn undo(&mut self) -> Option<&T> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        self.stack.get(self.index)
    }

    /// 重做操作，返回下一个状态
    /// 
    /// 如果没有可重做的状态，返回 None
    pub fn redo(&mut self) -> Option<&T> {
        if self.index >= self.stack.len() {
            return None;
        }
        self.index += 1;
        self.stack.get(self.index - 1)
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    /// 检查是否可以重做
    pub fn can_redo(&self) -> bool {
        self.index < self.stack.len()
    }

    /// 获取当前状态
    pub fn current(&self) -> Option<&T> {
        if self.index == 0 {
            None
        } else {
            self.stack.get(self.index - 1)
        }
    }

    /// 清空栈
    pub fn clear(&mut self) {
        self.stack.clear();
        self.index = 0;
    }

    /// 获取栈中状态数量
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// 检查栈是否为空
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl<T: Clone> Default for UndoStack<T> {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_stack_basic() {
        let mut stack = UndoStack::new(10);
        
        // 初始状态
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.current(), None);

        // 推送状态
        stack.push("state1");
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.current(), Some(&"state1"));

        stack.push("state2");
        assert_eq!(stack.current(), Some(&"state2"));

        // 撤销
        let state = stack.undo();
        assert_eq!(state, Some(&"state1"));
        assert!(stack.can_redo());

        // 重做
        let state = stack.redo();
        assert_eq!(state, Some(&"state2"));
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_redo_sequence() {
        let mut stack = UndoStack::new(10);
        
        stack.push("a");
        stack.push("b");
        stack.push("c");

        assert_eq!(stack.current(), Some(&"c"));
        
        assert_eq!(stack.undo(), Some(&"b"));
        assert_eq!(stack.undo(), Some(&"a"));
        assert_eq!(stack.undo(), None);
        
        assert_eq!(stack.redo(), Some(&"a"));
        assert_eq!(stack.redo(), Some(&"b"));
        assert_eq!(stack.redo(), Some(&"c"));
        assert_eq!(stack.redo(), None);
    }

    #[test]
    fn test_push_after_undo() {
        let mut stack = UndoStack::new(10);
        
        stack.push("a");
        stack.push("b");
        stack.push("c");
        
        stack.undo(); // 回到 b
        stack.undo(); // 回到 a
        
        // 在中间推送新状态，丢弃 b 和 c
        stack.push("d");
        
        assert_eq!(stack.current(), Some(&"d"));
        assert!(!stack.can_redo());
        assert_eq!(stack.len(), 2); // a, d
    }

    #[test]
    fn test_max_size() {
        let mut stack = UndoStack::new(3);
        
        stack.push("a");
        stack.push("b");
        stack.push("c");
        stack.push("d"); // 应该移除 a
        
        assert_eq!(stack.len(), 3);
        
        // 无法再撤销到 a
        stack.undo(); // c
        stack.undo(); // b
        assert_eq!(stack.undo(), None); // 不是 a
    }

    #[test]
    fn test_clear() {
        let mut stack = UndoStack::new(10);
        
        stack.push("a");
        stack.push("b");
        stack.clear();
        
        assert!(stack.is_empty());
        assert!(!stack.can_undo());
        assert_eq!(stack.current(), None);
    }
}
