//! fxd step 3 — Tactus (the X-Y pad) and Calculus (the formula controller) heard end to end
//! (ADRs 0144, 0145), on the boot path: an app made by `CantusApp::assemble` with the real
//! engine handle (`EngineHandle::manual`: the processor behind it is the one a driver's
//! callback runs), every edit one command through the app as the windows issue it, and the
//! song rendered as an export renders it (`compile_for_render`) or played live by the app's
//! own processor.
//!
//! The song: a Firmus sine pair, A3 (220 Hz) and A6 (1760 Hz), struck for a quarter of a
//! beat on every beat at 120 BPM (each note over long before the next, so a render from any
//! beat plays what a full one plays there), on a strip with an EQ inserted as the picker
//! inserts it and two bells set on it: band 0 at 220 Hz and band 1 at 1760 Hz, Q 2, 0 dB.
//! A controller moving band 0's gain is heard as the low tone's level, band 1's as the
//! high tone's — three octaves apart, so each is measured alone (a Hann-windowed Goertzel
//! probe at the tone, 10…50 ms into a note, against the same window of a reference).
//!
//! - **a pad linked to two parameters, moved**: X heard on the low tone, Y on the high one,
//!   each alone and both together;
//! - **a recorded gesture**: a write pass (the app's recording) writes two lanes, X and Y,
//!   whose values at every recorded tick are the hand's; played back, a render from inside
//!   each held stretch equals to the bit a render with the hand held there, and the lanes'
//!   render is block-size independent and the live processor's;
//! - **a path in time**: the figure-eight preset's path read at the song's beats is what
//!   the tones' levels follow; a render from bar 3 is the full render's bars 3… to the bit
//!   from its second note on (its first: the EQ's own glide from its stored value, a
//!   pre-fxd engine behaviour measured here), and every sample once the routes drive the
//!   strip's pan and fader;
//! - **a formula in time**: a square a bar, live in odd blocks equal to the offline render,
//!   and from bar 5 the full render's bars 5… (as the path's);
//! - **an edit heard and undone to the bit** (the formula's text, a knob);
//! - **unlinked controllers, and routes at depth 0**: the dry master, to the bit.
//!
//! "To the bit" compares `f32::to_bits` of every interleaved sample.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::automation::compile::lane_for;
use cantus_core::modulator::{presets, ModHost, ModKind, ModSource, RouteLink, XyPath};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::f64::consts::TAU;
use std::path::Path;

const SR: usize = 48_000;
/// Seconds a beat at 120 BPM.
const BEAT_S: f64 = 0.5;
const LOW_HZ: f64 = 220.0;
const HIGH_HZ: f64 = 1_760.0;
/// The routes' depth: a full swing of an axis moves a band's gain by many decibels.
const DEPTH: f32 = 0.25;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn tick(beats: f64) -> Ticks {
    Ticks((beats * PPQ as f64).round() as i64)
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// The first sample index (interleaved) where `a` and `b` differ in their bits, and the
/// largest difference: for a failure's message.
fn first_diff(a: &[f32], b: &[f32]) -> String {
    let at = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits());
    let worst = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    let last = a
        .iter()
        .zip(b)
        .rposition(|(x, y)| x.to_bits() != y.to_bits());
    format!(
        "lengths {} / {}, differences from {at:?} to {last:?}, largest {worst:e}",
        a.len(),
        b.len()
    )
}

