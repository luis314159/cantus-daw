//! Calling the project's tools against a bare bus, as the MCP pump does: plan, execute with
//! `Origin::Mcp`, and check that one undo gives back the project exactly (the id counter
//! aside: ids are never reused) and one redo the edit again.

#![allow(dead_code)]

use cantus_ai::bridge::BareHost;
use cantus_ai::tools_project::TOOLS;
use cantus_ai::{ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::{CommandBus, Origin, Project};
use serde_json::Value;

pub fn tool(name: &str) -> &'static ToolDef {
    TOOLS
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no tool {name}"))
}

pub fn host() -> BareHost {
    BareHost {
        bus: CommandBus::new(Project::new("test")),
        dir: None,
    }
}

/// The project with its id counter aside.
pub fn bare(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

pub fn read(h: &mut BareHost, name: &str, args: Value) -> Result<Value, ToolError> {
    let t = tool(name);
    match t.run {
        ToolRun::Read(f) => f(&ToolCx { host: h }, &args),
        _ => panic!("{name} is not a read"),
    }
}

pub fn read_ok(h: &mut BareHost, name: &str, args: Value) -> Value {
    read(h, name, args).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Plan and execute one mutating call; check its undo and redo; answer its result.
pub fn edit(h: &mut BareHost, name: &str, args: Value) -> Result<Value, ToolError> {
    let t = tool(name);
    assert!(
        matches!(t.class, ToolClass::Edit | ToolClass::Destructive),
        "{name} edits but is {:?}",
        t.class
    );
    let ToolRun::Edit(f) = t.run else {
        panic!("{name} is not an edit")
    };
    let before = bare(h.bus.project());
    let depth = h.bus.history().len();
    let plan = f(&mut ToolCx { host: h }, &args)?;
    h.bus
        .execute(plan.command, Origin::Mcp)
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    assert_eq!(h.bus.history().len(), depth + 1, "{name}: one undo step");
    let after = bare(h.bus.project());
    assert!(h.bus.undo().is_some());
    let undone = bare(h.bus.project());
    assert!(
        undone == before,
        "{name}({args}): the undo gives back the project exactly; first difference: {}",
        diff(
            &serde_json::to_value(&before).unwrap(),
            &serde_json::to_value(&undone).unwrap(),
            String::new()
        )
    );
    assert!(h.bus.redo().is_some());
    assert_eq!(
        bare(h.bus.project()),
        after,
        "{name}: the redo gives the edit again"
    );
    Ok(plan.result)
}

pub fn edit_ok(h: &mut BareHost, name: &str, args: Value) -> Value {
    edit(h, name, args.clone()).unwrap_or_else(|e| panic!("{name}({args}): {e}"))
}

pub fn edit_err(h: &mut BareHost, name: &str, args: Value) -> ToolError {
    let before = bare(h.bus.project());
    let e = match edit(h, name, args) {
        Ok(v) => panic!("{name} should have failed, answered {v}"),
        Err(e) => e,
    };
    assert_eq!(
        bare(h.bus.project()),
        before,
        "{name}: a refused call changes nothing"
    );
    e
}

pub fn id(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or_else(|| panic!("no {k} in {v}"))
}

/// Where two JSON values first differ.
pub fn diff(a: &Value, b: &Value, at: String) -> String {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) if w == v => {}
                    Some(w) => return diff(v, w, format!("{at}.{k}")),
                    None => return format!("{at}.{k} missing after"),
                }
            }
            format!(
                "{at}: keys {:?} vs {:?}",
                x.keys().collect::<Vec<_>>(),
                y.keys().collect::<Vec<_>>()
            )
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                if v != w {
                    return diff(v, w, format!("{at}[{i}]"));
                }
            }
            format!("{at}: lengths {} vs {}", x.len(), y.len())
        }
        _ => format!("{at}: {a} vs {b}"),
    }
}
