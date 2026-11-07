//! Every agent tool that takes a path on disk is a *Files* tool (round 37, the integrator).
//!
//! Twice in round 37 a builder's new tool opened a path the agent gave as an ordinary
//! *Edit* tool: `make_kit_from_folder` and `slice_sample` could read any file on the
//! computer without the user's file permission, because nothing but review stood between
//! a new tool and the policy (ADR 0081: a tool that reads or writes outside the project's
//! own folder is `ToolClass::Files`, denied unless the user enabled it, and resolves every
//! path through `cantus_ai::resolve_touch`). This test makes the rule mechanical: any
//! argument whose name says it is a path, a file or a folder makes its tool a Files tool.
//! A tool that takes such a name for something else is listed below with the reason.

use cantus_ai::ToolClass;
use serde_json::Value;

/// Argument names that hold a path on disk.
fn is_path_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["path", "file", "folder", "dir", "directory"]
        .iter()
        .any(|w| {
            n == *w
                || n.ends_with(&format!("_{w}"))
                || n.starts_with(&format!("{w}_"))
                || n.ends_with(&format!("{w}s"))
        })
}

/// Tools allowed to carry a path-like argument name without being Files tools, each with
/// why: the argument is not a location on disk the tool opens.
const NOT_A_PATH: &[(&str, &str, &str)] = &[];

/// Whether a property's schema can hold text (a string, or an array of strings; an
/// untyped property counts, to be safe).
fn holds_text(v: &Value) -> bool {
    match v.get("type") {
        None => v.get("items").is_none_or(holds_text),
        Some(Value::String(t)) => {
            t == "string" || (t == "array" && v.get("items").is_none_or(holds_text))
        }
        Some(Value::Array(ts)) => ts.iter().any(|t| t == "string" || t == "array"),
        _ => true,
    }
}

/// The path-like argument names anywhere in `schema` (nested objects and arrays included).
fn path_args(schema: &Value, out: &mut Vec<String>) {
    match schema {
        Value::Object(m) => {
            if let Some(Value::Object(props)) = m.get("properties") {
                for (k, v) in props {
                    // A path is text: a switch named `discard_files` is not one.
                    if is_path_name(k) && holds_text(v) {
                        out.push(k.clone());
                    }
                    path_args(v, out);
                }
            }
            for key in ["items", "anyOf", "oneOf", "allOf"] {
                if let Some(v) = m.get(key) {
                    path_args(v, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|v| path_args(v, out)),
        _ => {}
    }
}

#[test]
fn every_tool_that_takes_a_path_is_a_files_tool() {
    let mut wrong = Vec::new();
    for tool in cantus_ui::ai_tools::registry() {
        let mut args = Vec::new();
        path_args(&(tool.input_schema)(), &mut args);
        args.retain(|a| {
            !NOT_A_PATH
                .iter()
                .any(|(t, arg, _)| *t == tool.name && arg == a)
        });
        if !args.is_empty() && tool.class != ToolClass::Files {
            wrong.push(format!("{} ({:?}) takes {:?}", tool.name, tool.class, args));
        }
    }
    assert!(
        wrong.is_empty(),
        "these tools take a path but are not ToolClass::Files (make them Files and resolve \
         every path through cantus_ai::resolve_touch, or list the argument in NOT_A_PATH \
         with the reason): {wrong:#?}"
    );
}

#[test]
fn the_path_names_are_recognised() {
    for yes in [
        "path",
        "file",
        "folder",
        "dir",
        "sample_path",
        "out_file",
        "files",
        "folders",
    ] {
        assert!(is_path_name(yes), "{yes}");
    }
    for no in ["profile", "filter", "direction", "tile", "pathos_level"] {
        assert!(!is_path_name(no), "{no}");
    }
}
