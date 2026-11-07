//! Agent tools for the preset library (round 37, builder library, ADR 0101):
//!
//! - `find_presets` — every factory preset of a unit (or of every unit), with its styles,
//!   roles and what it is for, filtered by style, role and words: what the browser's chips
//!   and search do.
//! - `audition_preset` — a preset rendered on what the browser hears it on (its role's
//!   phrase; an insert's clip), measured, and played to the user through the audition voice
//!   when the window is there. **Nothing in the project changes.**
//!
//! Loading one is `apply_preset` (one undo step, `agent_tools.rs`), which reads the same
//! presets.

use crate::preset_audition::{self as aud, HeardOn, Target};
use crate::preset_catalog::{entries, units, Entry, Unit};
use crate::preset_query::Query;
use crate::preset_tags::{Role, Style};
use cantus_ai::tools_project::args::{self, Res, A};
use cantus_ai::{Job, JobDone, Said, ToolClass, ToolCx, ToolDef, ToolError, ToolRun};
use cantus_core::{EffectKind, InstrumentSpec};
use serde_json::{json, Value};

fn style_words() -> Vec<&'static str> {
    Style::ALL.iter().map(|s| s.id()).collect()
}

fn role_words() -> Vec<&'static str> {
    Role::ALL.iter().map(|r| r.id()).collect()
}

/// The unit an agent names: an effect's or an instrument's id as `list_units` gives it.
fn unit_named(name: &str) -> Option<Unit> {
    if let Some(k) = args::any_effect_kind(name) {
        return Some(Unit::Effect(k));
    }
    match args::instrument_choice(name)? {
        args::InstrumentChoice::Native(k) => Some(Unit::Instrument(k)),
        args::InstrumentChoice::Pulsus => Some(Unit::Kit),
        _ => None,
    }
}

/// The unit of the device the arguments point at, and where a loaded preset would go.
fn target_of(cx: &ToolCx<'_>, a: A<'_>) -> Res<(Option<Unit>, Target)> {
    let p = cx.project();
    if a.has("effect_id") {
        let (strip, effect) = args::effect(p, a, "effect_id")?;
        let kind = p.effect(strip, effect).expect("found").spec.kind;
        if kind == EffectKind::External {
            return Err(ToolError::bad(
                "effect_id",
                "a hosted plug-in keeps its presets in its own window",
            ));
        }
        return Ok((Some(Unit::Effect(kind)), Target::Effect { strip, effect }));
    }
    if a.has("track_id") {
        let track = args::track(p, a, "track_id")?;
        let unit = match &p.track(track).expect("checked").instrument {
            InstrumentSpec::Native(n) => Unit::Instrument(n.kind),
            InstrumentSpec::Pulsus(_) => Unit::Kit,
            _ => {
                return Err(ToolError::bad(
                    "track_id",
                    "Firmus and hosted instruments have no presets here",
                ))
            }
        };
        return Ok((Some(unit), Target::Track(track)));
    }
    if let Some(u) = a.opt_str("unit")? {
        let unit = unit_named(u).ok_or_else(|| ToolError::NotFound(format!("presets of {u:?}")))?;
        return Ok((Some(unit), Target::None));
    }
    Ok((None, Target::None))
}

fn unit_id(u: Unit) -> &'static str {
    match u {
        Unit::Instrument(k) => args::native_id(k),
        Unit::Effect(k) => args::effect_names(k).0,
        Unit::Kit => "pulsus",
    }
}

fn row(e: &Entry) -> Value {
    let note = crate::preset_data::presets_of_unit(e.unit)
        .find(|p| p.key == e.id)
        .map(|p| p.note.clone())
        .unwrap_or_default();
    let mut v = json!({
        "unit": unit_id(e.unit),
        "name": crate::i18n::t_in(crate::i18n::Locale::En, &e.id),
        "styles": e.styles.iter().map(|s| s.id()).collect::<Vec<_>>(),
        "roles": e.roles.iter().map(|r| r.id()).collect::<Vec<_>>(),
    });
    if !note.is_empty() {
        v["what"] = json!(note);
    }
    v
}

