//! The host's transport in the exported plug-ins (ADR 0055): a synced unit in another DAW
//! follows that DAW's tempo and position — through CLAP's `clap_event_transport` here, and
//! through VST3's `ProcessContext` in `vst3.rs` — bit-identical to the engine's unit given
//! the same song position.

mod common;
use cantus_core::EffectKind;
use clap_sys::events::*;
use clap_sys::fixedpoint::CLAP_BEATTIME_FACTOR;
use common::*;

/// Set values before activation, as a host restoring a session does.
fn configure(plug: &Plug, values: &[(&str, f32)]) {
    let events: Vec<Ev> = values
        .iter()
        .map(|(k, v)| {
            let (id, value) = clap_value(plug.kind, k, *v);
            (0, id, value)
        })
        .collect();
    plug.flush(&events);
}

/// A square gate on the quarters, at full depth: open through the first half of every
/// beat, shut through the second.
const GATE: [(&str, f32); 5] = [
    ("depth", 1.0),
    ("sync", 1.0),
    ("div", 8.0),
    ("shape", 2.0),
    ("edge", 0.0),
];

fn transport(beats: f64, tempo: f64, playing: bool) -> clap_event_transport {
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
        tempo,
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

/// Run a constant 1 through the plug-in in `blocks`, the host's song at `tempo` from
/// `start` beats (and back to `restart` after `jump_at` samples, when given).
fn run_clap(
    plug: &Plug,
    n: usize,
    blocks: &[usize],
    tempo: Option<f64>,
    start: f64,
    jump: Option<(usize, f64)>,
) -> Vec<f32> {
    let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
    let (mut at, mut b) = (0usize, 0usize);
    while at < n {
        let m = blocks[b % blocks.len()].min(n - at);
        b += 1;
        if let Some(t) = tempo {
            let beats = match jump {
                Some((j, to)) if at >= j => to + (at - j) as f64 * t / 60.0 / SR as f64,
                _ => start + at as f64 * t / 60.0 / SR as f64,
            };
            plug.transport.set(Some(transport(beats, t, true)));
        }
        plug.process(&mut l[at..at + m], &mut r[at..at + m], None, &[], false);
        at += m;
    }
    l
}

/// The engine's DSP given the same song position, once per block of 512.
fn run_engine(n: usize, tempo: f64, start: f64) -> Vec<f32> {
    let spec = spec_with(EffectKind::Tremolo, &GATE);
    let mut d = cantus_dsp::tremor::Tremor::new(SR);
    d.set(&cantus_engine::fx::tremor::settings(&spec));
    d.reset();
    let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
    let start_samples = (start * 60.0 / tempo * SR as f64).round() as i64;
    let mut at = 0;
    while at < n {
        let m = 512.min(n - at);
        d.set_transport(&cantus_dsp::modfx_clock::Transport {
            playing: true,
            tempo_bpm: tempo,
            samples: start_samples + at as i64,
        });
        d.process(&mut l[at..at + m], &mut r[at..at + m]);
        at += m;
    }
    l
}

fn new_gate() -> Plug {
    let plug = Plug::new(EffectKind::Tremolo);
    configure(&plug, &GATE);
    plug.activate(4096);
    plug
}

/// Every sample in the open half of its beat is 1, every one in the shut half 0.
fn check_beats(out: &[f32], tempo: f64, beat_of: impl Fn(usize) -> f64, what: &str) {
    let mut checked = 0;
    for (i, v) in out.iter().enumerate() {
        let f = beat_of(i).fract();
        let _ = tempo;
        if (0.05..0.45).contains(&f) {
            assert_eq!(*v, 1.0, "{what}: sample {i} (beat fraction {f:.3}) is open");
            checked += 1;
        } else if (0.55..0.95).contains(&f) {
            assert_eq!(*v, 0.0, "{what}: sample {i} (beat fraction {f:.3}) is shut");
            checked += 1;
        }
    }
    assert!(checked > out.len() / 2, "{what}");
}

#[test]
fn a_synced_tremor_follows_the_hosts_tempo_and_position_through_clap() {
    let n = 96_000;
    let (tempo, start) = (90.0, 2.5);
    let expected = run_engine(n, tempo, start);
    for blocks in [&[512][..], &[1], &[7], &[4096], &[5, 333, 64]] {
        let out = run_clap(&new_gate(), n, blocks, Some(tempo), start, None);
        assert_identical(&format!("blocks {blocks:?}"), &out, &expected);
    }
    check_beats(
        &expected,
        tempo,
        |i| start + i as f64 * tempo / 60.0 / SR as f64,
        "90 BPM from beat 2.5",
    );
}

#[test]
fn without_a_transport_it_keeps_its_own_time_and_a_host_changes_it() {
    let n = 48_000;
    let own = run_clap(&new_gate(), n, &[512], None, 0.0, None);
    // Its own time: 120 BPM from the first sample.
    check_beats(
        &own,
        120.0,
        |i| i as f64 * 2.0 / SR as f64,
        "its own 120 BPM",
    );
    let hosted = run_clap(&new_gate(), n, &[512], Some(90.0), 0.0, None);
    assert!(diff(&own, &hosted).is_some(), "the host's 90 BPM is heard");
}

#[test]
fn a_jump_in_the_hosts_song_puts_the_gate_where_the_song_is() {
    let n = 72_000;
    let tempo = 120.0;
    // From beat 0.5 (shut) — then, 30 000 samples in, the host loops back to beat 0.
    let jump = 30_000;
    let out = run_clap(&new_gate(), n, &[480], Some(tempo), 0.5, Some((jump, 0.0)));
    let beat_of = |i: usize| {
        if i < jump {
            0.5 + i as f64 * 2.0 / SR as f64
        } else {
            (i - jump) as f64 * 2.0 / SR as f64
        }
    };
    check_beats(&out, tempo, beat_of, "a loop back to beat 0");
}

#[test]
fn the_editor_sees_the_hosts_transport() {
    use cantus_plugins::instance::{shared_for, HostTransport, Processor};
    let shared = shared_for(
        cantus_plugins::desc::of(EffectKind::Tremolo)
            .unwrap()
            .clap_id,
    )
    .unwrap();
    assert!(shared.host_transport().is_none(), "nothing before a block");
    let mut p = Processor::new(shared.clone(), SR);
    let (mut l, mut r) = (vec![0.5f32; 256], vec![0.5f32; 256]);
    p.process(&mut l, &mut r, None, &[]);
    assert!(shared.host_transport().is_none(), "a host that sends none");
    p.set_transport(Some(HostTransport {
        playing: true,
        tempo: Some(97.0),
        beats: Some(4.25),
        bar_start: Some(4.0),
        time_sig: Some((3, 4)),
        loop_beats: None,
    }));
    p.process(&mut l, &mut r, None, &[]);
    let t = shared.host_transport().expect("seen");
    assert_eq!(t.tempo, 97.0);
    assert!(t.playing);
    assert_eq!(t.time_sig, (3, 4));
    assert_eq!(t.beats, 4.25);
    // And where the block's last frame is (what Speculum's clock reads): 255 frames on.
    assert!((t.last_beats - (4.25 + 255.0 * 97.0 / 60.0 / f64::from(SR))).abs() < 1e-9);
}

#[test]
fn a_host_position_is_the_engines_song_position() {
    use cantus_plugins::instance::HostTransport;
    let t = HostTransport {
        playing: true,
        tempo: Some(128.0),
        beats: Some(16.0),
        bar_start: None,
        time_sig: Some((4, 4)),
        loop_beats: Some((16.0, 32.0)),
    };
    let p = t.position(100, 48_000.0, 0);
    // Bar 5 at 128 BPM is sample 360 000 of the song; 100 samples into the block.
    assert_eq!(p.samples, 360_100);
    assert_eq!(p.tempo_bpm, 128.0);
    assert_eq!(p.bar_start_beats, 16.0);
    assert!((p.beats - (16.0 + 100.0 * 128.0 / 60.0 / 48_000.0)).abs() < 1e-12);
    assert_eq!(p.loop_beats, Some((16.0, 32.0)));
    // Stopped, the position stays; no timeline, the stream is the timeline.
    let s = HostTransport {
        playing: false,
        ..t
    };
    assert_eq!(s.position(100, 48_000.0, 0).samples, 360_000);
    let free = HostTransport { beats: None, ..t };
    assert_eq!(free.position(10, 48_000.0, 5_000).samples, 5_010);
}