/// The energy of the left channel at `hz` over `from … to` seconds (a Hann-windowed
/// Goertzel probe).
fn tone(x: &[f32], from: f64, to: f64, hz: f64) -> f64 {
    let (i, j) = ((from * SR as f64) as usize, (to * SR as f64) as usize);
    let n = j - i;
    let c = 2.0 * (TAU * hz / SR as f64).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for k in 0..n {
        let w = 0.5 - 0.5 * (TAU * k as f64 / n as f64).cos();
        let s0 = f64::from(x[2 * (i + k)]) * w + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

/// The tone's level in the note struck at `beat` (10…50 ms into it), dB, from the start of
/// the render `x` (which began at `start` beats).
fn level_db(x: &[f32], start: f64, beat: f64, hz: f64) -> f64 {
    let t = (beat - start) * BEAT_S;
    10.0 * tone(x, t + 0.01, t + 0.05, hz).max(1e-30).log10()
}

/// How much louder the tone is in `x` than in `reference` in the note at `beat`, dB.
fn gain_db(x: &[f32], reference: &[f32], beat: f64, hz: f64) -> f64 {
    level_db(x, 0.0, beat, hz) - level_db(reference, 0.0, beat, hz)
}

fn rms_diff(a: &[f32], b: &[f32]) -> f64 {
    let s: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(x - y) * f64::from(x - y))
        .sum();
    (s / a.len().max(1) as f64).sqrt()
}

fn rms(a: &[f32]) -> f64 {
    rms_diff(a, &vec![0.0; a.len()])
}

/// The assembled app over the song, its processor, the strip and the EQ.
struct Song {
    app: CantusApp,
    proc_: Processor,
    eq: EffectId,
    /// A fresh merge key for each gesture.
    gestures: u64,
}

impl Song {
    /// The song for `beats` beats, the EQ inserted on its strip as the picker inserts it
    /// and its two bells set by commands.
    fn new(beats: i64) -> Song {
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        let (engine, proc_) = EngineHandle::manual(SR as f64);
        let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
        let mut p = Project::new("fxd e2e");
        p.tempo_bpm = 120.0;
        let mut lead = p.make_track("Lead");
        let f = lead.instrument.firmus_mut().expect("Firmus");
        f.osc1_wave = Waveform::Sine;
        f.osc2_wave = Waveform::Sine;
        f.osc_mix = 0.0;
        f.osc2_detune_cents = 0.0;
        // The cutoff and the level stay at Firmus's own defaults: its smoothers start there
        // when a render begins on a note (they glide 10-20 ms to any other setting, so a
        // render from a note would not be the full render's from it). The cutoff follows
        // the key instead (A3's at 1.85 kHz, A6's at 14.8 kHz): both tones pass.
        f.resonance = 0.0;
        f.filter_env_octaves = 0.0;
        f.key_track = 1.0;
        f.amp_attack_s = 0.0;
        f.amp_decay_s = 0.0;
        f.amp_sustain = 1.0;
        f.amp_release_s = 0.02;
        let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(beats));
        for b in 0..beats {
            for k in [57u8, 93] {
                let n = p.make_note(Ticks::beats(b), Ticks(PPQ / 4), k, 100);
                clip.notes.push(n);
            }
        }
        lead.clips.push(clip);
        p.tracks.push(lead);
        app.replace_project(p);
        let strip = app.project().tracks[0].strip.expect("a strip");
        app.add_effect(strip, EffectKind::Eq);
        let eq = app.project().strip(strip).unwrap().effects[0].id;
        for (band, hz) in [(0, LOW_HZ), (1, HIGH_HZ)] {
            for (key, value) in [
                ("used", 1.0),
                ("freq", hz as f32),
                ("q", 2.0),
                ("gain", 0.0),
            ] {
                let key = format!("b{band}.{key}");
                // A command each (none when the starting preset already holds the value).
                app.exec(Command::SetEffectParam {
                    strip,
                    effect: eq,
                    key: key.clone(),
                    value,
                });
                let spec = &app.project().effect(strip, eq).unwrap().spec;
                assert_eq!(spec.get(&key), value, "{key}");
            }
        }
        Song {
            app,
            proc_,
            eq,
            gestures: 0,
        }
    }

    /// Band `b`'s gain: what a route moves.
    fn band(&self, b: u8) -> AutoTarget {
        AutoTarget::Effect {
            effect: self.eq,
            key: format!("b{b}.gain"),
        }
    }

    /// The song from `from` to `to` beats as an export renders it, in blocks of `block`.
    fn render(&mut self, from: f64, to: f64, block: usize) -> Vec<f32> {
        let p = self.app.project().clone();
        let snap = self.app.engine.compile_for_render(&p, 1);
        let opts = RenderOptions {
            start: tick(from),
            end: Some(tick(to)),
            tail_seconds: 0.0,
            block_frames: block,
            ..Default::default()
        };
        render(snap, &opts).unwrap().samples
    }

    /// The whole of `beats` beats in the export's own blocks.
    fn whole(&mut self, beats: f64) -> Vec<f32> {
        self.render(0.0, beats, RenderOptions::default().block_frames)
    }

    /// What the app's own processor plays from the top for `frames` frames, in blocks that
    /// divide nothing, as a driver's callback asks for them. Once a test:
    /// the processor starts fresh, as the offline render does.
    fn live(&mut self, frames: usize) -> Vec<f32> {
        self.app.engine.set_heard_for_tests(Ticks::ZERO, false);
        self.app.seek(Ticks::ZERO);
        self.app.sync_engine();
        self.proc_.settle();
        self.app.toggle_play();
        let sizes = [256usize, 333, 512, 17, 1024, 1, 999];
        let mut out = Vec::with_capacity(frames * 2);
        let mut i = 0;
        while out.len() < frames * 2 {
            let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            self.proc_.process(&mut buf, 2);
            out.extend_from_slice(&buf);
        }
        self.app.toggle_play();
        out
    }

    /// A Tactus pad on the master, as the inspector adds it, X driving band 0 and Y band 1
    /// (the Y route's link set by the one command an axis change is).
    fn pad(&mut self) -> ModulatorId {
        let id = self
            .app
            .add_modulator(ModHost::Master, ModKind::XyPad, None)
            .expect("a pad on the master");
        let (low, high) = (self.band(0), self.band(1));
        self.app.set_route_depth(id, &low, DEPTH, false);
        self.app.set_route_depth(id, &high, DEPTH, false);
        self.link(id, &high, 1);
        id
    }

    /// Route `target` of `id` reads output `output` (an axis).
    fn link(&mut self, id: ModulatorId, target: &AutoTarget, output: u8) {
        let mut m = self.app.project().modulator(id).unwrap().2.clone();
        let links = m.source.links_mut().expect("a controller");
        links.retain(|l| &l.target != target);
        links.push(RouteLink {
            target: target.clone(),
            output,
            range: None,
        });
        assert!(self.app.edit_modulator(m, None), "the link is one command");
    }

    /// The pad's X and Y as the window's drag sets them: one `Batch` of two inputs under
    /// the gesture's key, then the hand lifted.
    fn move_pad(&mut self, id: ModulatorId, x: f32, y: f32) {
        self.gestures += 1;
        let key = MergeKey::of(&("fxd-e2e-drag", self.gestures));
        assert!(self.app.exec_merging(set_xy(id, x, y), key));
        self.app.end_gesture();
    }

    fn pad_xy(&self, id: ModulatorId) -> (f32, f32) {
        match &self.app.project().modulator(id).unwrap().2.source {
            ModSource::XyPad(x) => (x.x, x.y),
            _ => panic!("a pad"),
        }
    }
}

