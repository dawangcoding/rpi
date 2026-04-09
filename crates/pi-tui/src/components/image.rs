//! 图像显示组件
//! 支持 Kitty 和 iTerm2 终端图像协议

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use crate::tui::Component;
use crate::terminal_image::{
    detect_capabilities, encode_iterm2, encode_kitty, get_image_dimensions, 
    allocate_image_id, ImageDimensions, TerminalCapabilities, ImageProtocol
};

/// 图像组件
pub struct ImageComponent {
    data: Vec<u8>,
    mime_type: String,
    dimensions: Option<ImageDimensions>,
    rendered_lines: Vec<String>,
    needs_render: bool,
    image_id: Option<u32>,
    max_width: Option<u16>,
    max_height: Option<u16>,
    capabilities: TerminalCapabilities,
}

impl ImageComponent {
    /// 创建新的图像组件
    pub fn new(data: Vec<u8>, mime_type: &str) -> Self {
        let dimensions = get_image_dimensions(&data, mime_type);
        let capabilities = detect_capabilities();
        
        Self {
            data,
            mime_type: mime_type.to_string(),
            dimensions,
            rendered_lines: Vec::new(),
            needs_render: true,
            image_id: None,
            max_width: None,
            max_height: None,
            capabilities,
        }
    }

    /// 从 base64 数据创建图像组件
    pub fn from_base64(base64_data: &str, mime_type: &str) -> anyhow::Result<Self> {
        let data = BASE64.decode(base64_data)?;
        Ok(Self::new(data, mime_type))
    }

    /// 设置最大宽度
    pub fn set_max_width(&mut self, width: u16) {
        self.max_width = Some(width);
        self.needs_render = true;
    }

    /// 设置最大高度
    pub fn set_max_height(&mut self, height: u16) {
        self.max_height = Some(height);
        self.needs_render = true;
    }

    /// 获取图像尺寸
    pub fn dimensions(&self) -> Option<ImageDimensions> {
        self.dimensions
    }

    /// 获取 MIME 类型
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// 获取图像 ID（Kitty 协议）
    pub fn image_id(&self) -> Option<u32> {
        self.image_id
    }

    /// 计算图像在终端中的尺寸
    fn calculate_render_size(&self, available_width: u16) -> (u16, u16) {
        const DEFAULT_CELL_WIDTH: u16 = 9;
        const DEFAULT_CELL_HEIGHT: u16 = 18;

        let dims = match self.dimensions {
            Some(d) => d,
            None => return (20, 10), // 默认尺寸
        };

        let max_width = self.max_width.unwrap_or(available_width).min(available_width);
        let max_height = self.max_height.unwrap_or(30);

        // 计算目标宽度（像素）
        let target_width_px = max_width as u32 * DEFAULT_CELL_WIDTH as u32;
        
        // 计算缩放比例
        let scale = target_width_px as f32 / dims.width as f32;
        let scaled_height_px = dims.height as f32 * scale;
        
        // 计算行数
        let rows = (scaled_height_px / DEFAULT_CELL_HEIGHT as f32).ceil() as u16;
        
        (max_width, rows.min(max_height).max(1))
    }

    /// 渲染图像为终端字符串
    fn render_image(&mut self, width: u16) -> Vec<String> {
        if self.data.is_empty() {
            return vec!["[No image data]".to_string()];
        }

        let (cols, rows) = self.calculate_render_size(width);

        match self.capabilities.image_protocol {
            ImageProtocol::Kitty => {
                // 分配或复用图像 ID
                let id = self.image_id.unwrap_or_else(|| {
                    let new_id = allocate_image_id();
                    self.image_id = Some(new_id);
                    new_id
                });

                let sequence = encode_kitty(&self.data, id, cols, rows);
                
                // 返回多行：前 rows-1 行是空的，最后一行包含图像序列
                let mut lines = vec![String::new(); (rows as usize).saturating_sub(1)];
                
                // 移动光标到第一行然后输出图像
                let move_up = if rows > 1 {
                    format!("\x1b[{}A", rows - 1)
                } else {
                    String::new()
                };
                lines.push(format!("{}{}", move_up, sequence));
                
                lines
            }
            ImageProtocol::ITerm2 => {
                let sequence = encode_iterm2(&self.data, cols, rows);
                let mut lines = vec![String::new(); (rows as usize).saturating_sub(1)];
                
                let move_up = if rows > 1 {
                    format!("\x1b[{}A", rows - 1)
                } else {
                    String::new()
                };
                lines.push(format!("{}{}", move_up, sequence));
                
                lines
            }
            ImageProtocol::None => {
                // 终端不支持图像，显示回退信息
                self.render_fallback(cols, rows)
            }
        }
    }

