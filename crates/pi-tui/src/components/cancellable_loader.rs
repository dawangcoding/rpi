//! 可取消的加载动画组件
//! 提供带取消功能和可选进度条的加载指示器

use crate::tui::Component;

/// 可取消的加载动画指示器
pub struct CancellableLoader {
    /// 加载消息
    message: String,
    /// 动画帧
    frames: Vec<&'static str>,
    /// 当前帧索引
    current_frame: usize,
    /// 是否已取消
    cancelled: bool,
    /// 取消提示文本
    cancel_hint: String,
    /// 可选进度 (0.0-1.0)
    progress: Option<f64>,
    /// 取消回调
    on_cancel: Option<Box<dyn Fn() + Send>>,
    /// 是否需要重绘
    needs_redraw: bool,
}

impl CancellableLoader {
    /// 默认动画帧 - 点阵旋转动画（与 Loader 保持一致）
    const DEFAULT_FRAMES: [&'static str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// 创建新的可取消加载动画组件
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            frames: Self::DEFAULT_FRAMES.to_vec(),
            current_frame: 0,
            cancelled: false,
            cancel_hint: "Press Esc to cancel".to_string(),
            progress: None,
            on_cancel: None,
            needs_redraw: true,
        }
    }

    /// 设置自定义取消提示文本
    pub fn with_cancel_hint(mut self, hint: &str) -> Self {
        self.cancel_hint = hint.to_string();
        self
    }

    /// 设置进度 (0.0-1.0)
    pub fn with_progress(mut self, progress: f64) -> Self {
        self.progress = Some(progress.clamp(0.0, 1.0));
        self.needs_redraw = true;
        self
    }

    /// 设置取消回调
    pub fn set_on_cancel(&mut self, callback: Box<dyn Fn() + Send>) {
        self.on_cancel = Some(callback);
    }

    /// 推进到下一帧
    pub fn tick(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frames.len();
        self.needs_redraw = true;
    }

    /// 检查是否已取消
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// 设置消息文本
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.needs_redraw = true;
    }

    /// 设置进度 (0.0-1.0)
    pub fn set_progress(&mut self, progress: f64) {
        self.progress = Some(progress.clamp(0.0, 1.0));
        self.needs_redraw = true;
    }

    /// 获取当前消息
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 获取当前帧索引
    pub fn current_frame_index(&self) -> usize {
        self.current_frame
    }

    /// 获取当前进度
    pub fn progress(&self) -> Option<f64> {
        self.progress
    }

    /// 渲染进度条
    fn render_progress_bar(&self, width: usize) -> String {
        let progress = self.progress.unwrap_or(0.0);
        let percentage = (progress * 100.0) as usize;
        
        // 进度条宽度：减去百分比显示和括号
        // 格式: [████░░░░] 45%
        let bar_width = width.saturating_sub(4); // 空格 + 百分比显示预留
        
        if bar_width < 5 {
            // 宽度太窄，只显示百分比
            return format!(" {}%", percentage);
        }
        
        let filled = ((bar_width as f64) * progress) as usize;
        let empty = bar_width - filled;
        
        let filled_str = "█".repeat(filled);
        let empty_str = "░".repeat(empty);
        
        format!(" [{}{}] {:3}%", filled_str, empty_str, percentage)
    }
}

impl Component for CancellableLoader {
    fn render(&self, width: u16) -> Vec<String> {
        let mut lines = Vec::new();
        let frame = self.frames[self.current_frame];
        
        // 第一行：动画帧 + 消息
        lines.push(format!("{} {}", frame, self.message));
        
        // 第二行：进度条（如果有）
        if let Some(_progress) = self.progress {
            lines.push(self.render_progress_bar(width as usize));
        }
        
        // 最后一行：取消提示
        lines.push(format!("\x1b[90m{}\x1b[0m", self.cancel_hint));
        
        lines
    }

    fn handle_input(&mut self, data: &str) -> bool {
        // 检测 Esc 键 (\x1b)
        if data == "\x1b" {
            self.cancelled = true;
            // 调用取消回调
            if let Some(callback) = &self.on_cancel {
                callback();
            }
            return true;
        }
        false
    }