fn find_presets_schema() -> Value {
    args::object(
        json!({
            "effect_id": args::s_id("An insert effect: its unit's presets."),
            "track_id": args::s_id("A track: its instrument's presets."),
            "unit": {"type": "string", "description": "Or a unit's id (list_units). With none of the three, every unit's presets."},
            "style": {"enum": style_words(), "description": "Only presets made for this style."},
            "role": {"enum": role_words(), "description": "Only presets for this role: an instrument's part (bass, lead, pad, keys, pluck, fx, drums) or where an effect goes (vocal, drums, bus, master, creative)."},
            "search": {"type": "string", "description": "Words that must all be in the name, a style, a role or the unit (case and accents ignored)."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 500, "description": "At most this many (default 100)."}
        }),
        &[],
    )
}

fn find_presets(cx: &ToolCx<'_>, a: &Value) -> Res<Value> {
    let a = A(a);
    let (unit, _) = target_of(cx, a)?;
    let mut q = Query::default();
    if let Some(s) = a.opt_choice("style", &style_words())? {
        q.styles.insert(Style::from_id(s).expect("a style"));
    }
    if let Some(r) = a.opt_choice("role", &role_words())? {
        q.roles.insert(Role::from_id(r).expect("a role"));
    }
    q.text = a.opt_str("search")?.unwrap_or_default().to_string();
    let limit = a.opt_u64("limit")?.unwrap_or(100).clamp(1, 500) as usize;
    // r37:fixes — English for the answer only: the window keeps its language.
    let _en = crate::i18n::scoped_thread_locale(crate::i18n::Locale::En);
    let all: Vec<Entry> = match unit {
        Some(u) => entries(u, false),
        None => units()
            .into_iter()
            .filter(|u| !matches!(u, Unit::Effect(EffectKind::Reverb)))
            .flat_map(|u| entries(u, false))
            .collect(),
    };
    let found: Vec<Value> = q.run(&all).into_iter().map(|i| row(&all[i])).collect();
    let total = found.len();
    Ok(json!({"presets": found.into_iter().take(limit).collect::<Vec<_>>(), "found": total}))
}

fn audition_schema() -> Value {
    args::object(
        json!({
            "effect_id": args::s_id("An insert: heard on a clip its channel plays."),
            "track_id": args::s_id("A track: its instrument's preset heard on its role's phrase."),
            "unit": {"type": "string", "description": "Or a unit's id, heard on a phrase or on Cantus's own source."},
            "preset": {"type": "string", "description": "The preset's name (find_presets or list_presets), any case."},
            "play": {"type": "boolean", "description": "Play it to the user through the audition voice (default true). The project never changes."}
        }),
        &["preset"],
    )
}

fn heard_word(on: HeardOn) -> Value {
    match on {
        HeardOn::Phrase(r, bpm) => json!({"phrase": r.id(), "bpm": bpm}),
        HeardOn::SelectedClip => json!("the selected clip"),
        HeardOn::FirstClip => json!("the first clip on this channel"),
        HeardOn::Source(r) => json!({"cantus_source": r.id()}),
    }
}

/// A test's gate between an agent's audition rendered and its cancel checked: called on
/// the job's worker right after the render (a test holds it there, cancels the job, lets it
/// go). `None` outside the tests that set it. r37:fixes
#[doc(hidden)]
pub static RENDERED_HOOK: std::sync::Mutex<Option<RenderedHook>> = std::sync::Mutex::new(None);

/// What [`RENDERED_HOOK`] holds.
#[doc(hidden)]
pub type RenderedHook = std::sync::Arc<dyn Fn() + Send + Sync>;