    /// 渲染回退信息
    fn render_fallback(&self, width: u16, height: u16) -> Vec<String> {
        let mut lines = Vec::new();
        
        // 顶部边框
        let top = format!("┌{}┐", "─".repeat(width as usize - 2));
        lines.push(top);
        
        // 中间：显示图像信息
        let info = format!("[Image: {}]", self.mime_type);
        let padded_info = format!("│{:^width$}│", info, width = (width as usize - 2));
        
        let middle_rows = height.saturating_sub(2);
        let info_row = middle_rows / 2;
        
        for i in 0..middle_rows {
            if i == info_row {
                lines.push(padded_info.clone());
            } else {
                lines.push(format!("│{}│", " ".repeat(width as usize - 2)));
            }
        }
        
        // 底部边框
        let bottom = format!("└{}┘", "─".repeat(width as usize - 2));
        lines.push(bottom);
        
        lines
    }
}

impl Component for ImageComponent {
    fn render(&self, width: u16) -> Vec<String> {
        // 由于 self.render_image 需要 &mut self，我们需要克隆或使用缓存
        // 这里我们直接返回缓存的渲染结果
        if self.rendered_lines.is_empty() || self.needs_render {
            // 创建临时副本来渲染
            let mut temp = Self {
                data: self.data.clone(),
                mime_type: self.mime_type.clone(),
                dimensions: self.dimensions,
                rendered_lines: Vec::new(),
                needs_render: true,
                image_id: self.image_id,
                max_width: self.max_width,
                max_height: self.max_height,
                capabilities: self.capabilities.clone(),
            };
            temp.render_image(width)
        } else {
            self.rendered_lines.clone()
        }
    }

    fn invalidate(&mut self) {
        self.needs_render = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_component_new() {
        // 创建一个最小的 PNG 文件头
        let mut png_data = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        // IHDR chunk
        png_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0d]); // length
        png_data.extend_from_slice(b"IHDR");
        png_data.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]); // width: 256
        png_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x80]); // height: 128
        png_data.extend_from_slice(&[0x08, 0x02, 0x00, 0x00, 0x00]); // bit depth, color type, etc.
        
        let img = ImageComponent::new(png_data, "image/png");
        
        assert_eq!(img.mime_type(), "image/png");
        assert!(img.dimensions().is_some());
        
        let dims = img.dimensions().unwrap();
        assert_eq!(dims.width, 256);
        assert_eq!(dims.height, 128);
    }

    #[test]
    fn test_image_component_base64() {
        // 创建一个最小的 PNG 并编码为 base64
        let mut png_data = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        png_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0d]);
        png_data.extend_from_slice(b"IHDR");
        png_data.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]);
        png_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x80]);
        png_data.extend_from_slice(&[0x08, 0x02, 0x00, 0x00, 0x00]);
        
        let base64 = BASE64.encode(&png_data);
        let img = ImageComponent::from_base64(&base64, "image/png").unwrap();
        
        assert_eq!(img.mime_type(), "image/png");
        assert!(img.dimensions().is_some());
    }

    #[test]
    fn test_image_component_empty() {
        let img = ImageComponent::new(vec![], "image/png");
        let lines = img.render(40);
        
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("No image data"));
    }

    #[test]
    fn test_image_component_dimensions() {
        let mut png_data = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        png_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0d]);
        png_data.extend_from_slice(b"IHDR");
        png_data.extend_from_slice(&[0x00, 0x00, 0x02, 0x00]); // width: 512
        png_data.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]); // height: 256
        png_data.extend_from_slice(&[0x08, 0x02, 0x00, 0x00, 0x00]);
        
        let mut img = ImageComponent::new(png_data, "image/png");
        img.set_max_width(40);
        img.set_max_height(20);
        
        assert_eq!(img.max_width, Some(40));
        assert_eq!(img.max_height, Some(20));
    }
}
