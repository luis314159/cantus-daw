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
// The tool table (round 34, ADRs 0081–0082).
pub mod tool;
pub mod tools_project;

pub use bridge::{describe_project, to_command, tool_specs, BridgeError, Planned}; // r34:tools — Planned (review #18)
pub use provider::{
    AiError, AiMessage, AiProvider, AiRequest, AiResponse, Decision, Disclosure, NullProvider,
    Permissions, Role, ToolCall, ToolSpec,
};
pub use tool::{
    fill, is_blank, is_invisible, may_touch, open_touch, quote_name, resolve_touch,
    strip_invisible, EditPlan, Job, JobCx, JobDone, JobWork, PromptArg, PromptDef, PromptMessage,
    ResourceBody, ResourceDef, ResourceTemplateDef, Said, SaidArg, TemplateVars, ThenPlan,
    ToolClass, ToolCx, ToolDef, ToolError, ToolHost, ToolRun, Touch, Wording,
};
