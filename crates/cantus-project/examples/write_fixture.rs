//! Writes a fixture for the *current* schema version so future migrations are tested
//! against a real file: `cargo run -p cantus-project --example write_fixture`.
use cantus_core::*;
use std::path::Path;

fn main() {
    let mut p = Project::new("Fixture");
    p.tempo_bpm = 96.0;
    p.loop_enabled = true;
    let mut t = p.make_track("Keys");
    t.gain_db = -4.5;
    t.pan = 0.25;
    let InstrumentSpec::Firmus(f) = &mut t.instrument;
    f.cutoff_hz = 900.0;
    f.osc1_wave = Waveform::Square;
    let mut delay = p.make_effect(EffectKind::Delay);
    delay.spec.set("mix", 0.2).unwrap();
    t.effects.push(delay);
    let mut c = p.make_clip("Intro", Ticks::beats(4), Ticks::beats(8));
    for (i, pitch) in [60u8, 64, 67, 71].iter().enumerate() {
        c.notes.push(p.make_note(
            Ticks::beats(i as i64 * 2),
            Ticks::QUARTER,
            *pitch,
            80 + i as u8,
        ));
    }
    t.clips.push(c);
    // A second clip that exercises everything schema 4 added, so the 3→4 migration and
    // every future one are tested against a file that actually uses the fields.
    let mut looped = p.make_clip("Loop", Ticks::beats(16), Ticks::beats(16));
    looped.content_length = Some(Ticks::beats(4));
    looped.offset = Ticks::beats(2);
    looped.muted = true;
    looped.color = Some(Color::PALETTE[2]);
    looped
        .notes
        .push(p.make_note(Ticks::beats(2), Ticks::QUARTER, 55, 90));
    t.clips.push(looped);
    p.tracks.push(t);
    let mut bass = p.make_track("Bass");
    bass.muted = true;
    // Schema 5: the fixture uses width and polarity so migrations are tested against a
    // file that actually sets them.
    bass.width = 0.0;
    bass.phase_invert = true;
    p.master.pan = -0.25;
    p.master.width = 1.5;
    p.tracks.push(bass);
    let name = format!("v{}.cantus", cantus_project::SCHEMA_VERSION);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    cantus_project::save(&p, &path, "fixture").expect("save fixture");
    println!("wrote {}", path.display());
}
