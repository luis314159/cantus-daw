//! A plug-in's saved state is Cantus's own `EffectSpec` JSON — the same object a project
//! stores for an insert — so a preset moves between Cantus and any other host, both ways.
//!
//! ```json
//! {"kind":"Gate","params":{"thr":-32.0,"rel":90.0},"version":1,"sidechain":true}
//! ```
//!
//! - A sparse kind (the EQ, the compressor…) writes only what differs from the defaults,
//!   exactly as the project file does; the others write every key.
//! - `version`, `sidechain` and `bypass` are the plug-in's own and sit beside the spec's
//!   two fields, where `EffectSpec`'s reader ignores them — so Cantus reads a plug-in's
//!   state as an `EffectSpec` as it is.
//! - Loading also takes a bare `{"key": value}` map: a user preset saved in the DAW
//!   (`views::fx::save_user_preset`).
//! - Unknown keys are ignored and missing keys read as their default, the `EffectSpec`
//!   rule, so a state from a newer or older version loads.

use crate::params::{Role, Table};
use cantus_core::EffectSpec;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The state format's version, written as `version`.
pub const VERSION: u32 = 1;

/// The state of a plug-in whose values (one per parameter, the table's order) are
/// `values`.
pub fn save(table: &Table, values: &[f32]) -> Vec<u8> {
    let sparse = table.kind.sparse();
    let mut params = BTreeMap::new();
    for p in &table.params[..table.spec_count] {
        let v = values[p.index];
        if !sparse || v != p.default {
            params.insert(p.key.to_string(), v);
        }
    }
    let spec = EffectSpec {
        kind: table.kind,
        params,
        // Our own effects are native kinds, never a hosted plug-in.
        external: None,
    };
    let mut obj = match serde_json::to_value(&spec) {
        Ok(Value::Object(o)) => o,
        _ => Map::new(),
    };
    obj.insert("version".into(), VERSION.into());
    if values[table.sidechain] >= 0.5 {
        obj.insert("sidechain".into(), true.into());
    }
    if values[table.bypass] >= 0.5 {
        obj.insert("bypass".into(), true.into());
    }
    serde_json::to_vec(&Value::Object(obj)).unwrap_or_default()
}

/// Read a state (or a DAW preset) into one value per parameter.
pub fn load(table: &Table, bytes: &[u8]) -> Result<Vec<f32>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let Value::Object(obj) = value else {
        return Err("not a Cantus effect state".into());
    };
    let mut values = table.defaults();
    let params: BTreeMap<String, f32> = if obj.contains_key("kind") {
        let spec: EffectSpec =
            serde_json::from_value(Value::Object(obj.clone())).map_err(|e| e.to_string())?;
        if spec.kind != table.kind {
            return Err(format!(
                "a state of {:?}, not of {:?}",
                spec.kind, table.kind
            ));
        }
        spec.params
    } else {
        obj.iter()
            .filter_map(|(k, v)| v.as_f64().map(|v| (k.clone(), v as f32)))
            .collect()
    };
    for (key, v) in &params {
        if let Some(p) = table.by_key(key).filter(|p| p.role == Role::Spec) {
            values[p.index] = p.clamp(*v as f64);
        }
    }
    let flag = |name: &str| obj.get(name).and_then(Value::as_bool).unwrap_or(false);
    values[table.sidechain] = if flag("sidechain") { 1.0 } else { 0.0 };
    values[table.bypass] = if flag("bypass") { 1.0 } else { 0.0 };
    Ok(values)
}
