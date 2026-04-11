//! RPC HTTP 服务器
//!
//! 基于 hyper 实现的 JSON-RPC HTTP 服务器

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use super::methods::RpcMethodHandler;
use super::types::*;

/// RPC 服务器
pub struct RpcServer {
    handler: Arc<RpcMethodHandler>,
    addr: SocketAddr,
}

impl RpcServer {
    /// 创建新的 RPC 服务器
    pub fn new(port: u16) -> Self {
        Self {
            handler: Arc::new(RpcMethodHandler::new()),
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
        }
    }

    /// 启动 RPC 服务器
    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        tracing::info!("RPC server listening on {}", self.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream);
            let handler = self.handler.clone();

            tokio::spawn(async move {
                let service = service_fn(move |req: Request<Incoming>| {
                    let handler = handler.clone();
                    async move { handle_request(handler, req).await }
                });

                if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                    tracing::error!("RPC connection error: {}", e);
                }
            });
        }
    }

    /// 获取服务器地址
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

/// 构建 CORS 响应头
fn with_cors_headers(builder: hyper::http::response::Builder) -> hyper::http::response::Builder {
    builder
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
}

/// 处理 HTTP 请求
async fn handle_request(
    handler: Arc<RpcMethodHandler>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, anyhow::Error> {
    // 处理 OPTIONS 预检请求
    if req.method() == Method::OPTIONS {
        return Ok(with_cors_headers(Response::builder())
            .status(StatusCode::NO_CONTENT)
            .body(Full::new(Bytes::new()))?);
    }

    // 只接受 POST 请求
    if req.method() != Method::POST {
        return Ok(with_cors_headers(Response::builder())
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Full::new(Bytes::from("Method not allowed. Use POST.")))?);
    }

    // 读取请求体
    let body_bytes = req.collect().await?.to_bytes();
    let body_str = match std::str::from_utf8(&body_bytes) {
        Ok(s) => s,
        Err(e) => {
            let error_response = JsonRpcResponse::error(
                None,
                PARSE_ERROR,
                format!("Invalid UTF-8 in request body: {}", e),
            );
            let json = serde_json::to_string(&error_response).unwrap();
            return Ok(with_cors_headers(Response::builder())
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from(json)))?);
        }
    };

    // 尝试解析为 JSON
    let json_value: serde_json::Value = match serde_json::from_str(body_str) {
        Ok(v) => v,
        Err(e) => {
            let error_response =
                JsonRpcResponse::error(None, PARSE_ERROR, format!("Invalid JSON: {}", e));
            let json = serde_json::to_string(&error_response).unwrap();
            return Ok(with_cors_headers(Response::builder())
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from(json)))?);
        }
    };

    // 处理单个请求或批量请求
    let response_json = if json_value.is_array() {
        // 批量请求
        let requests: Vec<JsonRpcRequest> = match serde_json::from_value(json_value) {
            Ok(r) => r,
            Err(e) => {
                let error_response =
                    JsonRpcResponse::error(None, INVALID_REQUEST, format!("Invalid request batch: {}", e));
                let json = serde_json::to_string(&error_response).unwrap();
                return Ok(with_cors_headers(Response::builder())
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from(json)))?);
            }
        };

        // 处理每个请求
        let responses: Vec<JsonRpcResponse> = futures::future::join_all(
            requests
                .iter()
                .map(|req| handler.dispatch(req))
                .collect::<Vec<_>>(),
        )
        .await
        .into_iter()
        .flatten()
        .collect();

        serde_json::to_string(&responses).unwrap()
    } else {
        // 单个请求
        let request: JsonRpcRequest = match serde_json::from_value(json_value) {
            Ok(r) => r,
            Err(e) => {
                let error_response =
                    JsonRpcResponse::error(None, INVALID_REQUEST, format!("Invalid request: {}", e));
                let json = serde_json::to_string(&error_response).unwrap();
                return Ok(with_cors_headers(Response::builder())
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from(json)))?);
            }
        };

        // 处理请求
        match handler.dispatch(&request).await {
            Some(response) => serde_json::to_string(&response).unwrap(),
            None => {
                // 通知请求不返回响应
                return Ok(with_cors_headers(Response::builder())
                    .status(StatusCode::NO_CONTENT)
                    .body(Full::new(Bytes::new()))?);
            }
        }
    };

    Ok(with_cors_headers(Response::builder())
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(response_json)))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = RpcServer::new(3100);
        assert_eq!(server.addr().port(), 3100);
        assert_eq!(server.addr().ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn test_server_default_port() {
        let server = RpcServer::new(3100);
        assert_eq!(server.addr().port(), 3100);
    }
}
