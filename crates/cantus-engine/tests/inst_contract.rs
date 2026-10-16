//! The instrument contract (ADR 0023), checked for every native kind at once, and the unit
//! lifecycle checked with a probe unit whose behaviour is known.
//!
//! Each instrument's own tests go further; these hold the family's floor: finite output
//! identical at every block size, headroom on a loud chord, exact silence once the notes
//! are over (so an idle track can be skipped), and units that arrive before the snapshot
//! that needs them and go back when it no longer does.
//!
//! And, for ADR 0031 (`at_rest` below): **split invariance** of the unit alone — notes,
//! controllers and expressions at their samples, blocks of 1 and of 4096 and random splits
//! between, at the defaults, with the bench's 32 voices held and at settings drawn over
//! every parameter's range — and the **twin test** of `at_rest`: play, release, run silent
//! until the unit says it is at rest; then a twin that keeps processing (exact silence, still
//! at rest), a twin that is skipped (told so by `skip`), a twin that is reset and a new unit
//! (both skipped as far as the first has run) must render the same bits for what is played
//! next. When the parameters also move at that moment, the skipped twin must still agree;
//! the reset and new ones are only reported.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst as einst;
use cantus_engine::*;
use support::*;

fn with_native(kind: InstrumentKind) -> Project {
    let mut p = demo_project();
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(kind));
    p
}

/// The native track alone: the Firmus bass removed.
fn alone(kind: InstrumentKind) -> Project {
    let mut p = with_native(kind);
    p.tracks.truncate(1);
    p
}

#[test]
fn every_kind_is_finite_and_block_size_independent_at_its_defaults() {
    for kind in InstrumentKind::ALL {
        let p = with_native(kind);
        let reference = render_with(&p, 512);
        assert!(
            reference.iter().all(|x| x.is_finite()),
            "{kind:?} produced a sample that is not a number"
        );
        for block in [1usize, 64, 1000] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("{kind:?} at block size {block}"),
            );
        }
    }
}

#[test]
fn every_kind_keeps_headroom_on_a_loud_chord() {
    for kind in InstrumentKind::ALL {
        let mut p = alone(kind);
        let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(4));
        for pitch in [36u8, 48, 55, 60, 64, 67, 72, 79] {
            clip.notes
                .push(p.make_note(Ticks::ZERO, Ticks::beats(3), pitch, 127));
        }
        p.tracks[0].clips = vec![clip];
        let out = render_of(&p);
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(
            peak < 1.0,
            "{kind:?}: an eight-note chord at full velocity peaks at {peak} (0 dBFS or over)"
        );
    }
}

#[test]
fn every_kind_is_exactly_silent_once_its_notes_are_over() {
    for kind in InstrumentKind::ALL {
        let p = alone(kind);
        let opts = RenderOptions {
            tail_seconds: 6.0,
            block_frames: 256,
            ..Default::default()
        };
        let out = render(std::sync::Arc::new(Snapshot::compile(&p, 1)), &opts)
            .expect("render")
            .samples;
        let last = &out[out.len() - 48_000 * 2..];
        assert!(
            last.iter().all(|x| *x == 0.0),
            "{kind:?} still sounds six seconds after its last note: an idle track must reach \
             exact zero (flush denormals, gate the tail) so the engine can skip it"
        );
    }
}

// ---- the lifecycle, with a probe ----------------------------------------------------------

/// A unit that adds 0.25 per held note, and reports what it heard in its telemetry:
/// [0] voices, [8] the last controller value, [9] how many times it was configured.
#[derive(Debug, Default)]
struct Probe {
    held: Vec<u64>,
    last_controller: f32,
    configured: f32,
}

impl InstrumentUnit for Probe {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &einst::Assets) {
        self.configured += 1.0;
    }
    fn note_on(&mut self, note: NoteOn) {
        self.held.push(note.id);
    }
    fn note_off(&mut self, _key: u8, id: u64, _velocity: f32) {
        self.held.retain(|h| *h != id);
    }
    fn controller(&mut self, _c: Controller, value: f32) {
        self.last_controller = value;
    }
    fn all_notes_off(&mut self) {
        self.held.clear();
    }
    fn reset(&mut self) {
        self.held.clear();
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        if self.held.is_empty() {
            return false;
        }
        let v = 0.25 * self.held.len() as f32;
        left.iter_mut().for_each(|x| *x += v);
        right.iter_mut().for_each(|x| *x += v);
        true
    }
    fn active_voices(&self) -> usize {
        self.held.len()
    }
    fn telemetry(&self) -> einst::Telemetry {
        let mut t = [0.0; einst::TELEMETRY_LEN];
        t[0] = self.held.len() as f32;
        t[8] = self.last_controller;
        t[9] = self.configured;
        t
    }
}

struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    proc: Processor,
}

impl Rig {
    fn new() -> Self {
        let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        Rig {
            tx,
            rx,
            proc: Processor::without_midi(48_000.0, rx_engine, tx_engine),
        }
    }
    fn send(&mut self, m: EngineMessage) {
        assert!(self.tx.push(m).is_ok());
    }
    fn probe(&mut self, track: TrackId, kind: InstrumentKind, serial: u64) {
        self.send(EngineMessage::AddInstrument(Box::new(NativeUnit {
            track,
            kind: cantus_engine::UnitKind::Native(kind),
            serial,
            unit: Box::new(Probe::default()),
            automation: Default::default(),
        })));
    }
    fn snapshot(&mut self, p: &Project, rev: u64) {
        let mut s = Snapshot::compile(p, rev);
        s.loop_region = None;
        s.metronome = false;
        self.send(EngineMessage::SetSnapshot(std::sync::Arc::new(s)));
    }
    fn run(&mut self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        for chunk in out.chunks_mut(200) {
            self.proc.process(chunk, 2);
        }
        out
    }
    /// Serials handed back, and the last telemetry seen.
    fn events(&mut self) -> (Vec<u64>, einst::Telemetry) {
        let mut garbage = Vec::new();
        let mut tel = [0.0; einst::TELEMETRY_LEN];
        while let Ok(e) = self.rx.pop() {
            match e {
                EngineEvent::GarbageInstrument(u) => garbage.push(u.serial),
                EngineEvent::Meters(m) => tel = m.inst,
                _ => {}
            }
        }
        (garbage, tel)
    }
}

fn one_note_project(kind: InstrumentKind) -> Project {
    let mut p = Project::new("probe");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Probe");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
    let mut clip = p.make_clip("One", Ticks::ZERO, Ticks::beats(4));
    clip.notes
        .push(p.make_note(Ticks::beats(1), Ticks::beats(1), 60, 100));
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

#[test]
fn a_delivered_unit_plays_its_notes_sample_accurately() {
    let p = one_note_project(InstrumentKind::Organ);
    let track = p.tracks[0].id;
    let mut rig = Rig::new();
    rig.probe(track, InstrumentKind::Organ, 1);
    rig.snapshot(&p, 1);
    rig.send(EngineMessage::Play);
    let out = rig.run(48_000);
    let first = out.iter().position(|x| *x != 0.0).map(|i| i / 2);
    // Beat 1 at 120 BPM is half a second: sample 24 000, whatever the block size.
    assert_eq!(first, Some(24_000));
}

#[test]
fn units_go_back_when_the_track_changes_instrument_or_nobody_claims_them() {
    let mut p = one_note_project(InstrumentKind::Organ);
    let track = p.tracks[0].id;
    let mut rig = Rig::new();
    rig.probe(track, InstrumentKind::Organ, 1);
    rig.snapshot(&p, 1);
    rig.run(400);
    assert_eq!(
        rig.events().0,
        Vec::<u64>::new(),
        "the first unit is in use"
    );

    // Another kind: its unit replaces the first, which comes back.
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Fm));
    rig.probe(track, InstrumentKind::Fm, 2);
    rig.snapshot(&p, 2);
    rig.run(400);
    assert_eq!(rig.events().0, vec![1]);

    // The same kind again, with a new value: the unit stays (voices keep ringing).
    p.tracks[0]
        .instrument
        .native_mut()
        .unwrap()
        .set("gain", -6.0);
    rig.snapshot(&p, 3);
    rig.run(400);
    assert_eq!(rig.events().0, Vec::<u64>::new());

    // Firmus: the native unit goes back.
    p.tracks[0].instrument = InstrumentSpec::default();
    rig.snapshot(&p, 4);
    rig.run(400);
    assert_eq!(rig.events().0, vec![2]);

    // A unit no snapshot claims is stale, and goes back with the next snapshot.
    rig.probe(track, InstrumentKind::Sampler, 3);
    rig.snapshot(&p, 5);
    rig.run(400);
    assert_eq!(rig.events().0, vec![3]);
}

