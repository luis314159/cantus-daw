//! The listening tools' runs (round 34, ADRs 0083–0084): each prepares its listen on the
//! UI thread — the project as it is to be heard, the render's inputs, the checkpoints — and
//! hears it on a worker; the two that edit plan their one command back on the UI thread.

use super::listening::{self, host_of, prepare, Range};
use super::mastering::{self, Proposal, Target};
use super::mix;
use super::targets;
use super::{said, track_taps};
use cantus_ai::{EditPlan, Job, JobDone, ToolCx, ToolError};
use cantus_core::{Command, EffectId, Project, TrackId};
use cantus_dsp::speculum::ear::{Ear, Reading, OCTAVES};
use cantus_engine::stems::StemSource;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// A tone reference: a genre's curve, or a file opened on the UI thread (through
/// `cantus_ai::open_touch`, inside the allowed folders) and read on the worker.
#[derive(Clone, Debug)]
enum Reference {
    Curve(&'static targets::Curve),
    File(std::sync::Arc<std::fs::File>, PathBuf),
}

/// The `reference` argument: a curve's id, or a file inside the project's folder or a
/// folder the user allowed (`cantus_ai::open_touch`) — read without asking, never outside.
fn reference_of(
    args: &Value,
    host: &dyn cantus_ai::ToolHost,
    key: &str,
) -> Result<Option<Reference>, ToolError> {
    let Some(r) = args.get(key).and_then(Value::as_str) else {
        return Ok(None);
    };
    if let Some(c) = targets::curve(r.trim()) {
        return Ok(Some(Reference::Curve(c)));
    }
    let path = if Path::new(r).is_absolute() {
        PathBuf::from(r)
    } else {
        host.project_dir()
            .ok_or_else(|| {
                ToolError::Refused(
                    "a relative reference needs the project saved in a folder".into(),
                )
            })?
            .join(r)
    };
    // Opened, not only checked (review 1): what is decoded is the very file the check
    // passed, whatever happens to the path afterwards.
    let refused = || {
        ToolError::Refused(format!(
            "{} is not a file in the project's folder or a folder allowed in Settings, AI",
            path.display()
        ))
    };
    let (file, at) =
        cantus_ai::open_touch(host, &path, cantus_ai::Touch::Read).map_err(|_| refused())?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(refused());
    }
    Ok(Some(Reference::File(std::sync::Arc::new(file), at)))
}

