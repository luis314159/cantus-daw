//! Resources and prompts as the pump serves them (on the UI thread: a resource reads the
//! project, a prompt is worded in the interface's language).

use crate::agents::AgentHost;
use crate::catalog::{template_match, Catalog};
use crate::door::{UiOp, UiReply};
use crate::jsonrpc::{self, RpcError};
use cantus_ai::{ResourceBody, Role, Said, ToolCx, ToolError};
use serde_json::{json, Value};

pub(crate) fn serve<H: AgentHost>(catalog: &Catalog, host: &mut H, op: UiOp) -> UiReply {
    match op {
        UiOp::ListResources => {
            let list: Vec<Value> = (catalog.resources)()
                .into_iter()
                .map(|r| {
                    json!({"uri": r.uri, "name": r.name, "title": r.title,
                           "description": r.description, "mimeType": r.mime_type})
                })
                .collect();
            UiReply::Value(json!({ "resources": list }))
        }
        UiOp::ListResourceTemplates => {
            let list: Vec<Value> = (catalog.templates)()
                .into_iter()
                .map(|r| {
                    json!({"uriTemplate": r.uri_template, "name": r.name, "title": r.title,
                           "description": r.description, "mimeType": r.mime_type})
                })
                .collect();
            UiReply::Value(json!({ "resourceTemplates": list }))
        }
        UiOp::ReadResource { uri } => read(catalog, host, &uri),
        UiOp::ListPrompts => {
            let say = |k: &'static str| host.say(&Said::new(k));
            let list: Vec<Value> = (catalog.prompts)()
                .into_iter()
                .map(|p| {
                    let args: Vec<Value> = p
                        .arguments
                        .iter()
                        .map(|a| {
                            json!({"name": a.name, "description": say(a.description),
                                   "required": a.required})
                        })
                        .collect();
                    json!({"name": p.name, "title": say(p.title),
                           "description": say(p.description), "arguments": args})
                })
                .collect();
            UiReply::Value(json!({ "prompts": list }))
        }
        UiOp::GetPrompt { name, args } => {
            let Some(p) = (catalog.prompts)().into_iter().find(|p| p.name == name) else {
                return UiReply::Rpc(RpcError::invalid_params(format!("unknown prompt: {name}")));
            };
            if let Some(a) = p
                .arguments
                .iter()
                .find(|a| a.required && args.get(a.name).is_none_or(Value::is_null))
            {
                return UiReply::Rpc(RpcError::invalid_params(format!(
                    "the prompt {name} needs the argument {}",
                    a.name
                )));
            }
            match (p.get)(&ToolCx { host }, &args) {
                Ok(messages) => {
                    let messages: Vec<Value> = messages
                        .iter()
                        .map(|m| {
                            let role = match m.role {
                                Role::User => "user",
                                Role::Assistant => "assistant",
                            };
                            json!({"role": role,
                                   "content": {"type": "text", "text": host.say(&m.said)}})
                        })
                        .collect();
                    let description = host.say(&Said::new(p.description));
                    UiReply::Value(json!({"description": description, "messages": messages}))
                }
                Err(e) => UiReply::Rpc(RpcError::invalid_params(e.to_string())),
            }
        }
        UiOp::CallTool { .. } => UiReply::Rpc(RpcError::new(
            jsonrpc::INTERNAL_ERROR,
            "a tool call is not a catalogue request",
        )),
    }
}

fn not_found(uri: &str) -> UiReply {
    UiReply::Rpc(
        RpcError::new(
            jsonrpc::RESOURCE_NOT_FOUND_LEGACY,
            format!("resource not found: {uri}"),
        )
        .with_data(json!({ "uri": uri })),
    )
}

fn read<H: AgentHost>(catalog: &Catalog, host: &mut H, uri: &str) -> UiReply {
    let found = if let Some(r) = (catalog.resources)().into_iter().find(|r| r.uri == uri) {
        Some(((r.read)(&ToolCx { host }), r.mime_type))
    } else {
        (catalog.templates)().into_iter().find_map(|t| {
            template_match(t.uri_template, uri)
                .map(|vars| ((t.read)(&ToolCx { host }, &vars), t.mime_type))
        })
    };
    match found {
        None | Some((Err(ToolError::NotFound(_)), _)) => not_found(uri),
        Some((Err(e), _)) => UiReply::Rpc(RpcError::new(jsonrpc::INTERNAL_ERROR, e.to_string())),
        Some((Ok(ResourceBody::Text(text)), mime)) => UiReply::Value(json!({
            "contents": [{"uri": uri, "mimeType": mime, "text": text}]
        })),
        Some((Ok(ResourceBody::Blob(bytes)), mime)) => UiReply::Value(json!({
            "contents": [{"uri": uri, "mimeType": mime, "blob": base64(&bytes)}]
        })),
    }
}

/// Standard base64 with padding (RFC 4648 §4), for a resource's bytes.
pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_is_rfc_4648() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(super::base64(&[0xff, 0xfe]), "//4=");
    }
}