/// The command the pad's drag issues.
fn set_xy(id: ModulatorId, x: f32, y: f32) -> Command {
    Command::Batch {
        label_key: "cmd.fxd_set_input".into(),
        commands: vec![
            Command::SetModulatorInput {
                id,
                input: 0,
                value: x,
            },
            Command::SetModulatorInput {
                id,
                input: 1,
                value: y,
            },
        ],
    }
}

/// The low and high tones' gains in `x` against `reference`, averaged over the notes of
/// beats `from … to`, dB.
fn gains(x: &[f32], reference: &[f32], from: i64, to: i64) -> (f64, f64) {
    let n = (to - from) as f64;
    let (mut lo, mut hi) = (0.0, 0.0);
    for b in from..to {
        lo += gain_db(x, reference, b as f64, LOW_HZ);
        hi += gain_db(x, reference, b as f64, HIGH_HZ);
    }
    (lo / n, hi / n)
}

/// A pad whose X drives band 0's gain and Y band 1's, moved as the window's drag moves it
/// (one `Batch` of X and Y, one undo step a gesture): at the centre (bipolar, the default)
/// it is the dry song to the bit; X alone raises the low tone by many decibels and leaves
/// the high one within a tenth of one; Y alone the other way round; both together, both,
/// each as much as alone. Undone gesture by gesture, back to the dry song to the bit.
#[test]
fn a_pad_linked_to_two_effect_parameters_moved_is_heard_on_both() {
    let mut s = Song::new(8);
    let dry = s.whole(8.0);
    assert!(rms(&dry) > 1e-3, "the song sounds: {}", rms(&dry));
    let pad = s.pad();
    let centre = s.whole(8.0);
    assert!(
        same_bits(&dry, &centre),
        "a pad at its centre is the dry song: {}",
        first_diff(&dry, &centre)
    );
    let steps = s.app.bus.history().len();
    s.move_pad(pad, 0.9, 0.5);
    let x_only = s.whole(8.0);
    s.move_pad(pad, 0.5, 0.9);
    let y_only = s.whole(8.0);
    s.move_pad(pad, 0.9, 0.9);
    let both = s.whole(8.0);
    assert_eq!(s.app.bus.history().len(), steps + 3, "a gesture, a step");
    let (x_lo, x_hi) = gains(&x_only, &dry, 0, 8);
    let (y_lo, y_hi) = gains(&y_only, &dry, 0, 8);
    let (b_lo, b_hi) = gains(&both, &dry, 0, 8);
    eprintln!(
        "X only: low {x_lo:+.2} dB, high {x_hi:+.2} dB; Y only: low {y_lo:+.2}, high {y_hi:+.2}; \
         both: low {b_lo:+.2}, high {b_hi:+.2}; RMS of the differences: X {:.2e}, Y {:.2e}, both {:.2e}",
        rms_diff(&x_only, &dry),
        rms_diff(&y_only, &dry),
        rms_diff(&both, &dry)
    );
    assert!(x_lo > 6.0, "X is heard on band 0: {x_lo:+.2} dB");
    assert!(x_hi.abs() < 0.1, "X leaves band 1: {x_hi:+.2} dB");
    assert!(y_hi > 6.0, "Y is heard on band 1: {y_hi:+.2} dB");
    assert!(y_lo.abs() < 0.1, "Y leaves band 0: {y_lo:+.2} dB");
    assert!(
        (b_lo - x_lo).abs() < 0.1 && (b_hi - y_hi).abs() < 0.1,
        "both at once, each as much as alone"
    );
    // Undone gesture by gesture: the pad back where each began, the centre the dry song.
    s.app.undo();
    assert_eq!(s.pad_xy(pad), (0.5, 0.9));
    assert!(same_bits(&s.whole(8.0), &y_only), "one undo, Y only again");
    s.app.undo();
    s.app.undo();
    assert_eq!(s.pad_xy(pad), (0.5, 0.5));
    let back = s.whole(8.0);
    assert!(
        same_bits(&back, &dry),
        "undone, the dry song: {}",
        first_diff(&back, &dry)
    );
}

