//! The master's insert chain (ADR 0026): it runs on the summed mix before the master's
//! fader, balance and width; empty or bypassed it touches no sample; its latency moves
//! what is heard and nothing inside the mix; a strip can key it; the master take is
//! post-chain.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, peak};
use cantus_engine::*;
use std::sync::Arc;
use support::*;

const DELAY: usize = 100;

/// A pure delay of `n` samples that says so.
#[derive(Debug)]
struct PureDelay {
    l: Vec<f32>,
    r: Vec<f32>,
    pos: usize,
}

impl PureDelay {
    fn new(n: usize) -> Self {
        PureDelay {
            l: vec![0.0; n],
            r: vec![0.0; n],
            pos: 0,
        }
    }
}

impl FxUnit for PureDelay {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, l: &mut [f32], r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        for i in 0..l.len() {
            let (ol, or) = (self.l[self.pos], self.r[self.pos]);
            self.l[self.pos] = l[i];
            self.r[self.pos] = r[i];
            self.pos = (self.pos + 1) % self.l.len();
            l[i] = ol;
            r[i] = or;
        }
    }
    fn reset(&mut self) {
        self.l.iter_mut().for_each(|x| *x = 0.0);
        self.r.iter_mut().for_each(|x| *x = 0.0);
    }
    fn latency(&self) -> usize {
        self.l.len()
    }
}

/// A processor for a project whose `latent` slots (on strips or the master) are pure
/// delays of [`DELAY`], with a capture ring, settled the way an offline render is.
struct Rig {
    tx: rtrb::Producer<EngineMessage>,
    rx: rtrb::Consumer<EngineEvent>,
    cap: rtrb::Consumer<f32>,
    p: Processor,
    events: Vec<EngineEvent>,
    took: Vec<f32>,
}

impl Rig {
    fn new(project: &Project, latent: &[EffectId]) -> Self {
        Rig::with(project, latent, DELAY, false)
    }

    /// [`Rig::new`] with delays of `delay` samples, and the project's loop when `looped`.
    fn with(project: &Project, latent: &[EffectId], delay: usize, looped: bool) -> Self {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let (cap_tx, cap) = rtrb::RingBuffer::<f32>::new(CAPTURE_SAMPLES);
        let mut p = Processor::without_midi(48_000.0, rx_engine, tx_engine);
        p.attach_capture(cap_tx);
        let mut rig = Rig {
            tx: {
                let mut snap = Snapshot::compile(project, 1);
                if !looped {
                    snap.loop_region = None;
                }
                for slot in snap.effect_slots() {
                    let unit = if latent.contains(&slot.id) {
                        EffectUnit::from_fx(slot, Box::new(PureDelay::new(delay)))
                    } else {
                        EffectUnit::new(slot, 48_000.0)
                    };
                    tx.push(EngineMessage::AddEffect(Box::new(unit))).unwrap();
                }
                tx.push(EngineMessage::SetSnapshot(Arc::new(snap))).unwrap();
                tx
            },
            rx,
            cap,
            p,
            events: Vec::new(),
            took: Vec::new(),
        };
        rig.settle();
        rig
    }

    /// Answer requests for lines and memory before the first block, as a render does.
    fn settle(&mut self) {
        self.p.settle();
        loop {
            let mut answered = false;
            while let Ok(ev) = self.rx.pop() {
                match ev {
                    EngineEvent::PdcNeed { count, delay } => {
                        let lines = cantus_engine::pdc::lines_for(count, delay);
                        self.tx.push(EngineMessage::PdcLines(lines)).unwrap();
                        answered = true;
                    }
                    other => self.events.push(other),
                }
            }
            if !answered {
                break;
            }
            self.p.settle();
        }
    }

    fn send(&mut self, m: EngineMessage) {
        self.tx.push(m).unwrap();
    }

