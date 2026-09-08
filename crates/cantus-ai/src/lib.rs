//! # cantus-ai
//!
//! The AI layer designed in from day one (idea.md §4). It is strictly asynchronous, never
//! touches the audio thread, and can only act on a project by emitting ordinary commands
//! through the same bus the UI uses.
//!
//! Providers (cloud or local) implement [`AiProvider`]; nothing else in Cantus knows which
//! one is active. The MVP ships the trait, the tool catalogue and the command bridge with
//! [`NullProvider`] as the only implementation.

#![forbid(unsafe_code)]

pub mod bridge;
pub mod provider;

pub use bridge::{describe_project, to_command, tool_specs, BridgeError};
pub use provider::{
    AiError, AiMessage, AiProvider, AiRequest, AiResponse, Decision, Disclosure, NullProvider,
    Permissions, Role, ToolCall, ToolSpec,
};