/// A reference file, read whole from the file opened for it and heard by an ear (on the
/// worker). The decoders read by path and by extension: the opened file's bytes go to a
/// scratch copy of the same extension in the system's temporary folder, removed after.
fn read_reference(file: &std::fs::File, path: &Path) -> Result<Reading, ToolError> {
    use std::io::Read;
    let failed = |e: String| ToolError::Failed(format!("{}: {e}", path.display()));
    let mut bytes = Vec::new();
    let mut f = file;
    f.read_to_end(&mut bytes)
        .map_err(|e| failed(e.to_string()))?;
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ext = path
        .extension()
        .map_or_else(String::new, |e| e.to_string_lossy().into_owned());
    let copy =
        std::env::temp_dir().join(format!("cantus-reference-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&copy, &bytes).map_err(|e| failed(e.to_string()))?;
    let decoded = cantus_engine::samples::assets::decode_any(&copy);
    let _ = std::fs::remove_file(&copy);
    let data = decoded.map_err(failed)?;
    let mut ear = Ear::new(
        data.rate as f32,
        data.frames.len() as f64 / data.rate.max(1) as f64,
    );
    if data.channels >= 2 {
        let (l, r): (Vec<f32>, Vec<f32>) =
            data.frames.chunks_exact(2).map(|f| (f[0], f[1])).unzip();
        ear.push(&l, &r);
    } else {
        ear.push(&data.frames, &data.frames);
    }
    Ok(ear.whole())
}

/// A reference's curve relative to the 1 kHz octave, and its name.
fn curve_of(r: &Reference) -> Result<(String, [f32; OCTAVES]), ToolError> {
    match r {
        Reference::Curve(c) => Ok((c.id.to_string(), c.octaves)),
        Reference::File(f, p) => {
            let read = read_reference(f, p)?;
            let k1 = read.octaves_db[5];
            let name = p
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            Ok((
                name,
                std::array::from_fn(|o| {
                    listening::round((read.octaves_db[o] - k1) as f64, 1) as f32
                }),
            ))
        }
    }
}

fn flag(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn range_words(project: &Project, r: Range) -> String {
    let end = r.end.unwrap_or_else(|| project.content_end());
    let a = listening::tick_json(project, r.start);
    let b = listening::tick_json(project, end);
    format!(
        "{} – {}",
        a["bar_beat"].as_str().unwrap_or(""),
        b["bar_beat"].as_str().unwrap_or("")
    )
}

pub(super) fn audition(cx: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let project = cx.project().clone();
    let range = listening::range_of(&project, args)?;
    let solo = listening::ids(args, "solo")?;
    let mute = listening::ids(args, "mute")?;
    let heard = listening::heard_project(&project, &solo, &mute, flag(args, "master", true))?;
    let reference = reference_of(args, &*cx.host, "reference")?
        .unwrap_or(Reference::Curve(&targets::CURVES[0]));
    let tail = args
        .get("tail_s")
        .and_then(Value::as_f64)
        .unwrap_or(2.0)
        .clamp(0.0, 30.0);
    let wav = flag(args, "wav", true);
    let host = host_of(cx.host)?;
    let wav_dir = host.listen_service().wav_dir.clone();
    let mut prep = prepare(host, &heard, range, Vec::new(), wav, tail)?;
    if flag(args, "exact", false) {
        prep.opts.preroll_seconds = f64::INFINITY;
    }
    let words = range_words(&project, range);
    Ok(Job {
        said: said("listen.said.audition").arg("range", words),
        work: Box::new(move |jcx| {
            let h = prep.run(jcx)?;
            let (name, curve) = curve_of(&reference)?;
            let mut v = mix::audition_json(&project, range, &h, Some((&name, &curve)));
            if wav {
                let file = wav_name("audition");
                let path = listening::write_wav(&wav_dir, &file, &h)?;
                v["wav"] = json!({
                    "path": path.display().to_string(),
                    "uri": listening::file_uri(&path),
                    "seconds": listening::round(h.audio.len() as f64 / 2.0 / h.sample_rate as f64, 2),
                    "format": "32-bit float WAV, stereo",
                });
            }
            Ok(JobDone::Answer(v))
        }),
    })
}

pub(super) fn analyse_tracks(cx: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let project = cx.project().clone();
    let range = listening::range_of(&project, args)?;
    let heard = listening::heard_project(&project, &[], &[], flag(args, "master", true))?;
    let (idx, taps) = track_taps(&project, args)?;
    let mut prep = prepare(host_of(cx.host)?, &heard, range, taps, false, 0.0)?;
    if flag(args, "exact", false) {
        prep.opts.preroll_seconds = f64::INFINITY;
    }
    let words = range_words(&project, range);
    Ok(Job {
        said: said("listen.said.analyse").arg("range", words),
        work: Box::new(move |jcx| {
            let h = prep.run(jcx)?;
            Ok(JobDone::Answer(mix::tracks_json(&project, range, &h, &idx)))
        }),
    })
}

pub(super) fn masking_matrix(cx: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let project = cx.project().clone();
    let range = listening::range_of(&project, args)?;
    let (idx, taps) = track_taps(&project, args)?;
    let octaves = match args.get("bands").and_then(Value::as_str) {
        None | Some("octave") => true,
        Some("third") => false,
        Some(_) => return Err(ToolError::bad("bands", "octave or third")),
    };
    let top = args
        .get("top")
        .and_then(Value::as_u64)
        .unwrap_or(12)
        .clamp(1, 200) as usize;
    let mut prep = prepare(host_of(cx.host)?, &project, range, taps, false, 0.0)?;
    if flag(args, "exact", false) {
        prep.opts.preroll_seconds = f64::INFINITY;
    }
    let words = range_words(&project, range);
    Ok(Job {
        said: said("listen.said.masking").arg("range", words),
        work: Box::new(move |jcx| {
            let h = prep.run(jcx)?;
            Ok(JobDone::Answer(mix::masking_json(
                &project, range, &h, &idx, octaves, top,
            )))
        }),
    })
}

/// A WAV's name in the cache: what it is, and a number no other has this session.
pub(super) fn wav_name(what: &str) -> String {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("{what}-{t}-{n}.wav")
}

pub(super) fn set_mix_targets(cx: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let project = cx.project().clone();
    let revision = cx.bus().revision();
    let range = listening::range_of(&project, args)?;
    let list = args
        .get("targets")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| ToolError::bad("targets", "one or more {track_id, lufs}"))?;
    let mut wanted: Vec<(TrackId, f64)> = Vec::new();
    for t in list {
        let id = t
            .get("track_id")
            .and_then(Value::as_u64)
            .map(TrackId)
            .ok_or_else(|| ToolError::bad("targets", "each needs a track_id"))?;
        let lufs = t
            .get("lufs")
            .and_then(Value::as_f64)
            .filter(|l| (-70.0..=0.0).contains(l))
            .ok_or_else(|| ToolError::bad("targets", "each needs lufs, −70 … 0"))?;
        if project.track(id).is_none() {
            return Err(ToolError::bad("targets", format!("no track {}", id.0)));
        }
        if wanted.iter().any(|w| w.0 == id) {
            return Err(ToolError::bad("targets", format!("track {} twice", id.0)));
        }
        wanted.push((id, lufs));
    }
    let taps: Vec<StemSource> = wanted
        .iter()
        .map(|(id, _)| {
            let i = project.tracks.iter().position(|t| t.id == *id).unwrap_or(0);
            StemSource::Track(i)
        })
        .collect();
    let mut prep = prepare(host_of(cx.host)?, &project, range, taps, false, 0.0)?;
    if flag(args, "exact", false) {
        prep.opts.preroll_seconds = f64::INFINITY;
    }
    let n = wanted.len();
    let named: Vec<String> = wanted
        .iter()
        .map(|(id, lufs)| {
            let name = project
                .track(*id)
                .map_or_else(String::new, |t| t.name.clone());
            format!("“{name}” {lufs:.1} LUFS")
        })
        .collect();
    Ok(Job {
        said: said("listen.said.targets")
            .nested("count", tracks_phrase(n))
            .arg("tracks", named.join(", ")),
        work: Box::new(move |jcx| {
            let h = prep.run(jcx)?;
            let measured: Vec<Option<f64>> = h
                .taps
                .iter()
                .map(|(_, ear)| ear.reading(0, h.range_frames as u64).integrated_lufs)
                .collect();
            let trims = mix::trims(&project, &wanted, &measured);
            let rows: Vec<Value> = trims
                .iter()
                .map(|t| {
                    json!({
                        "track_id": t.track.0,
                        "name": project.track(t.track).map(|x| x.name.clone()),
                        "measured_lufs": t.measured.map_or(Value::Null, listening::level),
                        "target_lufs": t.target,
                        "old_gain_db": listening::round(t.old_gain_db as f64, 2),
                        "new_gain_db": listening::round(t.new_gain_db as f64, 2),
                        "limited": t.limited,
                        "silent": t.measured.is_none(),
                    })
                })
                .collect();
            let answer = json!({ "range": mix::range_json(&project, range), "tracks": rows });
            let then = answer.clone();
            Ok(JobDone::Then(
                answer,
                Box::new(move |cx: &mut ToolCx<'_>| {
                    // Measured against the project as it was: refused if it moved since.
                    if cx.bus().revision() != revision {
                        return Err(ToolError::Stale(cx.bus().revision()));
                    }
                    let commands: Vec<Command> = trims
                        .iter()
                        .filter(|t| t.measured.is_some() && t.new_gain_db != t.old_gain_db)
                        .map(|t| Command::SetTrackGain {
                            id: t.track,
                            gain_db: t.new_gain_db,
                        })
                        .collect();
                    if commands.is_empty() {
                        return Err(ToolError::Failed(
                            "every track is at its target already, or silent in the range".into(),
                        ));
                    }
                    Ok(EditPlan {
                        command: Command::Batch {
                            label_key: "cmd.set_mix_targets".into(),
                            commands,
                        },
                        said: said("listen.said.trims")
                            .nested("count", tracks_phrase(changed(&trims).count()))
                            .arg("trims", trim_words(cx.project(), &trims)),
                        result: then,
                    })
                }),
            ))
        }),
    })
}

/// A proposal kept for `apply_master_chain`, with what it was listened for.
pub struct Kept {
    pub id: u64,
    pub proposal: Proposal,
    /// It replaces the master's chain (listened before it), or adds after it.
    pub replace: bool,
    /// The project's revision it was listened at.
    pub revision: u64,
}

/// The proposals of this session, newest last (a few are kept).
#[derive(Default)]
pub struct Proposals {
    pub kept: Vec<Kept>,
    next: u64,
}

impl Proposals {
    pub fn keep(&mut self, proposal: Proposal, replace: bool, revision: u64) -> u64 {
        self.next += 1;
        let id = self.next;
        self.kept.push(Kept {
            id,
            proposal,
            replace,
            revision,
        });
        if self.kept.len() > 8 {
            self.kept.remove(0);
        }
        id
    }

    pub fn get(&self, id: u64) -> Option<&Kept> {
        self.kept.iter().find(|k| k.id == id)
    }
}

/// The project as the assistant listens to it: before the master's chain (or after it, to
/// add), its fader, balance and width at rest — they come after the chain.
pub fn assist_project(project: &Project, replace: bool) -> Project {
    use cantus_core::automation::AutoTarget as A;
    let post = |t: &A| matches!(t, A::MasterGain | A::MasterPan | A::MasterWidth);
    let mut p = project.clone();
    p.master.fx_enabled = !replace && p.master.fx_enabled;
    p.master.gain_db = 0.0;
    p.master.pan = 0.0;
    p.master.phase_invert = false;
    p.master.width = 1.0;
    // Automation and modulation of the strip after the chain come after it too: out of
    // the mix the chain is made for (the proposal warns of them).
    for t in &mut p.tracks {
        t.clips
            .retain(|c| !c.automation.as_ref().is_some_and(|a| post(&a.target)));
    }
    let hosts: Vec<cantus_core::ModHost> = p.all_modulators().map(|(h, _)| h).collect();
    for h in hosts {
        if let Some(ms) = p.modulators_mut(h) {
            for m in ms.iter_mut() {
                m.routes.retain(|r| !post(&r.target));
            }
        }
    }
    p
}

/// What of the master strip after the chain moves during the song (automation or a
/// modulator on its level, balance or width): the assistant's numbers hold for its resting
/// values only, and says so (review 1).
pub fn post_moves(project: &Project) -> Vec<&'static str> {
    use cantus_core::automation::AutoTarget as A;
    let mut targets: Vec<A> = project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter_map(|c| c.automation.as_ref().map(|a| a.target.clone()))
        .collect();
    targets.extend(
        project
            .all_modulators()
            .flat_map(|(_, m)| m.routes.iter().map(|r| r.target.clone())),
    );
    let mut out = Vec::new();
    for (t, name) in [
        (A::MasterGain, "level"),
        (A::MasterPan, "balance"),
        (A::MasterWidth, "width"),
    ] {
        if targets.contains(&t) {
            out.push(name);
        }
    }
    out
}