    /// Play `frames` frames in irregular blocks; returns the output.
    fn play(&mut self, frames: usize) -> Vec<f32> {
        let sizes = [128usize, 333, 512, 96, 1024, 17];
        let mut out = Vec::new();
        let mut i = 0;
        while out.len() / 2 < frames {
            let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            self.p.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while let Ok(ev) = self.rx.pop() {
                self.events.push(ev);
            }
            while let Ok(x) = self.cap.pop() {
                self.took.push(x);
            }
        }
        out
    }

    fn last_position(&self) -> Option<(Ticks, Ticks)> {
        self.events.iter().rev().find_map(|e| match e {
            EngineEvent::Position { tick, heard, .. } => Some((*tick, *heard)),
            _ => None,
        })
    }

    fn last_meters(&self) -> Option<&Meters> {
        self.events.iter().rev().find_map(|e| match e {
            EngineEvent::Meters(m) => Some(m),
            _ => None,
        })
    }

    /// The deepest master reduction any block reported.
    fn deepest_gr(&self) -> f32 {
        self.events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::Meters(m) => Some(m.master_gr),
                _ => None,
            })
            .fold(0.0, f32::max)
    }
}

fn on_master(p: &mut Project, kind: EffectKind) -> EffectId {
    let slot = p.make_effect(kind);
    let id = slot.id;
    p.master.effects.push(slot);
    id
}

fn db(x: f32) -> f32 {
    20.0 * math::log10(x.max(1e-12))
}

/// The kinds with no latency at their defaults: bypassed, they are wires.
const WIRES: [EffectKind; 5] = [
    EffectKind::Eq,
    EffectKind::Compressor,
    EffectKind::Delay,
    EffectKind::Reverb,
    EffectKind::Gate,
];

#[test]
fn an_empty_or_bypassed_master_chain_is_bit_exact() {
    let base = render_of(&demo_project());
    assert!(peak(&base) > 0.05);
    // Empty, with the chain switched off: nothing changes.
    let mut p = demo_project();
    p.master.fx_enabled = false;
    assert_null(&render_of(&p), &base, 0.0, "an empty chain switched off");
    // Every zero-latency kind, bypassed.
    let mut p = demo_project();
    for kind in WIRES {
        let id = on_master(&mut p, kind);
        p.master
            .effects
            .iter_mut()
            .find(|e| e.id == id)
            .unwrap()
            .bypass = true;
    }
    assert_null(&render_of(&p), &base, 0.0, "a chain of bypassed inserts");
    // The same chain in place but switched off.
    for e in &mut p.master.effects {
        e.bypass = false;
    }
    p.master.fx_enabled = false;
    assert_null(&render_of(&p), &base, 0.0, "a chain switched off");
    // At mix 0 they run and are not heard.
    p.master.fx_enabled = true;
    for e in &mut p.master.effects {
        e.mix = 0.0;
    }
    assert_null(&render_of(&p), &base, 0.0, "a chain at 0 % mix");
}

/// A latent insert on the master, bypassed, is a soft bypass (ADR 0022): the mix comes
/// out exactly as before, only later by the insert's latency, and the engine says so.
#[test]
fn a_bypassed_latent_master_insert_delays_the_mix_exactly() {
    let frames = 96_000;
    let mut rig = Rig::new(&demo_project(), &[]);
    rig.send(EngineMessage::Play);
    let base = rig.play(frames);
    let mut p = demo_project();
    let id = on_master(&mut p, EffectKind::Limiter);
    p.master.effects[0].bypass = true;
    let mut rig = Rig::new(&p, &[id]);
    rig.send(EngineMessage::Play);
    let out = rig.play(frames);
    assert_eq!(rig.p.stats().latency_samples as usize, DELAY);
    assert_null(
        &out[DELAY * 2..],
        &base[..base.len() - DELAY * 2],
        0.0,
        "the bypassed latent insert is its dry line",
    );
    assert_eq!(peak(&out[..DELAY * 2]), 0.0);
}

