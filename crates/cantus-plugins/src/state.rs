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
    save_with_bank(table, values, None)
}

/// fx:lapsus — Lapsus's bank, if the plug-in holds one (`EffectSpec::lapsus`).
pub fn load_bank(bytes: &[u8]) -> Option<Box<cantus_core::fx::lapsus::Bank>> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let bank = value.get("lapsus")?.clone();
    serde_json::from_value::<cantus_core::fx::lapsus::Bank>(bank)
        .ok()
        .filter(|b| !b.is_empty())
        .map(Box::new)
}

/// fx3:tabula — Tabula's board, if the plug-in holds one (`EffectSpec::board`), sanitized.
pub fn load_board(bytes: &[u8]) -> Option<Box<cantus_core::fx::tabula::Board>> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let board = value.get("board")?.clone();
    let mut b = serde_json::from_value::<cantus_core::fx::tabula::Board>(board).ok()?;
    b.sanitize();
    (!b.pedals.is_empty() || !b.macros.is_empty()).then(|| Box::new(b))
}

/// fxa:fractus — Fractus's lanes, if the state holds any (`EffectSpec::fractus`), sanitized.
pub fn load_fractus(bytes: &[u8]) -> Option<Box<cantus_core::fx::fractus::Pattern>> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let lanes = value.get("fractus")?.clone();
    let p = serde_json::from_value::<cantus_core::fx::fractus::Pattern>(lanes)
        .ok()?
        .sanitized();
    (!p.is_empty()).then(|| Box::new(p))
}

/// fxc:comparatio — Comparatio's references in a state saved by [`save_spec_extras`]: the
/// list (each path with the size and hash it was loaded with) and the one compared, as the
/// project keeps them (`EffectSpec::refs`, `EffectSpec::ir`). Never their audio.
pub fn with_references(
    bytes: &[u8],
    refs: Option<&cantus_core::fx::comparatio::References>,
    ir: Option<&cantus_core::IrFile>,
) -> Vec<u8> {
    let Ok(Value::Object(mut obj)) = serde_json::from_slice::<Value>(bytes) else {
        return bytes.to_vec();
    };
    if let Some(r) = refs.filter(|r| !r.is_empty()) {
        if let Ok(v) = serde_json::to_value(r) {
            obj.insert("refs".into(), v);
        }
    }
    if let Some(f) = ir {
        if let Ok(v) = serde_json::to_value(f) {
            obj.insert("ir".into(), v);
        }
    }
    serde_json::to_vec(&Value::Object(obj)).unwrap_or_else(|_| bytes.to_vec())
}

/// fxc:comparatio — Comparatio's references from a state, repaired as a project's are (a
/// path twice or empty dropped, the one compared listed).
pub fn load_references(
    bytes: &[u8],
) -> (
    Option<cantus_core::fx::comparatio::References>,
    Option<cantus_core::IrFile>,
) {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return (None, None);
    };
    let ir = value
        .get("ir")
        .and_then(|v| serde_json::from_value::<cantus_core::IrFile>(v.clone()).ok())
        .filter(|f| !f.path.trim().is_empty());
    let refs = value.get("refs").and_then(|v| {
        serde_json::from_value::<cantus_core::fx::comparatio::References>(v.clone()).ok()
    });
    let mut spec = EffectSpec::new(cantus_core::EffectKind::Reference);
    spec.refs = refs.map(Box::new);
    spec.ir = ir.map(Box::new);
    let mut p = cantus_core::Project::default();
    let s = p.strips[0].id;
    let mut slot = p.make_effect(cantus_core::EffectKind::Reference);
    slot.spec = spec;
    if let Some(strip) = p.strip_mut(s) {
        strip.effects.push(slot);
    }
    cantus_core::fx::comparatio::repair_project(&mut p);
    let spec = p
        .strip(s)
        .and_then(|st| st.effects.last())
        .map(|e| e.spec.clone())
        .unwrap_or_else(|| EffectSpec::new(cantus_core::EffectKind::Reference));
    (spec.refs.map(|b| *b), spec.ir.map(|b| *b))
}

/// [`save`], with Lapsus's bank beside the values (fx:lapsus).
pub fn save_with_bank(
    table: &Table,
    values: &[f32],
    lapsus: Option<&cantus_core::fx::lapsus::Bank>,
) -> Vec<u8> {
    save_spec_extras(table, values, lapsus, None, None)
}

/// [`save`], with the content a spec holds beside its values: Lapsus's bank (fx:lapsus),
/// Tabula's board (fx3:tabula) and Fractus's lanes (fxa:fractus), as the project saves them.
pub fn save_spec_extras(
    table: &Table,
    values: &[f32],
    lapsus: Option<&cantus_core::fx::lapsus::Bank>,
    board: Option<&cantus_core::fx::tabula::Board>,
    fractus: Option<&cantus_core::fx::fractus::Pattern>,
) -> Vec<u8> {
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
        ir: None,                                    // r39:conv
        lapsus: lapsus.map(|b| Box::new(b.clone())), // fx:lapsus
        board: board.map(|b| Box::new(b.clone())),   // fx3:tabula
        fractus: fractus
            .filter(|p| !p.is_empty())
            .map(|p| Box::new(p.clone())), // fxa:fractus
        refs: None,                            // fxc:comparatio
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
