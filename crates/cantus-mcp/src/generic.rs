//! The server's own tools (ADR 0081 §2): jobs (`wait_job`, `cancel_job`, `list_jobs`),
//! turns (`begin_change`, `end_change`) and the agent's own history (`undo_last_ai_action`,
//! `list_ai_actions`). They act on the pump's state, not on the project, so the pump serves
//! them itself by name ([`is_generic`]); their `run` is never called. They join the tool
//! table through `cantus_ui::ai_tools::registry()`.

use cantus_ai::{ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use serde_json::{json, Value};

pub const WAIT_JOB: &str = "wait_job";
pub const CANCEL_JOB: &str = "cancel_job";
pub const LIST_JOBS: &str = "list_jobs";
pub const BEGIN_CHANGE: &str = "begin_change";
pub const END_CHANGE: &str = "end_change";
pub const UNDO_LAST_AI_ACTION: &str = "undo_last_ai_action";
pub const LIST_AI_ACTIONS: &str = "list_ai_actions";

/// Whether the pump serves `name` itself.
pub fn is_generic(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

/// Never called: the pump serves these tools before their `run` could be.
fn served_by_the_pump(_: &mut ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
    Err(ToolError::Failed("served by the MCP pump".into()))
}

fn job_arg() -> Value {
    json!({"type": "string", "description": "The job's handle, as the call that started it answered (\"job-…\")."})
}

fn wait_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "job": job_arg(),
            "timeout_s": {"type": "number", "minimum": 0, "maximum": 60, "default": 30,
                "description": "The longest to wait, in seconds; the answer says the job is still running if it is."}
        },
        "required": ["job"],
        "additionalProperties": false
    })
}

fn job_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"job": job_arg()},
        "required": ["job"],
        "additionalProperties": false
    })
}

fn empty_schema() -> Value {
    json!({"type": "object", "additionalProperties": false})
}

fn begin_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "label": {"type": "string", "minLength": 1, "maxLength": 80,
                "description": "What this change does, in the user's words if you have them (\"write the verse\"): the history shows it as \"AI: <label>\"."}
        },
        "required": ["label"],
        "additionalProperties": false
    })
}

fn end_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"change": {"type": "string", "description": "The handle begin_change answered (\"change-…\")."}},
        "required": ["change"],
        "additionalProperties": false
    })
}

fn actions_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 20,
                "description": "How many of the most recent to list."},
            "all_agents": {"type": "boolean", "default": false,
                "description": "List every agent's steps, not only yours."}
        },
        "additionalProperties": false
    })
}

/// The server's own tools, in their fixed order.
pub static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: WAIT_JOB,
        title: "Wait for a job",
        description: "Wait for a long job (a render, a separation, a transcription) that a tool answered with a job handle. Answers its result when it ends, or its state and progress (0-1) when timeout_s passes first; call again to keep waiting. Jobs are kept for 10 minutes after they end.",
        class: ToolClass::Read,
        input_schema: wait_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
    ToolDef {
        name: CANCEL_JOB,
        title: "Cancel a job",
        description: "Cancel a running job. It stops at its next check; nothing it would have changed in the project is changed.",
        class: ToolClass::Read,
        input_schema: job_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
    ToolDef {
        name: LIST_JOBS,
        title: "List jobs",
        description: "Your jobs: running, done, failed or cancelled, with progress (0-1) and what each does.",
        class: ToolClass::Read,
        input_schema: empty_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
    ToolDef {
        name: BEGIN_CHANGE,
        title: "Begin a change",
        description: "Group the edits that follow into ONE undo step labelled with what they do, until end_change (or 2 minutes without a call). Use it when one request of the user takes several tool calls. Answers a change handle. One change at a time; an edit of the user in between starts a new step.",
        class: ToolClass::Read,
        input_schema: begin_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
    ToolDef {
        name: END_CHANGE,
        title: "End a change",
        description: "End the change begin_change opened: its edits stay one undo step. Answers how many calls it grouped and the project's revision.",
        class: ToolClass::Read,
        input_schema: end_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
    ToolDef {
        name: UNDO_LAST_AI_ACTION,
        title: "Undo your last step",
        description: "Undo the last step in the project's history if it is yours (one tool call, or one change). If the last step is the user's or another agent's, nothing is undone and the answer says whose it is. The user may be asked first.",
        class: ToolClass::Destructive,
        input_schema: empty_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
    ToolDef {
        name: LIST_AI_ACTIONS,
        title: "List your steps",
        description: "The steps agents made in this project, newest first: the tool, what it did, whether it is still in the project or was undone, and the change it belongs to.",
        class: ToolClass::Read,
        input_schema: actions_schema,
        output_schema: None,
        run: ToolRun::Host(served_by_the_pump),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_generic_tool_is_found_and_described() {
        for t in TOOLS {
            assert!(is_generic(t.name));
            assert_eq!((t.input_schema)()["type"], "object", "{}", t.name);
            assert!(!t.description.is_empty());
        }
        assert!(!is_generic("set_tempo"));
        assert_eq!(TOOLS.len(), 7);
    }
}
