//! 会话压缩模块
//!
//! 提供长会话自动/手动压缩功能，通过 LLM 生成摘要替代早期消息，释放上下文空间。

pub mod compactor;
pub mod summary_prompt;

pub use compactor::*;