/// The automation clips on target `t`, wherever the pass put them.
fn lanes_on(app: &CantusApp, t: &AutoTarget) -> usize {
    app.project()
        .tracks
        .iter()
        .flat_map(|tr| tr.clips.iter())
        .filter(|c| c.automation.as_ref().is_some_and(|a| &a.target == t))
        .count()
}

/// The hand on the pad during the pass: from beat 2½ it holds A, from 4½ B, from 6½ C,
/// and lifts at 8½ (each change in the silence between two notes).
const HELD: [(f64, f64, f32, f32); 3] = [
    (2.5, 4.5, 0.9, 0.2),
    (4.5, 6.5, 0.1, 0.8),
    (6.5, 8.5, 0.7, 0.95),
];

fn hand(beat: f64) -> Option<(f32, f32)> {
    HELD.iter()
        .find(|h| (h.0..h.1).contains(&beat))
        .map(|h| (h.2, h.3))
}

/// A gesture recorded while the song plays, as the app records: the pass started by
/// `start_recording` (the record options write automation, as FL's do), the frames
/// driven by the heard position, the pad's drag issuing its `Batch` each frame it moved,
/// the pass ended by `stop_recording` — it writes **two lanes**, X and Y, whose values at
/// every frame's tick the hand touched are the hand's X and Y to the bit (`f32`
/// equality: an input's travel is its value).
///
/// Played back with the knobs put back to rest (the lanes, not the knobs, play): a render
/// starting inside each held stretch (beats 3, 5, 7, through the two notes the hand held
/// still over) equals **to the bit** a render of the same stretch with no lanes and the
/// pad held where the hand was; the whole song with the lanes is the same in blocks of
/// 64, 333 and 4096 frames and played live by the app's processor in odd blocks, sample
/// for sample; its notes measure the held positions' gains; one undo takes the pass away,
/// the song back to the bit.
#[test]
fn recording_the_gesture_writes_two_lanes_and_playback_reproduces_the_sound() {
    let mut s = Song::new(12);
    let pad = s.pad();
    let rest = s.whole(12.0);
    let track = s.app.project().tracks[0].id;
    s.app.select_track(track);
    let steps = s.app.bus.history().len();
    // The song plays (heard at beat 2): the pass starts there, no count-in.
    s.app.engine.set_heard_for_tests(Ticks::beats(2), true);
    s.app.start_recording();
    assert!(
        s.app.automation.write.writing(),
        "the pass writes automation"
    );
    let step = 1.0 / 16.0;
    let mut beat = 2.0;
    let mut touched: Vec<(i64, f32, f32)> = Vec::new();
    let mut holding = false;
    let key = MergeKey::of(&("fxd-e2e-record", pad.0));
    while beat <= 10.0 {
        let t = tick(beat);
        s.app.engine.set_heard_for_tests(t, true);
        s.app.auto_write_frame();
        match hand(beat) {
            Some((x, y)) => {
                // The window issues the Batch only when the puck moved.
                if s.pad_xy(pad) != (x, y) {
                    assert!(s.app.exec_merging(set_xy(pad, x, y), key));
                }
                holding = true;
                touched.push((t.0, x, y));
            }
            None if holding => {
                s.app.end_gesture();
                holding = false;
            }
            None => {}
        }
        s.app.auto_write_settle();
        beat += step;
    }
    let written = s.app.written_points();
    let (xt, yt) = (
        AutoTarget::ModInput {
            modulator: pad,
            input: 0,
        },
        AutoTarget::ModInput {
            modulator: pad,
            input: 1,
        },
    );
    assert!(
        written.contains_key(&xt) && written.contains_key(&yt),
        "X and Y written: {:?}",
        written.keys().collect::<Vec<_>>()
    );
    s.app.engine.set_heard_for_tests(Ticks::beats(10), true);
    s.app.stop_recording();
    s.app.engine.set_heard_for_tests(Ticks::beats(10), false);
    assert!(!s.app.automation.write.writing(), "the pass is over");
    assert_eq!(
        (lanes_on(&s.app, &xt), lanes_on(&s.app, &yt)),
        (1, 1),
        "two lanes, X and Y"
    );
    assert_eq!(
        s.app.bus.history().len(),
        steps + 1,
        "the pass and the drags in it, one step"
    );
    // The lanes' values at every touched tick are the hand's.
    let (lx, ly) = (
        lane_for(s.app.project(), &xt).expect("X's lane"),
        lane_for(s.app.project(), &yt).expect("Y's lane"),
    );
    for &(t, x, y) in &touched {
        assert_eq!(
            (lx.value_at(t as f64), ly.value_at(t as f64)),
            (x, y),
            "the lanes at tick {t}"
        );
    }
    eprintln!(
        "{} touched frames, the lanes equal to the hand at each",
        touched.len()
    );
    let lanes = s.whole(12.0);
    // The knobs at rest: the lanes, not the knobs, play.
    let put_back = s.app.exec(set_xy(pad, 0.5, 0.5));
    assert!(
        same_bits(&s.whole(12.0), &lanes),
        "the lanes play whatever the knobs say"
    );
    // Block-size independent; the app's processor plays it.
    for block in [64usize, 333, 4096] {
        let b = s.render(0.0, 12.0, block);
        assert!(
            same_bits(&b, &lanes),
            "blocks of {block}: {}",
            first_diff(&b, &lanes)
        );
    }
    let live = s.live(lanes.len() / 2);
    assert!(
        same_bits(&live, &lanes),
        "live equals the render: {}",
        first_diff(&live, &lanes)
    );
    // Each held stretch, heard through the lanes, from inside it.
    let from_lanes: Vec<Vec<f32>> = [3.0, 5.0, 7.0]
        .iter()
        .map(|&b| s.render(b, b + 1.4, 512))
        .collect();
    // The heard gains of the held positions (12 dB a full swing at this depth).
    for (i, h) in HELD.iter().enumerate() {
        let b = 3.0 + 2.0 * i as f64;
        let lo = gain_db(&lanes, &rest, b, LOW_HZ);
        let hi = gain_db(&lanes, &rest, b, HIGH_HZ);
        let (want_lo, want_hi) = (
            f64::from(2.0 * h.2 - 1.0) * 15.0,
            f64::from(2.0 * h.3 - 1.0) * 15.0,
        );
        eprintln!("held {:?}: low {lo:+.2} dB (want {want_lo:+.2}), high {hi:+.2} dB (want {want_hi:+.2})", (h.2, h.3));
        assert!(
            (lo - want_lo).abs() < 0.3 && (hi - want_hi).abs() < 0.3,
            "the held position heard"
        );
    }
    // Undone: the knob set back first, then the pass; the song as it was.
    if put_back {
        s.app.undo();
    }
    s.app.undo();
    assert_eq!((lanes_on(&s.app, &xt), lanes_on(&s.app, &yt)), (0, 0));
    let undone = s.whole(12.0);
    assert!(
        same_bits(&undone, &rest),
        "the pass undone: {}",
        first_diff(&undone, &rest)
    );
    // The same stretches with the hand held there and no lanes.
    for (i, h) in HELD.iter().enumerate() {
        let b = 3.0 + 2.0 * i as f64;
        s.move_pad(pad, h.2, h.3);
        let held = s.render(b, b + 1.4, 512);
        assert!(
            same_bits(&held, &from_lanes[i]),
            "from beat {b}: the lanes are the hand held at {:?}: {}",
            (h.2, h.3),
            first_diff(&held, &from_lanes[i])
        );
    }
}