/// A proposal as an agent reads it.
pub fn proposal_json(id: u64, p: &Proposal) -> Value {
    let units: Vec<Value> = p
        .chain
        .iter()
        .map(|slot| {
            let infos = slot.spec.kind.params();
            let set: serde_json::Map<String, Value> = infos
                .iter()
                .filter(|i| slot.spec.get(i.key) != i.default)
                .map(|i| {
                    (
                        i.key.to_string(),
                        json!({ "value": listening::round(slot.spec.get(i.key) as f64, 3), "unit": i.unit }),
                    )
                })
                .collect();
            json!({ "kind": format!("{:?}", slot.spec.kind), "settings": set })
        })
        .collect();
    let why: Vec<Value> = p
        .why
        .iter()
        .map(|w| {
            let args: Vec<(&str, &dyn std::fmt::Display)> = w
                .args
                .iter()
                .map(|(k, v)| (*k, v as &dyn std::fmt::Display))
                .collect();
            json!({
                "unit": format!("{:?}", w.kind),
                "why": crate::i18n::tf_in(crate::i18n::Locale::En, w.key, &args),
            })
        })
        .collect();
    json!({
        "proposal_id": id,
        "target": {
            "platform": p.target.platform.id,
            "name": p.target.platform.name,
            "integrated_lufs": p.target.platform.lufs,
            "ceiling_dbtp": p.target.platform.ceiling_dbtp,
            "source": p.target.platform.source,
            "checked": p.target.platform.checked,
            "curve": p.target.curve_id.unwrap_or("reference"),
            "curve_db": p.target.curve.to_vec(),
        },
        "before": listening::reading_json(&p.before),
        "before_spectrum": listening::spectrum_json(&p.before, Some(&p.target.curve)),
        "chain": units,
        "why": why,
        "predicted": listening::reading_json(&p.predicted),
        "short_of_target_lu": p.short_of_target_lu.map(|x| listening::round(x, 2)),
        "predicted_spectrum": listening::spectrum_json(&p.predicted, Some(&p.target.curve)),
        "master_strip_after_chain": {
            "gain_db": p.post.gain_db,
            "pan": p.post.pan,
            "width": p.post.width,
            "phase_invert": p.post.invert,
        },
    })
}

