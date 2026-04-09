//! 加载动画组件
//! 提供旋转的加载指示器

use crate::tui::Component;

/// 加载动画指示器
pub struct Loader {
    message: String,
    frames: Vec<&'static str>,
    current_frame: usize,
    needs_render: bool,
}

impl Loader {
    /// 默认动画帧 - 点阵旋转动画
    const DEFAULT_FRAMES: [&'static str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// 创建新的加载动画组件
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            frames: Self::DEFAULT_FRAMES.to_vec(),
            current_frame: 0,
            needs_render: true,
        }
    }

    /// 创建带有自定义动画帧的加载器
    pub fn with_frames(message: &str, frames: Vec<&'static str>) -> Self {
        Self {
            message: message.to_string(),
            frames,
            current_frame: 0,
            needs_render: true,
        }
    }

    /// 设置消息文本
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.needs_render = true;
    }

    /// 获取当前消息
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 推进到下一帧
    pub fn tick(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frames.len();
        self.needs_render = true;
    }

    /// 获取当前帧索引
    pub fn current_frame_index(&self) -> usize {
        self.current_frame
    }

    /// 重置动画到第一帧
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.needs_render = true;
    }
}

impl Component for Loader {
    fn render(&self, _width: u16) -> Vec<String> {
        let frame = self.frames[self.current_frame];
        vec![format!("{} {}", frame, self.message)]
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_new() {
        let loader = Loader::new("Loading...");
        let lines = loader.render(80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Loading..."));
        assert!(lines[0].starts_with("⠋")); // 第一帧
    }

    #[test]
    fn test_loader_tick() {
        let mut loader = Loader::new("Loading...");
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
    fn test_loader_set_message() {
        let mut loader = Loader::new("Loading...");
        loader.set_message("Processing...");
        assert_eq!(loader.message(), "Processing...");
        
        let lines = loader.render(80);
        assert!(lines[0].contains("Processing..."));
    }

    #[test]
    fn test_loader_reset() {
        let mut loader = Loader::new("Loading...");
        loader.tick();
        loader.tick();
        assert_eq!(loader.current_frame_index(), 2);
        
        loader.reset();
        assert_eq!(loader.current_frame_index(), 0);
    }

    #[test]
    fn test_loader_custom_frames() {
        let frames = vec!["-", "\\", "|", "/"];
        let loader = Loader::with_frames("Loading...", frames);
        let lines = loader.render(80);
        assert!(lines[0].starts_with("-"));
    }
}
