//! The demo songs, proved without ears (round 19): each one renders offline through the
//! real engine — not silent, nothing that is not a number, mastered near −14 LUFS with
//! the true peak under −1 dBTP — every track is heard, nothing lies outside the
//! song, every native instrument appears, and every song survives save and open.

use cantus_core::{InstrumentKind, InstrumentSpec, Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions, Rendered};
use cantus_ui::demos::{self, Demo};
use std::sync::Arc;

mod measure;
use measure::{db, lufs, lufs_of, rms, true_peak_db, true_peak_of_db};

fn render_range(p: &Project, start: Ticks, end: Option<Ticks>, tail: f64) -> Rendered {
    let snap = Snapshot::compile(p, 0);
    render(
        Arc::new(snap),
        &RenderOptions {
            start,
            end,
            tail_seconds: tail,
            ..Default::default()
        },
    )
    .expect("the demo renders")
}

fn solo(p: &Project, i: usize) -> Project {
    let mut q = p.clone();
    for (j, t) in q.tracks.iter_mut().enumerate() {
        t.soloed = j == i;
    }
    q
}

/// Where a track plays: from its first clip, at most `bars` bars.
fn window(p: &Project, i: usize, bars: i64) -> (Ticks, Ticks) {
    let t = &p.tracks[i];
    // From the first note, not the first clip: an intro's clip can open on rests.
    let start = t
        .clips
        .iter()
        .filter_map(|c| c.notes.iter().map(|n| n.start).min().map(|n| c.start + n))
        .min()
        .unwrap_or(Ticks::ZERO);
    let end = t.clips.iter().map(|c| c.end()).max().unwrap_or(Ticks::ZERO);
    let bar = p.time_signature.ticks_per_bar();
    (start, end.min(start + bar * bars))
}

/// Run `f` on every demo, the demos side by side on threads (a render is the slow part).
fn each_demo(f: impl Fn(&Demo, &Project) + Sync) {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let built: Vec<(&Demo, Project)> = demos::ALL.iter().map(|d| (d, (d.build)())).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = built
            .iter()
            .map(|(d, p)| {
                let f = &f;
                scope.spawn(move || f(d, p))
            })
            .collect();
        for h in handles {
            if let Err(e) = h.join() {
                std::panic::resume_unwind(e);
            }
        }
    });
}

/// Where each demo's whole band plays, in bars: the window its parts are measured in.
fn reference(id: &str) -> (i64, i64) {
    match id {
        "ballad" => (12, 20),
        "house" => (16, 24),
        "funk" => (12, 20),
        "lofi" => (20, 28),
        "ambient" => (16, 24),
        "trap" => (4, 12),
        "heart" => (32, 40),
        _ => (0, 8),
    }
}

/// The numbers to mix by: `cargo test -p cantus-ui --test demos -- --ignored --nocapture
/// levels`, with `DEMO=<id>` to pick one, `FULL=1` to render the whole song too and
/// `NOPARTS=1` to leave out the parts soloed.
#[test]
#[ignore]
fn levels() {
    let only = std::env::var("DEMO").ok();
    let full = std::env::var("FULL").is_ok();
    let parts = std::env::var("NOPARTS").is_err();
    each_demo(|d, p| {
        if only.as_deref().is_some_and(|o| o != d.id) {
            return;
        }
        use std::fmt::Write as _;
        let mut out = String::new();
        let bar = p.time_signature.ticks_per_bar();
        let (ra, rb) = reference(d.id);
        let t0 = std::time::Instant::now();
        let r = render_range(p, bar * ra, Some(bar * rb), 0.5);
        let _ = writeln!(
            out,
            "== {} ({:.0} s): bars {ra}..{rb}: peak {:.1} dBFS, {:.1} dBTP, {:.1} LUFS (rendered in {:.1} s)",
            d.id,
            demos::seconds(p),
            db(r.peak),
            true_peak_db(&r),
            lufs(&r),
            t0.elapsed().as_secs_f64()
        );
        if full {
            let t0 = std::time::Instant::now();
            let r = render_range(p, Ticks::ZERO, None, 2.0);
            let _ = writeln!(
                out,
                "   whole song: peak {:.1} dBFS, {:.1} dBTP, {:.1} LUFS (rendered in {:.1} s)",
                db(r.peak),
                true_peak_db(&r),
                lufs(&r),
                t0.elapsed().as_secs_f64()
            );
        }
        for i in (0..p.tracks.len()).filter(|_| parts) {
            let plays = p.tracks[i]
                .clips
                .iter()
                .any(|c| c.start < bar * rb && c.end() > bar * ra);
            let (a, b) = if plays {
                (bar * ra, bar * rb)
            } else {
                window(p, i, 8)
            };
            let t1 = std::time::Instant::now();
            let r = render_range(&solo(p, i), a, Some(b), 0.5);
            let _ = writeln!(
                out,
                "  {:<12} peak {:>6.1} dBFS  rms {:>6.1} dBFS  {:>6.1} LUFS  {} ({:.1} s)",
                p.tracks[i].name,
                db(r.peak),
                db(rms(&r)),
                lufs(&r),
                if plays { "ref " } else { "own " },
                t1.elapsed().as_secs_f64(),
            );
        }
        println!("{out}");
    });
}