#[test]
fn the_chain_runs_before_the_master_fader() {
    let mut p = demo_project();
    let id = on_master(&mut p, EffectKind::Limiter);
    {
        let e = p.effect_mut(StripId::MASTER, id).unwrap();
        e.spec.set("ceiling", -6.0).unwrap();
        e.spec.set("gain", 12.0).unwrap();
    }
    let at_unity = db(peak(&render_of(&p)));
    p.master.gain_db = 6.0;
    let boosted = db(peak(&render_of(&p)));
    // At unity the limiter's ceiling is the output's; with the master 6 dB up after it,
    // the peaks are 6 dB higher — 0 dBFS, not −6.
    assert!(
        (-6.6..=-5.99).contains(&at_unity),
        "the limiter holds the mix at its ceiling: {at_unity:.2} dBFS"
    );
    assert!(
        (-0.6..=0.01).contains(&boosted),
        "the fader comes after the chain: {boosted:.2} dBFS"
    );
    assert!((boosted - at_unity - 6.0).abs() < 0.01);
}

#[test]
fn a_master_chain_renders_the_same_at_any_block_size() {
    let mut p = demo_project();
    let lim = on_master(&mut p, EffectKind::Limiter);
    p.effect_mut(StripId::MASTER, lim)
        .unwrap()
        .spec
        .set("gain", 9.0)
        .unwrap();
    let comp = on_master(&mut p, EffectKind::Compressor);
    p.effect_mut(StripId::MASTER, comp).unwrap().mix = 0.5;
    on_master(&mut p, EffectKind::Reverb);
    let a = render_with(&p, 256);
    assert!(peak(&a) > 0.05);
    for block in [7, 64, 1000, 4096] {
        assert_null(&render_with(&p, block), &a, 0.0, &format!("block {block}"));
    }
    // And it changed the mix (it is not silently skipped).
    let base = render_of(&demo_project());
    assert!(peak(&a) > 0.05 && a != base);
}

/// Latency on the master delays every path equally: nothing inside the mix moves, the
/// output is the mix later by exactly that, and the heard position allows for it.
#[test]
fn latency_on_the_master_moves_the_heard_position_by_exactly_that() {
    let frames = 48_000;
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    let base = rig.play(frames);
    let (tick_at_end, heard) = rig.last_position().unwrap();
    assert_eq!(
        heard, tick_at_end,
        "no latency: heard is the engine's position"
    );
    let mut early = Rig::new(&p, &[]);
    early.send(EngineMessage::Play);
    early.play(frames - DELAY);
    let (tick_earlier, _) = early.last_position().unwrap();

    let id = on_master(&mut p, EffectKind::Eq);
    let mut rig = Rig::new(&p, &[id]);
    assert_eq!(rig.p.stats().latency_samples as usize, DELAY);
    rig.send(EngineMessage::Play);
    let out = rig.play(frames);
    assert_null(
        &out[DELAY * 2..],
        &base[..base.len() - DELAY * 2],
        0.0,
        "the mix, later by the master's latency",
    );
    let (tick, heard) = rig.last_position().unwrap();
    assert_eq!(tick, tick_at_end, "the engine's own position does not move");
    assert_eq!(
        heard, tick_earlier,
        "what is heard is where the engine was {DELAY} samples ago"
    );
    // With latency on a strip as well, the two add up.
    let s0 = p.tracks[0].strip.unwrap();
    let slot = p.make_effect(EffectKind::Eq);
    let sid = slot.id;
    p.strip_mut(s0).unwrap().effects.push(slot);
    let rig = Rig::new(&p, &[id, sid]);
    assert_eq!(rig.p.stats().latency_samples as usize, 2 * DELAY);
}

