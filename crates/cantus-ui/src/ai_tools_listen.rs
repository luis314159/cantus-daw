//! Agent tools: the listening tools — audition, per-track analysis, the masking matrix, mix
//! targets — and the mastering assistant (builder listen, ADRs 0083–0084).
//!
//! Every one renders through the engine's listen (`cantus_engine::listen`): the same
//! render as an export, from a checkpoint where the song before the range has not changed,
//! with analysis taps on the tracks it asks about. They run as jobs (a render takes
//! seconds); the two that change the project — `set_mix_targets` and
//! `apply_master_chain` — plan one undo step each.

use cantus_ai::{Said, ToolClass, ToolDef, ToolError, ToolRun};
use cantus_engine::stems::StemSource;
use serde_json::{json, Value};

// The listening and mastering code this list runs (files of this builder, declared here
// so the shared module files stay untouched).
#[path = "mastering_listen.rs"]
pub mod listening;
#[path = "mastering.rs"]
pub mod mastering;
#[path = "mastering_mix.rs"]
pub mod mix;
#[path = "views/mastering.rs"]
pub mod panel;
#[path = "mastering_tools.rs"]
pub mod run;
#[path = "mastering_targets.rs"]
pub mod targets;

pub use listening::{ListenHost, RenderInputs, Service};

/// The range arguments every listening tool takes.
fn range_props() -> Value {
    json!({
        "start": {
            "description": "Where to start: beats from the song's start (a number) or \"bar:beat\" (1-based, e.g. \"17:1\"). Default: the song's start.",
            "type": ["number", "string"]
        },
        "end": {
            "description": "Where to stop (exclusive), as `start`. Default: the song's end. A range is at most 20 minutes long (when its audio is kept — audition's WAV, master_assist — 20 minutes at 48 kHz, less at higher rates): choose a range for a longer set.",
            "type": ["number", "string"]
        },
        "section": {
            "description": "A marker's name: from that marker to the next one (instead of start and end).",
            "type": "string"
        },
        "exact": {
            "description": "With a hosted plug-in in the project the song's state cannot be saved, and a range starts a pre-roll before itself (close, not exact: a reverb such as Space keeps modulation that turns from where it started); true renders from the song's start instead (exact, slower).",
            "type": "boolean"
        }
    })
}

fn with_range(mut props: Value) -> Value {
    for (k, v) in range_props().as_object().expect("an object") {
        props[k] = v.clone();
    }
    props
}

fn ids_prop(what: &str) -> Value {
    json!({ "type": "array", "items": { "type": "integer" }, "description": what })
}

fn audition_schema() -> Value {
    json!({
        "type": "object",
        "properties": with_range(json!({
            "solo": ids_prop("Track ids to hear alone (with what they are routed through)."),
            "mute": ids_prop("Track ids to leave out."),
            "master": { "type": "boolean", "description": "Through the master's insert chain (default true)." },
            "reference": {
                "type": "string",
                "description": "The spectrum's reference: a genre curve (pop, rock, electronic, hiphop, acoustic, classical; default pop) or a WAV/FLAC/MP3/OGG file inside the project's folder."
            },
            "tail_s": { "type": "number", "minimum": 0, "maximum": 30, "description": "Seconds rung out after the range (default 2), in the WAV only." },
            "wav": { "type": "boolean", "description": "Write the rendered range as a WAV in Cantus's cache and link it (default true)." }
        })),
        "additionalProperties": false
    })
}

fn tracks_schema() -> Value {
    json!({
        "type": "object",
        "properties": with_range(json!({
            "tracks": ids_prop("Track ids to analyse (default: every track)."),
            "master": { "type": "boolean", "description": "The mix heard through the master's chain (default true)." }
        })),
        "additionalProperties": false
    })
}

fn masking_schema() -> Value {
    json!({
        "type": "object",
        "properties": with_range(json!({
            "tracks": ids_prop("Track ids to compare (default: every track)."),
            "bands": { "type": "string", "enum": ["octave", "third"], "description": "Octave (default) or third-octave bands." },
            "top": { "type": "integer", "minimum": 1, "maximum": 200, "description": "How many pairs to return, the worst first (default 12)." }
        })),
        "additionalProperties": false
    })
}