/// Setting the master's drive by numbers: each song rendered up to the last insert of its
/// master chain (Culmen), then Culmen run offline at drives around the one set — the
/// integrated loudness and true peak of each. `cargo test --release -p cantus-ui --test
/// demos -- --ignored --nocapture master_drive` (`DEMO=<id>` for one).
#[test]
#[ignore]
fn master_drive() {
    let only = std::env::var("DEMO").ok();
    each_demo(|d, p| {
        if only.as_deref().is_some_and(|o| o != d.id) {
            return;
        }
        let mut q = p.clone();
        let Some(last) = q.master.effects.pop() else {
            return;
        };
        if last.spec.kind != cantus_core::EffectKind::Maximizer {
            println!("{}: the master chain does not end on Culmen", d.id);
            return;
        }
        let r = render_range(&q, Ticks::ZERO, None, 2.0);
        let (sr, n) = (r.sample_rate as f32, r.frames());
        let set = last.spec.get("gain");
        let mut out = format!(
            "== {}: into Culmen {:.1} LUFS, {:.1} dBTP; drive set {set:+.1} dB\n",
            d.id,
            lufs(&r),
            true_peak_db(&r)
        );
        for step in -6..=6 {
            let g = set + step as f32 * 0.5;
            let mut spec = last.spec.clone();
            spec.set("gain", g);
            let mut c = cantus_dsp::culmen::Culmen::new(sr);
            c.set(&cantus_engine::fx::maximizer::settings(&spec));
            c.reset();
            let mut l: Vec<f32> = (0..n).map(|i| r.samples[2 * i]).collect();
            let mut rr: Vec<f32> = (0..n).map(|i| r.samples[2 * i + 1]).collect();
            for (a, b) in l.chunks_mut(512).zip(rr.chunks_mut(512)) {
                c.process(a, b);
            }
            let y: Vec<f32> = l.iter().zip(&rr).flat_map(|(a, b)| [*a, *b]).collect();
            out += &format!(
                "   drive {g:+5.1} dB: {:6.2} LUFS  {:6.2} dBTP\n",
                lufs_of(&y, 2, sr),
                true_peak_of_db(&y, 2)
            );
        }
        println!("{out}");
    });
}

#[test]
fn every_demo_renders_clean_and_at_a_sane_loudness() {
    each_demo(|d, p| {
        let r = render_range(p, Ticks::ZERO, None, 2.0);
        assert!(
            r.samples.iter().all(|x| x.is_finite()),
            "{}: a sample that is not a number",
            d.id
        );
        assert!(r.peak < 1.0, "{} clips: peak {:.2} dBFS", d.id, db(r.peak));
        assert!(
            r.peak > 0.25,
            "{} is too quiet: peak {:.1} dBFS",
            d.id,
            db(r.peak)
        );
        // Mastered for streaming (round 19: the master chain plays): near −14 LUFS, the
        // true peak under −1 dBTP by the meters' detector (a tenth of a dB of slack).
        // Round 28's showcase is mastered as a current pop single is: −12…−10 LUFS, the
        // true peak at or under −1 dBTP, and it may run to three minutes.
        let (lo, hi, ceiling, longest) = if d.id == "heart" {
            (-12.0, -10.0, -1.0, 180.0)
        } else {
            (-16.0, -12.0, -0.9, 150.0)
        };
        let l = lufs(&r);
        assert!(
            (lo..=hi).contains(&l),
            "{}: integrated loudness {l:.1} LUFS is outside {lo}…{hi}",
            d.id
        );
        let tp = true_peak_db(&r);
        eprintln!("{}: {l:.1} LUFS, {tp:.2} dBTP", d.id);
        assert!(tp <= ceiling, "{}: true peak {tp:.2} dBTP", d.id);
        let secs = demos::seconds(p);
        assert!((55.0..=longest).contains(&secs), "{} lasts {secs:.0} s", d.id);
    });
}

