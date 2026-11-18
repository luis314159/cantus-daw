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

/// fxb:cedo — Cedo's drawn shape, if the state holds one (`EffectSpec::cedo`), sanitized;
/// `None` (the classic pump) when it holds none.
pub fn load_cedo(bytes: &[u8]) -> Option<Box<cantus_core::fx::cedo::Shape>> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    let shape = value.get("cedo")?.clone();
    serde_json::from_value::<cantus_core::fx::cedo::Shape>(shape)
        .ok()
        .map(|s| Box::new(s.sanitized()))
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
    save_spec_with(table, values, |spec| {
        spec.lapsus = lapsus.map(|b| Box::new(b.clone())); // fx:lapsus
        spec.board = board.map(|b| Box::new(b.clone())); // fx3:tabula
        spec.fractus = fractus
            .filter(|p| !p.is_empty())
            .map(|p| Box::new(p.clone())); // fxa:fractus
    })
}

/// fxb:cedo — [`save`], with what `extras` puts in the spec beside the values (Cedo's
/// shape; [`save_spec_extras`] puts Lapsus's bank and Tabula's board through it).
pub fn save_spec_with(
    table: &Table,
    values: &[f32],
    extras: impl FnOnce(&mut EffectSpec),
) -> Vec<u8> {
    let sparse = table.kind.sparse();
    let mut params = BTreeMap::new();
    for p in &table.params[..table.spec_count] {
        let v = values[p.index];
        if !sparse || v != p.default {
            params.insert(p.key.to_string(), v);
        }
    }
    let mut spec = EffectSpec {
        kind: table.kind,
        params,
        // Our own effects are native kinds, never a hosted plug-in.
        external: None,
        ir: None,      // r39:conv
        lapsus: None,  // fx:lapsus (`extras`)
        board: None,   // fx3:tabula (`extras`)
        fractus: None, // fxa:fractus (`extras`)
        cedo: None,    // fxb:cedo (`extras`)
    };
    extras(&mut spec);
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
