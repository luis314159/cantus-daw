//! Real, free, open-source instruments hosted by Cantus (ADR 0028) — Surge XT, Dexed,
//! OB-Xf, Six Sines, Firefly Synth 2, JC303, RipplerX, sfizz… whatever CLAP and VST3
//! instruments sit in the folder `CANTUS_PLUGINS` names. Ignored by default (the plug-ins
//! are not in the repository):
//!
//! ```text
//! set CANTUS_PLUGINS=D:\cantus-plugins   (what to download: docs/instruments/TEST-PLUGINS.md)
//! cargo test -p cantus-host --test real_instruments -- --ignored --nocapture --test-threads 1
//! ```
//!
//! Each instrument is scanned in the separate scanner process, then loaded twice:
//! - **through the adapter** (`HostedInstrument`, as the engine runs it): a chord at
//!   120 BPM with the transport playing, the mod wheel and a pitch bend, at blocks of 512
//!   — the output must be finite and not silent; the chord is rendered again at blocks of
//!   97 and the two are compared (reported, not asserted: a plug-in may run its own clock);
//!   its state is saved, loaded into a fresh instance and every value compared;
//! - **through the whole host** (`PluginHost`, the engine, the command bus): put on a
//!   track as the picker does, the riff played, the project saved with its state and opened
//!   again in a fresh host.
//!
//! A table is printed and written to `target/real-instruments.md`; the test fails at the
//! end if any instrument was silent, not finite, or lost its values in the round trip.

mod support;
use cantus_core::*;
use cantus_engine::inst::{Controller, InstrumentUnit, NoteOn};
use cantus_engine::{SongPosition, MAX_BLOCK_FRAMES};
use cantus_host::{
    backend_for, AudioSetup, HostedInstrument, MainThread, PluginHost, PluginInfo, PluginKind,
};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::*;

const RATE: f64 = 48_000.0;

#[derive(Default, Debug)]
struct Row {
    name: String,
    vendor: String,
    format: &'static str,
    load_ms: u128,
    params: usize,
    latency: usize,
    peak_db: f32,
    finite: bool,
    /// A fresh instance at the same block size gives the same samples (the plug-in is
    /// deterministic).
    repeat_same: Option<bool>,
    blocks_same: Option<bool>,
    /// The values that did not come back, by name (at most three).
    lost: Vec<String>,
    state_bytes: usize,
    state_values: Option<(usize, usize)>,
    state_identical: bool,
    host_peak_db: Option<f32>,
    reopened: Option<bool>,
    editor: bool,
    error: Option<String>,
}

fn db(x: f32) -> f32 {
    if x > 0.0 {
        20.0 * x.log10()
    } else {
        -200.0
    }
}

fn position(playing: bool, frame: usize) -> SongPosition {
    let beats = frame as f64 / RATE * 2.0;
    SongPosition {
        playing,
        tempo_bpm: 120.0,
        beats,
        samples: frame as i64,
        tempo_inc: 0.0,
        seconds: frame as f64 / RATE,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        bar_number: (beats / 4.0).floor() as i32,
        loop_seconds: None,
        time_sig_num: 4,
        time_sig_den: 4,
        loop_beats: None,
        wrap: None,
    }
}

/// The chord through the adapter at `block` frames per call: 1 s held, 1 s of release.
fn chord(unit: &mut HostedInstrument, block: usize) -> Vec<f32> {
    let total = (RATE * 2.0) as usize;
    let mut out = Vec::with_capacity(total * 2);
    let mut l = vec![0f32; block];
    let mut r = vec![0f32; block];
    let mut done = 0;
    while done < total {
        let n = block.min(total - done);
        if done == 0 {
            for (i, key) in [60u8, 64, 67, 72].into_iter().enumerate() {
                unit.note_on(NoteOn {
                    key,
                    velocity: 0.8,
                    id: 100 + i as u64,
                    channel: 0,
                });
            }
        }
        if done == (RATE * 0.5) as usize / block * block {
            unit.controller(Controller::ModWheel, 0.5);
            unit.controller(Controller::PitchBend, 1.0);
        }
        if done == (RATE as usize) / block * block {
            unit.controller(Controller::PitchBend, 0.0);
            for (i, key) in [60u8, 64, 67, 72].into_iter().enumerate() {
                unit.note_off(key, 100 + i as u64, 0.5);
            }
        }
        unit.set_position(&position(true, done));
        l[..n].fill(0.0);
        r[..n].fill(0.0);
        unit.process(&mut l[..n], &mut r[..n]);
        for i in 0..n {
            out.push(l[i]);
            out.push(r[i]);
        }
        done += n;
    }
    unit.all_notes_off();
    out
}

