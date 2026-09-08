//! Provider abstraction (idea.md §4.2). Every AI feature in Cantus talks to this trait
//! and nothing else, so no vendor can become a dependency.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("no AI provider is configured")]
    NoProvider,
    #[error("provider {provider} is unavailable: {reason}")]
    Unavailable { provider: String, reason: String },
    #[error("request rejected: {0}")]
    Rejected(String),
    #[error("malformed response: {0}")]
    Malformed(String),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: Role,
    pub content: String,
}

/// A tool the model may call. `input_schema` is JSON Schema, the lingua franca of every
/// provider (and of MCP).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub input: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiRequest {
    pub system: String,
    pub messages: Vec<AiMessage>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
}

/// What a request would send off the machine. Shown to the user *before* sending
/// (transparency principle, idea.md §4.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disclosure {
    pub provider: String,
    pub local: bool,
    pub bytes: usize,
    pub includes_project_state: bool,
    pub tool_count: usize,
}

impl AiRequest {
    pub fn disclosure(&self, provider: &dyn AiProvider) -> Disclosure {
        let bytes = serde_json::to_vec(self).map(|v| v.len()).unwrap_or(0);
        Disclosure {
            provider: provider.display_name().to_string(),
            local: provider.is_local(),
            bytes,
            includes_project_state: self.system.contains("<project>"),
            tool_count: self.tools.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AiResponse {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Stable identifier (`anthropic`, `openai`, `ollama`, …).
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    /// `true` if no byte leaves the machine.
    fn is_local(&self) -> bool;
    async fn complete(&self, request: AiRequest) -> Result<AiResponse, AiError>;
}

/// The provider you get when none is configured. Every feature must degrade gracefully
/// against it: Cantus is a complete DAW without AI (idea.md §4.6).
#[derive(Debug, Default)]
pub struct NullProvider;

#[async_trait]
impl AiProvider for NullProvider {
    fn id(&self) -> &str {
        "none"
    }
    fn display_name(&self) -> &str {
        "No provider"
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn complete(&self, _request: AiRequest) -> Result<AiResponse, AiError> {
        Err(AiError::NoProvider)
    }
}

/// Which tools an agent may run without asking (idea.md §4.4, granular permissions).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Permissions {
    /// Tools that run without confirmation. Everything else requires the user to approve.
    pub auto_approve: Vec<String>,
    /// Tools that are never allowed, even with confirmation.
    pub denied: Vec<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    AskUser,
    Deny,
}

impl Permissions {
    pub fn decide(&self, tool: &str) -> Decision {
        if self.denied.iter().any(|t| t == tool) {
            Decision::Deny
        } else if self.auto_approve.iter().any(|t| t == tool) {
            Decision::Allow
        } else {
            Decision::AskUser
        }
    }

    /// Sensible default: reading and suggesting is free, mutations ask, export never
    /// happens behind the user's back.
    pub fn conservative() -> Self {
        Permissions {
            auto_approve: vec!["describe_project".into()],
            denied: vec!["export".into()],
        }
    }
}