pub(super) fn master_assist(cx: &mut ToolCx<'_>, args: &Value) -> Result<Job, ToolError> {
    let project = cx.project().clone();
    let revision = cx.bus().revision();
    let range = listening::range_of(&project, args)?;
    let platform = match args.get("platform").and_then(Value::as_str) {
        None => &targets::PLATFORMS[0],
        Some(id) => targets::platform(id)
            .ok_or_else(|| ToolError::bad("platform", "one of the listed platforms"))?,
    };
    let curve = match args.get("curve").and_then(Value::as_str) {
        None => &targets::CURVES[0],
        Some(id) => {
            targets::curve(id).ok_or_else(|| ToolError::bad("curve", "one of the listed curves"))?
        }
    };
    let reference = match reference_of(args, &*cx.host, "reference")? {
        Some(Reference::File(f, p)) => Some((f, p)),
        Some(Reference::Curve(_)) => {
            return Err(ToolError::bad(
                "reference",
                "a file; a genre goes in `curve`",
            ))
        }
        None => None,
    };
    let replace = match args.get("mode").and_then(Value::as_str) {
        None | Some("replace") => true,
        Some("append") => false,
        Some(_) => return Err(ToolError::bad("mode", "replace or append")),
    };
    let heard = assist_project(&project, replace);
    let host = host_of(cx.host)?;
    let mut prep = prepare(host, &heard, range, Vec::new(), true, 0.0)?;
    let proposals = prep.proposals();
    if flag(args, "exact", false) {
        prep.opts.preroll_seconds = f64::INFINITY;
    }
    let post = mastering::Post::of(&project.master);
    let moves = post_moves(&project);
    let stale = prep.stale_flag();
    let mut scratch = project.clone();
    Ok(Job {
        said: said("listen.said.assist").arg("platform", platform.name),
        work: Box::new(move |jcx| {
            let h = prep.run(jcx)?;
            let target = match &reference {
                Some((f, p)) => Target::reference(platform, &read_reference(f, p)?),
                None => Target::genre(platform, curve),
            };
            let mut new_slot = |k| scratch.make_effect(k);
            let proposal = mastering::propose(
                &h.audio,
                h.sample_rate,
                &target,
                &post,
                &mut new_slot,
                &|| (jcx.cancelled)() || stale.load(std::sync::atomic::Ordering::Relaxed),
            )
            .ok_or(ToolError::Cancelled)?;
            // Another project opened meanwhile: no proposal for it (review 2).
            if stale.load(std::sync::atomic::Ordering::Acquire) {
                return Err(ToolError::Cancelled);
            }
            let mut list = proposals
                .lock()
                .map_err(|_| ToolError::Failed("poisoned".into()))?;
            let v = proposal_json(0, &proposal);
            let id = list.keep(proposal, replace, revision);
            let mut v = v;
            v["proposal_id"] = json!(id);
            v["replaces_master_chain"] = json!(replace);
            v["range"] = mix::range_json(&project, range);
            let mut warnings = Vec::new();
            if !moves.is_empty() {
                warnings.push(format!(
                    "the master strip's {} moves during the song (automation or a modulator) after the chain: these numbers hold at its resting value",
                    moves.join(", ")
                ));
            }
            if let Some(lu) = v["short_of_target_lu"].as_f64() {
                warnings.push(format!(
                    "kept {lu:.1} LU under the platform's loudness to stay under its ceiling (the master strip after the chain adds to the peaks)"
                ));
            }
            if !warnings.is_empty() {
                v["warnings"] = json!(warnings);
            }
            Ok(JobDone::Answer(v))
        }),
    })
}

