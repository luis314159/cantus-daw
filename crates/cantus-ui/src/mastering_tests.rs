//! The listening tools and the mastering assistant, called as the MCP server will call them
//! (round 34, ADRs 0083–0084): their `ToolRun`s on a test host, their jobs on this thread.

use super::listening::tests::TestHost;
use super::TOOLS;
use cantus_ai::{JobCx, JobDone, ToolCx, ToolError, ToolRun};
use cantus_core::*;
use serde_json::{json, Value};

/// A drum-less little band: a saw lead, a square bass an octave and a fifth under it,
/// a sine pad high up, eight bars at 120 BPM with a marker at bar 5.
fn band() -> Project {
    let mut p = Project::new("listen-tools");
    p.tempo_bpm = 120.0;
    let parts: [(&str, Waveform, u8, i64); 3] = [
        ("Lead", Waveform::Saw, 60, 1),
        ("Bass", Waveform::Square, 36, 2),
        ("Pad", Waveform::Sine, 84, 4),
    ];
    for (name, wave, root, every) in parts {
        let mut t = p.make_track(name);
        let f = t.instrument.firmus_mut().unwrap();
        f.osc1_wave = wave;
        f.amp_release_s = 0.1;
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(32));
        for b in (0..32i64).step_by(every as usize) {
            let pitch = root + [0u8, 3, 7, 5][(b / every % 4) as usize];
            c.notes
                .push(p.make_note(Ticks::beats(b), Ticks::beats(every) - Ticks(60), pitch, 100));
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    let id = cantus_core::tempo::MarkerId(p.alloc_id());
    p.markers.push(cantus_core::tempo::Marker {
        id,
        tick: Ticks::beats(16),
        name: "B".into(),
        color: None,
    });
    p
}

fn tool(name: &str) -> &'static cantus_ai::ToolDef {
    TOOLS
        .iter()
        .find(|t| t.name == name)
        .expect("a tool by that name")
}

/// Run a job tool to its end on this thread: its answer, and the edit it planned back.
fn job(
    host: &mut TestHost,
    name: &str,
    args: Value,
) -> Result<(Value, Option<cantus_ai::EditPlan>), ToolError> {
    let ToolRun::Job(prepare) = tool(name).run else {
        panic!("{name} is a job");
    };
    let job = prepare(&mut ToolCx { host }, &args)?;
    let progress = |_: f32| {};
    let cancelled = || false;
    let done = (job.work)(&JobCx {
        progress: &progress,
        cancelled: &cancelled,
    })?;
    Ok(match done {
        JobDone::Answer(v) => (v, None),
        JobDone::Then(v, then) => {
            let plan = then(&mut ToolCx { host })?;
            (v, Some(plan))
        }
    })
}

fn lufs(v: &Value) -> f64 {
    v["integrated_lufs"].as_f64().expect("a loudness")
}

#[test]
fn every_listening_tool_is_well_formed() {
    let names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
    for want in [
        "audition",
        "analyse_tracks",
        "masking_matrix",
        "set_mix_targets",
        "master_assist",
        "apply_master_chain",
    ] {
        assert!(names.contains(&want), "{want}");
    }
    for t in TOOLS {
        let s = (t.input_schema)();
        assert_eq!(s["type"], "object", "{}", t.name);
        assert!(!t.description.is_empty() && !t.title.is_empty());
    }
}