/// Review of fix5: the heard playhead folds back through the loop's wraps as a late
/// insert's song position does — never through passes that were never played, never on a
/// first entry. A one-beat loop (8–9 at 120 BPM, 24 000 samples) behind 60 000 samples of
/// latency, 6 000 samples after its first wrap: the song before the loop, beat 6.75 (the
/// old fold said 8.75); after a second wrap, the pass before and then the lead-in. And a
/// tempo change on the first entry into a loop (beats 8–16) does not fold: 7.6, not 15.6.
#[test]
fn the_heard_position_folds_only_through_the_wraps_played() {
    let beats = |t: f64| t / PPQ as f64;
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(8),
        end: Ticks::beats(9),
    });
    p.loop_enabled = true;
    let id = on_master(&mut p, EffectKind::Eq);
    let mut rig = Rig::with(&p, &[id], 60_000, true);
    assert_eq!(rig.p.stats().latency_samples, 60_000);
    rig.send(EngineMessage::Play);
    rig.play(9 * 24_000 + 6_000);
    let heard = beats(rig.p.heard_position_f64());
    assert!((heard - 6.75).abs() < 1e-6, "one wrap: {heard}");
    rig.play(24_000);
    let heard = beats(rig.p.heard_position_f64());
    assert!((heard - 7.75).abs() < 1e-6, "two wraps: {heard}");
    rig.play(2 * 24_000);
    let heard = beats(rig.p.heard_position_f64());
    assert!((heard - 8.75).abs() < 1e-6, "four wraps: {heard}");

    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(8),
        end: Ticks::beats(16),
    });
    let mut rig = Rig::with(&p, &[id], 12_000, true);
    rig.send(EngineMessage::Play);
    rig.play(8 * 24_000 + 2_400);
    let mut faster = p.clone();
    faster.tempo_bpm = 121.0;
    rig.send(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &faster, 2,
    ))));
    rig.play(1);
    let heard = beats(rig.p.heard_position_f64());
    assert!(
        heard < 8.0 && (heard - 7.6).abs() < 0.01,
        "a first entry: {heard}"
    );
}

/// Review of fix5: a wrap the engine forces counts, through the processor — the loop's end
/// moved behind the playing playhead (beat 8½ in a loop 8–9 that becomes 8–8¼) wraps at
/// the next pass, and what is heard half a beat later is where the playhead was then,
/// played before the edit (8½ less 11 000 samples), not the song before the loop (the
/// wraps forgotten, as `seek` did: 7.54) nor a fold into the new loop.
#[test]
fn a_forced_wrap_is_heard_where_the_playhead_was() {
    let beats = |t: f64| t / PPQ as f64;
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(8),
        end: Ticks::beats(9),
    });
    p.loop_enabled = true;
    let id = on_master(&mut p, EffectKind::Eq);
    let mut rig = Rig::with(&p, &[id], 12_000, true);
    rig.send(EngineMessage::Play);
    rig.play(9 * 24_000 + 12_000);
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(8),
        end: Ticks::beats(8) + Ticks(PPQ / 4),
    });
    rig.send(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
        &p, 2,
    ))));
    rig.play(1_000);
    let heard = beats(rig.p.heard_position_f64());
    let want = 8.5 - 11_000.0 / 24_000.0;
    assert!((heard - want).abs() < 1e-6, "heard {heard}, want {want}");
}

