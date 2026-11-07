//! Bounce in place of a split kit's track (round 37, ADR 0100): it bounces the whole kit —
//! every pad summed as the unsplit kit sums it, to the bit — into the kit's strip, and says
//! that the pads' own strips are not in it.
mod support;
use cantus_core::*;
use cantus_engine::{render, RenderOptions};
use cantus_ui::bounce::{Plan, Range, Through};
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};

fn kit_song() -> Project {
    let mut p = cantus_ui::starter_project();
    p.tracks.clear();
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let pitches: Vec<u8> = t
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.pitch)
        .collect();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        for k in 0..1 + (step % 3) {
            let pitch = pitches[((step * 5 + k * 3) as usize) % pitches.len()];
            c.notes
                .push(p.make_note(at, Ticks(PPQ / 3), pitch, 50 + (step * 13 % 70) as u8));
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn app(name: &str, p: Project) -> CantusApp {
    let dir = std::env::temp_dir()
        .join("cantus-padout-bounce")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = support::app(p);
    app.engine.frozen.set_app_dir(dir.join("renders"));
    app.path = Some(dir.join("song.cantus"));
    app
}

fn settle(app: &mut CantusApp, ctx: &egui::Context) {
    let t0 = Instant::now();
    loop {
        app.sync_engine();
        cantus_ui::freeze::tick(app, ctx);
        if !app.freeze.busy() || t0.elapsed() > Duration::from_secs(120) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cantus_ui::freeze::tick(app, ctx);
}

fn mix(app: &mut CantusApp, p: &Project) -> Vec<f32> {
    app.sync_engine();
    let snap = app.engine.compile_for_render(p, 1);
    render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert!(a.len() >= b.len(), "{what}: lengths");
    if let Some(i) = a[..b.len()]
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {} vs {}", a[i], b[i]);
    }
    assert!(b.iter().any(|x| *x != 0.0), "{what}: silent");
}

#[test]
fn a_split_kits_track_bounces_as_the_whole_kit_to_the_bit() {
    let ctx = support::ctx();
    let song = kit_song();
    let mut app = app("source", song);
    let t = app.project().tracks[0].id;
    let unsplit = app.project().clone();
    let whole = mix(&mut app, &unsplit);
    assert!(app.split_kit(t));
    let split = app.project().clone();
    let tracks = split.tracks.len();
    app.bounce(Plan {
        track: t,
        through: Through::Source,
        range: Range::Track,
    });
    // Said as it starts (the render's own notice follows when it lands).
    let mut said = String::new();
    for _ in 0..200 {
        app.sync_engine();
        cantus_ui::freeze::tick(&mut app, &ctx);
        if app.status.text.contains("whole kit") {
            said = app.status.text.clone();
            break;
        }
    }
    assert!(
        said.contains("whole kit"),
        "the notice: {}",
        app.status.text
    );
    settle(&mut app, &ctx);
    let p = app.project().clone();
    assert_eq!(p.tracks.len(), tracks + 1, "a new track");
    assert!(p.tracks[0].muted, "the original muted");
    assert_eq!(
        p.tracks[1].strip, split.tracks[0].strip,
        "into the kit's strip"
    );
    // The song as it sounded: the bounce holds every pad, summed as the kit sums it.
    same_bits(&mix(&mut app, &p), &whole, "the bounced kit");
    // A pad strip's fader is not in the bounce (as the notice says): the bounce is the
    // kit as it sounds unsplit.
    app.undo();
    let s = app.project().tracks[0].instrument.output_strips()[0];
    assert!(app.exec(Command::SetStripGain {
        id: s,
        gain_db: -20.0
    }));
    app.bounce(Plan {
        track: t,
        through: Through::Source,
        range: Range::Track,
    });
    settle(&mut app, &ctx);
    let mut bounced_only = app.project().clone();
    bounced_only.tracks.remove(0);
    same_bits(&mix(&mut app, &bounced_only), &whole, "the bounce alone");
}

/// A kit whose pad plays through a chain (review 2 #3): the bounce of its track keeps the
/// chain — it is the instrument's sound — split or not, to the bit; its new track plays
/// at the kit's level, not doubled.
#[test]
fn a_kit_with_a_pads_chain_bounces_with_the_chain_to_the_bit() {
    let ctx = support::ctx();
    let mut app = app("chain", kit_song());
    let t = app.project().tracks[0].id;
    let pad = app.project().tracks[0].instrument.pulsus().unwrap().pads[1].id;
    assert!(app.add_to_pad_chain(t, pad, EffectKind::Saturator));
    let chained = app.project().clone();
    let whole = mix(&mut app, &chained);
    // The chain is heard: without it the mix is another.
    let mut plain = chained.clone();
    for p in &mut plain.tracks[0].instrument.pulsus_mut().unwrap().pads {
        p.output = None;
    }
    assert_ne!(mix(&mut app, &plain), whole, "the chain changes the snare");
    for split in [false, true] {
        if split {
            assert!(app.split_kit(t));
        }
        app.bounce(Plan {
            track: t,
            through: Through::Source,
            range: Range::Track,
        });
        settle(&mut app, &ctx);
        let p = app.project().clone();
        assert!(p.tracks[0].muted, "split {split}: the original muted");
        same_bits(
            &mix(&mut app, &p),
            &whole,
            &format!("split {split}: the bounced kit with its chain"),
        );
        app.undo();
    }
}