fn adapter(
    info: &PluginInfo,
    setup: AudioSetup,
) -> Result<(HostedInstrument, Box<dyn MainThread>), String> {
    let backend = backend_for(info.format).ok_or("format not hosted")?;
    let loaded = backend.load(info, setup).map_err(|e| e.to_string())?;
    let main = loaded.main;
    let params = main.params();
    let unit = HostedInstrument::new(
        loaded.audio,
        &params,
        |id| main.param_value(id),
        main.latency(),
        setup.sample_rate,
        Arc::new(AtomicU64::new(0)),
    );
    Ok((unit, main))
}

fn check(info: &PluginInfo, scanned: &cantus_host::PluginDb) -> Row {
    let mut row = Row {
        name: info.name.clone(),
        vendor: info.vendor.clone(),
        format: match info.format {
            cantus_host::Format::Clap => "CLAP",
            cantus_host::Format::Vst3 => "VST3",
        },
        ..Row::default()
    };
    let setup = AudioSetup {
        sample_rate: RATE,
        max_block: MAX_BLOCK_FRAMES,
    };
    // ---- through the adapter --------------------------------------------------------
    let started = Instant::now();
    let (mut unit, mut main) = match adapter(info, setup) {
        Ok(x) => x,
        Err(e) => {
            row.error = Some(format!("load: {e}"));
            return row;
        }
    };
    row.load_ms = started.elapsed().as_millis();
    row.params = main.params().len();
    row.latency = main.latency();
    row.editor = main.has_editor();
    let out = chord(&mut unit, 512);
    row.finite = out.iter().all(|x| x.is_finite());
    row.peak_db = db(peak(&out));
    unit.stop();
    drop(unit);
    eprintln!("  [adapter done; fresh instances next]");
    // Again from a fresh instance: the same blocks, then other blocks.
    if let Ok((mut again, _m)) = adapter(info, setup) {
        let same = chord(&mut again, 512);
        row.repeat_same = Some(differing(&out, &same) == 0);
        again.stop();
    }
    if let Ok((mut again, _m)) = adapter(info, setup) {
        let other = chord(&mut again, 97);
        row.blocks_same = Some(differing(&out, &other) == 0);
        again.stop();
    }
    // State: saved, loaded into a fresh instance, every value compared — after a frame's
    // idle, as the application runs one, so a VST3 controller has heard what its processor
    // was sent (the mod wheel reaches sfizz as its CC 1 parameter).
    let _ = main.idle();
    match main.save_state() {
        Ok(bytes) => {
            row.state_bytes = bytes.len();
            if let Ok(fresh) = backend_for(info.format).unwrap().load(info, setup) {
                let mut fresh = fresh.main;
                match fresh.load_state(&bytes) {
                    Ok(()) => {
                        let params = main.params();
                        let writable: Vec<_> = params.iter().filter(|p| !p.read_only).collect();
                        let back = |p: &&&cantus_host::ParamInfo| {
                            let a = main.param_value(p.id);
                            let b = fresh.param_value(p.id);
                            match (a, b) {
                                (Some(a), Some(b)) => {
                                    (a - b).abs() <= 1e-6 * (p.max - p.min).abs().max(1.0)
                                }
                                (None, None) => true,
                                _ => false,
                            }
                        };
                        let same = writable.iter().filter(back).count();
                        row.lost = writable
                            .iter()
                            .filter(|p| !back(p))
                            .take(3)
                            .map(|p| {
                                format!(
                                    "{} ({:?} to {:?})",
                                    p.name,
                                    main.param_value(p.id),
                                    fresh.param_value(p.id)
                                )
                            })
                            .collect();
                        row.state_values = Some((same, writable.len()));
                        row.state_identical =
                            fresh.save_state().ok().as_deref() == Some(&bytes[..]);
                    }
                    Err(e) => row.error = Some(format!("load_state: {e}")),
                }
            }
        }
        Err(e) => row.error = Some(format!("save_state: {e}")),
    }
    drop(main);
    cantus_host::clap::collect_garbage();
    eprintln!("  [state done; the whole host next]");
    // ---- through the whole host ---------------------------------------------------------
    let mut host = PluginHost::new(None, RATE);
    host.db = scanned.clone();
    let project = demo_project();
    let track = project.tracks[0].id;
    let mut rig = Rig::new(project, host);
    match rig.host.make_instrument(track, info) {
        Ok(spec) => {
            rig.exec(Command::SetInstrument {
                track,
                spec: Box::new(spec),
            });
            rig.settle();
            let played = rig.play(96_000, 256);
            row.host_peak_db = Some(db(peak(&played)));
            let saved = rig.host.project_for_save(rig.bus.project());
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("real.cantus");
            if cantus_project::save(&saved, &path, "test").is_ok() {
                if let Ok(opened) = cantus_project::load(&path, true) {
                    let mut host = PluginHost::new(None, RATE);
                    host.db = rig.host.db.clone();
                    let mut again = Rig::new(opened.project, host);
                    again.settle();
                    let loaded = again.host.hosted_instrument(track).is_some();
                    let sound = peak(&again.play(96_000, 256)) > 1e-5;
                    row.reopened = Some(loaded && sound);
                }
            }
        }
        Err(e) => row.error = Some(format!("host: {e}")),
    }
    row
}