/// Review of fix6: the wraps played stay played when the loop changes. A loop 8–9 behind
/// 12 000 samples of latency, turned off — or moved ahead of the playhead, to 12–13 —
/// 1 000 samples after a wrap: what is heard is still the pass before the wrap, beat 9
/// less 11 000 samples (≈ 8.54), not the song straight back from the playhead (≈ 7.54:
/// the wraps hidden because no loop lay ahead). Once the latency has passed the wrap, the
/// song straight from the loop's start.
#[test]
fn the_heard_position_keeps_the_wraps_played_when_the_loop_changes() {
    let beats = |t: f64| t / PPQ as f64;
    let ahead = LoopRegion {
        start: Ticks::beats(12),
        end: Ticks::beats(13),
    };
    for (what, region, enabled) in [
        ("the loop turned off", None, false),
        ("the loop moved ahead", Some(ahead), true),
    ] {
        let mut p = demo_project();
        p.tempo_bpm = 120.0;
        p.loop_region = Some(LoopRegion {
            start: Ticks::beats(8),
            end: Ticks::beats(9),
        });
        p.loop_enabled = true;
        let id = on_master(&mut p, EffectKind::Eq);
        let mut rig = Rig::with(&p, &[id], 12_000, true);
        rig.send(EngineMessage::Play);
        rig.play(9 * 24_000 + 1_000);
        if region.is_some() {
            p.loop_region = region.clone();
        }
        p.loop_enabled = enabled;
        rig.send(EngineMessage::SetSnapshot(Arc::new(Snapshot::compile(
            &p, 2,
        ))));
        rig.play(1);
        let heard = beats(rig.p.heard_position_f64());
        let want = 9.0 - 10_999.0 / 24_000.0;
        assert!(
            (heard - want).abs() < 1e-6,
            "{what}: heard {heard}, want {want}"
        );
        rig.play(12_000);
        let heard = beats(rig.p.heard_position_f64());
        let want = 8.0 + 1_001.0 / 24_000.0;
        assert!(
            (heard - want).abs() < 1e-6,
            "{what}, later: heard {heard}, want {want}"
        );
    }
}

/// The click goes through a line of the whole output's latency, the master's included, so
/// a beat's click still comes out with that beat.
#[test]
fn the_click_waits_for_the_master_chain() {
    let mut p = Project::new("click");
    p.tempo_bpm = 120.0;
    p.metronome = true;
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    let dry = rig.play(30_000);
    let id = on_master(&mut p, EffectKind::Eq);
    let mut rig = Rig::new(&p, &[id]);
    rig.send(EngineMessage::Play);
    let late = rig.play(30_000);
    assert!(peak(&dry) > 0.01);
    assert_null(
        &late[DELAY * 2..],
        &dry[..dry.len() - DELAY * 2],
        0.0,
        "the click is later by the master's latency",
    );
}

/// A strip's take is delayed to the output's time — the master chain's latency included —
/// and the master's take is the output, after the chain.
#[test]
fn takes_line_up_with_a_latent_master_chain() {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    let s1 = p.tracks[1].strip.unwrap();
    let id = on_master(&mut p, EffectKind::Eq);
    let mut rig = Rig::new(&p, &[id]);
    rig.send(EngineMessage::SetCapture(CaptureTarget::Strip(s1)));
    rig.send(EngineMessage::Play);
    rig.play(96_000);
    let take = std::mem::take(&mut rig.took);
    // What the bass alone sounds like through the master.
    let mut alone = p.clone();
    alone.tracks[0].muted = true;
    let mut rig = Rig::new(&alone, &[id]);
    rig.send(EngineMessage::Play);
    let render = rig.play(96_000);
    assert!(peak(&render) > 0.05);
    assert_eq!(take.len(), render.len());
    assert_null(
        &take,
        &render,
        0.0,
        "the strip's take is in time with the output",
    );
    // The master's take is post-chain: a limiter at -6 dB shows in it.
    let mut p = demo_project();
    let lim = on_master(&mut p, EffectKind::Limiter);
    {
        let e = p.effect_mut(StripId::MASTER, lim).unwrap();
        e.spec.set("ceiling", -6.0).unwrap();
        e.spec.set("gain", 12.0).unwrap();
    }
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::SetCapture(CaptureTarget::Master));
    rig.send(EngineMessage::Play);
    let out = rig.play(96_000);
    assert_null(&rig.took, &out, 0.0, "the master take is the output");
    assert!(db(peak(&rig.took)) < -5.9);
}

