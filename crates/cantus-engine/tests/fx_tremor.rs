//! Tremor through the whole engine (ADR 0055): a wire at its defaults and at depth 0 in
//! every mode, the same render at every block size while it works, live equal to offline,
//! and a synced LFO that follows the song position — from the start of the song and from
//! a later bar.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use modfx::*;
use support::*;

#[test]
fn tremor_at_its_defaults_renders_exactly_as_no_tremor() {
    let reference = render_of(&demo_project());
    let mut p = demo_project();
    add_effect(&mut p, EffectKind::Tremolo);
    assert_null(&reference, &render_of(&p), 0.0, "Tremor at its defaults");
}

#[test]
fn at_depth_zero_every_mode_is_a_wire() {
    let reference = render_of(&demo_project());
    for mode in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Tremolo);
        set_all(
            &mut p,
            fx,
            &[
                ("mode", mode),
                ("shape", 2.0),
                ("spread", 90.0),
                ("swing", 0.5),
            ],
        );
        assert_null(&reference, &render_of(&p), 0.0, &format!("mode {mode}"));
    }
}

fn busy(p: &mut Project, fx: EffectId, mode: f32) {
    set_all(
        p,
        fx,
        &[
            ("mode", mode),
            ("depth", 0.9),
            ("div", 13.0),
            ("shape", 2.0),
            ("swing", 0.3),
            ("spread", 60.0),
            ("out", -1.5),
        ],
    );
}

#[test]
fn the_render_does_not_depend_on_the_block_size_while_it_works() {
    for mode in [0.0, 1.0, 2.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Tremolo);
        busy(&mut p, fx, mode);
        let reference = render_with(&p, 512);
        assert!(reference != render_with(&demo_project(), 512), "it works");
        for block in [1usize, 7, 64, 1000, 4096] {
            assert_null(
                &reference,
                &render_with(&p, block),
                0.0,
                &format!("mode {mode}, block {block}"),
            );
        }
    }
}

#[test]
fn live_playback_renders_what_the_offline_render_does() {
    for sync in [1.0, 0.0] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Tremolo);
        busy(&mut p, fx, 0.0);
        set_all(&mut p, fx, &[("sync", sync), ("rate", 5.3)]);
        let offline = render_of(&p);
        assert_null(
            &offline,
            &realtime(&p, offline.len()),
            0.0,
            &format!("sync {sync}"),
        );
    }
}

/// A square at full depth, a quarter note long: the lead passes untouched through the
/// first half of every beat of the song and is silent through the second.
fn gated(p: &mut Project) {
    let fx = add_effect(p, EffectKind::Tremolo);
    set_all(
        p,
        fx,
        &[("depth", 1.0), ("div", 8.0), ("shape", 2.0), ("edge", 0.0)],
    );
}

fn check_gate(out: &[f32], reference: &[f32], first_beat_at: f64, what: &str) {
    let frames = out.len() / 2;
    let mut checked = 0;
    let mut beat = 0.0;
    while (first_beat_at + (beat + 1.0) * BEAT) < frames as f64 {
        let at = |share: f64| (first_beat_at + (beat + share) * BEAT) as usize;
        for f in at(0.6)..at(0.9) {
            assert_eq!(out[2 * f], 0.0, "{what}: frame {f} is in a shut half");
        }
        for f in at(0.1)..at(0.4) {
            assert_eq!(
                out[2 * f].to_bits(),
                reference[2 * f].to_bits(),
                "{what}: frame {f} is in an open half"
            );
        }
        checked += 1;
        beat += 1.0;
    }
    assert!(checked >= 4, "{what}: {checked} beats");
}

#[test]
fn a_synced_gate_follows_the_song_from_the_start_and_from_a_later_bar() {
    let base = lead_only();
    let mut p = base.clone();
    gated(&mut p);
    let reference = render_with(&base, 256);
    check_gate(&render_with(&p, 256), &reference, 0.0, "from the start");
    // From beat 3: the render's first sample is on a beat, so the halves line up again.
    let start = Ticks::beats(3);
    let reference = render_from(&base, start, 300);
    check_gate(&render_from(&p, start, 300), &reference, 0.0, "from beat 3");
}

