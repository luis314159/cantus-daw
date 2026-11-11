//! Aula, the convolution reverb (round 39, ADR 0112): its response from a file. Its numbers
//! (which of Cantus's own responses, stretch, decay, pre-delay…) are `set_effect`'s params;
//! a file is not a number, so it has this tool of its own. One undo step that only stores a
//! path — but it names a file on disk, so it is a *Files* tool (ADR 0081: denied unless the
//! user allowed files), and the path is resolved through [`resolve_touch`] (inside the
//! project's folder or the folders the user allowed, links and `..` followed): the resolved
//! path is the one checked and the one stored, never the one given.

use super::args::{self, plan, s_id, Res, A};
use super::words;
use crate::tool::{resolve_touch, EditPlan, Said, ToolCx, ToolError};
use cantus_core::{Command, EffectKind, IrFile};
use serde_json::{json, Value};

/// The file types a response is read from.
pub const EXTENSIONS: [&str; 6] = ["wav", "aif", "aiff", "aifc", "flac", "ogg"];

pub fn set_impulse_response_schema() -> Value {
    args::object(
        json!({
            "effect_id": s_id("An Aula (convolution reverb) effect (get_mixer lists them)."),
            "path": {"type": ["string", "null"], "maxLength": 4096,
                "description": "A WAV, AIFF, FLAC or Ogg impulse response — 1 channel (mono), 2 (stereo) or 4 (true stereo: L→L, L→R, R→L, R→R) — inside the folders the user allowed; null goes back to Cantus's own response (the effect's `ir` parameter)."}
        }),
        &["effect_id", "path"],
    )
}

pub fn set_impulse_response(cx: &mut ToolCx<'_>, a: &Value) -> Res<EditPlan> {
    let a = A(a);
    let p = cx.project();
    let (strip, effect) = args::effect(p, a, "effect_id")?;
    let kind = p.effect(strip, effect).map(|e| e.spec.kind);
    if kind != Some(EffectKind::Aula) {
        return Err(ToolError::bad(
            "effect_id",
            "only Aula (the convolution reverb) plays an impulse response",
        ));
    }
    let path = match a.0.get("path") {
        Some(Value::Null) | None => None,
        Some(_) => Some(
            a.opt_str("path")?
                .ok_or_else(|| ToolError::bad("path", "a path, or null"))?
                .to_string(),
        ),
    };
    let path = match path {
        None => None,
        Some(path) => {
            let lower = path.to_ascii_lowercase();
            if !EXTENSIONS.iter().any(|e| lower.ends_with(&format!(".{e}"))) {
                return Err(ToolError::bad("path", "a WAV, AIFF, FLAC or Ogg file"));
            }
            // Resolved on disk: what is stored (and later read) is where the file really
            // is, judged inside the allowed folders — a link or `..` never steps out.
            match resolve_touch(&*cx.host, std::path::Path::new(&path)) {
                Some(at) if at.is_file() => Some(at.to_string_lossy().into_owned()),
                _ => {
                    return Err(ToolError::bad(
                        "path",
                        "no such file inside the folders the user allowed",
                    ))
                }
            }
        }
    };
    let ir = path.as_ref().map(|p| Box::new(IrFile::new(p.clone())));
    let name = ir
        .as_ref()
        .map_or_else(|| "-".to_string(), |f| args::clean(f.name()));
    let p = cx.project();
    let said = Said::new("ai.tools.set_impulse_response")
        .name("name", name)
        .nested("strip", words::strip(p, strip));
    Ok(plan(
        Command::SetEffectIr { strip, effect, ir },
        said,
        json!({"effect_id": effect.0, "path": path}),
    ))
}