/// Any strip can key a master insert (stem-aware limiting), and the master is processed
/// after every strip, so the key is that block's.
#[test]
fn a_strip_keys_a_master_compressor() {
    let mut p = demo_project();
    let bass = p.tracks[1].strip.unwrap();
    let comp = on_master(&mut p, EffectKind::Compressor);
    {
        let e = p.effect_mut(StripId::MASTER, comp).unwrap();
        e.spec.set("thr", -40.0).unwrap();
        e.spec.set("ratio", 8.0).unwrap();
    }
    let unkeyed = render_of(&p);
    p.effect_mut(StripId::MASTER, comp).unwrap().sidechain = Some(bass);
    let keyed = render_with(&p, 256);
    assert!(peak(&keyed) > 0.01);
    assert_ne!(keyed, unkeyed, "the key changes what the compressor does");
    for block in [17, 1000] {
        assert_null(
            &render_with(&p, block),
            &keyed,
            0.0,
            &format!("block {block}"),
        );
    }
}

/// The master's gain reduction reaches the meters, from the chain's dynamics.
#[test]
fn the_master_reports_its_gain_reduction() {
    let mut p = demo_project();
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    rig.play(24_000);
    assert_eq!(rig.last_meters().unwrap().master_gr, 0.0);
    let lim = on_master(&mut p, EffectKind::Limiter);
    {
        let e = p.effect_mut(StripId::MASTER, lim).unwrap();
        e.spec.set("gain", 18.0).unwrap();
        e.spec.set("ceiling", -12.0).unwrap();
    }
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    rig.play(24_000);
    let gr = rig.deepest_gr();
    assert!(gr > 6.0, "a limiter driven 18 dB into -12 dB reduces: {gr}");
    // Bypassed, it reduces nothing that is heard.
    p.master.effects[0].bypass = true;
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    rig.play(24_000);
    assert_eq!(rig.deepest_gr(), 0.0);
}

/// Culmen on the master shows on the master's reduction meter as Terminus does: the
/// brickwall's reduction (or a band's cut), positive dB; at rest under its ceiling, none.
#[test]
fn a_maximizer_on_the_master_shows_on_its_reduction_meter() {
    let mut p = demo_project();
    let max = on_master(&mut p, EffectKind::Maximizer);
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    rig.play(24_000);
    let quiet = rig.deepest_gr();
    assert!(quiet < 0.5, "a default Culmen on a quiet mix: {quiet}");
    {
        let e = p.effect_mut(StripId::MASTER, max).unwrap();
        e.spec.set("gain", 18.0).unwrap();
        e.spec.set("ceiling", -12.0).unwrap();
    }
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    rig.play(24_000);
    let gr = rig.deepest_gr();
    assert!(gr > 6.0, "Culmen driven 18 dB into -12 dB reduces: {gr}");
    p.master.effects[0].bypass = true;
    let mut rig = Rig::new(&p, &[]);
    rig.send(EngineMessage::Play);
    rig.play(24_000);
    assert_eq!(rig.deepest_gr(), 0.0);
}

/// A master insert taken out goes back to be freed off the audio thread; the units of the
/// chain are built and delivered by the handle like a strip's.
#[test]
fn master_units_come_and_go_like_a_strips() {
    let mut p = demo_project();
    let lim = on_master(&mut p, EffectKind::Limiter);
    let (mut handle, mut proc) = EngineHandle::manual(48_000.0);
    let snap = handle.compile(&p, 1);
    handle.set_snapshot(snap);
    assert!(
        handle.knows_effect(lim),
        "the handle delivered the master's unit"
    );
    let mut buf = vec![0f32; 1024];
    proc.process(&mut buf, 2);
    handle.poll();
    p.master.effects.clear();
    let snap = handle.compile(&p, 2);
    handle.set_snapshot(snap);
    proc.process(&mut buf, 2);
    handle.poll();
    assert!(
        !handle.knows_effect(lim),
        "the removed unit came back to be freed"
    );
}

