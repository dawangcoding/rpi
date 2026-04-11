//! JSON-RPC 2.0 服务模式
//!
//! 提供 HTTP API 接口，支持第三方工具集成
//!
//! # 功能
//!
//! - 完整的 JSON-RPC 2.0 协议实现
//! - 支持 HTTP/1.1 传输
//! - CORS 跨域支持
//! - 批量请求支持
//!
//! # 支持的方法
//!
//! - `initialize`: 初始化会话
//! - `sendMessage`: 发送消息
//! - `getMessages`: 获取消息列表
//! - `executeTool`: 执行工具
//! - `getTools`: 获取工具列表
//! - `setModel`: 设置模型
//! - `getModels`: 获取模型列表
//! - `compactSession`: 压缩会话
//! - `ping`: 健康检查
//!
//! # 示例
//!
//! ```ignore
//! use pi_coding_agent::rpc::RpcServer;
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = RpcServer::new(3100);
//!     server.run().await.unwrap();
//! }
//! ```

pub mod methods;
pub mod server;
pub mod types;

pub use methods::RpcMethodHandler;
pub use server::RpcServer;
pub use types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
