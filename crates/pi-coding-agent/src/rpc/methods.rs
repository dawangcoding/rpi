//! RPC 方法处理
//!
//! 实现 JSON-RPC 方法路由和处理

use super::types::*;
use serde_json::json;

/// RPC 方法处理器
pub struct RpcMethodHandler {
    // 未来可注入 AgentSession 等依赖
}

impl RpcMethodHandler {
    /// 创建新的方法处理器
    pub fn new() -> Self {
        Self {}
    }

    /// 分发方法调用
    pub async fn dispatch(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        // 验证请求
        if let Err(error) = request.validate() {
            return Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(error),
                id: request.id.clone(),
            });
        }

        // 通知不返回响应
        if request.is_notification() {
            // 仍然处理通知，但不返回响应
            tracing::debug!("Received notification: {}", request.method);
            return None;
        }

        let response = match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "sendMessage" => self.handle_send_message(request).await,
            "getMessages" => self.handle_get_messages(request).await,
            "executeTool" => self.handle_execute_tool(request).await,
            "getTools" => self.handle_get_tools(request).await,
            "setModel" => self.handle_set_model(request).await,
            "getModels" => self.handle_get_models(request).await,
            "compactSession" => self.handle_compact_session(request).await,
            "ping" => self.handle_ping(request).await,
            _ => JsonRpcResponse::error(
                request.id.clone(),
                METHOD_NOT_FOUND,
                format!("Method '{}' not found", request.method),
            ),
        };

        Some(response)
    }

    /// 初始化会话
    async fn handle_initialize(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        // 提取客户端信息（可选）
        let _client_info = request.params.as_ref().and_then(|p| p.get("clientInfo"));

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "version": "0.1.0",
                "capabilities": [
                    "streaming",
                    "tools",
                    "sessions",
                    "compaction"
                ],
                "serverInfo": {
                    "name": "pi-coding-agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    /// 发送消息
    async fn handle_send_message(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match request.params.as_ref() {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    INVALID_PARAMS,
                    "Missing params for sendMessage",
                );
            }
        };

        let _message = params.get("message").and_then(|m| m.as_str());

        // TODO: 实现实际的消息发送逻辑
        // 需要 AgentSession 集成

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "status": "accepted",
                "message": "Message processing not yet implemented. Requires AgentSession integration."
            }),
        )
    }

    /// 获取消息列表
    async fn handle_get_messages(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let _session_id = request
            .params
            .as_ref()
            .and_then(|p| p.get("sessionId"))
            .and_then(|s| s.as_str());

        // TODO: 实现实际的消息获取逻辑

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "messages": [],
                "note": "Message retrieval not yet implemented. Requires session management integration."
            }),
        )
    }

    /// 执行工具
    async fn handle_execute_tool(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match request.params.as_ref() {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    INVALID_PARAMS,
                    "Missing params for executeTool",
                );
            }
        };

        let tool_name = params.get("tool").and_then(|t| t.as_str());
        let _arguments = params.get("arguments");

        if tool_name.is_none() {
            return JsonRpcResponse::error(
                request.id.clone(),
                INVALID_PARAMS,
                "Missing 'tool' parameter",
            );
        }

        // TODO: 实现实际的工具执行逻辑

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "status": "pending",
                "message": "Tool execution not yet implemented. Requires tool registry integration."
            }),
        )
    }

    /// 获取工具列表
    async fn handle_get_tools(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        // TODO: 从工具注册表获取实际工具列表

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "tools": [
                    {
                        "name": "read",
                        "description": "Read file contents",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "write",
                        "description": "Write content to file",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["path", "content"]
                        }
                    },
                    {
                        "name": "edit",
                        "description": "Edit file using search and replace",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "search": { "type": "string" },
                                "replace": { "type": "string" }
                            },
                            "required": ["path", "search", "replace"]
                        }
                    },
                    {
                        "name": "bash",
                        "description": "Execute shell command",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "command": { "type": "string" }
                            },
                            "required": ["command"]
                        }
                    },
                    {
                        "name": "grep",
                        "description": "Search for pattern in files",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string" },
                                "path": { "type": "string" }
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "find",
                        "description": "Find files matching pattern",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string" }
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "ls",
                        "description": "List directory contents",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            }
                        }
                    }
                ],
                "note": "Tool list is static placeholder. Requires tool registry integration."
            }),
        )
    }

    /// 设置模型
    async fn handle_set_model(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match request.params.as_ref() {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    INVALID_PARAMS,
                    "Missing params for setModel",
                );
            }
        };

        let model_id = params.get("model").and_then(|m| m.as_str());

        if model_id.is_none() {
            return JsonRpcResponse::error(
                request.id.clone(),
                INVALID_PARAMS,
                "Missing 'model' parameter",
            );
        }

        let model_id = model_id.unwrap();

        // 验证模型是否存在
        if pi_ai::models::get_model(model_id).is_none() {
            return JsonRpcResponse::error(
                request.id.clone(),
                INVALID_PARAMS,
                format!("Unknown model: {}", model_id),
            );
        }

        // TODO: 实际设置模型的逻辑需要会话管理

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "status": "ok",
                "model": model_id,
                "message": "Model set successfully. Session integration pending."
            }),
        )
    }

    /// 获取模型列表
    async fn handle_get_models(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        // 获取所有模型
        let models = pi_ai::models::get_models();

        // 转换为简化的 JSON 格式
        let model_list: Vec<serde_json::Value> = models
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "name": m.name,
                    "provider": format!("{:?}", m.provider),
                    "contextWindow": m.context_window,
                    "maxTokens": m.max_tokens,
                    "reasoning": m.reasoning,
                    "cost": {
                        "input": m.cost.input,
                        "output": m.cost.output,
                        "cacheRead": m.cost.cache_read,
                        "cacheWrite": m.cost.cache_write
                    }
                })
            })
            .collect();

        JsonRpcResponse::success(request.id.clone(), json!({ "models": model_list }))
    }

    /// 压缩会话
    async fn handle_compact_session(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let _session_id = request
            .params
            .as_ref()
            .and_then(|p| p.get("sessionId"))
            .and_then(|s| s.as_str());

        // TODO: 实现实际的会话压缩逻辑

        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "status": "pending",
                "message": "Session compaction not yet implemented. Requires compaction module integration."
            }),
        )
    }

    /// Ping 健康检查
    async fn handle_ping(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(request.id.clone(), json!({ "status": "ok" }))
    }
}

