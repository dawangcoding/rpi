//! JSON-RPC 2.0 类型定义
//!
//! 实现标准 JSON-RPC 2.0 规范的请求和响应类型

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC 版本，必须为 "2.0"
    pub jsonrpc: String,
    /// 方法名
    pub method: String,
    /// 方法参数（可选）
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// 请求 ID（可选，无 ID 为通知）
    pub id: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC 版本，固定为 "2.0"
    pub jsonrpc: String,
    /// 成功结果（与 error 互斥）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 错误信息（与 result 互斥）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// 请求 ID（必须与请求 ID 一致）
    pub id: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// 错误码
    pub code: i64,
    /// 错误消息
    pub message: String,
    /// 附加错误数据（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// 标准错误码
/// 解析错误：无效的 JSON
pub const PARSE_ERROR: i64 = -32700;
/// 无效请求：JSON 不是有效的请求对象
pub const INVALID_REQUEST: i64 = -32600;
/// 方法未找到：方法不存在或不可用
pub const METHOD_NOT_FOUND: i64 = -32601;
/// 无效参数：无效的方法参数
pub const INVALID_PARAMS: i64 = -32602;
/// 内部错误：JSON-RPC 内部错误
pub const INTERNAL_ERROR: i64 = -32603;

impl JsonRpcResponse {
    /// 创建成功响应
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// 创建错误响应
    pub fn error(id: Option<serde_json::Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }

    /// 创建带附加数据的错误响应
    pub fn error_with_data(
        id: Option<serde_json::Value>,
        code: i64,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
            id,
        }
    }
}

impl JsonRpcRequest {
    /// 检查是否为通知（无 ID 的请求）
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// 验证请求是否有效
    ///
    /// 返回 Ok(()) 如果请求有效，否则返回 Err(JsonRpcError)
    pub fn validate(&self) -> Result<(), JsonRpcError> {
        if self.jsonrpc != "2.0" {
            return Err(JsonRpcError {
                code: INVALID_REQUEST,
                message: format!("Invalid jsonrpc version: expected '2.0', got '{}'", self.jsonrpc),
                data: None,
            });
        }

        if self.method.is_empty() {
            return Err(JsonRpcError {
                code: INVALID_REQUEST,
                message: "Method name cannot be empty".to_string(),
                data: None,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test".to_string(),
            params: Some(json!({"key": "value"})),
            id: Some(json!(1)),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test\""));
    }

    #[test]
    fn test_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{"key":"value"},"id":1}"#;
        let request: JsonRpcRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "test");
        assert_eq!(request.id, Some(json!(1)));
    }

    #[test]
    fn test_response_success() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({"status": "ok"}));

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_response_error() {
        let response = JsonRpcResponse::error(Some(json!(1)), METHOD_NOT_FOUND, "Method not found");

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, METHOD_NOT_FOUND);
        assert_eq!(error.message, "Method not found");
    }

    #[test]
    fn test_response_error_with_data() {
        let response = JsonRpcResponse::error_with_data(
            Some(json!(1)),
            INVALID_PARAMS,
            "Invalid params",
            json!({"details": "Missing required field"}),
        );

        let error = response.error.unwrap();
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.data.is_some());
    }

    #[test]
    fn test_is_notification() {
        let notification = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "notify".to_string(),
            params: None,
            id: None,
        };
        assert!(notification.is_notification());

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "call".to_string(),
            params: None,
            id: Some(json!(1)),
        };
        assert!(!request.is_notification());
    }

    #[test]
    fn test_validate_valid_request() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_version() {
        let request = JsonRpcRequest {
            jsonrpc: "1.0".to_string(),
            method: "test".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let err = request.validate().unwrap_err();
        assert_eq!(err.code, INVALID_REQUEST);
    }

    #[test]
    fn test_validate_empty_method() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let err = request.validate().unwrap_err();
        assert_eq!(err.code, INVALID_REQUEST);
    }

    #[test]
    fn test_skip_none_fields_in_response() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!("result"));
        let json = serde_json::to_string(&response).unwrap();

        // error 字段应该被跳过
        assert!(!json.contains("\"error\""));
    }
}