#[test]
fn the_probed_track_reports_its_telemetry_and_live_controllers_arrive() {
    let p = one_note_project(InstrumentKind::Organ);
    let track = p.tracks[0].id;
    let mut rig = Rig::new();
    rig.probe(track, InstrumentKind::Organ, 1);
    rig.snapshot(&p, 1);
    rig.send(EngineMessage::SetInstrumentProbe(Some(track)));
    rig.send(EngineMessage::SetLiveTrack(Some(track)));
    rig.send(EngineMessage::NoteOn {
        track,
        pitch: 64,
        velocity: 90,
    });
    rig.send(EngineMessage::NoteOn {
        track,
        pitch: 67,
        velocity: 90,
    });
    rig.send(EngineMessage::Midi(MidiEvent {
        status: 0xB0,
        data1: 64,
        data2: 127,
    }));
    rig.run(400);
    let (_, tel) = rig.events();
    assert_eq!(tel[0], 2.0, "two live notes held, told apart by their ids");
    assert_eq!(tel[8], 1.0, "the sustain pedal arrived as a controller");
    assert!(tel[9] >= 1.0, "the unit was configured by the snapshot");
    rig.send(EngineMessage::NoteOff { track, pitch: 64 });
    rig.run(400);
    assert_eq!(
        rig.events().1[0],
        1.0,
        "the note-off released one note, not both"
    );
}

// ---- the unit alone: split invariance and at rest (ADR 0031) ------------------------------

use support::heavy::{self, Rng, SAMPLE_RATE};

#[derive(Copy, Clone, Debug)]
enum Ev {
    On(NoteOn),
    Off { key: u8, id: u64, velocity: f32 },
    Choke { key: u8, id: u64 },
    Ctl(Controller, f32),
    Expr(u64, NoteExpression, f32),
    AllOff,
}

/// What is played: events at frames, over `frames`.
#[derive(Clone, Debug, Default)]
struct Score {
    frames: usize,
    events: Vec<(usize, Ev)>,
}

impl Score {
    fn silence(frames: usize) -> Self {
        Score {
            frames,
            events: Vec::new(),
        }
    }

    /// Notes of every length, overlapping, with velocity, a choke now and then, the
    /// pedal, the wheels and per-note expressions — all of it let go before the end (keys
    /// up, pedal up, wheels home), so what follows can come to rest.
    fn music(frames: usize, voices: usize, rng: &mut Rng) -> Self {
        let mut ev = Vec::new();
        let mut held: Vec<(u8, u64)> = Vec::new();
        let mut id = rng.next_u64() >> 16;
        let end = frames * 3 / 4;
        let mut f = 0;
        while f < end {
            let roll = rng.range(0, 99);
            if roll < 45 && held.len() < voices {
                id += 1;
                let key = rng.range(24, 96) as u8;
                let on = NoteOn {
                    key,
                    velocity: rng.range(1, 127) as f32 / 127.0,
                    id,
                    channel: rng.range(0, 15) as u8,
                };
                ev.push((f, Ev::On(on)));
                held.push((key, id));
            } else if roll < 75 && !held.is_empty() {
                let (key, id) = held.remove(rng.range(0, held.len() - 1));
                if rng.chance(0.1) {
                    ev.push((f, Ev::Choke { key, id }));
                } else {
                    let velocity = rng.unit();
                    ev.push((f, Ev::Off { key, id, velocity }));
                }
            } else if roll < 85 {
                let (c, v) = match rng.range(0, 3) {
                    0 => (Controller::PitchBend, rng.unit() * 4.0 - 2.0),
                    1 => (Controller::ModWheel, rng.unit()),
                    2 => (Controller::Sustain, if rng.chance(0.5) { 1.0 } else { 0.0 }),
                    _ => (Controller::Pressure, rng.unit()),
                };
                ev.push((f, Ev::Ctl(c, v)));
            } else if roll < 95 && !held.is_empty() {
                let (_, nid) = held[rng.range(0, held.len() - 1)];
                let (e, v) = match rng.range(0, 2) {
                    0 => (NoteExpression::Tuning, rng.unit() * 2.0 - 1.0),
                    1 => (NoteExpression::Pressure, rng.unit()),
                    _ => (NoteExpression::Brightness, rng.unit()),
                };
                ev.push((f, Ev::Expr(nid, e, v)));
            } else if roll == 99 {
                ev.push((f, Ev::AllOff));
                held.clear();
            }
            f += rng.range(1, 4_000);
        }
        let f = end;
        for (key, id) in held {
            ev.push((
                f,
                Ev::Off {
                    key,
                    id,
                    velocity: 0.5,
                },
            ));
        }
        for c in [
            Controller::Sustain,
            Controller::PitchBend,
            Controller::ModWheel,
            Controller::Pressure,
        ] {
            ev.push((f, Ev::Ctl(c, 0.0)));
        }
        Score { frames, events: ev }
    }