/// Interleaved samples from `beat` of a render that began at 0.
fn from_beat(x: &[f32], beat: f64) -> &[f32] {
    &x[(beat * BEAT_S * SR as f64) as usize * 2..]
}

/// Preset `id` applied to controller `id` as the window's header applies it: one command.
fn apply_preset(s: &mut Song, id: ModulatorId, preset: &str) {
    let m = s.app.project().modulator(id).unwrap().2.clone();
    let p = presets::by_id(preset).expect("the preset");
    assert!(s.app.edit_modulator(presets::apply(&m, p), None));
}

/// The tones' gains in `full` against `dry` at the notes of beats `0 … beats`, each against
/// what the path says (15 dB a unit of the bipolar output at this depth, the path read at
/// the middle of the 40 ms measured); the worst miss and band 0's lowest and highest.
fn follows(path: &XyPath, full: &[f32], dry: &[f32], beats: i64) -> (f64, f64, f64) {
    let mut worst = 0.0f64;
    let mut spread = (f64::MAX, f64::MIN);
    for b in 0..beats {
        let b = b as f64;
        let [x, y] = path.at(b + 0.06);
        let (lo, hi) = (
            gain_db(full, dry, b, LOW_HZ),
            gain_db(full, dry, b, HIGH_HZ),
        );
        let (want_lo, want_hi) = (
            f64::from(2.0 * x - 1.0) * 15.0,
            f64::from(2.0 * y - 1.0) * 15.0,
        );
        eprintln!(
            "beat {b:>2}: path ({x:.3}, {y:.3}); low {lo:+6.2} dB (path {want_lo:+6.2}), high {hi:+6.2} dB (path {want_hi:+6.2})"
        );
        worst = worst.max((lo - want_lo).abs()).max((hi - want_hi).abs());
        spread = (spread.0.min(lo), spread.1.max(lo));
    }
    (worst, spread.0, spread.1)
}