#[test]
fn every_track_of_every_demo_is_heard() {
    each_demo(|d, p| {
        let bar = p.time_signature.ticks_per_bar();
        let (ra, _) = reference(d.id);
        for i in 0..p.tracks.len() {
            // Two bars from the first note at or after the reference section (or the
            // track's first note when it only plays before it).
            let mut starts: Vec<Ticks> = p.tracks[i]
                .clips
                .iter()
                .flat_map(|c| c.notes.iter().map(move |n| c.start + n.start))
                .collect();
            starts.sort();
            let from = starts
                .iter()
                .find(|t| **t >= bar * ra)
                .or(starts.first())
                .copied()
                .unwrap_or(Ticks::ZERO);
            let a = bar * (from.0 / bar.0);
            let b = a + bar * 2;
            let r = render_range(&solo(p, i), a, Some(b), 0.3);
            assert!(
                db(rms(&r)) > -50.0 && r.peak > 0.01,
                "{}: track {} is not heard (rms {:.1} dBFS)",
                d.id,
                p.tracks[i].name,
                db(rms(&r))
            );
        }
    });
}

#[test]
fn nothing_lies_outside_the_song() {
    each_demo(|d, p| {
        let end = p.content_end();
        let lp = p.loop_region.clone().expect("a demo has its loop region");
        assert!(
            lp.end <= end && lp.start >= Ticks::ZERO,
            "{}: the loop",
            d.id
        );
        for t in &p.tracks {
            assert!(!t.clips.is_empty(), "{}: {} has no clip", d.id, t.name);
            for c in &t.clips {
                assert!(
                    c.start >= Ticks::ZERO && c.end() <= end,
                    "{}: clip {}",
                    d.id,
                    c.name
                );
                // An automation clip (round 28's showcase has them) holds points, not notes.
                if let Some(a) = &c.automation {
                    assert!(
                        a.is_valid() && !a.points.is_empty(),
                        "{}: automation clip {} on {}",
                        d.id,
                        c.name,
                        t.name
                    );
                    assert!(
                        a.points.iter().all(|pt| pt.tick <= c.length),
                        "{}: a point of {} on {} lies past its clip",
                        d.id,
                        c.name,
                        t.name
                    );
                    continue;
                }
                assert!(
                    !c.notes.is_empty(),
                    "{}: clip {} on {} is empty",
                    d.id,
                    c.name,
                    t.name
                );
                for n in &c.notes {
                    assert!(
                        n.start >= Ticks::ZERO && n.end() <= c.length,
                        "{}: a note of {} on {} leaves its clip",
                        d.id,
                        c.name,
                        t.name
                    );
                }
            }
        }
    });
}

/// No two notes of a clip start together on one pitch: a doubled hit is a mistake in the
/// writing, never a sound.
#[test]
fn no_note_is_written_twice() {
    each_demo(|d, p| {
        for t in &p.tracks {
            for c in &t.clips {
                let mut seen = std::collections::HashSet::new();
                for n in &c.notes {
                    assert!(
                        seen.insert((n.start, n.pitch)),
                        "{}: {} doubles pitch {} at {:?} in {}",
                        d.id,
                        t.name,
                        n.pitch,
                        n.start,
                        c.name
                    );
                }
            }
        }
    });
}

#[test]
fn every_native_instrument_plays_in_some_demo() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut seen = Vec::new();
    let mut pulsus = false;
    for d in demos::ALL.iter() {
        let p = (d.build)();
        pulsus |= p.tracks.iter().any(|t| t.instrument.pulsus().is_some());
        for t in &p.tracks {
            if let InstrumentSpec::Native(n) = &t.instrument {
                seen.push(n.kind);
            }
        }
    }
    // A scaffold is not in the picker, nor in a demo (round 28's Chorda was, until it
    // played).
    for kind in InstrumentKind::ready_kinds() {
        assert!(seen.contains(&kind), "no demo plays {kind:?}");
    }
    assert!(pulsus, "no demo plays Pulsus II");
}

