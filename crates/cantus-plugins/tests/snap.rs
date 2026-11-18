//! fx/final — the exported plug-ins snap where the host's song jumps (ADR 0027, amended),
//! through CLAP here (VST3 in `vst3.rs`): playback started from stop, a seek (the song more
//! than `JUMP_SAMPLES` from where the last block left it), a seek inside a block (CLAP's
//! transport event), and an offline render begun (`clap.render`) — after the values in
//! force are applied, the unit takes them at once, so a value changed since the last block
//! lands as a fresh instance made at it plays, to the bit. A change while the song plays on
//! still glides, as the DAW's insert does. (`alloc.rs` holds the snap to no allocation.)

mod common;
use cantus_core::EffectKind;
use clap_sys::events::*;
use clap_sys::fixedpoint::CLAP_BEATTIME_FACTOR;
use common::*;

/// Utility (Libra): its output gain glides over 20 ms, and silence leaves it as new.
const KIND: EffectKind = EffectKind::Utility;
const TEMPO: f64 = 120.0;
/// Frames of silence before the change: a multiple of the grid and of every unit's own.
const BEFORE: usize = 4_096;
const AFTER: usize = 8_192;
const BLOCK: usize = 512;

fn transport(beats: f64, playing: bool) -> clap_event_transport {
    clap_event_transport {
        header: clap_event_header {
            size: std::mem::size_of::<clap_event_transport>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: CLAP_EVENT_TRANSPORT,
            flags: 0,
        },
        flags: CLAP_TRANSPORT_HAS_TEMPO
            | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
            | CLAP_TRANSPORT_HAS_TIME_SIGNATURE
            | if playing {
                CLAP_TRANSPORT_IS_PLAYING
            } else {
                0
            },
        song_pos_beats: (beats * CLAP_BEATTIME_FACTOR as f64).round() as i64,
        song_pos_seconds: 0,
        tempo: TEMPO,
        tempo_inc: 0.0,
        loop_start_beats: 0,
        loop_end_beats: 0,
        loop_start_seconds: 0,
        loop_end_seconds: 0,
        bar_start: 0,
        bar_number: 0,
        tsig_num: 4,
        tsig_denom: 4,
    }
}

fn beats_after(start: f64, frames: usize) -> f64 {
    start + frames as f64 * TEMPO / 60.0 / SR as f64
}

/// The output gain's event, `db` in CLAP's units, at `offset`.
fn out_at(offset: u32, db: f32) -> Ev {
    let (id, v) = clap_value(KIND, "out", db);
    (offset, id, v)
}

/// A fresh instance made at +9 dB, playing from beat `from`: the reference.
fn fresh(from: f64) -> Vec<f32> {
    let plug = Plug::new(KIND);
    plug.flush(&[out_at(0, 9.0)]);
    plug.activate(BLOCK as u32);
    play(&plug, from, &[], AFTER)
}

/// `frames` of the test signal through `plug` in blocks, the host's song playing from
/// `from`; `events` in the first block.
fn play(plug: &Plug, from: f64, events: &[Ev], frames: usize) -> Vec<f32> {
    let (mut l, mut r) = signal(frames, 11);
    let mut at = 0;
    while at < frames {
        let m = BLOCK.min(frames - at);
        plug.transport
            .set(Some(transport(beats_after(from, at), true)));
        let evs = if at == 0 { events } else { &[] };
        plug.process(&mut l[at..at + m], &mut r[at..at + m], None, evs, false);
        at += m;
    }
    l.extend_from_slice(&r);
    l
}

/// An instance at 0 dB that has heard `BEFORE` frames of silence, its host's song
/// `playing` from beat 0 (or stopped there).
fn warmed(playing: bool) -> Plug {
    let plug = Plug::new(KIND);
    plug.flush(&[out_at(0, 0.0)]);
    plug.activate(BLOCK as u32);
    let mut at = 0;
    while at < BEFORE {
        let (mut l, mut r) = (vec![0.0f32; BLOCK], vec![0.0f32; BLOCK]);
        let beats = if playing { beats_after(0.0, at) } else { 0.0 };
        plug.transport.set(Some(transport(beats, playing)));
        plug.process(&mut l, &mut r, None, &[], false);
        at += BLOCK;
    }
    plug
}

#[test]
fn a_seek_lands_a_value_changed_since_the_last_block_at_once() {
    let plug = warmed(true);
    // The host seeks to beat 8 and the gain is +9 dB there (its lane, a recall).
    let heard = play(&plug, 8.0, &[out_at(0, 9.0)], AFTER);
    assert_identical("seek", &heard, &fresh(8.0));
}

#[test]
fn playback_started_from_stop_lands_at_once() {
    let plug = warmed(false);
    let heard = play(&plug, 0.0, &[out_at(0, 9.0)], AFTER);
    assert_identical("play from stop", &heard, &fresh(0.0));
}

#[test]
fn a_seek_inside_a_block_lands_at_once() {
    let plug = warmed(true);
    // One block that carries the move to beat 8 at its first sample, as a transport event.
    let (mut l, mut r) = signal(AFTER, 11);
    plug.transport
        .set(Some(transport(beats_after(0.0, BEFORE), true)));
    let list = EventList::with_moves(&[out_at(0, 9.0)], &[(0, transport(8.0, true))]);
    plug.process_list(&mut l[..BLOCK], &mut r[..BLOCK], None, &list);
    let mut at = BLOCK;
    while at < AFTER {
        plug.transport
            .set(Some(transport(beats_after(8.0, at), true)));
        plug.process(
            &mut l[at..at + BLOCK],
            &mut r[at..at + BLOCK],
            None,
            &[],
            false,
        );
        at += BLOCK;
    }
    l.extend_from_slice(&r);
    assert_identical("seek inside a block", &l, &fresh(8.0));
}

#[test]
fn an_offline_render_begun_lands_at_once() {
    let plug = warmed(true);
    assert!(plug.set_render(true), "clap.render is offered");
    let heard = play(&plug, beats_after(0.0, BEFORE), &[out_at(0, 9.0)], AFTER);
    assert_identical("offline render", &heard, &fresh(beats_after(0.0, BEFORE)));
}

#[test]
fn a_change_while_the_song_plays_on_still_glides() {
    let plug = warmed(true);
    let heard = play(&plug, beats_after(0.0, BEFORE), &[out_at(0, 9.0)], AFTER);
    let landed = fresh(beats_after(0.0, BEFORE));
    let (at, _, _, _) = diff(&heard, &landed).expect("it glides: not the landed gain");
    assert!(
        at < 960,
        "the glide begins at once (first difference at {at})"
    );
    // …and it has landed by the end: the same bits there.
    assert_identical(
        "after the glide",
        &heard[AFTER - 1_024..AFTER],
        &landed[AFTER - 1_024..AFTER],
    );
}
