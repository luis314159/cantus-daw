//! A tool's arguments checked against its `inputSchema` before it plans anything (the
//! integrator, after tools: a misspelt key was silently ignored and the model believed it
//! had set it). A small, hand-written subset of JSON Schema 2020-12 — what the tool table
//! uses — enforced:
//!
//! - `type` (one or a list: `object`, `array`, `string`, `number`, `integer`, `boolean`,
//!   `null`);
//! - `properties`, `required`, and unknown keys: **refused** where `properties` is given,
//!   unless `additionalProperties` is `true` or a schema (then checked against it);
//! - `items` (one schema), `minItems`, `maxItems`;
//! - `enum`, `const`;
//! - `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`;
//! - `minLength`, `maxLength`;
//! - `anyOf` / `oneOf` (the value matches one of them — `oneOf`'s "exactly one" is not
//!   checked). A branch's own `properties` and the schema's around it are one set of keys:
//!   a key the outer schema names is never "unknown" to a branch, nor a branch's key to the
//!   outer schema (review 4 #5).
//!
//! An optional argument sent as `null` is taken as left out — [`drop_nulls`] removes it
//! before the check (many clients write every optional key, `null` when unset; review 4
//! #5) — unless its own schema allows `null`, when `null` is a value.
//!
//! Anything else (`$ref`, `pattern`, `format`, `if`/`then`…) is not enforced: the tool
//! checks what it needs itself. The server's own two arguments (`change`,
//! `expected_revision`) are taken out before this runs.

use serde_json::{Map, Value};

/// Why arguments do not fit a schema: the path (`strip.width`, `notes[2].pitch`) and
/// what is wrong, for the model to correct itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Misfit {
    pub path: String,
    pub reason: String,
}

