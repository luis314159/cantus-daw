//! What the server offers, and how each thing is described on the wire: the tool table
//! (`cantus_ui::ai_tools::registry()` in the application), the resources, their templates
//! and the prompts — each list a function, so the server's threads read the same static
//! tables the UI thread runs.

use cantus_ai::{PromptDef, ResourceDef, ResourceTemplateDef, TemplateVars, ToolClass, ToolDef};
use serde_json::{json, Value};

/// Everything a server lists. Plain function pointers: `Copy`, `Send`, the same on every
/// thread.
#[derive(Clone, Copy)]
pub struct Catalog {
    pub tools: fn() -> Vec<&'static ToolDef>,
    pub resources: fn() -> Vec<&'static ResourceDef>,
    pub templates: fn() -> Vec<&'static ResourceTemplateDef>,
    pub prompts: fn() -> Vec<&'static PromptDef>,
}

fn none<T: 'static>() -> Vec<&'static T> {
    Vec::new()
}

impl Catalog {
    /// A catalogue of tools only.
    pub fn tools_only(tools: fn() -> Vec<&'static ToolDef>) -> Self {
        Catalog {
            tools,
            resources: none::<ResourceDef>,
            templates: none::<ResourceTemplateDef>,
            prompts: none::<PromptDef>,
        }
    }

    pub fn tool(&self, name: &str) -> Option<&'static ToolDef> {
        (self.tools)().into_iter().find(|t| t.name == name)
    }
}

/// Whether a class changes the project or the disk: such tools take `expected_revision`.
pub fn mutates(class: ToolClass) -> bool {
    matches!(
        class,
        ToolClass::Edit | ToolClass::Destructive | ToolClass::Files
    )
}

/// The argument every mutating tool takes, added to its schema by the server (tools never
/// see it: the pump checks it and takes it out).
pub const EXPECTED_REVISION: &str = "expected_revision";

/// The argument every mutating tool takes to join a change (`begin_change`'s handle): the
/// only way in 2026-07-28, where nothing is implied by an earlier call (review 1 #3).
pub const CHANGE: &str = "change";

/// The hints a client may show (MCP `ToolAnnotations`): read from the class. Cantus decides
/// by class itself; these only help a client word its own prompt.
pub fn annotations(title: &str, class: ToolClass) -> Value {
    let (read_only, destructive, idempotent, open_world) = match class {
        ToolClass::Read => (true, false, true, false),
        ToolClass::Edit => (false, false, false, false),
        ToolClass::Destructive => (false, true, false, false),
        ToolClass::Files => (false, true, false, true),
        ToolClass::Transport => (false, false, true, false),
    };
    json!({
        "title": title,
        "readOnlyHint": read_only,
        "destructiveHint": destructive,
        "idempotentHint": idempotent,
        "openWorldHint": open_world,
    })
}

/// One tool as `tools/list` gives it.
pub fn tool_json(t: &ToolDef) -> Value {
    let mut input = (t.input_schema)();
    if mutates(t.class) {
        if let Some(props) = input
            .as_object_mut()
            .map(|o| o.entry("properties").or_insert_with(|| json!({})))
            .and_then(Value::as_object_mut)
        {
            props.entry(EXPECTED_REVISION).or_insert_with(|| {
                json!({
                    "type": "integer",
                    "minimum": 0,
                    "description": "The project's revision this call was planned against (every result says the revision). If the project changed since, nothing is done and the error gives the new revision: read again and retry."
                })
            });
            // The server's own tools join no change (review 2 #7).
            if !crate::generic::is_generic(t.name) {
                props.entry(CHANGE).or_insert_with(|| {
                    json!({
                        "type": "string",
                        "description": "A change's handle from begin_change: this call joins that change's one undo step."
                    })
                });
            }
        }
    }
    let mut tool = json!({
        "name": t.name,
        "title": t.title,
        "description": t.description,
        "inputSchema": input,
        "annotations": annotations(t.title, t.class),
    });
    if let Some(out) = t.output_schema {
        let mut out = out();
        // A mutating call's result says the revision it left (review 1 #7).
        if mutates(t.class) {
            if let Some(props) = out
                .as_object_mut()
                .filter(|o| o.get("type").and_then(Value::as_str) == Some("object"))
                .map(|o| o.entry("properties").or_insert_with(|| json!({})))
                .and_then(Value::as_object_mut)
            {
                props.entry("revision").or_insert_with(|| {
                    json!({"type": "integer", "minimum": 0, "description": "The project's revision after this call."})
                });
            }
        }
        // A job answers its result, or — still running after a few seconds — its handle:
        // the schema says both, so `structuredContent` always conforms (review 2 #1).
        if matches!(t.run, cantus_ai::ToolRun::Job(_)) {
            // The root stays an object, as 2025-06-18 and 2025-11-25 require (review 3 #1).
            out = json!({ "type": "object", "anyOf": [out, job_descriptor()] });
        }
        tool["outputSchema"] = out;
    }
    tool
}

/// What a job's handle answer looks like (`wait_job`, `list_jobs` and a job tool's first
/// answer when the job runs on).
pub fn job_descriptor() -> Value {
    json!({
        "type": "object",
        "properties": {
            "job": {"type": "string", "description": "The handle: pass it to wait_job or cancel_job."},
            "tool": {"type": "string"},
            "said": {"type": "string"},
            "state": {"enum": ["running", "cancelling", "done", "failed", "cancelled"]},
            "progress": {"type": "number", "minimum": 0, "maximum": 1},
            "seconds": {"type": "number"},
            "next": {"type": "string"}
        },
        "required": ["job", "state"]
    })
}