#[test]
fn every_demo_survives_save_and_open() {
    let dir = tempfile::tempdir().unwrap();
    each_demo(|d, p| {
        let path = dir.path().join(format!("{}.cantus", d.id));
        cantus_project::save(p, &path, "test").expect("saves");
        let back = cantus_project::load(&path, true).expect("opens").project;
        assert_eq!(&back, p, "{} changed through save and open", d.id);
    });
}

// ---- the picker and the startup project ---------------------------------------------

fn app(p: Project) -> cantus_ui::CantusApp {
    cantus_ui::CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false)
}

#[test]
fn the_picker_opens_a_demo_and_asks_before_losing_work() {
    use cantus_ui::views::demos as picker;
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut a = app(cantus_ui::welcome_project());
    a.run_action(cantus_ui::keymap::Action::DemoSongs);
    assert!(a.demos.open, "the action shows the picker");

    // Nothing unsaved: the demo replaces the project at once, as New does.
    picker::request(&mut a, 1);
    assert!(!a.demos.open);
    assert_eq!(a.project().name, cantus_ui::i18n::t(demos::ALL[1].name_key));
    assert!(a.path.is_none(), "a demo opens as a new, unsaved session");
    assert!(!a.bus.can_undo(), "opening a demo is not an edit to undo");

    // Unsaved work: the picker asks, and the project stays until it is answered.
    let id = a.project().tracks[0].id;
    a.exec(cantus_core::Command::SetTrackMute { id, muted: true });
    picker::show(&mut a);
    picker::request(&mut a, 0);
    assert!(a.demos.confirm, "unsaved work is asked about");
    assert_eq!(a.project().name, cantus_ui::i18n::t(demos::ALL[1].name_key));
    // "Don't save" asks again with the question answered.
    picker::request(&mut a, 0);
    assert_eq!(a.project().name, cantus_ui::i18n::t(demos::ALL[0].name_key));
    assert!(!a.demos.open && !a.demos.confirm);
}

#[test]
fn the_startup_project_is_a_full_session_that_loops() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let p = demos::startup_project();
    assert!(p.loop_enabled, "it loops out of the box");
    let lp = p.loop_region.clone().unwrap();
    let bars = (lp.end - lp.start).0 / p.time_signature.ticks_per_bar().0;
    assert!((8..=16).contains(&bars), "{bars} bars");
    assert!(
        p.tracks.len() >= 8,
        "a full session: {} tracks",
        p.tracks.len()
    );
    let kinds: std::collections::BTreeSet<_> = p
        .tracks
        .iter()
        .filter_map(|t| t.instrument.native().map(|n| n.kind))
        .collect();
    assert!(kinds.len() >= 4, "several native instruments: {kinds:?}");
    assert!(p.tracks.iter().any(|t| t.instrument.pulsus().is_some()));
    assert!(!p.master.effects.is_empty(), "the master chain");
    assert!(
        p.strips.iter().any(|s| !s.sends.is_empty()),
        "sends to the returns"
    );
    assert!(
        p.strips.iter().filter(|s| !s.effects.is_empty()).count() >= 6,
        "effects on the strips"
    );
    for t in &p.tracks {
        let s = p.strip(t.strip.unwrap()).unwrap();
        assert_eq!(s.name, t.name, "the strip is named after its track");
        assert!(s.color.is_some());
    }
    let r = render_range(&p, lp.start, Some(lp.end), 1.0);
    assert!(r.samples.iter().all(|x| x.is_finite()));
    assert!(r.peak < 1.0 && r.peak > 0.25, "peak {:.1} dBFS", db(r.peak));
    let l = lufs(&r);
    let tp = true_peak_db(&r);
    eprintln!("startup: {l:.1} LUFS, {tp:.2} dBTP");
    assert!((-16.0..=-11.0).contains(&l), "{l:.1} LUFS");
    assert!(tp <= -0.9, "{tp:.2} dBTP");
}

// ---- round 28's showcase: *With All Your Heart* -----------------------------------------

fn heart() -> Project {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    (demos::find("heart").expect("the showcase is in the list").build)()
}