#[test]
#[ignore = "needs real plug-ins: set CANTUS_PLUGINS to a folder of them"]
fn real_free_instruments_play_and_keep_their_state() {
    let Some(folder) = std::env::var_os("CANTUS_PLUGINS").map(PathBuf::from) else {
        eprintln!("CANTUS_PLUGINS is not set: nothing to test");
        return;
    };
    let mut host = PluginHost::new(None, RATE);
    host.scanner = scanner(Duration::from_secs(120));
    host.db.folders.push(folder.clone());
    let report = host.scan_now(false);
    eprintln!("scan: {report:?}");
    let only = std::env::var("CANTUS_PLUGIN_FILTER")
        .ok()
        .map(|f| f.to_lowercase());
    let mut infos: Vec<PluginInfo> = host
        .db
        .plugins()
        .into_iter()
        .filter(|p| p.kind == PluginKind::Instrument)
        .filter(|p| {
            only.as_ref()
                .is_none_or(|f| p.name.to_lowercase().contains(f))
        })
        .cloned()
        .collect();
    infos.sort_by(|a, b| {
        (a.name.to_lowercase(), a.format as u8).cmp(&(b.name.to_lowercase(), b.format as u8))
    });
    assert!(
        !infos.is_empty(),
        "no instruments found in {}",
        folder.display()
    );
    let mut table = String::from(
        "| Instrument | Maker | Format | Load ms | Params | Latency | Chord peak dBFS | Finite | Repeatable | Blocks 512 = 97 | State bytes | Values back | Same bytes | In Cantus dBFS | Reopened | Editor | Notes |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut failures = Vec::new();
    for info in &infos {
        eprintln!(
            "--- {} ({:?}) {}",
            info.name,
            info.format,
            info.path.display()
        );
        let r = check(info, &host.db);
        eprintln!("{r:?}");
        let yes = |b: bool| if b { "yes" } else { "no" };
        let _ = writeln!(
            table,
            "| {} | {} | {} | {} | {} | {} | {:.1} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.name,
            r.vendor,
            r.format,
            r.load_ms,
            r.params,
            r.latency,
            r.peak_db,
            yes(r.finite),
            r.repeat_same.map_or("–", yes),
            r.blocks_same.map_or("–", yes),
            r.state_bytes,
            r.state_values.map_or("–".into(), |(a, b)| format!("{a}/{b}")),
            yes(r.state_identical),
            r.host_peak_db.map_or("–".into(), |p| format!("{p:.1}")),
            r.reopened.map_or("–", yes),
            yes(r.editor),
            r.error.clone().unwrap_or_else(|| r.lost.join("; ")),
        );
        let silent = r.peak_db < -100.0;
        let lost = r.state_values.is_some_and(|(a, b)| a != b);
        if r.error.is_some() || silent || !r.finite || lost || r.reopened == Some(false) {
            failures.push(format!("{} ({})", r.name, r.format));
        }
    }
    eprintln!("\n{table}");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("real-instruments.md");
    let _ = std::fs::write(&out, &table);
    assert!(failures.is_empty(), "failed: {failures:?}");
}