#[test]
fn an_audition_answers_with_numbers_a_wav_and_resumes_from_a_checkpoint() {
    let mut host = TestHost::new("audition", band());
    let (v, _) = job(&mut host, "audition", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(v["rendered"]["from"], "start");
    let whole = lufs(&v["loudness"]);
    assert!((-40.0..-3.0).contains(&whole), "{whole}");
    assert_eq!(
        v["sections"].as_array().unwrap().len(),
        2,
        "split at the marker"
    );
    assert_eq!(v["sections"][1]["name"], "B");
    let octaves = v["spectrum"]["octaves"].as_array().unwrap();
    assert_eq!(octaves.len(), 10);
    assert_eq!(octaves[5]["db"], 0.0, "relative to 1 kHz");
    assert!(octaves[3]["reference_db"].is_number());
    let path = std::path::PathBuf::from(v["wav"]["path"].as_str().unwrap());
    assert!(path.is_file(), "{}", path.display());
    assert_eq!(v["clicks"]["boundaries"][0]["click"], false);
    // The section after the marker again: from the checkpoint the first listen left.
    let (b, _) =
        job(&mut host, "audition", json!({ "section": "B" })).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(b["rendered"]["from"], "checkpoint", "{}", b["rendered"]);
    assert_eq!(b["range"]["start"]["bar_beat"], "5:1");
    let section = v["sections"][1]["integrated_lufs"].as_f64().unwrap();
    assert!(
        (lufs(&b["loudness"]) - section).abs() < 0.05,
        "the same audio, read alone"
    );
    // Bars and beats or beats: the same place.
    let (c, _) = job(
        &mut host,
        "audition",
        json!({ "start": 16.0, "end": "7:1", "wav": false }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(c["range"]["end"]["beats"], 24.0);
    assert!(c.get("wav").is_none());
}

#[test]
fn a_solo_is_quieter_than_the_band_and_a_bad_range_is_refused() {
    let p = band();
    let lead = p.tracks[0].id;
    let mut host = TestHost::new("solo", p);
    let (all, _) =
        job(&mut host, "audition", json!({ "wav": false })).unwrap_or_else(|e| panic!("{e}"));
    let (solo, _) = job(
        &mut host,
        "audition",
        json!({ "solo": [lead.0], "wav": false }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(lufs(&solo["loudness"]) < lufs(&all["loudness"]) - 1.0);
    for bad in [
        json!({ "start": "0:1" }),
        json!({ "start": 8.0, "end": 4.0 }),
        json!({ "section": "Nope" }),
        json!({ "solo": [999_999] }),
    ] {
        assert!(
            matches!(
                job(&mut host, "audition", bad.clone()),
                Err(ToolError::BadArgument { .. })
            ),
            "{bad}"
        );
    }
    let outside = job(
        &mut host,
        "audition",
        json!({ "reference": "../../../etc/hosts" }),
    );
    assert!(
        matches!(outside, Err(ToolError::Refused(_))),
        "a file outside the project"
    );
}

#[test]
fn the_tracks_and_who_masks_whom() {
    let mut p = band();
    // A second lead doubling the first: the two fight in the lead's octaves.
    let mut twin = p.tracks[0].clone();
    twin.id = TrackId(p.alloc_id());
    twin.name = "Twin".into();
    twin.strip = None;
    p.tracks.push(twin);
    let mut host = TestHost::new("masking", p);
    let (t, _) = job(&mut host, "analyse_tracks", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let rows = t["tracks"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    for r in rows {
        assert!(r["loudness"]["integrated_lufs"].is_number(), "{r}");
        assert!(r["share_of_mix_lu"].as_f64().unwrap() < 0.5, "{r}");
        assert_eq!(r["octaves_db"].as_array().unwrap().len(), 10);
    }
    let (m, _) =
        job(&mut host, "masking_matrix", json!({ "top": 5 })).unwrap_or_else(|e| panic!("{e}"));
    let first = &m["pairs"][0];
    let names = [
        first["a"]["name"].as_str().unwrap(),
        first["b"]["name"].as_str().unwrap(),
    ];
    assert!(names.contains(&"Lead") && names.contains(&"Twin"), "{m}");
    assert!(first["overlap"].as_f64().unwrap() > 0.5, "{first}");
}

#[test]
fn mix_targets_land_within_a_tenth_of_a_lu_in_one_undo_step() {
    let p = band();
    let (lead, bass) = (p.tracks[0].id, p.tracks[1].id);
    let before = p.clone();
    let mut host = TestHost::new("targets", p);
    let args = json!({ "targets": [{ "track_id": lead.0, "lufs": -24.0 }, { "track_id": bass.0, "lufs": -20.0 }] });
    let (answer, plan) = job(&mut host, "set_mix_targets", args).unwrap_or_else(|e| panic!("{e}"));
    let plan = plan.expect("an edit planned");
    assert_eq!(answer["tracks"].as_array().unwrap().len(), 2);
    host.bus
        .execute(plan.command, Origin::Mcp)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(host.bus.history().len(), 1, "one undo step");
    let (t, _) = job(
        &mut host,
        "analyse_tracks",
        json!({ "tracks": [lead.0, bass.0] }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let got = |i: usize| {
        t["tracks"][i]["loudness"]["integrated_lufs"]
            .as_f64()
            .unwrap()
    };
    assert!((got(0) - -24.0).abs() < 0.1, "{}", got(0));
    assert!((got(1) - -20.0).abs() < 0.1, "{}", got(1));
    host.bus.undo();
    assert_eq!(
        host.bus.project(),
        &before,
        "undo restores the project exactly"
    );
}

#[test]
fn the_assistant_lands_the_platform_loudness_under_its_ceiling() {
    for platform in ["spotify", "apple_music", "club"] {
        let p = band();
        let before = p.clone();
        let mut host = TestHost::new(&format!("assist-{platform}"), p);
        let (prop, _) = job(&mut host, "master_assist", json!({ "platform": platform }))
            .unwrap_or_else(|e| panic!("{e}"));
        let target = prop["target"]["integrated_lufs"].as_f64().unwrap();
        let ceiling = prop["target"]["ceiling_dbtp"].as_f64().unwrap();
        let predicted = &prop["predicted"];
        assert!(
            (lufs(predicted) - target).abs() <= 0.5,
            "{platform}: {predicted}"
        );
        let ToolRun::Edit(apply) = tool("apply_master_chain").run else {
            panic!()
        };
        let plan = apply(
            &mut ToolCx { host: &mut host },
            &json!({ "proposal_id": prop["proposal_id"] }),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        host.bus
            .execute(plan.command, Origin::Mcp)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!host.bus.project().master.effects.is_empty());
        // Measured on a real render with the chain in place.
        let (heard, _) =
            job(&mut host, "audition", json!({ "wav": false })).unwrap_or_else(|e| panic!("{e}"));
        let got = lufs(&heard["loudness"]);
        let tp = heard["loudness"]["true_peak_dbtp"].as_f64().unwrap();
        println!(
            "{platform}: target {target} got {got:.2} LUFS, true peak {tp:.2} (ceiling {ceiling})"
        );
        assert!(
            (got - target).abs() <= 0.5,
            "{platform}: {got} against {target}"
        );
        assert!(
            tp <= ceiling + 1e-3,
            "{platform}: true peak {tp} over {ceiling}"
        );
        host.bus.undo();
        let (mut now, mut was) = (host.bus.project().clone(), before.clone());
        (now.next_id, was.next_id) = (0, 0);
        assert_eq!(now, was, "undo restores the project (ids are never reused)");
    }
}

fn edit_tool(
    host: &mut TestHost,
    name: &str,
    args: Value,
) -> Result<cantus_ai::EditPlan, ToolError> {
    let ToolRun::Edit(plan) = tool(name).run else {
        panic!("{name} is an edit");
    };
    plan(&mut ToolCx { host }, &args)
}

/// Review 1: taking the user's own master inserts out is a destructive tool's, and says
/// what goes; a proposal listened to before the project changed is stale.
#[test]
fn replacing_the_users_master_chain_is_destructive_and_a_stale_proposal_is_refused() {
    let mut p = band();
    let eq = p.make_effect(EffectKind::Eq);
    p.master.effects.push(eq);
    let mut host = TestHost::new("replace", p);
    let (prop, _) = job(&mut host, "master_assist", json!({ "platform": "spotify" }))
        .unwrap_or_else(|e| panic!("{e}"));
    let id = json!({ "proposal_id": prop["proposal_id"] });
    assert_eq!(
        tool("replace_master_chain").class,
        cantus_ai::ToolClass::Destructive
    );
    assert_eq!(tool("apply_master_chain").class, cantus_ai::ToolClass::Edit);
    let refused = edit_tool(&mut host, "apply_master_chain", id.clone());
    assert!(
        matches!(&refused, Err(ToolError::Refused(r)) if r.contains("replace_master_chain")),
        "an Edit takes nothing out"
    );
    let plan =
        edit_tool(&mut host, "replace_master_chain", id.clone()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(plan.said.key, "listen.said.apply_replace");
    assert!(
        plan.said
            .args
            .iter()
            .any(|(k, v)| *k == "removed"
                && matches!(v, cantus_ai::SaidArg::Text(t) if !t.is_empty())),
        "{:?}",
        plan.said
    );
    // The project moves on: the proposal is stale.
    host.bus
        .execute(Command::SetTempo { bpm: 121.0 }, Origin::User)
        .unwrap_or_else(|e| panic!("{e}"));
    let stale = edit_tool(&mut host, "replace_master_chain", id);
    assert!(matches!(stale, Err(ToolError::Stale(_))));
}

/// Review 1: another project opened forgets the last one's checkpoints and proposals — an
/// id from it never applies to the new one.
#[test]
fn another_project_forgets_the_last_ones_checkpoints_and_proposals() {
    let mut host = TestHost::new("epoch", band());
    let (prop, _) = job(&mut host, "master_assist", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let _ = job(&mut host, "audition", json!({ "wav": false })).unwrap_or_else(|e| panic!("{e}"));
    assert!(!host.service.checkpoints.lock().unwrap().is_empty());
    // Open another project: the application bumps its epoch.
    host.bus = CommandBus::new(band());
    host.epoch += 1;
    let gone = edit_tool(
        &mut host,
        "apply_master_chain",
        json!({ "proposal_id": prop["proposal_id"] }),
    );
    assert!(matches!(gone, Err(ToolError::NotFound(_))));
    assert!(
        host.service.checkpoints.lock().unwrap().is_empty(),
        "the last project's engines went"
    );
}

/// Review 3: ten minutes with no listen and the checkpoints are let go — not five, and not
/// while a listen holds them.
#[test]
fn quiet_listening_lets_the_checkpoints_go() {
    let mut host = TestHost::new("quiet", band());
    let _ = job(&mut host, "audition", json!({ "wav": false })).unwrap_or_else(|e| panic!("{e}"));
    let now = std::time::Instant::now();
    assert!(!host.service.checkpoints.lock().unwrap().is_empty());
    host.service
        .release_if_quiet(now + std::time::Duration::from_secs(5 * 60));
    assert!(
        !host.service.checkpoints.lock().unwrap().is_empty(),
        "five minutes"
    );
    {
        // A listen running holds them: never waited for, never taken from it.
        let held = host.service.checkpoints.clone();
        let _running = held.lock().unwrap();
        host.service
            .release_if_quiet(now + super::listening::QUIET * 2);
    }
    assert!(!host.service.checkpoints.lock().unwrap().is_empty(), "held");
    host.service
        .release_if_quiet(now + super::listening::QUIET + std::time::Duration::from_secs(1));
    assert!(
        host.service.checkpoints.lock().unwrap().is_empty(),
        "ten minutes"
    );
}

/// Review 1: the master strip after the chain — a wide master (1.6) a couple of dB up — is
/// in the assistant's trials, so the real render after applying still lands the loudness
/// under the ceiling; and a master fader moved by automation is warned of.
#[test]
fn the_master_strip_after_the_chain_is_in_the_prediction() {
    let mut p = band();
    p.master.width = 1.6;
    p.master.gain_db = 2.0;
    let mut host = TestHost::new("post", p);
    let (prop, _) = job(&mut host, "master_assist", json!({ "platform": "spotify" }))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        prop["predicted"]["true_peak_dbtp"].as_f64().unwrap() <= -1.0,
        "{}",
        prop["predicted"]
    );
    let plan = edit_tool(
        &mut host,
        "apply_master_chain",
        json!({ "proposal_id": prop["proposal_id"] }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    host.bus
        .execute(plan.command, Origin::Mcp)
        .unwrap_or_else(|e| panic!("{e}"));
    let (heard, _) =
        job(&mut host, "audition", json!({ "wav": false })).unwrap_or_else(|e| panic!("{e}"));
    let got = lufs(&heard["loudness"]);
    let tp = heard["loudness"]["true_peak_dbtp"].as_f64().unwrap();
    assert!((got - -14.0).abs() <= 0.5, "{got}");
    assert!(
        tp <= -1.0 + 1e-3,
        "true peak {tp} over the ceiling with the master wide"
    );
    assert!(prop.get("warnings").is_none());
    // A fader ride on the master: warned, and left out of the mix the chain is made for.
    let mut q = band();
    let ride = q.make_automation_clip(
        "ride",
        cantus_core::automation::AutoTarget::MasterGain,
        Ticks::ZERO,
        Ticks::beats(32),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.4),
            AutoPoint::new(Ticks::beats(32), 0.9),
        ],
    );
    q.tracks[0].clips.push(ride);
    assert!(super::run::assist_project(&q, true).tracks[0]
        .clips
        .iter()
        .all(|c| c.automation.is_none()));
    let mut host = TestHost::new("post-ride", q);
    let (prop, _) = job(&mut host, "master_assist", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        prop["warnings"][0].as_str().unwrap().contains("level"),
        "{prop}"
    );
}

/// Review 1: a range longer than a listen may keep is refused with what to do, never
/// rendered into gigabytes.
#[test]
fn a_range_too_long_is_refused() {
    let mut host = TestHost::new("long", band());
    for tool_name in ["audition", "master_assist"] {
        let r = job(&mut host, tool_name, json!({ "end": "999:1" }));
        assert!(
            matches!(&r, Err(ToolError::Refused(m)) if m.contains("choose a range")),
            "{tool_name}"
        );
    }
    // Without audio kept, up to twenty minutes — the length in minutes and seconds
    // (review 3: rounded, "20 … 20 at most"): 998 bars at 120 BPM.
    let r = job(&mut host, "analyse_tracks", json!({ "end": "999:1" }));
    assert!(
        matches!(&r, Err(ToolError::Refused(m)) if m.contains("is 33 min 16 s long") && m.contains("at most 20 min")),
        "{:?}",
        r.err()
    );
}

fn sine_wav(path: &std::path::Path, hz: f32) {
    let n = 48_000 * 2;
    let samples: Vec<f32> = (0..n * 2)
        .map(|i| {
            0.3 * cantus_dsp::math::sin(2.0 * std::f32::consts::PI * hz * (i / 2) as f32 / 48_000.0)
        })
        .collect();
    let r = cantus_engine::Rendered {
        sample_rate: 48_000,
        channels: 2,
        samples,
        peak: 0.3,
        cut_notes: 0,
    };
    cantus_engine::render::write_wav(&r, path).unwrap();
}

/// Review 2: the reference is the file opened when the call was made — another file put at
/// its path before the worker reads it is not what is heard (it was decoded by path, on
/// the worker).
#[test]
fn the_reference_heard_is_the_file_opened_at_the_call() {
    let mut host = TestHost::new("reference-swap", band());
    let dir = host.dir.clone().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("swap.wav");
    sine_wav(&path, 440.0);
    let ToolRun::Job(prepare) = tool("audition").run else {
        panic!()
    };
    let job = prepare(
        &mut ToolCx { host: &mut host },
        &json!({ "reference": "swap.wav", "wav": false }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    // Swapped before the worker runs: the 440 Hz file moved away, a 4 kHz one in its place.
    std::fs::rename(&path, dir.join("was.wav")).unwrap();
    sine_wav(&path, 4_000.0);
    let progress = |_: f32| {};
    let cancelled = || false;
    let JobDone::Answer(v) = (job.work)(&JobCx {
        progress: &progress,
        cancelled: &cancelled,
    })
    .unwrap_or_else(|e| panic!("{e}")) else {
        panic!()
    };
    let oct = &v["spectrum"]["octaves"];
    let at = |i: usize| oct[i]["reference_db"].as_f64().unwrap();
    assert!(
        at(4) > at(7) + 20.0,
        "the 440 Hz file was heard: 500 Hz {} against 4 kHz {}",
        at(4),
        at(7)
    );
}

/// Review 2: opening another project while a listen holds the checkpoints' lock neither
/// waits for it (the window would freeze for the render's length) nor lets it run on: the
/// switch is immediate, the listen ends cancelled, and the new project's stores are new.
#[test]
fn another_project_never_waits_for_a_running_listen() {
    let mut host = TestHost::new("epoch-busy", band());
    let ToolRun::Job(prepare) = tool("audition").run else {
        panic!()
    };
    let job = prepare(&mut ToolCx { host: &mut host }, &json!({ "wav": false }))
        .unwrap_or_else(|e| panic!("{e}"));
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let told = std::sync::atomic::AtomicBool::new(false);
        let progress = |_: f32| {
            if !told.swap(true, std::sync::atomic::Ordering::Relaxed) {
                // The lock is held from here on: slow the render so the switch meets it.
                let _ = held_tx.send(());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let cancelled = || false;
        (job.work)(&JobCx {
            progress: &progress,
            cancelled: &cancelled,
        })
        .map(|_| ())
    });
    held_rx.recv().unwrap();
    let old = host.service.checkpoints.clone();
    assert!(old.try_lock().is_err(), "the listen holds the lock");
    host.epoch += 1;
    let t0 = std::time::Instant::now();
    host.service.sync_epoch(host.epoch);
    assert!(
        t0.elapsed() < std::time::Duration::from_millis(200),
        "the switch waited {:?}",
        t0.elapsed()
    );
    assert!(
        !std::sync::Arc::ptr_eq(&old, &host.service.checkpoints),
        "fresh stores"
    );
    assert!(matches!(worker.join().unwrap(), Err(ToolError::Cancelled)));
}

/// Review 3: whatever the searches leave, the proposal is never over the ceiling — one
/// search only (forced), a master strip after the chain that adds a lot to the peaks
/// (width 2, +4 dB): the gain comes down until the true peak is under, and the shortfall is
/// said.
#[test]
fn the_proposal_is_never_over_the_ceiling() {
    let mut p = band();
    p.master.width = 2.0;
    p.master.gain_db = 4.0;
    // Wide parts, so the master's width has side to double.
    for (i, pan) in [(0usize, -0.9f32), (1, 0.9), (2, -0.5)] {
        let s = p.tracks[i].strip.unwrap();
        p.strip_mut(s).unwrap().pan = pan;
    }
    let mut host = TestHost::new("ceiling-once", band());
    // The mix as the assistant hears it, rendered as the panel's worker renders it.
    let prep = super::listening::prepare(
        &mut host,
        &super::run::assist_project(&p, true),
        super::listening::Range {
            start: Ticks::ZERO,
            end: None,
        },
        Vec::new(),
        true,
        0.0,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let progress = |_: f32| {};
    let cancelled = || false;
    let h = prep
        .run(&JobCx {
            progress: &progress,
            cancelled: &cancelled,
        })
        .unwrap_or_else(|e| panic!("{e}"));
    let target = super::mastering::Target::genre(
        super::targets::platform("spotify").unwrap(),
        &super::targets::CURVES[0],
    );
    let mut scratch = band();
    let prop = super::mastering::propose_passes(
        &h.audio,
        h.sample_rate,
        &target,
        &super::mastering::Post::of(&p.master),
        &mut |k| scratch.make_effect(k),
        &|| false,
        1,
    )
    .unwrap();
    assert!(
        prop.predicted.true_peak_dbtp <= -1.0,
        "{}",
        prop.predicted.true_peak_dbtp
    );
    // One search leaves this master over the ceiling (the strip adds after the limiter):
    // the gain came down, and the shortfall is said as it is.
    let lu = prop
        .short_of_target_lu
        .expect("the gain came down after the search ran out");
    let got = prop.predicted.integrated_lufs.unwrap();
    assert!((-14.0 - got - lu).abs() < 0.05, "said {lu}, got {got}");
}

/// Review 3: a `master_assist` queued across a project switch that no frame has synced yet
/// keeps its proposal in the new project's store — `apply_master_chain` finds it.
#[test]
fn a_proposal_made_across_a_switch_is_found() {
    let mut host = TestHost::new("switch-assist", band());
    host.service.sync_epoch(host.epoch);
    // Another project opened; the frame's sync has not run yet.
    host.bus = CommandBus::new(band());
    host.epoch += 1;
    let (prop, _) = job(&mut host, "master_assist", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let plan = edit_tool(
        &mut host,
        "apply_master_chain",
        json!({ "proposal_id": prop["proposal_id"] }),
    );
    assert!(plan.is_ok(), "the proposal was kept where apply looks");
}

/// Review 2: another project stops the A/B's preview (the panel silences it at its next
/// frame, where the engine is at hand).
#[test]
fn another_project_stops_the_ab() {
    let mut s = super::listening::Service::new(std::env::temp_dir().join("cantus-listen-ab"));
    s.sync_epoch(0);
    s.panel.ab = Some((super::panel::Side::Mix, std::time::Instant::now()));
    s.sync_epoch(1);
    assert!(s.panel.ab.is_none());
    assert!(
        s.panel.stop_preview,
        "the preview is stopped at the next frame"
    );
}

/// Review 2: the cap on kept audio is in frames — a whole 19-minute song (570 bars at
/// 120 BPM) may be mastered at 48 kHz, where 15 minutes refused it; at 192 kHz, holding four
/// times the frames, five minutes is the most.
#[test]
fn the_cap_on_kept_audio_is_in_frames() {
    let p = band();
    let r = |bars: i64| super::listening::Range {
        start: Ticks::ZERO,
        end: Some(p.tempo_map().bar_tick(bars + 1)),
    };
    assert!(super::listening::length_check(&p, r(570), true, 48_000).is_ok());
    let t = super::listening::length_check(&p, r(570), true, 192_000).unwrap_err();
    assert!((t.most_minutes - 5.0).abs() < 1e-9, "{t:?}");
    assert!(super::listening::length_check(&p, r(570), false, 192_000).is_ok());
}

/// Review 2: the reason the limiter gives quotes its own ceiling — lower than the
/// platform's when the master strip after it adds to the peaks (here +2 dB and width 1.6).
#[test]
fn the_limiters_reason_quotes_its_own_ceiling() {
    let mut p = band();
    p.master.width = 1.6;
    p.master.gain_db = 2.0;
    let mut host = TestHost::new("ceiling", p);
    let _ = job(&mut host, "master_assist", json!({ "platform": "spotify" }))
        .unwrap_or_else(|e| panic!("{e}"));
    let list = host.service.proposals.lock().unwrap();
    let prop = &list.kept[0].proposal;
    let lim = prop.chain.last().unwrap();
    assert_eq!(lim.spec.kind, EffectKind::Limiter);
    let ceiling = lim.spec.get("ceiling");
    assert!(ceiling < -1.0, "the strip's +2 dB lowered it: {ceiling}");
    let why = prop
        .why
        .iter()
        .find(|w| w.key == "listen.why.limit")
        .unwrap();
    let quoted = why
        .args
        .iter()
        .find(|(k, _)| *k == "ceiling")
        .unwrap()
        .1
        .clone();
    assert_eq!(quoted, format!("{ceiling:.1}"));
}

/// Review 3: one effect is "the master's effect", not "1 effects"; one track "1 track".
#[test]
fn counts_of_one_read_in_the_singular() {
    crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    assert_eq!(
        crate::i18n::tf_count("listen.panel.apply_replace", 1, &[]),
        "Apply: replace the master's effect"
    );
    assert_eq!(
        crate::i18n::tf_count("listen.panel.apply_replace", 3, &[]),
        "Apply: replace the master's 3 effects"
    );
    crate::i18n::set_thread_locale(crate::i18n::Locale::Es);
    assert_eq!(
        crate::i18n::tf_count("listen.panel.apply_replace", 1, &[]),
        "Aplicar: reemplazar el efecto del máster"
    );
    crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    let lead = band().tracks[0].id;
    let mut host = TestHost::new("singular", band());
    let ToolRun::Job(prepare) = tool("set_mix_targets").run else {
        panic!()
    };
    let job = prepare(
        &mut ToolCx { host: &mut host },
        &json!({ "targets": [{ "track_id": lead.0, "lufs": -20.0 }] }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let count = job.said.args.iter().find(|(k, _)| *k == "count").unwrap();
    assert!(
        matches!(&count.1, cantus_ai::SaidArg::Said(s) if s.key == "listen.phrase.tracks.one"),
        "{:?}",
        count.1
    );
}

/// Review 3: the panel says a song too long for it in the interface's words, its length in
/// minutes and seconds (never rounded to look within the limit), and what to do: set a
/// loop — or, with the loop chosen, shorten it.
#[test]
fn the_panel_says_a_song_is_too_long_and_what_to_do() {
    crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    let mut p = band();
    // A clip to bar 611 at 120 BPM (2 s a bar): the song is 20 min 20 s — over the 20
    // minutes the panel keeps, by less than a minute rounding would hide.
    let c = p.make_clip("late", Ticks::ZERO, Ticks::beats(610 * 4));
    p.tracks[0].clips.push(c);
    // A loop of 609 bars: 20 min 18 s.
    p.loop_region = Some(cantus_core::LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(609 * 4),
    });
    let mut app =
        crate::app::CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    super::panel::open(&mut app);
    app.listen.panel.loop_only = false;
    super::panel::start(&mut app);
    assert!(app.listen.panel.running.is_none(), "nothing rendered");
    let e = app.listen.panel.error.clone().expect("refused");
    assert!(
        e.contains("The song is 20 min 20 s long") && e.contains("20 min at most"),
        "{e}"
    );
    assert!(e.contains("Set a loop"), "{e}");
    app.listen.panel.error = None;
    app.listen.panel.loop_only = true;
    super::panel::start(&mut app);
    assert!(app.listen.panel.running.is_none(), "nothing rendered");
    let e = app.listen.panel.error.clone().expect("refused");
    assert!(
        e.contains("The loop is 20 min 18 s long") && e.contains("shorter"),
        "{e}"
    );
    assert_eq!(super::listening::min_sec(20.0), "20 min");
    assert_eq!(super::listening::min_sec(0.5), "30 s");
}

/// Integration review: with checkpoints kept the window is woken when they turn quiet —
/// the time left, nothing once past it — and with none it is not woken at all.
#[test]
fn the_window_wakes_when_the_checkpoints_turn_quiet() {
    use std::time::Duration;
    let mut host = TestHost::new("wake", band());
    let now = std::time::Instant::now();
    assert_eq!(host.service.quiet_wake(now), None, "nothing kept");
    let _ = job(&mut host, "audition", json!({ "wav": false })).unwrap_or_else(|e| panic!("{e}"));
    let now = std::time::Instant::now();
    let left = host.service.quiet_wake(now).expect("kept: woken");
    let quiet = super::listening::QUIET;
    assert!(
        left <= quiet && left > quiet - Duration::from_secs(30),
        "{left:?}"
    );
    let later = now + Duration::from_secs(4 * 60);
    let left = host.service.quiet_wake(later).unwrap();
    assert!(left <= quiet - Duration::from_secs(4 * 60), "{left:?}");
    assert_eq!(
        host.service.quiet_wake(now + quiet * 2),
        Some(Duration::ZERO)
    );
    host.service.release_if_quiet(now + quiet * 2);
    assert_eq!(
        host.service.quiet_wake(now + quiet * 2),
        None,
        "let go: no more wakes"
    );
}