/// The showcase plays what round 28 made: Chorda pedalled from the arrangement, every new
/// effect on a strip or the master where it does something, the modulators of four kinds,
/// automation on the new units, and Speculum at the end of the master chain.
#[test]
fn the_showcase_plays_round_28() {
    use cantus_core::{AutoTarget, EffectKind as K, ModKind};
    let p = heart();
    assert!(
        p.tracks
            .iter()
            .any(|t| t.instrument.native().is_some_and(|n| n.kind == InstrumentKind::Chorda)),
        "Chorda plays"
    );
    let slots: Vec<&cantus_core::EffectSlot> = p
        .strips
        .iter()
        .flat_map(|s| s.effects.iter())
        .chain(p.master.effects.iter())
        .collect();
    for kind in K::ALL[12..].iter() {
        assert!(
            slots.iter().any(|e| e.spec.kind == *kind),
            "no {kind:?} in the showcase"
        );
    }
    assert_eq!(
        p.master.effects.last().map(|e| e.spec.kind),
        Some(K::Analyzer),
        "Speculum ends the master chain"
    );
    let tonus = slots.iter().find(|e| e.spec.kind == K::AutoTune).unwrap();
    assert_eq!(
        (tonus.spec.get("key"), tonus.spec.get("scale"), tonus.spec.get("speed")),
        (5.0, 2.0, 0.0),
        "Tonus hard-tunes the topline to F minor"
    );
    let kinds: std::collections::BTreeSet<_> = p
        .all_modulators()
        .filter(|(_, m)| !m.routes.is_empty() && !m.muted)
        .map(|(_, m)| format!("{:?}", m.source.kind()))
        .collect();
    for k in [ModKind::Lfo, ModKind::Follower, ModKind::Step, ModKind::Random] {
        assert!(kinds.contains(&format!("{k:?}")), "no {k:?} modulator: {kinds:?}");
    }
    let lanes: Vec<&AutoTarget> = p
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter_map(|c| c.automation.as_ref().map(|a| &a.target))
        .collect();
    assert!(
        lanes
            .iter()
            .any(|t| matches!(t, AutoTarget::Instrument { key, .. } if key == "pedal")),
        "Chorda's pedal is played from a lane"
    );
    for kind in [K::Vibrato, K::FreqShift, K::VolumeShaper, K::RingMod] {
        assert!(
            lanes.iter().any(|t| match t {
                AutoTarget::Effect { effect, .. } =>
                    slots.iter().any(|e| e.id == *effect && e.spec.kind == kind),
                _ => false,
            }),
            "no lane on {kind:?}"
        );
    }
}

/// Played live — irregular blocks, the units sent as the backend sends them, delay
/// compensation as the engine asks for it — the riser, the filter build and the first
/// chorus (bars 28–36: automation, modulators, the follower, every insert) are the same
/// samples as the offline render, to the bit.
#[test]
fn the_showcase_renders_the_same_offline_and_in_real_time() {
    use cantus_engine::{pdc, EffectUnit, EngineEvent, EngineMessage, Processor};
    const SR: f32 = 48_000.0;
    cantus_engine::inst::chorda::ensure(Default::default(), SR);
    let p = heart();
    let bar = p.time_signature.ticks_per_bar();
    let (start, end) = (bar * 28, bar * 36);
    let offline = render_range(&p, start, Some(end), 0.0);
    let frames = offline.frames();

    let snap = Arc::new(Snapshot::compile(&p, 1));
    let (mut tx, rx) = rtrb::RingBuffer::<EngineMessage>::new(256);
    let (etx, mut erx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut proc_ = Processor::without_midi(SR as f64, rx, etx);
    for unit in cantus_engine::inst::units_for(&snap, SR, |_, _| false) {
        tx.push(EngineMessage::AddInstrument(unit)).unwrap();
        proc_.settle();
    }
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(slot, SR))))
            .unwrap();
        proc_.settle();
    }
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    proc_.settle();
    while let Ok(ev) = erx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(pdc::lines_for(count, delay)))
                .unwrap();
            proc_.settle();
        }
    }
    tx.push(EngineMessage::Seek(start)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let sizes = [128usize, 333, 512, 96, 1024, 17];
    let mut live = Vec::with_capacity(frames * 2);
    let mut i = 0;
    while live.len() / 2 < frames {
        let n = sizes[i % sizes.len()].min(frames - live.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc_.process(&mut buf, 2);
        live.extend_from_slice(&buf);
        while erx.pop().is_ok() {}
    }
    let first = offline
        .samples
        .iter()
        .zip(&live)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(
        first,
        None,
        "live differs from offline at frame {:?}",
        first.map(|i| i / 2)
    );
}
