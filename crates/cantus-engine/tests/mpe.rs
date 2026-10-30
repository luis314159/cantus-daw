//! Per-note expression on the audio thread (round 33, ADR 0080): absent is a null to the
//! bit, present is heard, block-size independent, real time equal to the offline render
//! and parallel equal to serial; every value lands on its note's sample whether the unit
//! splits or takes offsets; and the instruments that listen bend by what the curve says —
//! measured from their sound.

use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
use cantus_core::*;
use cantus_dsp::math;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::inst::{self as einst, InstrumentUnit, NoteExpression, NoteOn};
use cantus_engine::*;
use std::sync::{Arc, Mutex};

const SR: f64 = 48_000.0;

fn st(x: f64) -> i64 {
    ExprKind::Bend.store(x)
}

fn u(x: f64) -> i64 {
    ExprKind::Pressure.store(x)
}

/// The four instruments that listen to per-note expression, each a track of three notes.
const KINDS: [InstrumentKind; 4] = [
    InstrumentKind::Wavetable,
    InstrumentKind::Analog,
    InstrumentKind::Fm,
    InstrumentKind::Bass,
];

fn base_project() -> Project {
    let mut p = Project::new("mpe");
    p.tempo_bpm = 120.0;
    for (i, kind) in KINDS.into_iter().enumerate() {
        let mut t = p.make_track(format!("T{i}"));
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        let mut clip = p.make_clip("phrase", Ticks::ZERO, Ticks::beats(6));
        for (k, pitch) in [45u8, 52, 57].iter().enumerate() {
            let n = p.make_note(Ticks::beats(k as i64 * 2), Ticks::beats(1), *pitch, 100);
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// A curve on every note: a bend up a tone and back with a vibrato, pressure swelling,
/// timbre opening.
fn moving() -> NoteExpr {
    let vib: Vec<ExprPoint> = (0..=40)
        .map(|i| {
            let t = 240 + i * 12;
            ExprPoint(t, st(2.0 + 0.3 * math::sin(i as f64 * 0.9)))
        })
        .collect();
    let mut bend = vec![ExprPoint(0, st(-1.0)), ExprPoint(200, st(2.0))];
    bend.extend(vib);
    bend.push(ExprPoint(900, 0));
    NoteExpr {
        bend,
        pressure: vec![ExprPoint(0, u(0.1)), ExprPoint(600, u(1.0))],
        timbre: vec![ExprPoint(0, u(0.3)), ExprPoint(960, u(0.9))],
    }
    .normalised()
}

fn expressive_project() -> Project {
    let mut p = base_project();
    for t in &mut p.tracks {
        for n in &mut t.clips[0].notes {
            n.expr = moving().boxed();
        }
    }
    p
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts(block: usize) -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.5,
        block_frames: block,
        ..Default::default()
    }
}

fn offline(p: &Project, block: usize) -> Vec<f32> {
    render(snap(p), &opts(block)).unwrap().samples
}

/// The null: expression only on muted notes, only on a take lane's clip, only on a muted
/// clip — every sample exactly as with none at all.
#[test]
fn no_expression_and_unheard_expression_render_exactly_as_none() {
    let base = offline(&base_project(), 256);
    let mut muted = base_project();
    for t in &mut muted.tracks {
        let mut extra = t.clips[0].clone();
        extra.id = ClipId(9_000 + t.id.0);
        for n in &mut extra.notes {
            n.expr = moving().boxed();
            n.muted = true;
        }
        t.clips.push(extra.clone());
        let mut lane = extra.clone();
        lane.id = ClipId(9_500 + t.id.0);
        lane.lane = 1;
        lane.notes.iter_mut().for_each(|n| n.muted = false);
        t.clips.push(lane);
        let mut off = extra;
        off.id = ClipId(9_800 + t.id.0);
        off.muted = true;
        off.notes.iter_mut().for_each(|n| n.muted = false);
        t.clips.push(off);
    }
    assert!(snap(&muted).tracks.iter().all(|t| t.expr.is_empty()));
    assert_null(&base, &offline(&muted, 256), 0.0, "unheard expression");
}

/// Present, it is heard; the same samples at any block size, to the bit.
#[test]
fn expression_is_heard_and_block_size_independent() {
    let base = offline(&base_project(), 256);
    let p = expressive_project();
    let a = offline(&p, 256);
    assert!(
        a.iter().zip(&base).any(|(x, y)| x != y),
        "expression changes the sound"
    );
    for block in [1usize, 37, 64, 1024] {
        assert_null(&a, &offline(&p, block), 0.0, &format!("block {block}"));
    }
}

/// The real-time processor, in uneven blocks, serial and with the pool, equals the
/// offline render to the bit.
#[test]
fn realtime_and_parallel_equal_the_offline_render() {
    let p = expressive_project();
    let s = snap(&p);
    let want = render(s.clone(), &opts(256)).unwrap();
    for workers in [Workers::SERIAL, Workers::parallel(3)] {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
        let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
        proc.set_workers(workers);
        let mut live = (*s).clone();
        live.loop_region = None;
        live.metronome = false;
        let live = Arc::new(live);
        for unit in einst::units_for(&live, SR as f32, |_, _| false) {
            tx.push(EngineMessage::AddInstrument(unit)).unwrap();
        }
        tx.push(EngineMessage::Automation(AutomationRuntime::build(&live)))
            .unwrap();
        tx.push(EngineMessage::SetSnapshot(live)).unwrap();
        tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let total = want.frames();
        let mut out = Vec::with_capacity(total * 2);
        let sizes = [128usize, 333, 512, 96, 1024, 17, 5];
        let mut i = 0;
        while out.len() / 2 < total {
            let n = sizes[i % sizes.len()].min(total - out.len() / 2);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            proc.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while rx.pop().is_ok() {}
        }
        assert_null(&want.samples, &out, 0.0, "realtime vs offline");
    }
}

/// What an instrument heard: `(sample, what, note id, value)`, `what` 0 a note-on, 1 an
/// expression's kind + 1.
#[derive(Debug, Default, PartialEq)]
struct Log(Vec<(u64, u8, u64, u32)>);

#[derive(Debug)]
struct Ear {
    log: Arc<Mutex<Log>>,
    at: u64,
    offsets: bool,
}

impl Ear {
    fn hear(&mut self, what: u8, id: u64, value: f32, offset: u32) {
        let at = self.at + u64::from(offset);
        self.log
            .lock()
            .unwrap()
            .0
            .push((at, what, id, value.to_bits()));
    }
}

impl InstrumentUnit for Ear {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &einst::Assets) {}
    fn note_on(&mut self, note: NoteOn) {
        // A note-on's value is its channel.
        self.hear(0, note.id, f32::from_bits(u32::from(note.channel)), 0);
    }
    fn note_off(&mut self, _key: u8, id: u64, velocity: f32) {
        // A note-off is 8.
        self.hear(8, id, velocity, 0);
    }
    fn expression(&mut self, id: u64, e: NoteExpression, value: f32) {
        self.expression_at(id, e, value, 0);
    }
    fn expression_at(&mut self, id: u64, e: NoteExpression, value: f32, offset: u32) {
        let what = match e {
            NoteExpression::Tuning => 1,
            NoteExpression::Pressure => 2,
            NoteExpression::Brightness => 3,
            _ => 9,
        };
        self.hear(what, id, value, offset);
    }
    fn expression_offsets(&self) -> bool {
        self.offsets
    }
    fn all_notes_off(&mut self) {}
    fn reset(&mut self) {}
    fn process(&mut self, l: &mut [f32], _r: &mut [f32]) -> bool {
        self.at += l.len() as u64;
        false
    }
    fn active_voices(&self) -> usize {
        0
    }
}

/// One track whose instrument is an ear, played from the top in blocks of `block`.
fn ear_render(p: &Project, block: usize, offsets: bool) -> Log {
    let log = Arc::new(Mutex::new(Log::default()));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut s = Snapshot::compile(p, 1);
    s.metronome = false;
    tx.push(EngineMessage::AddInstrument(Box::new(NativeUnit {
        track: p.tracks[0].id,
        kind: UnitKind::Native(KINDS[0]),
        serial: 1,
        unit: Box::new(Ear {
            log: log.clone(),
            at: 0,
            offsets,
        }),
        automation: Default::default(),
    })))
    .unwrap();
    tx.push(EngineMessage::SetSnapshot(Arc::new(s))).unwrap();
    tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let total = 6 * 24_000usize;
    let mut done = 0;
    while done < total {
        let n = block.min(total - done);
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        done += n;
        while rx.pop().is_ok() {}
    }
    let out = std::mem::take(&mut *log.lock().unwrap());
    out
}

/// Every value lands on the sample its tick is at — split there, or handed ahead with its
/// offset — at any block size; each note's first values follow its note-on at its sample;
/// nothing reaches a note after its end.
#[test]
fn values_land_on_their_samples_split_or_ahead() {
    let mut p = expressive_project();
    p.tracks.truncate(1);
    let a = ear_render(&p, 512, false);
    for (block, offsets) in [(37, false), (512, true), (37, true), (1, true)] {
        assert_eq!(
            a,
            ear_render(&p, block, offsets),
            "block {block}, offsets {offsets}"
        );
    }
    let notes: Vec<_> = a.0.iter().filter(|e| e.1 == 0).collect();
    assert_eq!(notes.len(), 3);
    for (k, on) in notes.iter().enumerate() {
        // 120 BPM at 48 kHz: tick t is sample 25 t; the notes are two beats apart.
        assert_eq!(on.0, k as u64 * 48_000);
        let mine: Vec<_> =
            a.0.iter()
                .filter(|e| e.1 > 0 && e.1 < 8 && e.2 == on.2)
                .collect();
        let i = a.0.iter().position(|e| e == *on).unwrap();
        assert_eq!(
            a.0[i + 1].0,
            on.0,
            "the first value at the note-on's sample"
        );
        assert_eq!(
            f32::from_bits(a.0[i + 1].3),
            -1.0,
            "the bend starts a tone down"
        );
        assert!(mine.iter().all(|e| e.0 >= on.0 && e.0 < on.0 + 24_000));
        // The bend's point at 200 ticks is at its own sample, exactly 2 semitones.
        assert!(mine
            .iter()
            .any(|e| e.1 == 1 && e.0 == on.0 + 5_000 && f32::from_bits(e.3) == 2.0));
    }
}

/// The frequency of the left channel between two times (positive zero crossings).
fn frequency(x: &[f32], from: f64, to: f64) -> f64 {
    let (a, b) = ((from * SR) as usize, (to * SR) as usize);
    let (mut first, mut last, mut count) = (None, 0.0, 0usize);
    for i in a..b {
        let (y0, y1) = (x[2 * i] as f64, x[2 * i + 2] as f64);
        if y0 < 0.0 && y1 >= 0.0 {
            let t = i as f64 + y0 / (y0 - y1);
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    count as f64 * SR / (last - first.expect("crossings"))
}

/// Unda, Calor, Modulus and Gravis bend by what the curve says: a long A2 held still for
/// a beat, then bent a fifth up and held — the frequency measured in each stretch.
#[test]
fn every_listening_instrument_bends_by_the_curve() {
    for kind in KINDS {
        let mut p = Project::new("bend");
        p.tempo_bpm = 120.0;
        let mut t = p.make_track("t");
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        let mut n = p.make_note(Ticks::ZERO, Ticks::beats(4), 45, 100);
        n.expr = NoteExpr {
            bend: vec![ExprPoint(960, 0), ExprPoint(1200, st(7.0))],
            ..NoteExpr::default()
        }
        .boxed();
        clip.notes.push(n);
        t.clips.push(clip);
        p.tracks.push(t);
        let out = offline(&p, 256);
        let still = frequency(&out, 0.15, 0.45);
        let bent = frequency(&out, 0.9, 1.6);
        let cents = 1200.0 * math::log2(bent / still);
        println!("{kind:?}: {still:.2} Hz → {bent:.2} Hz, {cents:+.1} ct");
        // Measured 700.0 (Unda, Modulus, Gravis) and 698.2 (Calor: its patch drifts).
        assert!((cents - 700.0).abs() < 2.0, "{kind:?}: {cents:+.1} ct");
    }
}

/// How many note-offs a log holds.
fn log_offs(a: &[(u64, u8, u64, u32)]) -> usize {
    a.iter().filter(|e| e.1 == 8).count()
}

/// A rig whose live track is an ear, MPE set as given.
fn live_rig(
    zones: Option<cantus_core::model::note_expr::zones::MpeZones>,
) -> (
    rtrb::Producer<EngineMessage>,
    rtrb::Consumer<EngineEvent>,
    Processor,
    Arc<Mutex<Log>>,
) {
    live_rig_with(zones, 8192)
}

/// [`live_rig`] with an event ring of `ring` slots.
fn live_rig_with(
    zones: Option<cantus_core::model::note_expr::zones::MpeZones>,
    ring: usize,
) -> (
    rtrb::Producer<EngineMessage>,
    rtrb::Consumer<EngineEvent>,
    Processor,
    Arc<Mutex<Log>>,
) {
    let mut p = Project::new("live");
    let mut t = p.make_track("t");
    t.instrument = InstrumentSpec::Native(NativeSpec::new(KINDS[0]));
    let track = t.id;
    p.tracks.push(t);
    let log = Arc::new(Mutex::new(Log::default()));
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(512);
    let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(ring);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    tx.push(EngineMessage::AddInstrument(Box::new(NativeUnit {
        track,
        kind: UnitKind::Native(KINDS[0]),
        serial: 1,
        unit: Box::new(Ear {
            log: log.clone(),
            at: 0,
            offsets: false,
        }),
        automation: Default::default(),
    })))
    .unwrap();
    tx.push(EngineMessage::SetSnapshot(snap(&p))).unwrap();
    tx.push(EngineMessage::SetLiveTrack(Some(track))).unwrap();
    tx.push(EngineMessage::SetMpe(zones)).unwrap();
    let mut buf = vec![0f32; 128];
    proc.process(&mut buf, 2);
    (tx, rx, proc, log)
}

fn bytes(tx: &mut rtrb::Producer<EngineMessage>, msgs: &[[u8; 3]]) {
    for m in msgs {
        tx.push(EngineMessage::Midi(MidiEvent {
            status: m[0],
            data1: m[1],
            data2: m[2],
        }))
        .unwrap();
    }
}

/// A Seaboard-style stream (MPE 1.1's examples): the MCM for a lower zone of 15, then two
/// overlapping notes on member channels 2 and 3, each with its bend and CC 74 before its
/// note-on, pressure after, a bend while held. Each note is its own voice, its values its
/// own; the bend at ±48 semitones; a bend range RPN on a member channel is heard.
#[test]
fn an_mpe_controller_plays_each_note_with_its_own_expression() {
    use cantus_core::model::note_expr::zones::MpeZones;
    let (mut tx, _rx, mut proc, log) = live_rig(Some(MpeZones::default()));
    let mut block = |tx: &mut rtrb::Producer<EngineMessage>, m: &[[u8; 3]]| {
        bytes(tx, m);
        let mut buf = vec![0f32; 128];
        proc.process(&mut buf, 2);
    };
    // The layout is off until the controller says (an empty layout set): the MCM turns it on.
    block(&mut tx, &[[0xB0, 101, 0], [0xB0, 100, 6], [0xB0, 6, 15]]);
    block(
        &mut tx,
        &[
            [0xE1, 0, 64],
            [0xB1, 74, 64],
            [0x91, 60, 100],
            [0xD1, 80, 0],
            [0xE2, 0, 32],
            [0x92, 64, 90],
            [0xE1, 0, 80],
        ],
    );
    block(&mut tx, &[[0x81, 60, 64], [0x82, 64, 0]]);
    let a = std::mem::take(&mut log.lock().unwrap().0);
    let ons: Vec<_> = a.iter().filter(|e| e.1 == 0).collect();
    assert_eq!(ons.len(), 2, "{a:?}");
    assert_ne!(ons[0].2, ons[1].2, "a voice each");
    assert_eq!((ons[0].3, ons[1].3), (1, 2), "their member channels");
    let of = |id: u64, what: u8| -> Vec<f32> {
        a.iter()
            .filter(|e| e.2 == id && e.1 == what)
            .map(|e| f32::from_bits(e.3))
            .collect()
    };
    // Note 1: centred, then (80 << 7) − 8192 = 2048 steps up of 8192 at ±48: +12.
    assert_eq!(of(ons[0].2, 1), vec![0.0, 12.0]);
    // CC 74 at 64 is the middle exactly (Min-Center-Max).
    assert_eq!(of(ons[0].2, 3), vec![0.5]);
    let p80 = ExprKind::Pressure.read(cantus_core::model::note_expr::midi2::unit_of_7(80)) as f32;
    assert_eq!(of(ons[0].2, 2), vec![p80]);
    // Note 2: started bent down a quarter of the range, −24; no timbre was sent for it.
    assert_eq!(of(ons[1].2, 1), vec![-24.0]);
    assert!(of(ons[1].2, 3).is_empty());
    // A member's bend range set to 24 (RPN 0): the same throw is half the bend.
    block(
        &mut tx,
        &[
            [0xB3, 101, 0],
            [0xB3, 100, 0],
            [0xB3, 6, 24],
            [0x93, 62, 90],
            [0xE3, 0, 80],
        ],
    );
    let b = std::mem::take(&mut log.lock().unwrap().0);
    let bends: Vec<f32> = b
        .iter()
        .filter(|e| e.1 == 1)
        .map(|e| f32::from_bits(e.3))
        .collect();
    assert_eq!(bends, vec![0.0, 6.0]);
}

/// MPE off: member channel traffic is what it always was — notes on the live key's id,
/// no expression; a project without MPE ports hears nothing new.
#[test]
fn without_mpe_the_channels_mean_nothing_new() {
    let (mut tx, _rx, mut proc, log) = live_rig(None);
    bytes(
        &mut tx,
        &[[0xE1, 0, 80], [0x91, 60, 100], [0xD1, 80, 0], [0x81, 60, 0]],
    );
    let mut buf = vec![0f32; 128];
    proc.process(&mut buf, 2);
    let a = std::mem::take(&mut log.lock().unwrap().0);
    assert_eq!(a.iter().filter(|e| e.1 == 0).count(), 1);
    assert_eq!(
        a.iter().find(|e| e.1 == 0).unwrap().3,
        0,
        "channel 0 as before"
    );
    assert!(
        a.iter().all(|e| e.1 == 0 || e.1 == 8),
        "no per-note expression: {a:?}"
    );
}

/// While recording, every member value is reported with its note's key and the tick its
/// note-on was heard at — what the interface binds it by.
#[test]
fn recording_reports_each_value_with_its_note() {
    use cantus_core::model::note_expr::zones::MpeZones;
    let (mut tx, mut rx, mut proc, _log) = live_rig(Some(MpeZones::standard()));
    tx.push(EngineMessage::SetRecording(true)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 2048];
    proc.process(&mut buf, 2);
    bytes(&mut tx, &[[0x91, 60, 100], [0xE1, 0, 80]]);
    proc.process(&mut buf, 2);
    bytes(&mut tx, &[[0xD1, 64, 0], [0x81, 60, 0]]);
    proc.process(&mut buf, 2);
    let mut notes = 0;
    let mut exprs = Vec::new();
    while let Ok(ev) = rx.pop() {
        match ev {
            EngineEvent::NoteInput(_) => notes += 1,
            EngineEvent::ExprInput(e) => exprs.push(e),
            _ => {}
        }
    }
    use cantus_engine::inst::mpe::MpeIn;
    assert_eq!(
        notes, 0,
        "an MPE note is recorded by its channel, not as a plain note"
    );
    assert_eq!(exprs.len(), 5, "{exprs:?}");
    let on = exprs[0];
    assert_eq!(on.what, MpeIn::On { velocity: 100 });
    assert!(exprs
        .iter()
        .all(|e| e.pitch == 60 && e.channel == 1 && e.start == on.tick));
    assert_eq!(
        exprs[1].what,
        MpeIn::Value {
            kind: ExprKind::Bend,
            value: 0
        },
        "in tune"
    );
    assert_eq!(
        exprs[2].what,
        MpeIn::Value {
            kind: ExprKind::Bend,
            value: ExprKind::Bend.store(12.0)
        }
    );
    assert!(matches!(
        exprs[3].what,
        MpeIn::Value {
            kind: ExprKind::Pressure,
            ..
        }
    ));
    assert_eq!(exprs[4].what, MpeIn::Off { release: 0 });
    assert!(exprs[3].tick > on.tick);
}

/// Review fix 9: a panic on the member channels lets every MPE note go (and the next note
/// on a channel is not a stale one's), and once the MCM is done with, a later data entry
/// on channel 1 (another port's CC 6) is not read as a new MCM.
#[test]
fn a_panic_releases_mpe_notes_and_a_stray_data_entry_is_no_mcm() {
    use cantus_core::model::note_expr::zones::MpeZones;
    let (mut tx, _rx, mut proc, log) = live_rig(Some(MpeZones::default()));
    let mut block = |tx: &mut rtrb::Producer<EngineMessage>, m: &[[u8; 3]]| {
        bytes(tx, m);
        let mut buf = vec![0f32; 128];
        proc.process(&mut buf, 2);
    };
    // The MCM without its null RPN, two notes, then a keyboard's CC 6 moved to channel 1.
    block(
        &mut tx,
        &[
            [0xB0, 101, 0],
            [0xB0, 100, 6],
            [0xB0, 6, 15],
            [0x91, 60, 100],
            [0x92, 64, 100],
        ],
    );
    block(&mut tx, &[[0xB0, 6, 3], [0x95, 67, 100]]);
    let a = std::mem::take(&mut log.lock().unwrap().0);
    let ons: Vec<u32> = a.iter().filter(|e| e.1 == 0).map(|e| e.3).collect();
    assert_eq!(
        ons,
        vec![1, 2, 5],
        "channel 6 is still a member: the layout did not change"
    );
    // A panic on every channel.
    let panic: Vec<[u8; 3]> = (0..16u8).map(|c| [0xB0 | c, 123, 0]).collect();
    block(&mut tx, &panic);
    let b = std::mem::take(&mut log.lock().unwrap().0);
    assert_eq!(b.iter().filter(|e| e.1 == 0).count(), 0);
    let offs = log_offs(&b);
    assert_eq!(offs, 3, "the three MPE notes released: {b:?}");
}

/// A fresh instrument's MPE routes (`note_expr::defaults`): without expression they add
/// nothing, to the bit; with pressure and timbre they are heard.
#[test]
fn the_default_routes_are_silent_without_expression_and_heard_with_it() {
    use cantus_core::model::note_expr::defaults::with_routes;
    for kind in [
        InstrumentKind::Wavetable,
        InstrumentKind::Analog,
        InstrumentKind::Fm,
    ] {
        let build = |spec: NativeSpec, expr: Option<NoteExpr>| {
            let mut p = Project::new("routes");
            p.tempo_bpm = 120.0;
            let mut t = p.make_track("t");
            t.instrument = InstrumentSpec::Native(spec);
            let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(2));
            let mut n = p.make_note(Ticks::ZERO, Ticks::beats(2), 57, 100);
            n.expr = expr.and_then(NoteExpr::boxed);
            clip.notes.push(n);
            t.clips.push(clip);
            p.tracks.push(t);
            offline(&p, 256)
        };
        let plain = build(NativeSpec::new(kind), None);
        let routed = build(with_routes(NativeSpec::new(kind)), None);
        assert_null(
            &plain,
            &routed,
            0.0,
            &format!("{kind:?}: routes without expression"),
        );
        let pressed = NoteExpr {
            pressure: vec![ExprPoint(0, 0), ExprPoint(1920, u(1.0))],
            timbre: vec![ExprPoint(0, 0), ExprPoint(1920, u(1.0))],
            ..NoteExpr::default()
        };
        let heard = build(with_routes(NativeSpec::new(kind)), Some(pressed));
        let diff = heard
            .iter()
            .zip(&plain)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        println!("{kind:?}: pressure and timbre move the sound by up to {diff:.3}");
        assert!(diff > 0.01, "{kind:?}: {diff}");
    }
}

/// Review fix 5: in mono, a note that returns to a held one (Unda, Calor, Modulus) brings
/// none of the released note's expression with it — the held A is in tune again once the
/// bent B is let go (as Gravis always did).
#[test]
fn a_mono_voice_returning_to_a_held_note_forgets_the_released_ones_bend() {
    for (kind, mode_key) in [
        (InstrumentKind::Wavetable, "voice.mode"),
        (InstrumentKind::Analog, "voice.mode"),
        (InstrumentKind::Fm, "mode"),
    ] {
        let build = |with_b: bool| {
            let mut p = Project::new("mono");
            p.tempo_bpm = 120.0;
            let mut spec = NativeSpec::new(kind);
            assert!(spec.set(mode_key, 1.0).is_some(), "{kind:?}: {mode_key}");
            let mut t = p.make_track("t");
            t.instrument = InstrumentSpec::Native(spec);
            let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
            clip.notes
                .push(p.make_note(Ticks::ZERO, Ticks::beats(4), 45, 100));
            if with_b {
                let mut b = p.make_note(Ticks::beats(1), Ticks::beats(1), 52, 100);
                b.expr = NoteExpr {
                    bend: vec![ExprPoint(0, st(3.0))],
                    ..NoteExpr::default()
                }
                .boxed();
                clip.notes.push(b);
            }
            t.clips.push(clip);
            p.tracks.push(t);
            offline(&p, 256)
        };
        let alone = frequency(&build(false), 1.4, 1.9);
        let after = frequency(&build(true), 1.4, 1.9);
        let cents = 1200.0 * math::log2(after / alone);
        println!("{kind:?}: A after B {cents:+.2} ct");
        assert!(
            cents.abs() < 5.0,
            "{kind:?}: A is {cents:+.1} ct off after B"
        );
    }
}

/// Review D7: another port's panic (tagged by its callback) lets go of the plain live notes
/// only — the MPE controller's notes sound on.
#[test]
fn another_ports_panic_spares_the_mpe_notes() {
    use cantus_core::model::note_expr::zones::MpeZones;
    let (mut tx, _rx, mut proc, log) = live_rig(Some(MpeZones::standard()));
    let tag = cantus_engine::inst::mpe::PLAIN_PANIC;
    bytes(
        &mut tx,
        &[
            [0x91, 60, 100],
            [0x92, 64, 100],
            [0x90, 48, 100],
            [0xB0, 123, tag],
        ],
    );
    let mut buf = vec![0f32; 128];
    proc.process(&mut buf, 2);
    let a = std::mem::take(&mut log.lock().unwrap().0);
    let ons: Vec<(u64, u32)> = a.iter().filter(|e| e.1 == 0).map(|e| (e.2, e.3)).collect();
    let plain = ons.iter().find(|o| o.1 == 0).expect("the plain note").0;
    let offs: Vec<u64> = a.iter().filter(|e| e.1 == 8).map(|e| e.2).collect();
    assert!(offs.contains(&plain), "the plain note is let go: {a:?}");
    for (id, ch) in ons.iter().filter(|o| o.1 != 0) {
        assert!(!offs.contains(id), "the MPE note on channel {ch} sounds on");
    }
}

/// Review D8: with the event ring nearly full, values give way and a note's on and off are
/// still reported.
#[test]
fn a_full_event_ring_keeps_the_notes_ons_and_offs() {
    use cantus_core::model::note_expr::zones::MpeZones;
    use cantus_engine::inst::mpe::MpeIn;
    let (mut tx, mut rx, mut proc, _log) = live_rig_with(Some(MpeZones::standard()), 120);
    tx.push(EngineMessage::SetRecording(true)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0f32; 128];
    proc.process(&mut buf, 2);
    while rx.pop().is_ok() {}
    let mut msgs = vec![[0x91, 60, 100]];
    msgs.extend((0..300u16).map(|i| [0xE1, (i % 128) as u8, 64 + (i / 128) as u8]));
    for chunk in msgs.chunks(400) {
        bytes(&mut tx, chunk);
    }
    proc.process(&mut buf, 2);
    bytes(&mut tx, &[[0x81, 60, 0]]);
    proc.process(&mut buf, 2);
    let mut whats = Vec::new();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::ExprInput(e) = ev {
            whats.push(e.what);
        }
    }
    assert!(matches!(whats.first(), Some(MpeIn::On { .. })), "{whats:?}");
    assert!(
        matches!(whats.last(), Some(MpeIn::Off { .. })),
        "{} events",
        whats.len()
    );
}