fn path_of(s: &Song, id: ModulatorId) -> XyPath {
    match &s.app.project().modulator(id).unwrap().2.source {
        ModSource::XyPad(x) => {
            assert!(x.play_path, "the path plays");
            x.path.clone().expect("a path")
        }
        _ => panic!("a pad"),
    }
}

/// A render from `beat` against the full render's samples from there: to the bit from the
/// next beat's note on; the largest difference in the first beat (an effect's own glide
/// starting at the stored value: see [`a_path_plays_in_time_with_the_song`]).
fn from_beat_on(s: &mut Song, full: &[f32], beat: f64, to: f64, what: &str) -> f32 {
    let late = s.render(beat, to, RenderOptions::default().block_frames);
    let tail = from_beat(full, beat);
    let one = (BEAT_S * SR as f64) as usize * 2;
    eprintln!("{what} from beat {beat}: {}", first_diff(&late, tail));
    assert!(
        same_bits(&late[one..], &tail[one..]),
        "{what} from beat {beat}, the next beat on: {}",
        first_diff(&late[one..], &tail[one..])
    );
    late[..one]
        .iter()
        .zip(&tail[..one])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max)
}

/// Move controller `id`'s routes from the EQ's bands to the strip's pan (output 0) and
/// fader (output 1), as the inspector moves them: a route made, a route taken out (its
/// link goes with it), an axis set — a command each.
fn to_the_mixer(s: &mut Song, id: ModulatorId) -> (AutoTarget, AutoTarget) {
    let strip = s.app.project().tracks[0].strip.unwrap();
    let (pan, gain) = (AutoTarget::StripPan(strip), AutoTarget::StripGain(strip));
    s.app.set_route_depth(id, &pan, 0.8, false);
    s.app.set_route_depth(id, &gain, -0.4, false);
    let (low, high) = (s.band(0), s.band(1));
    assert!(s.app.disconnect(id, &low));
    let _ = s.app.disconnect(id, &high);
    if matches!(
        s.app.project().modulator(id).unwrap().2.source,
        ModSource::XyPad(_)
    ) {
        s.link(id, &gain, 1);
    }
    (pan, gain)
}