#[test]
fn the_unit_hears_the_projects_tempo() {
    let mut base = lead_only();
    base.tempo_bpm = 90.0;
    let mut p = base.clone();
    gated(&mut p);
    let out = render_with(&p, 256);
    // At 90 BPM a beat is 32 000 samples: frame 20 000 of the first beat is shut (at 128
    // BPM it would be open, in the second beat's first half).
    let f = (0.75 * 32_000.0) as usize;
    assert_eq!(out[2 * f], 0.0);
    assert!(
        render_with(&base, 256)[2 * f] != 0.0,
        "the lead plays there"
    );
}

/// Integration review: behind a latent insert (Flexus through zero at mix 0: the lead 240
/// samples later) the synced gate follows the beat it hears — the gated render is the gate
/// without the latent insert, 240 samples later — not the transport's, which leads the
/// audio by the delay compensation's 240 samples.
#[test]
fn a_synced_gate_behind_a_latent_insert_follows_the_beat_it_hears() {
    let mut alone = lead_only();
    gated(&mut alone);
    let reference = render_of(&alone);
    let mut p = lead_only();
    let flexus = add_effect(&mut p, EffectKind::Flanger);
    set_all(&mut p, flexus, &[("tz", 1.0), ("depth", 8.0)]);
    gated(&mut p);
    let out = render_of(&p);
    let shift = 2 * 240;
    assert!(out[..shift].iter().all(|x| *x == 0.0));
    let off = reference[..reference.len() - shift]
        .iter()
        .zip(&out[shift..])
        .filter(|(a, b)| (*a - *b).abs() > 1e-4)
        .count();
    let edges = 2 * (reference.len() / 2) / BEAT as usize;
    assert!(
        off <= 2 * edges,
        "{off} samples off the gate the lead hears ({edges} edges)"
    );
}

/// The same on the master's chain: the lead reaches it 240 samples late (its strip's
/// latent insert), and the master's synced gate follows what it hears.
#[test]
fn a_masters_synced_gate_follows_the_compensated_beat() {
    let on_master = |p: &mut Project| {
        let mut slot = p.make_effect(EffectKind::Tremolo);
        for (k, v) in [("depth", 1.0), ("div", 8.0), ("shape", 2.0), ("edge", 0.0)] {
            assert!(slot.spec.set(k, v).is_some());
        }
        p.master.effects.push(slot);
    };
    let mut alone = lead_only();
    on_master(&mut alone);
    let reference = render_of(&alone);
    let mut p = lead_only();
    let flexus = add_effect(&mut p, EffectKind::Flanger);
    set_all(&mut p, flexus, &[("tz", 1.0), ("depth", 8.0)]);
    on_master(&mut p);
    let out = render_of(&p);
    let shift = 2 * 240;
    let off = reference[..reference.len() - shift]
        .iter()
        .zip(&out[shift..])
        .filter(|(a, b)| (*a - *b).abs() > 1e-4)
        .count();
    let edges = 2 * (reference.len() / 2) / BEAT as usize;
    assert!(off <= 2 * edges, "{off} samples off ({edges} edges)");
}

