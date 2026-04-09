//! 终端控制模块
//! 提供终端抽象 trait 和基于 crossterm 的实现

use anyhow::Result;
use crossterm::{
    cursor::{MoveTo, Show, Hide},
    event::{DisableBracketedPaste, EnableBracketedPaste},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType},
    ExecutableCommand, QueueableCommand,
};
use std::io::{self, Write};

/// 终端抽象 trait
pub trait Terminal: Send {
    /// 写入数据到终端
    fn write(&mut self, data: &str) -> Result<()>;
    
    /// 刷新输出
    fn flush(&mut self) -> Result<()>;
    
    /// 获取终端尺寸 (width, height)
    fn size(&self) -> (u16, u16);
    
    /// 启用原始模式
    fn enable_raw_mode(&mut self) -> Result<()>;
    
    /// 禁用原始模式
    fn disable_raw_mode(&mut self) -> Result<()>;
    
    /// 进入备用屏幕
    fn enter_alternate_screen(&mut self) -> Result<()>;
    
    /// 离开备用屏幕
    fn leave_alternate_screen(&mut self) -> Result<()>;
    
    /// 显示光标
    fn show_cursor(&mut self) -> Result<()>;
    
    /// 隐藏光标
    fn hide_cursor(&mut self) -> Result<()>;
    
    /// 移动光标到指定位置 (row, col)，0-indexed
    fn move_cursor(&mut self, row: u16, col: u16) -> Result<()>;
    
    /// 清除当前行
    fn clear_line(&mut self) -> Result<()>;
    
    /// 清除整个屏幕
    fn clear_screen(&mut self) -> Result<()>;
}

/// 基于 crossterm 的进程终端实现
pub struct ProcessTerminal {
    stdout: io::Stdout,
    raw_mode_enabled: bool,
    in_alternate_screen: bool,
}

impl ProcessTerminal {
    /// 创建新的进程终端
    pub fn new() -> Self {
        Self {
            stdout: io::stdout(),
            raw_mode_enabled: false,
            in_alternate_screen: false,
        }
    }
}

impl Default for ProcessTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for ProcessTerminal {
    fn write(&mut self, data: &str) -> Result<()> {
        self.stdout.write_all(data.as_bytes())?;
        Ok(())
    }
    
    fn flush(&mut self) -> Result<()> {
        self.stdout.flush()?;
        Ok(())
    }
    
    fn size(&self) -> (u16, u16) {
        terminal::size().unwrap_or((80, 24))
    }
    
    fn enable_raw_mode(&mut self) -> Result<()> {
        if !self.raw_mode_enabled {
            terminal::enable_raw_mode()?;
            self.raw_mode_enabled = true;
        }
        Ok(())
    }
    
    fn disable_raw_mode(&mut self) -> Result<()> {
        if self.raw_mode_enabled {
            terminal::disable_raw_mode()?;
            self.raw_mode_enabled = false;
        }
        Ok(())
    }
    
    fn enter_alternate_screen(&mut self) -> Result<()> {
        if !self.in_alternate_screen {
            self.stdout.execute(EnterAlternateScreen)?;
            self.in_alternate_screen = true;
        }
        Ok(())
    }
    
    fn leave_alternate_screen(&mut self) -> Result<()> {
        if self.in_alternate_screen {
            self.stdout.execute(LeaveAlternateScreen)?;
            self.in_alternate_screen = false;
        }
        Ok(())
    }
    
    fn show_cursor(&mut self) -> Result<()> {
        self.stdout.execute(Show)?;
        Ok(())
    }
    
    fn hide_cursor(&mut self) -> Result<()> {
        self.stdout.execute(Hide)?;
        Ok(())
    }
    
    fn move_cursor(&mut self, row: u16, col: u16) -> Result<()> {
        self.stdout.queue(MoveTo(col, row))?;
        Ok(())
    }
    
    fn clear_line(&mut self) -> Result<()> {
        self.stdout.execute(Clear(ClearType::CurrentLine))?;
        Ok(())
    }
    
    fn clear_screen(&mut self) -> Result<()> {
        self.stdout.execute(Clear(ClearType::All))?;
        self.stdout.execute(MoveTo(0, 0))?;
        Ok(())
    }
}

impl Drop for ProcessTerminal {
    fn drop(&mut self) {
        // 确保清理终端状态
        let _ = self.show_cursor();
        let _ = self.leave_alternate_screen();
        let _ = self.disable_raw_mode();
        let _ = self.flush();
    }
}

/// 启用 bracketed paste 模式
pub fn enable_bracketed_paste() -> Result<()> {
    io::stdout().execute(EnableBracketedPaste)?;
    Ok(())
}

/// 禁用 bracketed paste 模式
pub fn disable_bracketed_paste() -> Result<()> {
    io::stdout().execute(DisableBracketedPaste)?;
    Ok(())
}