fn misfit(path: &str, reason: impl Into<String>) -> Misfit {
    Misfit {
        path: if path.is_empty() {
            "arguments".into()
        } else {
            path.to_string()
        },
        reason: reason.into(),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn is_type(v: &Value, t: &str) -> bool {
    match t {
        // An integer is a number; a whole float (2.0) is an integer, as JSON Schema says.
        "number" => v.is_number(),
        "integer" => v.as_f64().is_some_and(|f| f.fract() == 0.0),
        other => type_name(v) == other,
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// Check `value` against `schema`.
pub fn check(schema: &Value, value: &Value) -> Result<(), Misfit> {
    at(schema, value, "", &[])
}

/// The keys an object schema names itself.
fn own_keys(s: &Map<String, Value>) -> Vec<&str> {
    s.get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

/// The keys its `anyOf` / `oneOf` branches name.
fn branch_keys(s: &Map<String, Value>) -> Vec<&str> {
    ["anyOf", "oneOf"]
        .iter()
        .filter_map(|k| s.get(*k).and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_object)
        .flat_map(own_keys)
        .collect()
}

/// The schema of the property `key`: the object's own, else a branch's.
fn property<'a>(s: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    let own = |m: &'a Map<String, Value>| {
        m.get("properties")
            .and_then(Value::as_object)
            .and_then(|p| p.get(key))
    };
    own(s).or_else(|| {
        ["anyOf", "oneOf"]
            .iter()
            .filter_map(|k| s.get(*k).and_then(Value::as_array))
            .flatten()
            .filter_map(Value::as_object)
            .find_map(own)
    })
}

fn allows_null(schema: &Value) -> bool {
    match schema.get("type") {
        Some(Value::String(t)) => t == "null",
        Some(Value::Array(many)) => many.iter().any(|t| t == "null"),
        _ => {
            schema
                .get("enum")
                .and_then(Value::as_array)
                .is_some_and(|e| e.contains(&Value::Null))
                || schema.get("const") == Some(&Value::Null)
        }
    }
}

/// Remove every optional key whose value is `null` (and whose schema does not take `null`
/// as a value), in nested objects and array items too: a client that writes `"x": null`
/// for an argument it leaves out is not refused for it (review 4 #5).
pub fn drop_nulls(schema: &Value, value: &mut Value) {
    let Some(s) = schema.as_object() else {
        return;
    };
    if let (Some(items), Some(list)) = (s.get("items"), value.as_array_mut()) {
        for v in list {
            drop_nulls(items, v);
        }
        return;
    }
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let required: Vec<&str> = s
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    obj.retain(|key, v| {
        let optional_null = v.is_null() && !required.contains(&key.as_str());
        !(optional_null && property(s, key).is_some_and(|p| !allows_null(p)))
    });
    for (key, v) in obj.iter_mut() {
        if let Some(p) = property(s, key) {
            drop_nulls(p, v);
        }
    }
}

fn at<'a>(schema: &'a Value, value: &Value, path: &str, outer: &[&'a str]) -> Result<(), Misfit> {
    let Some(s) = schema.as_object() else {
        // `true` (or a non-object) accepts anything; `false` nothing.
        return match schema {
            Value::Bool(false) => Err(misfit(path, "is not allowed")),
            _ => Ok(()),
        };
    };
    if let Some(t) = s.get("type") {
        let types: Vec<&str> = match t {
            Value::String(one) => vec![one.as_str()],
            Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if !types.is_empty() && !types.iter().any(|t| is_type(value, t)) {
            return Err(misfit(
                path,
                format!("must be {}, not {}", types.join(" or "), type_name(value)),
            ));
        }
    }
    if let Some(allowed) = s.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            let list: Vec<String> = allowed.iter().map(Value::to_string).collect();
            return Err(misfit(path, format!("must be one of {}", list.join(", "))));
        }
    }
    if let Some(c) = s.get("const") {
        if c != value {
            return Err(misfit(path, format!("must be {c}")));
        }
    }
    if let Some(x) = value.as_f64() {
        let bound = |k: &str| s.get(k).and_then(Value::as_f64);
        if let Some(m) = bound("minimum").filter(|m| x < *m) {
            return Err(misfit(path, format!("must be at least {m}")));
        }
        if let Some(m) = bound("maximum").filter(|m| x > *m) {
            return Err(misfit(path, format!("must be at most {m}")));
        }
        if let Some(m) = bound("exclusiveMinimum").filter(|m| x <= *m) {
            return Err(misfit(path, format!("must be more than {m}")));
        }
        if let Some(m) = bound("exclusiveMaximum").filter(|m| x >= *m) {
            return Err(misfit(path, format!("must be less than {m}")));
        }
    }
    if let Some(text) = value.as_str() {
        let n = text.chars().count() as u64;
        if let Some(m) = s
            .get("minLength")
            .and_then(Value::as_u64)
            .filter(|m| n < *m)
        {
            return Err(misfit(path, format!("must be at least {m} characters")));
        }
        if let Some(m) = s
            .get("maxLength")
            .and_then(Value::as_u64)
            .filter(|m| n > *m)
        {
            return Err(misfit(path, format!("must be at most {m} characters")));
        }
    }
    if let Some(items) = value.as_array() {
        let n = items.len() as u64;
        if let Some(m) = s.get("minItems").and_then(Value::as_u64).filter(|m| n < *m) {
            return Err(misfit(path, format!("needs at least {m} items")));
        }
        if let Some(m) = s.get("maxItems").and_then(Value::as_u64).filter(|m| n > *m) {
            return Err(misfit(path, format!("takes at most {m} items")));
        }
        if let Some(each) = s.get("items") {
            for (i, item) in items.iter().enumerate() {
                at(each, item, &format!("{path}[{i}]"), &[])?;
            }
        }
    }
    if let Some(obj) = value.as_object() {
        object(s, obj, path, outer)?;
    }
    // The branches last, so a key unknown to all is named with every key the schema
    // takes; a branch knows the keys named around it.
    let mut around: Vec<&str> = outer.to_vec();
    around.extend(own_keys(s));
    for key in ["anyOf", "oneOf"] {
        if let Some(branches) = s.get(key).and_then(Value::as_array) {
            if !branches.iter().any(|b| at(b, value, path, &around).is_ok()) {
                let first = branches
                    .iter()
                    .find_map(|b| at(b, value, path, &around).err())
                    .map(|m| m.reason)
                    .unwrap_or_default();
                return Err(misfit(path, format!("matches none of its forms ({first})")));
            }
        }
    }
    Ok(())
}

fn object(
    s: &Map<String, Value>,
    obj: &Map<String, Value>,
    path: &str,
    outer: &[&str],
) -> Result<(), Misfit> {
    let props = s.get("properties").and_then(Value::as_object);
    // Keys checked elsewhere: by the schema around this branch, or by a branch of this one.
    let mut elsewhere: Vec<&str> = outer.to_vec();
    elsewhere.extend(branch_keys(s));
    if let Some(required) = s.get("required").and_then(Value::as_array) {
        for r in required.iter().filter_map(Value::as_str) {
            if !obj.contains_key(r) {
                return Err(misfit(&join(path, r), "is required"));
            }
        }
    }
    let extra = s.get("additionalProperties");
    for (key, v) in obj {
        let here = join(path, key);
        if let Some(p) = props.and_then(|p| p.get(key)) {
            at(p, v, &here, &[])?;
            continue;
        }
        if elsewhere.contains(&key.as_str()) {
            continue;
        }
        match extra {
            Some(Value::Bool(true)) => {}
            Some(other @ Value::Object(_)) => at(other, v, &here, &[])?,
            // Unknown keys are refused wherever the schema names its keys, and wherever it
            // says `additionalProperties: false`.
            Some(Value::Bool(false)) | None if props.is_some() || extra.is_some() => {
                let mut known: Vec<&str> = props
                    .map(|p| p.keys().map(String::as_str).collect())
                    .unwrap_or_default();
                known.extend(&elsewhere);
                known.sort_unstable();
                known.dedup();
                let takes = if known.is_empty() {
                    "it takes none here".to_string()
                } else {
                    format!("it takes: {}", known.join(", "))
                };
                return Err(misfit(
                    &here,
                    format!("is not an argument of this tool ({takes})"),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "width": {"type": "number", "minimum": 0, "maximum": 2},
                "mode": {"enum": ["mono", "stereo"]},
                "name": {"type": "string", "maxLength": 4},
                "strip": {"type": "object", "properties": {"strength": {"type": "integer"}}},
                "notes": {"type": "array", "maxItems": 2,
                          "items": {"type": "object", "properties": {"pitch": {"type": "integer", "minimum": 0, "maximum": 127}},
                                    "required": ["pitch"], "additionalProperties": false}},
                "free": {"type": "object"}
            },
            "required": ["width"]
        })
    }

    fn why(v: Value) -> Misfit {
        check(&schema(), &v).unwrap_err()
    }

    #[test]
    fn fitting_arguments_pass() {
        let ok = json!({"width": 1, "mode": "mono", "name": "ab", "strip": {"strength": 3},
                        "notes": [{"pitch": 60}], "free": {"anything": [1, 2]}});
        assert_eq!(check(&schema(), &ok), Ok(()));
        assert_eq!(
            check(&json!({"type": "object"}), &json!({"x": 1})),
            Ok(()),
            "no properties named: free"
        );
    }

    #[test]
    fn a_misspelt_key_is_named_with_the_keys_allowed() {
        let m = why(json!({"width": 1, "widht": 2}));
        assert_eq!(m.path, "widht");
        assert!(
            m.reason.contains("free, mode, name, notes, strip, width"),
            "{}",
            m.reason
        );
        let m = why(json!({"width": 1, "strip": {"strenght": 3}}));
        assert_eq!(m.path, "strip.strenght");
        assert!(m.reason.contains("strength"));
        let m = why(json!({"width": 1, "notes": [{"pitch": 60, "velocty": 9}]}));
        assert_eq!(m.path, "notes[0].velocty");
    }

    #[test]
    fn types_ranges_enums_and_lengths_are_checked() {
        assert_eq!(why(json!({})).path, "width");
        assert!(why(json!({"width": "1"})).reason.contains("must be number"));
        assert!(why(json!({"width": 3})).reason.contains("at most 2"));
        assert!(why(json!({"width": -1})).reason.contains("at least 0"));
        assert!(why(json!({"width": 1, "mode": "quad"}))
            .reason
            .contains("one of"));
        assert!(why(json!({"width": 1, "name": "toolong"}))
            .reason
            .contains("4 characters"));
        assert!(why(json!({"width": 1, "strip": {"strength": 1.5}}))
            .reason
            .contains("integer"));
        assert_eq!(
            check(&schema(), &json!({"width": 1, "strip": {"strength": 2.0}})),
            Ok(())
        );
        let m = why(json!({"width": 1, "notes": [{"pitch": 1}, {"pitch": 2}, {"pitch": 3}]}));
        assert!(m.reason.contains("at most 2 items"));
        let m = why(json!({"width": 1, "notes": [{"pitch": 200}]}));
        assert_eq!(m.path, "notes[0].pitch");
        assert_eq!(
            why(json!({"width": 1, "notes": [{}]})).path,
            "notes[0].pitch"
        );
    }

    #[test]
    fn additional_properties_and_any_of_are_honoured() {
        let open = json!({"type": "object", "properties": {"a": {}}, "additionalProperties": true});
        assert_eq!(check(&open, &json!({"a": 1, "b": 2})), Ok(()));
        let typed =
            json!({"type": "object", "properties": {}, "additionalProperties": {"type": "number"}});
        assert_eq!(check(&typed, &json!({"b": 2})), Ok(()));
        assert!(check(&typed, &json!({"b": "x"})).is_err());
        let closed = json!({"type": "object", "additionalProperties": false});
        assert!(check(&closed, &json!({"b": 1})).is_err());
        let either = json!({"anyOf": [{"type": "string"}, {"type": "integer", "minimum": 1}]});
        assert_eq!(check(&either, &json!("x")), Ok(()));
        assert_eq!(check(&either, &json!(3)), Ok(()));
        assert!(check(&either, &json!(0)).is_err());
    }

    /// Review 4 #5: a client that writes every optional key, `null` when unset, is not
    /// refused; a required key sent as `null` still is, and so is a misspelt one.
    #[test]
    fn an_optional_null_is_left_out() {
        let mut v = json!({"width": 1, "mode": null, "strip": {"strength": null},
                           "notes": [{"pitch": 3, "x": null}]});
        drop_nulls(&schema(), &mut v);
        assert_eq!(
            v,
            json!({"width": 1, "strip": {}, "notes": [{"pitch": 3, "x": null}]})
        );
        // `x` is unknown, so it is not dropped: the check names it.
        assert_eq!(check(&schema(), &v).unwrap_err().path, "notes[0].x");
        let mut required = json!({"width": null});
        drop_nulls(&schema(), &mut required);
        assert!(check(&schema(), &required)
            .unwrap_err()
            .reason
            .contains("must be number"));
        let mut misspelt = json!({"width": 1, "widht": null});
        drop_nulls(&schema(), &mut misspelt);
        assert_eq!(check(&schema(), &misspelt).unwrap_err().path, "widht");
        // A schema that takes null keeps it as a value.
        let nullable =
            json!({"type": "object", "properties": {"to": {"type": ["string", "null"]}}});
        let mut clear = json!({"to": null});
        drop_nulls(&nullable, &mut clear);
        assert_eq!(clear, json!({"to": null}));
    }

    /// Review 4 #5: keys named outside an `anyOf` and inside its branches are one set.
    #[test]
    fn any_of_branches_and_the_schema_around_them_share_keys() {
        let s = json!({
            "type": "object",
            "properties": {"track": {"type": "integer"}},
            "anyOf": [
                {"properties": {"db": {"type": "number"}}, "required": ["db"]},
                {"properties": {"gain": {"type": "number"}}, "required": ["gain"]}
            ]
        });
        assert_eq!(check(&s, &json!({"track": 1, "db": -3.0})), Ok(()));
        assert_eq!(check(&s, &json!({"track": 1, "gain": 0.5})), Ok(()));
        assert!(
            check(&s, &json!({"track": 1})).is_err(),
            "a branch is needed"
        );
        let m = check(&s, &json!({"track": 1, "db": -3.0, "dbb": 1})).unwrap_err();
        assert!(m.reason.contains("db, gain, track"), "{m:?}");
        assert!(check(&s, &json!({"track": 1, "db": "loud"})).is_err());
        // A null optional in a branch is dropped too.
        let mut v = json!({"track": 1, "db": -3.0, "gain": null});
        drop_nulls(&s, &mut v);
        assert_eq!(check(&s, &v), Ok(()));
    }
}