/// Play `project` live with its loop on, `sizes` frames per callback in turn, for `len`
/// samples (interleaved stereo).
fn looping(project: &Project, len: usize, sizes: &[usize]) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<cantus_engine::EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<cantus_engine::EngineEvent>::new(4096);
    let mut p = cantus_engine::Processor::without_midi(48_000.0, rx_engine, tx_engine);
    let snap = Snapshot::compile(project, 1);
    assert!(snap.loop_region.is_some(), "the loop is on");
    for strip in &snap.strips {
        for slot in &strip.effects {
            let unit = cantus_engine::EffectUnit::new(slot, 48_000.0);
            tx.push(cantus_engine::EngineMessage::AddEffect(Box::new(unit)))
                .unwrap();
        }
    }
    tx.push(cantus_engine::EngineMessage::SetSnapshot(
        std::sync::Arc::new(snap),
    ))
    .unwrap();
    tx.push(cantus_engine::EngineMessage::Seek(Ticks::ZERO))
        .unwrap();
    tx.push(cantus_engine::EngineMessage::Play).unwrap();
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        let n = sizes[i % sizes.len()].min((len - out.len()) / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

/// Review of fix3: a synced gate behind a latent insert (240 samples) hears the loop wrap
/// where its audio wraps — the pass is cut there — so a loop that is not a whole number of
/// the gate's periods (4⅓ beats) plays the same bits at any block size, live; and on its
/// first entry from before the loop it hears the song before the loop, not its end.
#[test]
fn a_late_synced_gate_wraps_with_its_audio_at_any_block_size() {
    let mut p = lead_only();
    let flexus = add_effect(&mut p, EffectKind::Flanger);
    set_all(&mut p, flexus, &[("tz", 1.0), ("depth", 8.0)]);
    gated(&mut p);
    for (start, what) in [
        (Ticks::ZERO, "a loop from the start"),
        (Ticks::beats(2), "a loop entered"),
    ] {
        p.loop_region = Some(LoopRegion {
            start,
            end: start + Ticks::beats(4) + Ticks(PPQ / 3),
        });
        p.loop_enabled = true;
        let len = 2 * (BEAT * 15.0) as usize;
        let reference = looping(&p, len, &[512]);
        assert!(reference.iter().any(|x| *x != 0.0));
        for sizes in [&[37usize, 100, 1024, 7][..], &[1024], &[64, 333]] {
            assert_null(
                &reference,
                &looping(&p, len, sizes),
                0.0,
                &format!("{what}, blocks {sizes:?}"),
            );
        }
    }
}

/// The transport's position, in beats, at each of `frames` frames played from the start
/// with `project`'s loop — the transport's own arithmetic — and the frames it wrapped at.
fn positions(project: &Project, frames: usize) -> (Vec<f64>, Vec<usize>) {
    let mut t = cantus_engine::Transport::new(48_000.0);
    t.set_bpm(project.tempo_bpm);
    t.set_loop(project.loop_region.clone());
    t.seek(Ticks::ZERO);
    t.play();
    let (mut pos, mut wraps) = (Vec::with_capacity(frames), Vec::new());
    for f in 0..frames {
        pos.push(t.position_f64() / PPQ as f64);
        if t.advance(1) {
            wraps.push(f + 1);
        }
    }
    (pos, wraps)
}

/// Check a quarter-note gate `late` frames behind the transport against the position of
/// the audio it hears — where the transport was `late` frames before — frame by frame:
/// shut in the second half of that beat, the ungated bits in the first, a little
/// margin at each edge (its soft turn). How many frames it found open with sound, lying behind a wrap
/// (the transport already in the next pass, the audio still before it).
fn check_heard(out: &[f32], ungated: &[f32], project: &Project, late: usize, what: &str) -> usize {
    let frames = out.len() / 2;
    let (pos, wraps) = positions(project, frames);
    assert!(wraps.len() >= 2, "{what}: it loops");
    // The square turns over 0.002 of a beat (`modfx_lfo::MIN_EDGE`) around each edge.
    let margin = 0.003;
    let mut behind = 0;
    for f in late..frames {
        let heard = pos[f - late];
        let ph = heard - heard.floor();
        if ph.min(1.0 - ph) < margin || (ph - 0.5).abs() < margin {
            continue;
        }
        if ph > 0.5 {
            assert_eq!(
                out[2 * f],
                0.0,
                "{what}: frame {f} hears beat {heard}: shut"
            );
        } else {
            assert_eq!(
                out[2 * f].to_bits(),
                ungated[2 * f].to_bits(),
                "{what}: frame {f} hears beat {heard}: open"
            );
            let wrapped = wraps.iter().any(|w| f - late < *w && *w <= f);
            if wrapped && ungated[2 * f] != 0.0 {
                behind += 1;
            }
        }
    }
    behind
}

/// Review of fix4 (the block-size test above would pass with no fold at all): end to end,
/// the gate behind a latent insert hears every wrap where its audio wraps — 240 samples
/// after the transport's — and so the beat of the audio in its hands, frame by frame, in
/// a loop of 4⅓ beats entered from before it (where the loop's end and start are a third
/// of a beat apart, so a gate folding at the transport's wrap shuts what is open); and
/// until its audio first wraps, the first pass is the song played straight, to the bit.
#[test]
fn a_late_gate_hears_each_wrap_where_its_audio_wraps() {
    let mut ungated = lead_only();
    let flexus = add_effect(&mut ungated, EffectKind::Flanger);
    set_all(&mut ungated, flexus, &[("tz", 1.0), ("depth", 8.0)]);
    ungated.loop_region = Some(LoopRegion {
        start: Ticks::beats(2),
        end: Ticks::beats(6) + Ticks(PPQ / 3),
    });
    ungated.loop_enabled = true;
    // A note sounding into the loop's start, so the first entry has something to shut.
    let held = ungated.make_note(Ticks::beats(1), Ticks::beats(2), 52, 90);
    ungated.tracks[0].clips[0].notes.push(held);
    let mut p = ungated.clone();
    gated(&mut p);
    let len = 2 * (BEAT * 15.0) as usize;
    let out = looping(&p, len, &[37, 100, 1024, 7]);
    let open = looping(&ungated, len, &[512]);
    let behind = check_heard(&out, &open, &p, 240, "4⅓ beats");
    assert!(
        behind > 100,
        "the frames behind a wrap were checked: {behind}"
    );
    // The first entry: up to where the audio wraps, as with no loop — the audio from just
    // before the loop's start (beat 2, shut) sounds as the transport enters it.
    let entry = (2.0 * BEAT).ceil() as usize;
    assert!(open[2 * entry..2 * (entry + 240)].iter().any(|x| *x != 0.0));
    let (_, wraps) = positions(&p, len / 2);
    let straight = realtime(&p, len);
    let first = 2 * (wraps[0] + 240);
    assert_null(&straight[..first], &out[..first], 0.0, "the first entry");
    // In blocks of 7, a pass starts on nearly every few samples after the entry.
    let fine = looping(&p, first, &[7]);
    assert_null(
        &straight[..first],
        &fine,
        0.0,
        "the first entry, blocks of 7",
    );
    assert!(out[first..]
        .iter()
        .zip(&straight[first..])
        .any(|(a, b)| a != b));
}

/// Review of fix4: an insert later than the loop is long (four Flexus, 960 samples; a
/// loop of a 32nd note, 703⅛ samples — passes of 703 and 704) hears, right after the
/// first wrap, the song leading into the loop — not passes that were never played. The
/// gate cannot tell one pass length from another here (every loop phase is in its shut
/// half; review of fix5), so the fold it is given is checked against the transport frame
/// by frame: where the song was 960 frames before, to a quarter of a sample — a pass
/// counted a sample long or short is a whole sample off.
#[test]
fn a_gate_later_than_the_loop_never_hears_passes_not_played() {
    let mut ungated = lead_only();
    for _ in 0..4 {
        let flexus = add_effect(&mut ungated, EffectKind::Flanger);
        set_all(&mut ungated, flexus, &[("tz", 1.0), ("depth", 8.0)]);
    }
    // From the middle of beat 6 (the gate's edge), where the chord sounds.
    let start = Ticks::beats(6) + Ticks(PPQ / 2);
    ungated.loop_region = Some(LoopRegion {
        start,
        end: start + Ticks(PPQ / 32),
    });
    ungated.loop_enabled = true;
    let mut p = ungated.clone();
    gated(&mut p);
    let len = 2 * (BEAT * 8.0) as usize;
    let out = looping(&p, len, &[64, 333]);
    let behind = check_heard(
        &out,
        &looping(&ungated, len, &[512]),
        &p,
        960,
        "a 32nd's loop",
    );
    assert!(
        behind > 100,
        "the frames behind the first wrap were checked: {behind}"
    );
    let frames = len / 2;
    let (pos, wraps) = positions(&p, frames);
    assert!(wraps.len() > 20, "{} wraps", wraps.len());
    let per_sample = p.tempo_bpm / 60.0 / 48_000.0;
    let mut t = cantus_engine::Transport::new(48_000.0);
    t.set_bpm(p.tempo_bpm);
    t.set_loop(p.loop_region.clone());
    t.seek(Ticks::ZERO);
    t.play();
    let mut folded = 0;
    for f in 0..frames {
        if f >= 960 && t.wraps().is_some() {
            let now = cantus_engine::SongPosition {
                playing: true,
                tempo_bpm: p.tempo_bpm,
                beats: t.position_f64() / PPQ as f64,
                time_sig_num: 4,
                time_sig_den: 4,
                loop_beats: Some((6.5, 6.5 + 1.0 / 32.0)),
                wrap: t.wraps(),
                ..Default::default()
            };
            let heard = now.earlier(960, 48_000.0).beats;
            assert!(
                (heard - pos[f - 960]).abs() < 0.25 * per_sample,
                "frame {f}: folded to {heard}, the transport was at {}",
                pos[f - 960]
            );
            folded += 1;
        }
        t.advance(1);
    }
    assert!(folded > 20_000, "{folded}");
}