impl Default for RpcMethodHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_request(method: &str, id: Option<i64>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: None,
            id: id.map(|i| json!(i)),
        }
    }

    fn create_request_with_params(
        method: &str,
        params: serde_json::Value,
        id: Option<i64>,
    ) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params),
            id: id.map(|i| json!(i)),
        }
    }

    #[tokio::test]
    async fn test_dispatch_initialize() {
        let handler = RpcMethodHandler::new();
        let request = create_request("initialize", Some(1));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert_eq!(result["version"], "0.1.0");
    }

    #[tokio::test]
    async fn test_dispatch_ping() {
        let handler = RpcMethodHandler::new();
        let request = create_request("ping", Some(2));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.result.is_some());
        assert_eq!(response.result.unwrap()["status"], "ok");
    }

    #[tokio::test]
    async fn test_dispatch_unknown_method() {
        let handler = RpcMethodHandler::new();
        let request = create_request("unknownMethod", Some(3));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn test_dispatch_notification_returns_none() {
        let handler = RpcMethodHandler::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: None,
            id: None, // 无 ID，是通知
        };

        let response = handler.dispatch(&request).await;

        assert!(response.is_none());
    }

    #[tokio::test]
    async fn test_get_models_returns_list() {
        let handler = RpcMethodHandler::new();
        let request = create_request("getModels", Some(4));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        let models = result.get("models").unwrap().as_array().unwrap();
        assert!(!models.is_empty());
    }

    #[tokio::test]
    async fn test_set_model_missing_param() {
        let handler = RpcMethodHandler::new();
        let request = create_request("setModel", Some(5));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_set_model_unknown_model() {
        let handler = RpcMethodHandler::new();
        let request = create_request_with_params(
            "setModel",
            json!({ "model": "nonexistent-model-xyz" }),
            Some(6),
        );

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_send_message_missing_params() {
        let handler = RpcMethodHandler::new();
        let request = create_request("sendMessage", Some(7));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_execute_tool_missing_tool_param() {
        let handler = RpcMethodHandler::new();
        let request = create_request_with_params("executeTool", json!({}), Some(8));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_get_tools_returns_list() {
        let handler = RpcMethodHandler::new();
        let request = create_request("getTools", Some(9));

        let response = handler.dispatch(&request).await.unwrap();

        assert!(response.result.is_some());
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert!(!tools.is_empty());

        // 检查工具结构
        let read_tool = tools.iter().find(|t| t["name"] == "read").unwrap();
        assert!(read_tool.get("description").is_some());
        assert!(read_tool.get("inputSchema").is_some());
    }
}