/// A hosted-style unit (anything behind `FxUnit`) on the master is processed, and a panic
/// resets it with the strips'.
#[test]
fn a_unit_behind_the_contract_runs_on_the_master() {
    let mut p = demo_project();
    let id = on_master(&mut p, EffectKind::Eq);
    let mut rig = Rig::new(&p, &[id]);
    rig.send(EngineMessage::Play);
    let a = rig.play(20_000);
    assert!(peak(&a[..DELAY * 2]) == 0.0 && peak(&a) > 0.05);
    rig.send(EngineMessage::Stop);
    rig.send(EngineMessage::Panic);
    rig.play(1_000);
}

fn stem_opts(through_master: bool) -> StemOptions {
    StemOptions {
        render: RenderOptions {
            tail_seconds: 0.25,
            block_frames: 333,
            ..Default::default()
        },
        through_master,
        ..Default::default()
    }
}

fn strip_stems(p: &Project) -> Vec<StemSpec> {
    (0..2)
        .map(|i| StemSpec {
            source: StemSource::Strip(i),
            file_name: stem_file_name(i + 1, "s"),
        })
        .chain(std::iter::once(StemSpec {
            source: StemSource::Master,
            file_name: stem_file_name(0, "Master"),
        }))
        .take(p.strips.len() + 1)
        .collect()
}

/// Stems leave the master chain out by default — a strip's stem is what it sends on — and
/// with *through the master chain* each passes its own copy of it, aligned to the song.
#[test]
fn stems_skip_the_master_chain_unless_asked() {
    let bare = demo_project();
    let specs = strip_stems(&bare);
    let plain = render_stems(
        Arc::new(Snapshot::compile(&bare, 1)),
        &stem_opts(false),
        &specs,
    )
    .unwrap();
    // A pure delay on the master: strip stems untouched, the master's stem trimmed by it.
    let mut p = demo_project();
    let id = on_master(&mut p, EffectKind::Eq);
    let delayed = |slot: &EffectSlot, sr: f32| {
        if slot.id == id {
            EffectUnit::from_fx(slot, Box::new(PureDelay::new(DELAY)))
        } else {
            EffectUnit::new(slot, sr)
        }
    };
    for through in [false, true] {
        let mut make = delayed;
        let stems = cantus_engine::stems::render_stems_with(
            Arc::new(Snapshot::compile(&p, 1)),
            &stem_opts(through),
            &specs,
            &mut make,
        )
        .unwrap();
        for (a, b) in stems.iter().zip(&plain) {
            assert!(b.peak > 0.01 || a.spec.source != StemSource::Strip(0));
            assert_null(
                &a.samples,
                &b.samples,
                0.0,
                &format!("{:?}, through {through}", a.spec.source),
            );
        }
    }
    // A limiter at -6 dB on the master: in the strip stems only when asked.
    let mut p = demo_project();
    let lim = on_master(&mut p, EffectKind::Limiter);
    {
        let e = p.effect_mut(StripId::MASTER, lim).unwrap();
        e.spec.set("ceiling", -6.0).unwrap();
        e.spec.set("gain", 18.0).unwrap();
    }
    let snap = Arc::new(Snapshot::compile(&p, 1));
    let off = render_stems(snap.clone(), &stem_opts(false), &specs).unwrap();
    let on = render_stems(snap, &stem_opts(true), &specs).unwrap();
    assert_null(
        &off[0].samples,
        &plain[0].samples,
        0.0,
        "strip stem, no chain",
    );
    assert!(
        db(on[0].peak) <= -5.9,
        "through the chain: {}",
        db(on[0].peak)
    );
    assert!(db(on[0].peak) > -20.0);
    assert_eq!(on[0].samples.len(), off[0].samples.len());
    // The master's stem is the output either way.
    assert_null(&on[2].samples, &off[2].samples, 0.0, "the master stem");
}