    /// The bench's heavy load: `HEAVY_VOICES` notes held together (`sessions::instrument`),
    /// released at three quarters.
    fn chord(frames: usize) -> Self {
        let mut ev = Vec::new();
        for v in 0..heavy::HEAVY_VOICES {
            let key = 36 + ((v * 7) % 60) as u8;
            let on = NoteOn {
                key,
                velocity: 96.0 / 127.0,
                id: 1_000 + v as u64,
                channel: 0,
            };
            ev.push((0, Ev::On(on)));
        }
        for v in 0..heavy::HEAVY_VOICES {
            let key = 36 + ((v * 7) % 60) as u8;
            let id = 1_000 + v as u64;
            ev.push((
                frames * 3 / 4,
                Ev::Off {
                    key,
                    id,
                    velocity: 0.5,
                },
            ));
        }
        Score { frames, events: ev }
    }
}

fn apply(u: &mut dyn InstrumentUnit, e: Ev) {
    match e {
        Ev::On(n) => u.note_on(n),
        Ev::Off { key, id, velocity } => u.note_off(key, id, velocity),
        Ev::Choke { key, id } => u.choke(key, id),
        Ev::Ctl(c, v) => u.controller(c, v),
        Ev::Expr(id, e, v) => u.expression(id, e, v),
        Ev::AllOff => u.all_notes_off(),
    }
}

/// Play `score` on `u`, each event between calls at its frame, the calls split further
/// by `block`. A call that says it wrote nothing must have written nothing.
fn play(
    u: &mut dyn InstrumentUnit,
    score: &Score,
    mut block: impl FnMut() -> usize,
    mut after: impl FnMut(&dyn InstrumentUnit, &[f32], &[f32]),
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let n = score.frames;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let (mut at, mut ei) = (0, 0);
    while at < n {
        while ei < score.events.len() && score.events[ei].0 <= at {
            apply(u, score.events[ei].1);
            ei += 1;
        }
        let next = score.events.get(ei).map_or(n, |e| e.0.min(n));
        let b = block().clamp(1, n - at).min(next.max(at + 1) - at);
        let wrote = u.process(&mut l[at..at + b], &mut r[at..at + b]);
        if !wrote
            && !l[at..at + b]
                .iter()
                .chain(&r[at..at + b])
                .all(|v| *v == 0.0)
        {
            return Err(format!(
                "process said it wrote nothing at frame {at}, and did"
            ));
        }
        after(u, &l[at..at + b], &r[at..at + b]);
        at += b;
    }
    Ok((l, r))
}

/// A unit as the processor makes one: built, then configured by the snapshot.
fn inst_unit(spec: &NativeSpec) -> Box<dyn InstrumentUnit> {
    if spec.kind == InstrumentKind::Wavetable {
        einst::wavetable::prepare(spec, &[]);
    }
    // Chorda's reference keyboard designed first: a unit never waits for it (made before,
    // it is silent until the designer has made it — not what these checks are about).
    if spec.kind == InstrumentKind::Chorda {
        einst::chorda::ensure(Default::default(), SAMPLE_RATE);
    }
    let mut u = einst::make(spec.kind, SAMPLE_RATE);
    u.configure(spec, &[]);
    u
}