/// `tools/list`'s result: every tool, in the registry's fixed order.
pub fn tools_list(catalog: &Catalog) -> Value {
    let tools: Vec<Value> = (catalog.tools)().into_iter().map(tool_json).collect();
    json!({ "tools": tools })
}

/// Match `uri` against a level-1 URI template (`cantus://tracks/{id}/notes`): each `{name}`
/// takes one segment (no `/`), percent-decoded. `None` when it does not match.
pub fn template_match(template: &str, uri: &str) -> Option<TemplateVars> {
    let mut vars = TemplateVars::new();
    let mut rest = uri;
    let mut t = template;
    while !t.is_empty() {
        if let Some(open) = t.strip_prefix('{') {
            let close = open.find('}')?;
            let name = &open[..close];
            t = &open[close + 1..];
            // The variable runs to the template's next literal character (or a `/`).
            let stop = t.chars().next();
            let end = rest
                .find(|c: char| c == '/' || Some(c) == stop)
                .unwrap_or(rest.len());
            if end == 0 {
                return None;
            }
            // Decoded, a value stays one plain segment: no separator, no NUL, never `.` or
            // `..` (review 1 #9).
            let value = percent_decode(&rest[..end])?;
            // Nor a drive (`C:evil` joins as a drive-relative path on Windows), nor a
            // control character (review 2 #8). A template whose value names a file still
            // reads it through `cantus_ai::resolve_touch`.
            let bad = |c: char| matches!(c, '/' | '\\' | ':') || c.is_control();
            if value.contains(bad) || value == "." || value == ".." {
                return None;
            }
            vars.insert(name.to_string(), value);
            rest = &rest[end..];
        } else {
            let lit_end = t.find('{').unwrap_or(t.len());
            let lit = &t[..lit_end];
            rest = rest.strip_prefix(lit)?;
            t = &t[lit_end..];
        }
    }
    rest.is_empty().then_some(vars)
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_ai::{ToolError, ToolRun};

    fn schema() -> Value {
        json!({"type": "object", "properties": {"bpm": {"type": "number"}}})
    }

    fn read(_: &cantus_ai::ToolCx<'_>, _: &Value) -> Result<Value, ToolError> {
        Ok(json!({}))
    }

    static EDIT: ToolDef = ToolDef {
        name: "set_x",
        title: "Set x",
        description: "Sets x.",
        class: ToolClass::Edit,
        input_schema: schema,
        output_schema: None,
        run: ToolRun::Read(read),
    };

    static READ: ToolDef = ToolDef {
        name: "get_x",
        title: "Get x",
        description: "Gets x.",
        class: ToolClass::Read,
        input_schema: schema,
        output_schema: Some(schema),
        run: ToolRun::Read(read),
    };

    #[test]
    fn mutating_tools_take_expected_revision_and_reads_do_not() {
        let e = tool_json(&EDIT);
        assert_eq!(
            e["inputSchema"]["properties"]["expected_revision"]["type"],
            "integer"
        );
        assert_eq!(e["inputSchema"]["properties"]["bpm"]["type"], "number");
        assert_eq!(e["annotations"]["readOnlyHint"], false);
        assert!(e.get("outputSchema").is_none());
        let r = tool_json(&READ);
        assert!(r["inputSchema"]["properties"]
            .get("expected_revision")
            .is_none());
        assert_eq!(r["annotations"]["readOnlyHint"], true);
        assert_eq!(r["outputSchema"]["type"], "object");
        assert_eq!(r["title"], "Get x");
    }

    #[test]
    fn annotations_follow_the_class() {
        let d = annotations("t", ToolClass::Destructive);
        assert_eq!(d["destructiveHint"], true);
        let f = annotations("t", ToolClass::Files);
        assert_eq!(
            (f["openWorldHint"].clone(), f["readOnlyHint"].clone()),
            (json!(true), json!(false))
        );
        let t = annotations("t", ToolClass::Transport);
        assert_eq!(
            (t["destructiveHint"].clone(), t["idempotentHint"].clone()),
            (json!(false), json!(true))
        );
    }

    #[test]
    fn templates_match_one_segment_per_variable() {
        let v = template_match("cantus://tracks/{id}", "cantus://tracks/42").unwrap();
        assert_eq!(v["id"], "42");
        let v = template_match("cantus://clips/{id}/notes", "cantus://clips/7/notes").unwrap();
        assert_eq!(v["id"], "7");
        assert!(template_match("cantus://clips/{id}/notes", "cantus://clips/7").is_none());
        assert!(template_match("cantus://tracks/{id}", "cantus://tracks/1/2").is_none());
        assert!(template_match("cantus://tracks/{id}", "cantus://tracks/").is_none());
        let v =
            template_match("cantus://renders/{name}.wav", "cantus://renders/a%20b.wav").unwrap();
        assert_eq!(v["name"], "a b");
        assert!(template_match("cantus://tracks/{id}", "cantus://mixer").is_none());
        for escape in [
            "a%2Fb", "a%5Cb", "%2E%2E", "%2e", "..", "a%00b", "C:evil", "C%3Aevil", "a%01b",
        ] {
            let uri = format!("cantus://tracks/{escape}");
            assert!(
                template_match("cantus://tracks/{id}", &uri).is_none(),
                "{escape} stays out"
            );
        }
    }
}