/// A pad on the figure-eight preset's path (32 points over two bars, smooth, played), X
/// on band 0 and Y on band 1, plays in time with the song: at every beat of four bars the
/// low tone's gain is the path's X there and the high tone's its Y, within half a decibel
/// (15 dB a unit of the bipolar output at this depth; the path moves inside the 40 ms
/// measured).
///
/// From bar 3 the render is the full render's bars 3… to the bit from beat 9 on. Bar 3's
/// first note is not, and the difference is measured: a native effect's unit is made at
/// its stored values and, at the first control point of a render that starts later, its
/// own glide (the EQ's, about 20 ms) carries it from the stored value to the modulated one
/// — the same for a plain automation lane on the band (pre-fxd; reported, not fixed here).
/// The strip's pan and fader take their value at the first control point: with the routes
/// moved there (X the pan, Y the fader), a render from bar 3 is the full render's bars 3…
/// **to the bit**, every sample.
#[test]
fn a_path_plays_in_time_with_the_song() {
    let mut s = Song::new(16);
    let dry = s.whole(16.0);
    let pad = s.pad();
    apply_preset(&mut s, pad, "xy.figure_eight");
    let path = path_of(&s, pad);
    assert!(path.smooth && path.beats == 8.0 && path.points.len() == 32);
    let full = s.whole(16.0);
    let (worst, lo, hi) = follows(&path, &full, &dry, 16);
    assert!(
        worst < 0.5,
        "the tones follow the path: {worst:.2} dB off at most"
    );
    assert!(
        hi - lo > 15.0,
        "the path swings band 0 widely: {lo:+.1} … {hi:+.1} dB"
    );
    let glide = from_beat_on(&mut s, &full, 8.0, 16.0, "the EQ's bands");
    eprintln!("bar 3's first note, the EQ's glide: {glide:e}");
    // On the mixer: every sample.
    to_the_mixer(&mut s, pad);
    let full = s.whole(16.0);
    assert!(
        rms_diff(&full, &dry) > 1e-2,
        "the path moves the pan and the fader"
    );
    let late = s.render(8.0, 16.0, RenderOptions::default().block_frames);
    let tail = from_beat(&full, 8.0);
    assert!(
        same_bits(&late, tail),
        "the pan and the fader from bar 3: {}",
        first_diff(&late, tail)
    );
}

/// A Calculus formula on the square preset (a bar a cycle, bipolar, full depth) driving
/// band 0, its phase (knob b, set as its dial sets it) an eighth of a cycle so it turns in
/// the silence after beats 1 and 3 of every bar: the low tone is up 15 dB on the first two
/// beats of every bar and down 15 on the last two (within 0.3 dB), the high tone untouched;
/// the app's processor, played live in blocks that divide nothing (1 to 1024 frames), is
/// the offline render to the bit, every sample of six bars; from bar 5 the render is the
/// full render's to the bit from beat 17 on (bar 5's first note: the EQ's glide, as in
/// [`a_path_plays_in_time_with_the_song`]), and — the formula moved to the strip's pan —
/// every sample of bars 5… to the bit.
#[test]
fn a_formula_drives_a_parameter_in_time_offline_equals_live_and_from_bar_5() {
    let mut s = Song::new(24);
    let dry = s.whole(24.0);
    let low = s.band(0);
    let f = s
        .app
        .add_modulator(ModHost::Master, ModKind::Formula, Some(low.clone()))
        .expect("a formula on the master");
    s.app.set_route_depth(f, &low, DEPTH, false);
    apply_preset(&mut s, f, "formula.square");
    assert!(s.app.exec(Command::SetModulatorInput {
        id: f,
        input: 1,
        value: 0.125,
    }));
    let full = s.whole(24.0);
    for b in 0..24 {
        let lo = gain_db(&full, &dry, b as f64, LOW_HZ);
        let hi = gain_db(&full, &dry, b as f64, HIGH_HZ);
        let want = if b % 4 < 2 { 15.0 } else { -15.0 };
        assert!(
            (lo - want).abs() < 0.3 && hi.abs() < 0.1,
            "beat {b}: low {lo:+.2} dB (want {want:+}), high {hi:+.2} dB"
        );
    }
    let live = s.live(full.len() / 2);
    assert!(
        same_bits(&live, &full),
        "live equals the render: {}",
        first_diff(&live, &full)
    );
    let glide = from_beat_on(&mut s, &full, 16.0, 24.0, "the EQ's band");
    eprintln!("bar 5's first note, the EQ's glide: {glide:e}");
    let (pan, _) = to_the_mixer(&mut s, f);
    assert!(s
        .app
        .project()
        .modulator(f)
        .unwrap()
        .2
        .route(&pan)
        .is_some());
    let full = s.whole(24.0);
    assert!(rms_diff(&full, &dry) > 1e-2, "the formula moves the pan");
    let late = s.render(16.0, 24.0, RenderOptions::default().block_frames);
    let tail = from_beat(&full, 16.0);
    assert!(
        same_bits(&late, tail),
        "the pan from bar 5: {}",
        first_diff(&late, tail)
    );
}