/// A run's name and how it splits its blocks.
type Split<'a> = (&'a str, Box<dyn FnMut() -> usize>);

fn splits(mut rng: Rng) -> impl FnMut() -> usize {
    move || rng.block()
}

fn first_difference(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>)) -> Option<String> {
    for (ch, (x, y)) in [(&a.0, &b.0), (&a.1, &b.1)].into_iter().enumerate() {
        if x.len() != y.len() {
            return Some(format!("lengths {} and {}", x.len(), y.len()));
        }
        if let Some(i) = (0..x.len()).find(|&i| x[i].to_bits() != y[i].to_bits()) {
            return Some(format!(
                "channel {ch}, frame {i}: {:e} against {:e}",
                x[i], y[i]
            ));
        }
    }
    None
}

/// The settings a kind is checked at, each with what is played on it: the defaults with
/// music, the defaults with the bench's 32-voice chord ("heavy"), and `random` settings
/// drawn over every parameter's range with music.
fn inst_specs(
    kind: InstrumentKind,
    random: usize,
    rng: &mut Rng,
) -> Vec<(String, NativeSpec, bool)> {
    let mut v = vec![
        ("default".to_string(), NativeSpec::new(kind), false),
        ("heavy".to_string(), NativeSpec::new(kind), true),
    ];
    for i in 0..random {
        let p = if i % 3 == 2 { 1.0 } else { 0.5 };
        v.push((
            format!("random #{i}"),
            heavy::random_instrument(kind, p, rng),
            false,
        ));
    }
    v
}

fn score_for(chord: bool, frames: usize, rng: &mut Rng) -> Score {
    if chord {
        Score::chord(frames)
    } else {
        Score::music(frames, 16, rng)
    }
}