fn targets_schema() -> Value {
    json!({
        "type": "object",
        "properties": with_range(json!({
            "targets": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "track_id": { "type": "integer" },
                        "lufs": { "type": "number", "minimum": -70, "maximum": 0, "description": "The track's integrated loudness at its channel (after its own level, before its mixer strip), LUFS." }
                    },
                    "required": ["track_id", "lufs"],
                    "additionalProperties": false
                }
            }
        })),
        "required": ["targets"],
        "additionalProperties": false
    })
}

fn assist_schema() -> Value {
    let platforms: Vec<&str> = targets::PLATFORMS.iter().map(|p| p.id).collect();
    let curves: Vec<&str> = targets::CURVES.iter().map(|c| c.id).collect();
    json!({
        "type": "object",
        "properties": with_range(json!({
            "platform": { "type": "string", "enum": platforms, "description": "Where the master is going: its loudness and ceiling (default spotify)." },
            "curve": { "type": "string", "enum": curves, "description": "The genre's tonal balance to aim at (default pop)." },
            "reference": { "type": "string", "description": "A reference track inside the project's folder: its balance and width are matched instead of a curve." },
            "mode": { "type": "string", "enum": ["replace", "append"], "description": "Listen to the mix before the master's chain, to replace it (default), or after it, to add to it." }
        })),
        "additionalProperties": false
    })
}

fn apply_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "proposal_id": { "type": "integer", "description": "The id `master_assist` answered with." }
        },
        "required": ["proposal_id"],
        "additionalProperties": false
    })
}

/// The listening tools and the mastering assistant, in the order `tools/list` shows them.
pub static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "audition",
        title: "Listen to a range",
        description: "Render a range of the song (or the whole song) as an export would and answer with numbers: integrated, short-term and momentary loudness and the loudness range (EBU R128), true and sample peak, crest, stereo correlation (overall and the lowest 400 ms window), width and what folding to mono loses, the octave spectrum against a reference curve, each section's loudness, and clicks at section boundaries; plus a WAV of it in Cantus's cache. Solo or mute tracks and take the master chain out to hear parts. A range is rendered from a saved state of the song where nothing before it changed, so listening again after editing a later section is fast. A job.",
        class: ToolClass::Read,
        input_schema: audition_schema,
        output_schema: None,
        run: ToolRun::Job(run::audition),
    },
    ToolDef {
        name: "analyse_tracks",
        title: "Analyse every track in the mix",
        description: "Each track as it sounds in the mix (not soloed), from analysis taps at its channel: integrated loudness and its share of the mix's (LU), short-term maximum, true peak, crest, correlation and width, its octave spectrum in dB, and where its loudest three seconds start. A job.",
        class: ToolClass::Read,
        input_schema: tracks_schema,
        output_schema: None,
        run: ToolRun::Job(run::analyse_tracks),
    },
    ToolDef {
        name: "masking_matrix",
        title: "Who masks whom",
        description: "For every pair of tracks and every octave (or third-octave) band: the share of the band's energy over the range during which the two sound within 6 dB of each other and within 15 dB of the band's loudest track — the moments they fight for the same frequencies — and which is louder. The worst pairs first; with each band, how often three or more tracks crowd it. A job.",
        class: ToolClass::Read,
        input_schema: masking_schema,
        output_schema: None,
        run: ToolRun::Job(run::masking_matrix),
    },
    ToolDef {
        name: "set_mix_targets",
        title: "Set tracks to loudness targets",
        description: "Measure each named track's integrated loudness at its channel (after its own level, before its mixer strip) over the range, and set each track's level so it reaches its target — one undo step. The channel's level is linear, so one render solves it exactly; a level past the model's limits (−96 … +12 dB) stops there and says so. A job that ends in an edit.",
        class: ToolClass::Edit,
        input_schema: targets_schema,
        output_schema: None,
        run: ToolRun::Job(run::set_mix_targets),
    },
    ToolDef {
        name: "master_assist",
        title: "Propose a master",
        description: "The mastering assistant: listen to the mix as it reaches the master and propose a chain of Cantus's own effects on the master — an EQ toward a genre's tonal balance or a reference track's, glue compression when the loudness range is wider than the genre keeps, stereo width and bass mono when the mix folds badly, and a true-peak limiter whose gain lands the platform's loudness under its ceiling — with every setting and why, and the result predicted by running the chain over the mix. Changes nothing: `apply_master_chain` puts it on the master. A job.",
        class: ToolClass::Read,
        input_schema: assist_schema,
        output_schema: None,
        run: ToolRun::Job(run::master_assist),
    },
    ToolDef {
        name: "apply_master_chain",
        title: "Apply the proposed master",
        description: "Put a chain `master_assist` proposed on the master — one undo step; every value stays editable by hand. Adds after the master's own inserts, or onto an empty chain; a proposal that would take the master's own inserts out is refused here (`replace_master_chain` does that), and so is one made before the project last changed (listen again). Listen again with `audition` to read the result.",
        class: ToolClass::Edit,
        input_schema: apply_schema,
        output_schema: None,
        run: ToolRun::Edit(run::apply_master_chain),
    },
    ToolDef {
        name: "replace_master_chain",
        title: "Replace the master chain with the proposal",
        description: "Put a chain `master_assist` proposed on the master in place of the master's own inserts — one undo step, which names the effects it takes out; every value stays editable by hand. For a proposal listened to before the master's chain (mode replace) while the master has inserts; refused if the project changed since the proposal (listen again). A destructive edit: the user is asked.",
        class: ToolClass::Destructive,
        input_schema: apply_schema,
        output_schema: None,
        run: ToolRun::Edit(run::replace_master_chain),
    },
];