    fn invalidate(&mut self) {
        self.needs_redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellable_loader_new() {
        let loader = CancellableLoader::new("Loading...");
        assert_eq!(loader.message(), "Loading...");
        assert!(!loader.is_cancelled());
        assert!(loader.progress().is_none());
    }

    #[test]
    fn test_cancellable_loader_render() {
        let loader = CancellableLoader::new("Processing...");
        let lines = loader.render(80);
        assert_eq!(lines.len(), 2); // 消息 + 取消提示（无进度）
        assert!(lines[0].contains("Processing..."));
        assert!(lines[0].starts_with("⠋")); // 第一帧
        assert!(lines[1].contains("Press Esc to cancel"));
    }

    #[test]
    fn test_cancellable_loader_tick() {
        let mut loader = CancellableLoader::new("Loading...");
        assert_eq!(loader.current_frame_index(), 0);
        
        loader.tick();
        assert_eq!(loader.current_frame_index(), 1);
        
        // 测试循环
        for _ in 0..9 {
            loader.tick();
        }
        assert_eq!(loader.current_frame_index(), 0); // 回到第一帧
    }

    #[test]
    fn test_cancellable_loader_esc_cancel() {
        let mut loader = CancellableLoader::new("Loading...");
        assert!(!loader.is_cancelled());
        
        // 模拟按下 Esc 键
        let handled = loader.handle_input("\x1b");
        assert!(handled);
        assert!(loader.is_cancelled());
    }

    #[test]
    fn test_cancellable_loader_progress() {
        let loader = CancellableLoader::new("Loading...")
            .with_progress(0.45);
        
        assert_eq!(loader.progress(), Some(0.45));
        
        let lines = loader.render(80);
        assert_eq!(lines.len(), 3); // 消息 + 进度条 + 取消提示
        assert!(lines[1].contains("45%"));
    }

    #[test]
    fn test_cancellable_loader_set_message() {
        let mut loader = CancellableLoader::new("Loading...");
        loader.set_message("Processing...");
        assert_eq!(loader.message(), "Processing...");
        
        let lines = loader.render(80);
        assert!(lines[0].contains("Processing..."));
    }

    #[test]
    fn test_cancellable_loader_set_progress() {
        let mut loader = CancellableLoader::new("Loading...");
        loader.set_progress(0.75);
        
        assert_eq!(loader.progress(), Some(0.75));
        
        let lines = loader.render(80);
        assert!(lines[1].contains("75%"));
    }

    #[test]
    fn test_cancellable_loader_cancel_hint() {
        let loader = CancellableLoader::new("Loading...")
            .with_cancel_hint("按 Esc 取消");
        
        let lines = loader.render(80);
        assert!(lines[1].contains("按 Esc 取消"));
    }

    #[test]
    fn test_cancellable_loader_cancel_callback() {
        use std::sync::{Arc, Mutex};
        
        let called = Arc::new(Mutex::new(false));
        let called_clone = called.clone();
        
        let mut loader = CancellableLoader::new("Loading...");
        loader.set_on_cancel(Box::new(move || {
            *called_clone.lock().unwrap() = true;
        }));
        
        loader.handle_input("\x1b");
        
        assert!(*called.lock().unwrap());
    }

    #[test]
    fn test_cancellable_loader_progress_clamping() {
        // 测试进度值超过 1.0 会被限制
        let loader = CancellableLoader::new("Loading...")
            .with_progress(1.5);
        assert_eq!(loader.progress(), Some(1.0));
        
        // 测试进度值小于 0.0 会被限制
        let loader = CancellableLoader::new("Loading...")
            .with_progress(-0.5);
        assert_eq!(loader.progress(), Some(0.0));
    }

    #[test]
    fn test_cancellable_loader_other_keys_ignored() {
        let mut loader = CancellableLoader::new("Loading...");
        
        // 其他按键不应触发取消
        let handled = loader.handle_input("a");
        assert!(!handled);
        assert!(!loader.is_cancelled());
        
        let handled = loader.handle_input("\r"); // Enter
        assert!(!handled);
        assert!(!loader.is_cancelled());
    }
}
