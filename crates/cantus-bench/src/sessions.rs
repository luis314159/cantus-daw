//! The sessions the suite plays, built in code so every machine builds the same ones.
//!
//! - [`scaled`]: *the demo songs scaled up* — the house song's desk (drum bus, returns,
//!   mastering chain) with tracks from all six demos added until it has `n`, each with a
//!   copy of its own strip and inserts, every song's full-band window lined up on bars
//!   16–24 and looped. What a big production looks like to the engine.
//! - [`dsp`]: the DAWbench-style DSP session (`docs/perf/benchmark.md` §3): `n` tracks,
//!   each one sustained saw into `per_track` compressors that are working.
//! - [`vi`]: the DAWbench-style VI session (§4): `n` tracks of one instrument, each playing
//!   the same eight-voice chord pattern.
//! - [`instrument`]: one track of one instrument holding `voices` notes.

use cantus_core::{
    EffectKind, InstrumentKind, InstrumentSpec, LoopRegion, NativeSpec, Project, Ticks,
};
use cantus_ui::demos;

/// The engine's ceiling on tracks and strips (`cantus_engine::MAX_TRACKS`). A session
/// asked for more is built — the project allows it — but the engine plays only this many.
pub const ENGINE_TRACKS: usize = cantus_engine::MAX_TRACKS;

/// Where each demo's whole band plays, first bar (the demos' own test, `tests/demos.rs`).
fn full_band_bar(id: &str) -> i64 {
    match id {
        "ballad" | "funk" => 12,
        "lofi" => 20,
        "trap" => 4,
        _ => 16,
    }
}

/// The loop every scaled session plays: bars 16–24.
pub const LOOP_BARS: (i64, i64) = (16, 24);

/// A session of `n` tracks made of the demo songs (see the module's docs).
pub fn scaled(n: usize) -> Project {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut p = (demos::find("house").expect("the house demo").build)();
    let bar = p.time_signature.ticks_per_bar();
    let (from, to) = (bar * LOOP_BARS.0, bar * LOOP_BARS.1);
    p.loop_region = Some(LoopRegion {
        start: from,
        end: to,
    });
    p.loop_enabled = true;
    p.name = format!("Scaled demos, {n} tracks");
    if p.tracks.len() >= n {
        p.tracks.truncate(n);
        return p;
    }
    let sources: Vec<(&str, Project)> = ["ballad", "funk", "lofi", "ambient", "trap", "house"]
        .iter()
        .map(|id| (*id, (demos::find(id).expect("a demo").build)()))
        .collect();
    let mut k = 0usize;
    while p.tracks.len() < n {
        let (id, src) = &sources[k % sources.len()];
        let ti = (k / sources.len()) % src.tracks.len();
        k += 1;
        let shift = bar * (LOOP_BARS.0 - full_band_bar(id));
        add_copy(&mut p, src, ti, shift, (from, to));
    }
    p
}

/// Copy track `ti` of `src` into `p` with fresh ids: its clips moved by `shift` (only
/// those that sound inside `window`), and a copy of its strip with its inserts, routed to
/// the master (sends, keys and note sources point into the other song, so they go).
fn add_copy(p: &mut Project, src: &Project, ti: usize, shift: Ticks, window: (Ticks, Ticks)) {
    let st = &src.tracks[ti];
    let mut t = p.make_track(st.name.clone());
    t.color = st.color;
    t.icon = st.icon;
    t.instrument = st.instrument.clone();
    t.gain_db = st.gain_db;
    t.pan = st.pan;
    for c in &st.clips {
        let start = c.start + shift;
        if start >= window.1 || start + c.length <= window.0 {
            continue;
        }
        let mut clip = p.make_clip(c.name.clone(), start, c.length);
        clip.offset = c.offset;
        clip.content_length = c.content_length;
        clip.color = c.color;
        clip.audio = c.audio.clone();
        for n in &c.notes {
            let mut note = p.make_note(n.start, n.length, n.pitch, n.velocity);
            note.pitch = n.pitch;
            clip.notes.push(note);
        }
        t.clips.push(clip);
    }
    t.strip = None;
    if let Some(from) = st.strip.and_then(|id| src.strip(id)) {
        if p.strips.len() < ENGINE_TRACKS {
            let mut s = p.make_strip(from.name.clone());
            s.gain_db = from.gain_db;
            s.pan = from.pan;
            s.width = from.width;
            s.eq = from.eq.clone();
            s.color = from.color;
            for e in &from.effects {
                let mut slot = p.make_effect(e.spec.kind);
                slot.spec = e.spec.clone();
                slot.bypass = e.bypass;
                slot.mix = e.mix;
                s.effects.push(slot);
            }
            t.strip = Some(s.id);
            p.strips.push(s);
        }
    }
    p.tracks.push(t);
}