/// This builder's prompts: *Master for streaming*.
pub static PROMPTS: &[cantus_ai::PromptDef] = &[cantus_ai::PromptDef {
    name: "master_for_streaming",
    title: "listen.prompt.master.title",
    description: "listen.prompt.master.description",
    arguments: &[cantus_ai::PromptArg {
        name: "platform",
        description: "listen.prompt.master.platform",
        required: false,
    }],
    get: master_prompt,
}];

/// The prompt's words: master the open song for a platform (Spotify unless named).
fn master_prompt(
    _cx: &cantus_ai::ToolCx<'_>,
    args: &Value,
) -> Result<Vec<cantus_ai::PromptMessage>, ToolError> {
    let id = args
        .get("platform")
        .and_then(Value::as_str)
        .unwrap_or("spotify");
    let p = targets::platform(id)
        .ok_or_else(|| ToolError::bad("platform", "one of master_assist's platforms"))?;
    Ok(vec![cantus_ai::PromptMessage {
        role: cantus_ai::Role::User,
        said: Said::new("listen.prompt.master.text")
            .arg("platform", p.name)
            .arg("id", p.id)
            .arg("lufs", p.lufs)
            .arg("ceiling", p.ceiling_dbtp),
    }])
}

/// A sentence the host localises.
pub(crate) fn said(key: &'static str) -> Said {
    Said::new(key)
}

/// Taps for every track of `project` the arguments name (all by default), as snapshot
/// indices and their sources.
pub(crate) fn track_taps(
    project: &cantus_core::Project,
    args: &Value,
) -> Result<(Vec<usize>, Vec<StemSource>), ToolError> {
    let want = listening::ids(args, "tracks")?;
    let mut idx = Vec::new();
    if want.is_empty() {
        idx.extend(0..project.tracks.len());
    } else {
        for id in want {
            let i = project
                .tracks
                .iter()
                .position(|t| t.id == id)
                .ok_or_else(|| ToolError::bad("tracks", format!("no track {}", id.0)))?;
            idx.push(i);
        }
    }
    let taps = idx.iter().map(|i| StemSource::Track(*i)).collect();
    Ok((idx, taps))
}

#[cfg(test)]
#[path = "mastering_tests.rs"]
mod tests;
