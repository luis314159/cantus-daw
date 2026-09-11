//! Render each Pulsus drum and the welcome groove to WAV files, for listening.
//!
//! `cargo run -p cantus-ui --example render_kit -- <out_dir>`
//!
//! The null tests say the kit is deterministic and block-size independent; they cannot
//! say whether it sounds good. This is how that question gets answered — by ear, with the
//! files in front of whoever is judging — and it goes through the same offline render as
//! *Export WAV*, so what is written is exactly what the app plays.

use cantus_core::{DrumPad, InstrumentSpec, Project, PulsusParams, Snapshot, Ticks};
use cantus_engine::{render_to_wav, RenderOptions};
use std::path::Path;
use std::sync::Arc;

fn one_shot(name: &str, pad: DrumPad, velocities: &[u8]) -> Project {
    let mut p = Project::new(name);
    let pitch = pad.pitch;
    let mut t = p.make_track(name);
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::single(pad));
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(velocities.len() as i64));
    for (i, v) in velocities.iter().enumerate() {
        clip.notes
            .push(p.make_note(Ticks::beats(i as i64), Ticks::SIXTEENTH, pitch, *v));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn hats() -> Project {
    let mut p = Project::new("hats");
    let mut t = p.make_track("Hi-hat");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::of(vec![
        DrumPad::closed_hat(),
        DrumPad::open_hat(),
    ]));
    let mut clip = p.make_clip("hats", Ticks::ZERO, Ticks::beats(4));
    // Eighths on the closed hat, an open hat on the last eighth of each bar, choked by the
    // closed one that follows.
    for i in 0..8i64 {
        let pitch = if i == 7 { 46 } else { 42 };
        let vel = if i % 2 == 0 { 100 } else { 72 };
        clip.notes
            .push(p.make_note(Ticks::EIGHTH * i, Ticks::SIXTEENTH, pitch, vel));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "kit_render".to_string());
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("create output dir");
    let opts = RenderOptions {
        tail_seconds: 1.0,
        ..Default::default()
    };
    let write = |name: &str, project: Project| {
        let snap = Arc::new(Snapshot::compile(&project, 1));
        let path = out.join(format!("{name}.wav"));
        let r = render_to_wav(snap, &opts, &path).expect("render");
        println!(
            "{:<14} {:>5.2} s  peak {:>6.2} dBFS  -> {}",
            name,
            r.duration_seconds(),
            20.0 * r.peak.max(1e-9).log10(),
            path.display()
        );
    };
    write("kick", one_shot("Kick", DrumPad::kick(), &[127, 100, 72]));
    write(
        "snare",
        one_shot("Snare", DrumPad::snare(), &[127, 100, 72]),
    );
    write("rim", one_shot("Rim", DrumPad::rim(), &[120, 90]));
    write(
        "low_tom",
        one_shot("Low tom", DrumPad::low_tom(), &[120, 90]),
    );
    write(
        "high_tom",
        one_shot("High tom", DrumPad::high_tom(), &[120, 90]),
    );
    write(
        "closed_hat",
        one_shot("Closed hat", DrumPad::closed_hat(), &[110, 80, 60]),
    );
    write(
        "open_hat",
        one_shot("Open hat", DrumPad::open_hat(), &[110, 80]),
    );
    write("hats_choke", hats());
    let mut groove = cantus_ui::welcome_project();
    // Two passes of the four bars, so the loop point is heard.
    for t in &mut groove.tracks {
        for c in &mut t.clips {
            c.length = Ticks::beats(32);
        }
    }
    write("welcome_groove", groove);
}