/// A Calculus formula on the sine preset driving band 0. Its text changed as the display
/// commits a typed one (one `SetModulator` through `edit_modulator`) to the triangle's: the
/// render changes (RMS of the difference measured); undone, the render is the original to
/// the bit. Its depth knob c halved as its dial sets it (`SetModulatorInput`): the low
/// tone's swing halves (in decibels, within 0.3 dB at the sine's crest); undone, to the bit.
#[test]
fn an_edit_to_a_formula_is_heard_and_undone_to_the_bit() {
    let mut s = Song::new(8);
    let dry = s.whole(8.0);
    let low = s.band(0);
    let f = s
        .app
        .add_modulator(ModHost::Master, ModKind::Formula, Some(low.clone()))
        .expect("a formula on the master");
    s.app.set_route_depth(f, &low, DEPTH, false);
    apply_preset(&mut s, f, "formula.sine");
    let sine = s.whole(8.0);
    // The sine a bar a cycle: its crest at beat 1 (a quarter of the bar).
    let crest = gain_db(&sine, &dry, 1.0, LOW_HZ);
    assert!(crest > 13.0, "the sine heard at its crest: {crest:+.2} dB");

    let mut m = s.app.project().modulator(f).unwrap().2.clone();
    let ModSource::Formula(src) = &mut m.source else {
        panic!("a formula")
    };
    src.text = presets::TRIANGLE.into();
    let steps = s.app.bus.history().len();
    assert!(s.app.edit_modulator(m, None), "the text is one command");
    assert_eq!(s.app.bus.history().len(), steps + 1);
    let triangle = s.whole(8.0);
    let moved = rms_diff(&triangle, &sine);
    eprintln!("the triangle against the sine: RMS of the difference {moved:.3e}");
    assert!(moved > 1e-2, "the new text is heard: {moved:.3e}");
    s.app.undo();
    let back = s.whole(8.0);
    assert!(
        same_bits(&back, &sine),
        "the text undone: {}",
        first_diff(&back, &sine)
    );

    assert!(s.app.exec(Command::SetModulatorInput {
        id: f,
        input: 2,
        value: 0.5,
    }));
    let half = s.whole(8.0);
    let half_crest = gain_db(&half, &dry, 1.0, LOW_HZ);
    eprintln!("the crest: {crest:+.2} dB at full depth, {half_crest:+.2} dB at half");
    assert!(
        (half_crest - crest / 2.0).abs() < 0.3,
        "knob c halves the swing: {crest:+.2} → {half_crest:+.2} dB"
    );
    s.app.undo();
    let back = s.whole(8.0);
    assert!(
        same_bits(&back, &sine),
        "the knob undone: {}",
        first_diff(&back, &sine)
    );
}

/// Controllers that drive nothing leave the dry master to the bit: a pad and a formula
/// with no route, the pad moved off its centre and the formula's knobs turned; then a pad
/// routed at depth 0 into both bands and moved to a corner, and a formula on the square
/// preset routed at depth 0 into band 1 — every route there (the model keeps a route at
/// 0), and the song, rendered whole and live, the dry one to the bit.
#[test]
fn unlinked_controllers_and_routes_at_depth_0_give_the_dry_master_to_the_bit() {
    let mut s = Song::new(8);
    let dry = s.whole(8.0);
    let pad = s
        .app
        .add_modulator(ModHost::Master, ModKind::XyPad, None)
        .expect("a pad");
    let f = s
        .app
        .add_modulator(ModHost::Master, ModKind::Formula, None)
        .expect("a formula");
    s.move_pad(pad, 0.9, 0.1);
    for (input, value) in [(0u8, 0.8f32), (1, 0.3), (2, 1.0)] {
        assert!(s.app.exec(Command::SetModulatorInput {
            id: f,
            input,
            value,
        }));
    }
    let unlinked = s.whole(8.0);
    assert!(
        same_bits(&unlinked, &dry),
        "unlinked: {}",
        first_diff(&unlinked, &dry)
    );

    let (low, high) = (s.band(0), s.band(1));
    let pad0 = s
        .app
        .add_modulator(ModHost::Master, ModKind::XyPad, None)
        .expect("a pad");
    s.app.set_route_depth(pad0, &low, 0.0, false);
    s.app.set_route_depth(pad0, &high, 0.0, false);
    s.link(pad0, &high, 1);
    s.move_pad(pad0, 1.0, 0.0);
    let f0 = s
        .app
        .add_modulator(ModHost::Master, ModKind::Formula, None)
        .expect("a formula");
    s.app.set_route_depth(f0, &high, 0.0, false);
    apply_preset(&mut s, f0, "formula.square");
    for (id, n) in [(pad0, 2usize), (f0, 1)] {
        let m = &s.app.project().modulator(id).unwrap().2;
        assert_eq!(m.routes.len(), n, "the routes at 0 are kept");
        assert!(m.routes.iter().all(|r| r.depth == 0.0));
    }
    let zero = s.whole(8.0);
    assert!(
        same_bits(&zero, &dry),
        "routes at depth 0: {}",
        first_diff(&zero, &dry)
    );
    let live = s.live(dry.len() / 2);
    assert!(
        same_bits(&live, &dry),
        "live, the dry master: {}",
        first_diff(&live, &dry)
    );
}
