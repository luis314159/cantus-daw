//! JSON-RPC 2.0 as MCP uses it (https://www.jsonrpc.org/specification; MCP's base
//! protocol): one message per call — batches were removed in 2025-06-18 — ids that are a
//! string or an integer and never `null`, notifications without an id, and the error codes
//! below.

use serde_json::{json, Map, Value};

/// The message could not be parsed as JSON.
pub const PARSE_ERROR: i64 = -32700;
/// Not a valid request object (or a batch, or too large).
pub const INVALID_REQUEST: i64 = -32600;
/// No such method.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Bad parameters: an unknown tool or prompt, a missing `_meta` field, a malformed call.
pub const INVALID_PARAMS: i64 = -32602;
/// Something went wrong inside Cantus.
pub const INTERNAL_ERROR: i64 = -32603;
/// A resource that does not exist, as 2025-06-18 and 2025-11-25 say it (2026-07-28 says
/// [`INVALID_PARAMS`] and forbids this code).
pub const RESOURCE_NOT_FOUND_LEGACY: i64 = -32002;
/// 2026-07-28: the version a request asked for is not one this server speaks.
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// A request's id, kept as it came (a string or an integer).
pub type Id = Value;

/// An error answer before it is put in its envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        RpcError::new(INVALID_PARAMS, message)
    }
}

/// What one incoming message is.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    Request {
        id: Id,
        method: String,
        params: Map<String, Value>,
    },
    Notification {
        method: String,
        params: Map<String, Value>,
    },
    /// An answer to a request of ours (Cantus sends none today): accepted, ignored.
    Response,
}

/// Why a message could not be read: the error to send back, with the id when one could be
/// read (`Value::Null` otherwise).
pub type Unread = Box<(Id, RpcError)>;

/// Read one message.
pub fn parse(msg: Value) -> Result<Incoming, Unread> {
    let mut obj = match msg {
        Value::Object(o) => o,
        Value::Array(_) => return Err(Box::new((
            Value::Null,
            RpcError::new(
                INVALID_REQUEST,
                "batches are not supported: send one message at a time (MCP 2025-06-18 and later)",
            ),
        ))),
        _ => {
            return Err(Box::new((
                Value::Null,
                RpcError::new(INVALID_REQUEST, "a message is a JSON object"),
            )))
        }
    };
    let id = obj.remove("id");
    let id_for_error = id
        .clone()
        .filter(|i| i.is_string() || i.is_i64() || i.is_u64())
        .unwrap_or(Value::Null);
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(Box::new((
            id_for_error,
            RpcError::new(INVALID_REQUEST, "\"jsonrpc\" must be \"2.0\""),
        )));
    }
    let Some(method) = obj.remove("method") else {
        if obj.contains_key("result") || obj.contains_key("error") {
            return Ok(Incoming::Response);
        }
        return Err(Box::new((
            id_for_error,
            RpcError::new(INVALID_REQUEST, "a request needs a \"method\""),
        )));
    };
    let Value::String(method) = method else {
        return Err(Box::new((
            id_for_error,
            RpcError::new(INVALID_REQUEST, "\"method\" must be a string"),
        )));
    };
    let params = match obj.remove("params") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(p)) => p,
        Some(_) => {
            return Err(Box::new((
                id_for_error,
                RpcError::new(INVALID_REQUEST, "\"params\" must be an object"),
            )))
        }
    };
    match id {
        None => Ok(Incoming::Notification { method, params }),
        Some(id) if id.is_string() || id.is_i64() || id.is_u64() => {
            Ok(Incoming::Request { id, method, params })
        }
        Some(_) => Err(Box::new((
            Value::Null,
            RpcError::new(
                INVALID_REQUEST,
                "\"id\" must be a string or an integer, never null",
            ),
        ))),
    }
}

/// A result's envelope.
pub fn result(id: Id, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// An error's envelope.
pub fn error(id: Id, e: RpcError) -> Value {
    let mut err = json!({ "code": e.code, "message": e.message });
    if let Some(data) = e.data {
        err["data"] = data;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": err })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_notifications_and_responses_are_told_apart() {
        let r = parse(json!({"jsonrpc":"2.0","id":7,"method":"ping"})).unwrap();
        assert_eq!(
            r,
            Incoming::Request {
                id: json!(7),
                method: "ping".into(),
                params: Map::new()
            }
        );
        let n = parse(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).unwrap();
        assert!(matches!(n, Incoming::Notification { .. }));
        let a = parse(json!({"jsonrpc":"2.0","id":"x","result":{}})).unwrap();
        assert_eq!(a, Incoming::Response);
    }

    #[test]
    fn malformed_messages_are_refused_with_the_right_codes() {
        let (id, e) = *parse(json!([{"jsonrpc":"2.0","id":1,"method":"ping"}])).unwrap_err();
        assert_eq!((id, e.code), (Value::Null, INVALID_REQUEST));
        let (id, e) = *parse(json!({"jsonrpc":"1.0","id":3,"method":"ping"})).unwrap_err();
        assert_eq!((id, e.code), (json!(3), INVALID_REQUEST));
        let (id, e) = *parse(json!({"jsonrpc":"2.0","id":null,"method":"ping"})).unwrap_err();
        assert_eq!((id, e.code), (Value::Null, INVALID_REQUEST));
        let (_, e) = *parse(json!({"jsonrpc":"2.0","id":1,"method":5})).unwrap_err();
        assert_eq!(e.code, INVALID_REQUEST);
        let (_, e) = *parse(json!({"jsonrpc":"2.0","id":1,"method":"x","params":[1]})).unwrap_err();
        assert_eq!(e.code, INVALID_REQUEST);
        let (_, e) = *parse(json!("ping")).unwrap_err();
        assert_eq!(e.code, INVALID_REQUEST);
    }

    #[test]
    fn errors_carry_their_data() {
        let e = error(
            json!(1),
            RpcError::new(UNSUPPORTED_PROTOCOL_VERSION, "no").with_data(json!({"a":1})),
        );
        assert_eq!(e["error"]["data"]["a"], 1);
        assert_eq!(e["id"], 1);
    }
}