/// r37:fixes — a job, never a wait on the interface's thread (round 37's review, item 11):
/// the copy is prepared here, rendered and measured on the job's worker; when it plays, the
/// render is handed to the browser's audition as its own (`Audition::begin_elsewhere`), so
/// Stop, Esc and the next audition stop it, and the window never waits.
fn audition_preset(cx: &mut ToolCx<'_>, a: &Value) -> Res<Job> {
    let a = A(a);
    let (unit, target) = target_of(cx, a)?;
    let unit = unit.ok_or_else(|| ToolError::bad("unit", "give effect_id, track_id or unit"))?;
    if !unit.audible() {
        return Err(ToolError::bad("unit", "this unit makes no sound to hear"));
    }
    let wanted = crate::preset_query::fold(a.str("preset")?.trim());
    // English for the names only: the window keeps its language.
    let entry = {
        let _en = crate::i18n::scoped_thread_locale(crate::i18n::Locale::En);
        entries(unit, true)
    }
    .into_iter()
    .find(|e| crate::preset_query::fold(&e.name) == wanted || e.id == wanted)
    .ok_or_else(|| ToolError::NotFound(format!("a preset {wanted:?} (find_presets names them)")))?;
    let play = a.opt_bool("play")?.unwrap_or(true);
    let app = crate::ai_tools::project_app::app_mut(cx)?;
    let prepared = aud::prepare(app.project(), target, &entry, app.selected_clip, aud::TEMPO);
    let first = prepared
        .needs_engine
        .then(|| app.engine.compile_for_audition(&prepared.project, 0));
    let rate = app.engine.sample_rate.round().max(1.0) as u32;
    let progress = std::sync::Arc::new(cantus_engine::RenderProgress::default());
    // Played: the browser's audition takes it from here (what played before stops).
    let to_play = play.then(|| {
        app.stop_preset_audition();
        app.presets
            .audition
            .begin_elsewhere(&entry.id, progress.clone())
    });
    let name = entry.name.clone();
    let unit_word = unit_id(unit);
    Ok(Job {
        said: Said::new("library.said.audition").name("preset", args::clean(&name)),
        work: Box::new(move |jcx| {
            if (jcx.cancelled)() {
                progress.cancel();
                return Err(ToolError::Cancelled);
            }
            let heard = aud::work(first, &prepared, rate, &progress);
            if let Some(hook) = RENDERED_HOOK.lock().ok().and_then(|h| h.clone()) {
                hook();
            }
            // Cancelled by the agent while it rendered: never played (the channel dropped
            // unsent, the browser's audition lets go of it).
            if (jcx.cancelled)() {
                progress.cancel();
                return Err(ToolError::Cancelled);
            }
            let measured = heard.as_ref().map(|(sample, on)| {
                let lufs = crate::preset_measure::lufs(&sample.frames, 2, rate as f32);
                let peak = sample.frames.iter().fold(0f32, |m, x| m.max(x.abs()));
                let seconds = sample.len_frames() as f64 / f64::from(rate);
                (lufs, peak, seconds, *on)
            });
            let measured = match measured {
                Ok(m) => m,
                // Stopped (Stop, Esc, another audition) or cancelled by the agent.
                Err(_)
                    if progress.cancel.load(std::sync::atomic::Ordering::Relaxed)
                        || (jcx.cancelled)() =>
                {
                    return Ok(JobDone::Answer(json!({
                        "preset": name,
                        "unit": unit_word,
                        "stopped": true,
                        "played": false,
                        "changed_the_project": false
                    })))
                }
                Err(e) => return Err(ToolError::Failed(e.clone())),
            };
            if let Some(tx) = to_play {
                let _ = tx.send(heard);
            }
            let (lufs, peak, seconds, on) = measured;
            Ok(JobDone::Answer(json!({
                "preset": name,
                "unit": unit_word,
                "heard_on": heard_word(on),
                "loudness_lufs": (lufs * 10.0).round() / 10.0,
                "peak_dbfs": (20.0 * f64::from(peak.max(1e-9)).log10() * 10.0).round() / 10.0,
                "seconds": (seconds * 100.0).round() / 100.0,
                "played": play,
                "changed_the_project": false
            })))
        }),
    })
}

/// This builder's tools (registered by one marked line in `ai_tools.rs`).
pub static TOOLS: &[ToolDef] = &[
    ToolDef { name: "find_presets", title: "Find presets", description: "Factory presets of a unit (effect_id, track_id or unit) or of every unit, each with its styles (trap, pop, house, rnb, lofi, rock, latin, ambient, film, jazz, worship), roles and what it is for; filtered by style, role and words. apply_preset loads one; audition_preset lets the user hear it first.", class: ToolClass::Read, input_schema: find_presets_schema, output_schema: None, run: ToolRun::Read(find_presets) },
    ToolDef { name: "audition_preset", title: "Hear a preset", description: "Render a preset on what Cantus's preset browser hears it on — an instrument's role phrase (two bars at 120 BPM), an insert's clip on its channel, or Cantus's own source — and play it to the user (as the browser plays one: Stop or Esc stops it). Answers its loudness (LUFS), peak (dBFS) and length, or that it was stopped. The project, its undo history and its file never change; not an undo step. A job: the window never waits for it.", class: ToolClass::Transport, input_schema: audition_schema, output_schema: None, run: ToolRun::Job(audition_preset) },
];