fn inst_split_invariance(kind: InstrumentKind) {
    let mut rng = Rng::new(0x1257_0000 + kind as u64);
    let mut failures = Vec::new();
    for (label, spec, chord) in inst_specs(kind, 6, &mut rng) {
        let score = score_for(chord, 72_000, &mut rng);
        let reference = match play(&mut *inst_unit(&spec), &score, || 512, |_, _, _| {}) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{kind:?} {label}: {e}"));
                continue;
            }
        };
        if !reference
            .0
            .iter()
            .chain(&reference.1)
            .all(|v| v.is_finite())
        {
            failures.push(format!("{kind:?} {label}: not finite"));
            continue;
        }
        let runs: [Split; 5] = [
            ("blocks of 1", Box::new(|| 1)),
            ("blocks of 4096", Box::new(|| 4096)),
            ("blocks of 7", Box::new(|| 7)),
            ("random splits", Box::new(splits(rng.clone()))),
            ("random splits", Box::new(splits(Rng::new(rng.next_u64())))),
        ];
        for (name, block) in runs {
            match play(&mut *inst_unit(&spec), &score, block, |_, _, _| {}) {
                Ok(out) => {
                    if let Some(d) = first_difference(&reference, &out) {
                        failures.push(format!("{kind:?} {label}, {name}: {d}"));
                    }
                }
                Err(e) => failures.push(format!("{kind:?} {label}, {name}: {e}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "split invariance broken:\n{}",
        failures.join("\n")
    );
}

/// Play, release, run silent until at rest, then compare four twins on what is played
/// next. `Ok(Some(frames))`: at rest after that much silence and every twin agreed;
/// `Ok(None)`: no rest within `cap`. With `change`, the parameters move at the wake.
fn inst_twins(
    spec: &NativeSpec,
    chord: bool,
    seed: u64,
    cap: usize,
    change: Option<&NativeSpec>,
) -> Result<Option<usize>, String> {
    let mut rng = Rng::new(seed);
    // A unit that rests is as good as a new one skipped as far, so where a new one never
    // comes to rest a long wait is wasted: wait a little, enough to catch a unit that
    // claims rest where a new one cannot.
    let mut probe = inst_unit(spec);
    probe.skip(cap);
    let cap = if probe.at_rest() {
        cap
    } else {
        cap.min(48_000)
    };
    let history = score_for(chord, rng.range(4_800, 60_000), &mut rng);
    let (mut a, mut b, mut d) = (inst_unit(spec), inst_unit(spec), inst_unit(spec));
    let hist_seed = rng.next_u64();
    for u in [&mut a, &mut b, &mut d] {
        play(
            &mut **u,
            &history,
            splits(Rng::new(hist_seed)),
            |_, _, _| {},
        )?;
    }
    let mut silent = 0;
    while !a.at_rest() {
        if silent > cap {
            return Ok(None);
        }
        let n = rng.block();
        for u in [&mut a, &mut b, &mut d] {
            play(&mut **u, &Score::silence(n), || n, |_, _, _| {})?;
        }
        silent += n;
        if a.at_rest() != b.at_rest() || a.at_rest() != d.at_rest() {
            return Err("at_rest differs between identical twins".into());
        }
    }
    if a.active_voices() != 0 {
        return Err(format!("at rest with {} voices active", a.active_voices()));
    }
    let k = rng.range(0, 30_000);
    let mut broke = None;
    play(
        &mut *a,
        &Score::silence(k),
        splits(Rng::new(rng.next_u64())),
        |u, l, r| {
            if broke.is_none() && !(l.iter().chain(r).all(|v| *v == 0.0) && u.at_rest()) {
                broke = Some(if u.at_rest() {
                    "at rest, no notes gave sound"
                } else {
                    "at rest, no notes left it not at rest"
                });
            }
        },
    )?;
    if let Some(e) = broke {
        return Err(e.into());
    }
    let total = history.frames + silent + k;
    d.skip(k);
    if !d.at_rest() {
        return Err("skip left the unit not at rest".into());
    }
    b.reset();
    b.skip(total);
    let mut c = inst_unit(spec);
    c.skip(total);
    if !b.at_rest() || !c.at_rest() {
        return Err("a reset or a new unit, skipped, is not at rest".into());
    }
    if let Some(to) = change {
        if to.kind == InstrumentKind::Wavetable {
            einst::wavetable::prepare(to, &[]);
        }
        for u in [&mut a, &mut b, &mut c, &mut d] {
            u.configure(to, &[]);
        }
    }
    let back = score_for(chord, rng.range(4_800, 48_000), &mut rng);
    let out_a = play(
        &mut *a,
        &back,
        splits(Rng::new(rng.next_u64())),
        |_, _, _| {},
    )?;
    let out_d = play(
        &mut *d,
        &back,
        splits(Rng::new(rng.next_u64())),
        |_, _, _| {},
    )?;
    if let Some(e) = first_difference(&out_a, &out_d) {
        return Err(format!(
            "the skipped twin differs after {k} frames of rest: {e}"
        ));
    }
    for (name, u) in [("reset", &mut b), ("new", &mut c)] {
        let out = play(
            &mut **u,
            &back,
            splits(Rng::new(rng.next_u64())),
            |_, _, _| {},
        )?;
        if let Some(e) = first_difference(&out_a, &out) {
            if change.is_none() {
                return Err(format!("the {name} twin differs: {e}"));
            }
            // As good as new only while the parameters stay (a memory that outlives
            // silence may show once they move): reported, not required.
            eprintln!(
                "  ({:?}: parameters moved at the wake: the {name} twin differs: {e})",
                spec.kind
            );
        }
    }
    Ok(Some(silent))
}

/// Whether a kind must come to rest at its defaults and with the heavy chord, within the
/// test's silence (30 s). The reasons for a `false` are in `docs/round21/at-rest.md`.
fn inst_must_rest(kind: InstrumentKind) -> (bool, bool) {
    match kind {
        // The tone wheels, the scanner and the rotor turn for ever (see the notes).
        InstrumentKind::Organ => (false, false),
        _ => (true, true),
    }
}

/// The change made during a rest trial, with Chorda's sustain pedal let up: held down it
/// keeps the dampers off the strings, which is never rest — as controller 64 held is not
/// (`inst_chorda::the_pedals_arrive_as_controllers`). Every other contract draws it.
fn released(mut spec: NativeSpec) -> NativeSpec {
    if spec.kind == InstrumentKind::Chorda {
        spec.set("pedal", 0.0);
    }
    spec
}

fn inst_at_rest_is_exact(kind: InstrumentKind) {
    let mut rng = Rng::new(0x1a7_4e57 + kind as u64);
    let mut failures = Vec::new();
    let mut report = Vec::new();
    let (rest_default, rest_heavy) = inst_must_rest(kind);
    for (label, spec, chord) in inst_specs(kind, 4, &mut rng) {
        let fixed = label == "default" || label == "heavy";
        let cap = if fixed { 30 * 48_000 } else { 4 * 48_000 };
        let must = (label == "default" && rest_default) || (label == "heavy" && rest_heavy);
        for trial in 0..3 {
            // The first trial as the tests run, the others as the engine's `process` will
            // (denormals flushed): exact in both, but some state only reaches zero in the
            // second, so rest is required only there.
            let flushed = trial != 0;
            let _flush = flushed.then(heavy::Flush::new);
            let change =
                (trial == 2).then(|| released(heavy::random_instrument(kind, 0.5, &mut rng)));
            let seed = rng.next_u64();
            match inst_twins(&spec, chord, seed, cap, change.as_ref()) {
                Ok(Some(after)) => report.push(format!(
                    "{label} #{trial}: at rest after {:.2} s{}",
                    after as f32 / SAMPLE_RATE,
                    if flushed { " (FTZ)" } else { "" }
                )),
                Ok(None) => {
                    report.push(format!("{label} #{trial}: no rest"));
                    if must && flushed {
                        failures.push(format!(
                            "{kind:?} {label} #{trial}: no rest within {} s of silence",
                            cap / 48_000
                        ));
                    }
                }
                Err(e) => failures.push(format!("{kind:?} {label} #{trial} (seed {seed}): {e}")),
            }
        }
    }
    eprintln!("{kind:?}:\n  {}", report.join("\n  "));
    if rest_default {
        assert!(
            inst_unit(&NativeSpec::new(kind)).at_rest(),
            "{kind:?}: a new unit is at rest by definition"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- voices on several threads (ADR 0038) -----------------------------------------------

use cantus_dsp::par::{Job, Parallel, Reversed};

/// A pool a test can build: the jobs dealt to `.0` real threads (spawned per call — a test
/// may allocate), so voices really do render at the same time on several cores.
struct Threads(usize);

impl Parallel for Threads {
    fn width(&self) -> usize {
        self.0
    }

    fn run(&self, jobs: &mut [Job<'_>]) {
        let per = jobs.len().div_ceil(self.0.max(1)).max(1);
        std::thread::scope(|s| {
            for chunk in jobs.chunks_mut(per) {
                s.spawn(move || {
                    for j in chunk.iter_mut() {
                        if let Some(f) = j.take() {
                            f();
                        }
                    }
                });
            }
        });
    }
}

/// How a run splits its blocks.
type Blocks = Box<dyn FnMut() -> usize>;

/// [`play`] through `process_par`.
fn play_par(
    u: &mut dyn InstrumentUnit,
    score: &Score,
    block: impl FnMut() -> usize,
    par: &dyn Parallel,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    play_into(u, score, block, Some(par), false)
}

/// What a buffer already holds when `play_into(.., dirty = true)` adds into it: `-0.0`
/// mostly (the sign a sum can lose), `+0.0` and a few values.
fn dirt(i: usize) -> f32 {
    [-0.0, -0.0, 0.0, 0.25, -0.0, -3.0e-5, -0.0][i % 7]
}

/// [`play`] (`par = None`: `process`) or [`play_par`] — into buffers that start at zero, or
/// (`dirty`) at [`dirt`]: `process` *adds* into what it is given, and `process_par` must
/// add the same bits whatever the buffer held.
fn play_into(
    u: &mut dyn InstrumentUnit,
    score: &Score,
    mut block: impl FnMut() -> usize,
    par: Option<&dyn Parallel>,
    dirty: bool,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let n = score.frames;
    let start: Vec<f32> = (0..n).map(|i| if dirty { dirt(i) } else { 0.0 }).collect();
    let (mut l, mut r) = (
        start.clone(),
        start.iter().rev().copied().collect::<Vec<_>>(),
    );
    let (mut at, mut ei) = (0, 0);
    while at < n {
        while ei < score.events.len() && score.events[ei].0 <= at {
            apply(u, score.events[ei].1);
            ei += 1;
        }
        let next = score.events.get(ei).map_or(n, |e| e.0.min(n));
        let b = block().clamp(1, n - at).min(next.max(at + 1) - at);
        let (lb, rb) = (&mut l[at..at + b], &mut r[at..at + b]);
        let wrote = match par {
            Some(par) => u.process_par(lb, rb, par),
            None => u.process(lb, rb),
        };
        if !dirty
            && !wrote
            && !l[at..at + b]
                .iter()
                .chain(&r[at..at + b])
                .all(|v| *v == 0.0)
        {
            return Err(format!(
                "process_par said it wrote nothing at frame {at}, and did"
            ));
        }
        at += b;
    }
    Ok((l, r))
}

/// **Parallel = serial, to the bit, for the unit alone**: every kind's `process_par` —
/// its voices run last-first on one thread, or at once on real threads — gives `process`'s
/// samples, at the defaults, under the bench's 32-voice chord and at random settings, at
/// fixed and random splits. A kind that does not split must too (the default method).
fn inst_voice_split(kind: InstrumentKind) {
    let mut rng = Rng::new(0x5917_0000 + kind as u64);
    let mut failures = Vec::new();
    for (label, spec, chord) in inst_specs(kind, 4, &mut rng) {
        let score = score_for(chord, 48_000, &mut rng);
        let reference = match play(&mut *inst_unit(&spec), &score, || 512, |_, _, _| {}) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{kind:?} {label}: {e}"));
                continue;
            }
        };
        let runs: [(&str, &dyn Parallel, Blocks); 4] = [
            ("backwards, blocks of 512", &Reversed(4), Box::new(|| 512)),
            (
                "backwards, random splits",
                &Reversed(4),
                Box::new(splits(rng.clone())),
            ),
            ("4 threads, blocks of 256", &Threads(4), Box::new(|| 256)),
            (
                "3 threads, random splits",
                &Threads(3),
                Box::new(splits(Rng::new(rng.next_u64()))),
            ),
        ];
        for (name, par, block) in runs {
            match play_par(&mut *inst_unit(&spec), &score, block, par) {
                Ok(out) => {
                    if let Some(d) = first_difference(&reference, &out) {
                        failures.push(format!("{kind:?} {label}, {name}: {d}"));
                    }
                }
                Err(e) => failures.push(format!("{kind:?} {label}, {name}: {e}")),
            }
        }
        // Into a buffer that already holds something — `-0.0` above all, which a lane
        // that starts at `+0.0` would turn into `+0.0`: the same bits as `process` adding.
        let dirty = play_into(&mut *inst_unit(&spec), &score, || 512, None, true);
        let runs: [(&str, &dyn Parallel); 2] = [
            ("into a dirty buffer, backwards", &Reversed(4)),
            ("into a dirty buffer, 3 threads", &Threads(3)),
        ];
        for (name, par) in runs {
            let out = play_into(&mut *inst_unit(&spec), &score, || 512, Some(par), true);
            match (&dirty, &out) {
                (Ok(a), Ok(b)) => {
                    if let Some(d) = first_difference(a, b) {
                        failures.push(format!("{kind:?} {label}, {name}: {d}"));
                    }
                }
                (Err(e), _) | (_, Err(e)) => {
                    failures.push(format!("{kind:?} {label}, {name}: {e}"))
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "voices on several threads moved the bits:
{}",
        failures.join(
            "
"
        )
    );
}

macro_rules! per_kind {
    ($($name:ident => $kind:expr),* $(,)?) => {
        mod split {
            use super::*;
            $( #[test] fn $name() { inst_split_invariance($kind) } )*
        }
        mod voices {
            use super::*;
            $( #[test] fn $name() { inst_voice_split($kind) } )*
        }
        mod at_rest {
            use super::*;
            $( #[test] fn $name() { inst_at_rest_is_exact($kind) } )*
        }
    };
}

per_kind! {
    wavetable => InstrumentKind::Wavetable,
    analog => InstrumentKind::Analog,
    fm => InstrumentKind::Fm,
    sampler => InstrumentKind::Sampler,
    bass => InstrumentKind::Bass,
    epiano => InstrumentKind::ElectricPiano,
    organ => InstrumentKind::Organ,
    chorda => InstrumentKind::Chorda,
}

#[test]
fn every_native_instrument_is_checked() {
    // `per_kind!` lists eight: a new kind must be added there.
    assert_eq!(InstrumentKind::ALL.len(), 8);
}