pub(super) fn apply_master_chain(cx: &mut ToolCx<'_>, args: &Value) -> Result<EditPlan, ToolError> {
    plan_apply(cx, args, false)
}

pub(super) fn replace_master_chain(
    cx: &mut ToolCx<'_>,
    args: &Value,
) -> Result<EditPlan, ToolError> {
    plan_apply(cx, args, true)
}

/// The master's own inserts a proposal would take out, by name (none when it adds after
/// them, or the chain is empty).
pub fn removed_names(project: &Project, replace: bool) -> Vec<String> {
    if !replace {
        return Vec::new();
    }
    project
        .master
        .effects
        .iter()
        .map(|e| crate::i18n::t(e.spec.kind.name_key()))
        .collect()
}

/// Put a kept proposal on the master (review 1): refused when the project moved since it
/// was listened to (listen again), and — from `apply_master_chain`, an Edit — refused when
/// it would take the user's own master inserts out: that is `replace_master_chain`'s, a
/// Destructive tool, whose sentence names what goes.
fn plan_apply(cx: &mut ToolCx<'_>, args: &Value, replacing: bool) -> Result<EditPlan, ToolError> {
    let id = args
        .get("proposal_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::bad("proposal_id", "the id master_assist answered with"))?;
    let host = host_of(cx.host)?;
    let epoch = host.listen_epoch();
    let service = host.listen_service();
    service.sync_epoch(epoch);
    let proposals = service.proposals.clone();
    let list = proposals
        .lock()
        .map_err(|_| ToolError::Failed("poisoned".into()))?;
    let kept = list
        .get(id)
        .ok_or_else(|| ToolError::NotFound(format!("proposal {id}")))?;
    if cx.bus().revision() != kept.revision {
        return Err(ToolError::Stale(cx.bus().revision()));
    }
    let removed = removed_names(cx.project(), kept.replace);
    if !removed.is_empty() && !replacing {
        return Err(ToolError::Refused(format!(
            "this proposal replaces the master's own effects ({}); replace_master_chain does that",
            removed.join(", ")
        )));
    }
    let mut proposal = kept.proposal.clone();
    // Fresh ids from the bus: the proposal's were a scratch copy's.
    for slot in &mut proposal.chain {
        slot.id = EffectId(cx.bus_mut().alloc_id());
    }
    let command = mastering::apply_command(cx.project(), &proposal, kept.replace);
    let units = proposal
        .chain
        .iter()
        .map(|s| crate::i18n::t(s.spec.kind.name_key()))
        .collect::<Vec<_>>()
        .join(", ");
    let said = if removed.is_empty() {
        said("listen.said.apply")
    } else {
        said("listen.said.apply_replace").arg("removed", removed.join(", "))
    };
    Ok(EditPlan {
        command,
        said: said
            .arg("platform", proposal.target.platform.name)
            .arg("units", units)
            .arg("count", removed.len()),
        result: json!({
            "effects": proposal.chain.iter().map(|s| json!({ "effect_id": s.id.0, "kind": format!("{:?}", s.spec.kind) })).collect::<Vec<_>>(),
            "removed": removed,
        }),
    })
}

/// The trims that change a level.
fn changed(trims: &[mix::Trim]) -> impl Iterator<Item = &mix::Trim> {
    trims
        .iter()
        .filter(|t| t.measured.is_some() && t.new_gain_db != t.old_gain_db)
}

/// The trims an edit makes, in words — “Name” +1.5 dB, … — what the confirmation card
/// names.
fn trim_words(project: &Project, trims: &[mix::Trim]) -> String {
    changed(trims)
        .map(|t| {
            let name = project
                .track(t.track)
                .map_or_else(String::new, |x| x.name.clone());
            format!("“{name}” {:+.1} dB", t.new_gain_db - t.old_gain_db)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// "1 track" / "3 tracks": a counted phrase, its plural chosen here (review 3: "1 tracks").
fn tracks_phrase(n: usize) -> cantus_ai::Said {
    said(if n == 1 {
        "listen.phrase.tracks.one"
    } else {
        "listen.phrase.tracks.many"
    })
    .arg("n", n)
}