/// A clip on a new track playing `notes` (pitch, start beat, length in beats, velocity)
/// over `beats`, looped by the project.
fn clip_of(p: &mut Project, beats: i64, notes: &[(u8, f64, f64, u8)]) -> cantus_core::Clip {
    let mut clip = p.make_clip("Bench", Ticks::ZERO, Ticks::beats(beats));
    for &(pitch, at, len, vel) in notes {
        let q = Ticks::QUARTER.0 as f64;
        let n = p.make_note(Ticks((at * q) as i64), Ticks((len * q) as i64), pitch, vel);
        clip.notes.push(n);
    }
    clip
}

/// An empty project looping its first `beats`, with no strips of its own.
fn bare(name: &str, beats: i64) -> Project {
    let mut p = Project::empty(name);
    p.loop_region = Some(LoopRegion {
        start: Ticks::ZERO,
        end: Ticks::beats(beats),
    });
    p.loop_enabled = true;
    p
}

/// The DAWbench-style DSP session: `n` tracks, each a sustained saw (Firmus, one voice)
/// into `per_track` Comprimo compressors at −30 dB threshold, 4:1 — every one working.
pub fn dsp(n: usize, per_track: usize) -> Project {
    let mut p = bare(&format!("DSP bench, {n} × {per_track}"), 16);
    for i in 0..n {
        let mut t = p.make_track(format!("DSP {}", i + 1));
        let clip = clip_of(&mut p, 16, &[(36 + (i % 24) as u8, 0.0, 16.0, 100)]);
        t.clips.push(clip);
        let mut s = p.make_strip(format!("DSP {}", i + 1));
        for _ in 0..per_track {
            let mut c = p.make_effect(EffectKind::Compressor);
            c.spec.set("thr", -30.0);
            c.spec.set("ratio", 4.0);
            s.effects.push(c);
        }
        t.strip = Some(s.id);
        p.strips.push(s);
        p.tracks.push(t);
    }
    p
}

/// The DAWbench-style VI session: `n` tracks of `kind` at its defaults, each playing an
/// eight-note chord every half note for 16 beats (gated at 90 %, so the releases overlap
/// the next chord as a player's do).
pub fn vi(n: usize, kind: InstrumentKind) -> Project {
    let mut p = bare(&format!("VI bench, {n} × {kind:?}"), 16);
    const CHORDS: [[u8; 8]; 4] = [
        [36, 48, 55, 60, 64, 67, 72, 76],
        [41, 53, 57, 60, 65, 69, 72, 77],
        [43, 55, 59, 62, 67, 71, 74, 79],
        [33, 45, 52, 57, 60, 64, 69, 72],
    ];
    let mut notes = Vec::new();
    for step in 0..8 {
        for pitch in CHORDS[step / 2] {
            notes.push((pitch, step as f64 * 2.0, 1.8, 96));
        }
    }
    for i in 0..n {
        let mut t = p.make_track(format!("VI {}", i + 1));
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        let clip = clip_of(&mut p, 16, &notes);
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// One track of `instrument` holding `voices` notes for 16 beats (a drum kit: `voices`
/// hits on every sixteenth instead, spread over its pads).
pub fn instrument(instrument: InstrumentSpec, voices: usize) -> Project {
    let mut p = bare("Instrument bench", 16);
    let drums = matches!(instrument, InstrumentSpec::Pulsus(_));
    let mut t = p.make_track("Instrument");
    t.instrument = instrument;
    let notes: Vec<(u8, f64, f64, u8)> = if drums {
        (0..64)
            .flat_map(|step| {
                (0..voices).map(move |v| (36 + (v % 12) as u8, step as f64 * 0.25, 0.2, 100))
            })
            .collect()
    } else {
        (0..voices)
            .map(|v| (36 + ((v * 7) % 60) as u8, 0.0, 16.0, 96))
            .collect()
    };
    let clip = clip_of(&mut p, 16, &notes);
    t.clips.push(clip);
    p.tracks.push(t);
    p
}
