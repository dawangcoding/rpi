//! 扩展系统
//!
//! 支持工具注册、命令注册和生命周期管理的扩展框架

pub mod types;
pub mod loader;
pub mod runner;
pub mod api;
pub mod builtin;

pub use types::{ExtensionManifest, SlashCommand, EventResult, ExtensionToolWrapper, CommandSource, CommandArgs, CommandResult};
pub use loader::{ExtensionLoader, ExtensionFactory, ExtensionRegistry};
pub use runner::ExtensionManager;
pub use api::ExtensionContext;
