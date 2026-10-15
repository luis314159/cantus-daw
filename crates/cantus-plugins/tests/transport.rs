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
        seconds: None,
    }));
    p.process(&mut l, &mut r, None, &[]);
    let t = shared.host_transport().expect("seen");
    assert_eq!(t.tempo, 97.0);
    assert!(t.playing);
    assert_eq!(t.time_sig, (3, 4));
    // The song where the block ended: 256 samples past beat 4.25 at 97 BPM.
    let end = 4.25 + 256.0 * 97.0 / 60.0 / SR as f64;
    assert!((t.beats.unwrap() - end).abs() < 1e-9);
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
        seconds: None,
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

// ---- review 1: the processor with partial, moving and looping transports --------------

use cantus_plugins::instance::{shared_for, HostTransport, Processor};

/// A Tremor gate processor, as a host would make one.
fn gate_processor() -> Processor {
    let shared = shared_for(
        cantus_plugins::desc::of(EffectKind::Tremolo)
            .unwrap()
            .clap_id,
    )
    .unwrap();
    for (k, v) in GATE {
        let i = shared.table.by_key(k).unwrap().index;
        shared.set_value(i, f64::from(v));
    }
    Processor::new(shared, SR)
}

fn host(beats: Option<f64>, tempo: Option<f64>) -> HostTransport {
    HostTransport {
        playing: true,
        tempo,
        beats,
        bar_start: None,
        time_sig: Some((4, 4)),
        loop_beats: None,
        seconds: None,
    }
}

/// Run constant 1s through `p` in `blocks`, the host's transport for each block from
/// `at(block start)`, a parameter event (the same value: it only splits the block) at
/// `split` into each block when given.
fn run_processor(
    p: &mut Processor,
    n: usize,
    blocks: &[usize],
    at: impl Fn(usize) -> Option<HostTransport>,
    split: Option<u32>,
) -> Vec<f32> {
    let depth = p.shared().table.by_key("depth").unwrap().index as u32;
    let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
    let (mut s, mut b) = (0usize, 0usize);
    while s < n {
        let m = blocks[b % blocks.len()].min(n - s);
        b += 1;
        p.set_transport(at(s));
        let ev =
            split
                .filter(|o| (*o as usize) < m)
                .map(|o| cantus_plugins::instance::ParamEvent {
                    offset: o,
                    index: depth,
                    value: 1.0,
                    modulation: false,
                });
        let evs: Vec<_> = ev.into_iter().collect();
        p.process(&mut l[s..s + m], &mut r[s..s + m], None, &evs);
        s += m;
    }
    l
}

/// A host that plays but gives no beat timeline: a block cut by an event plays the same
/// as one that is not (each chunk's place is counted once).
#[test]
fn a_host_without_beats_plays_the_same_however_its_blocks_are_cut() {
    let n = 48_000;
    let no_beats = |_s: usize| Some(host(None, Some(120.0)));
    let whole = run_processor(&mut gate_processor(), n, &[4096], no_beats, None);
    let cut = run_processor(&mut gate_processor(), n, &[4096], no_beats, Some(1_000));
    assert_identical("a block cut by an event", &cut, &whole);
}

/// A loop that wraps inside a block: the block plays as the same block cut at the wrap,
/// the second part from the loop's start.
#[test]
fn a_loop_that_wraps_inside_a_block_is_the_block_cut_at_the_wrap() {
    let tempo = 120.0;
    let inc = tempo / 60.0 / SR as f64;
    let (ls, le) = (0.0, 3.0);
    let start = 2.9; // 0.1 beat before the loop's end: 2 400 samples
    let looped = |b: f64| HostTransport {
        loop_beats: Some((ls, le)),
        ..host(Some(b), Some(tempo))
    };
    let n = 4096;
    let one = run_processor(
        &mut gate_processor(),
        n,
        &[n],
        |_| Some(looped(start)),
        None,
    );
    let wrap = ((le - start) / inc).ceil() as usize;
    let two = run_processor(
        &mut gate_processor(),
        n,
        &[wrap, n - wrap],
        |s| {
            Some(looped(if s == 0 {
                start
            } else {
                ls + (s - wrap) as f64 * inc
            }))
        },
        None,
    );
    assert_identical("a wrap inside the block", &one, &two);
    // And the gate is where the song is: past its anti-click edge, beat 0.05 is open.
    assert_eq!(one[wrap + 1_200], 1.0);
    assert_eq!(one[100], 0.0, "beat 2.9 is shut");
}

/// A transport that changes inside a block (CLAP sends it as an event) takes effect at its
/// sample: the same as two blocks cut there.
#[test]
fn a_transport_event_inside_a_block_takes_effect_at_its_sample() {
    let n = 4096;
    let k = 1_500;
    let mut p = gate_processor();
    p.set_transport(Some(host(Some(0.5), Some(120.0))));
    p.push_transport_event(k as u32, host(Some(2.0), Some(120.0)));
    let (mut l, mut r) = (vec![1.0f32; n], vec![1.0f32; n]);
    p.process(&mut l, &mut r, None, &[]);
    let two = run_processor(
        &mut gate_processor(),
        n,
        &[k, n - k],
        |s| Some(host(Some(if s == 0 { 0.5 } else { 2.0 }), Some(120.0))),
        None,
    );
    assert_identical("a transport event at 1 500", &l, &two);
}

/// A host that gives the song in seconds (or samples) and a tempo, but no beats: the same
/// as one that gives the beats.
#[test]
fn a_seconds_timeline_with_a_tempo_is_a_beat_timeline() {
    let n = 48_000;
    let tempo = 100.0;
    let beats_at = |s: usize| 1.0 + s as f64 * tempo / 60.0 / SR as f64;
    let by_beats = run_processor(
        &mut gate_processor(),
        n,
        &[512],
        |s| Some(host(Some(beats_at(s)), Some(tempo))),
        None,
    );
    let by_seconds = run_processor(
        &mut gate_processor(),
        n,
        &[512],
        |s| {
            Some(HostTransport {
                seconds: Some(beats_at(s) * 60.0 / tempo),
                ..host(None, Some(tempo))
            })
        },
        None,
    );
    assert_identical("seconds and a tempo", &by_seconds, &by_beats);
}

/// A host that gives beats but no tempo: the tempo is read from how the beats move, so after
/// the first block it plays as the host that says its tempo — no step at every block.
#[test]
fn beats_without_a_tempo_find_the_tempo_from_the_beats() {
    let n = 48_000;
    let tempo = 90.0;
    let beats_at = |s: usize| 0.25 + s as f64 * tempo / 60.0 / SR as f64;
    let told = run_processor(
        &mut gate_processor(),
        n,
        &[512],
        |s| Some(host(Some(beats_at(s)), Some(tempo))),
        None,
    );
    let found = run_processor(
        &mut gate_processor(),
        n,
        &[512],
        |s| Some(host(Some(beats_at(s)), None)),
        None,
    );
    assert_identical("from the second block", &found[512..], &told[512..]);
}

/// What the editor shows: the host's transport at the end of the block, the tempo filtered,
/// no playhead without a timeline, nothing once the host stops sending one.
#[test]
#[cfg(feature = "editor")]
fn the_editors_transport_is_the_hosts_at_the_end_of_the_block() {
    use cantus_plugins::editor::{EditSink, PluginHost};
    use cantus_ui::views::fx::host::{FxHost, Transport};
    struct Nothing;
    impl EditSink for Nothing {
        fn edit(&mut self, _e: cantus_plugins::instance::Edit) {}
    }
    let mut p = gate_processor();
    let shared = p.shared().clone();
    let view = PluginHost::new(shared, Box::new(Nothing));
    assert_eq!(
        view.transport(),
        Transport::default(),
        "nothing before a block"
    );
    let ppq = cantus_core::PPQ as f64;
    p.set_transport(Some(HostTransport {
        time_sig: Some((6, 8)),
        ..host(Some(4.25), Some(90.0))
    }));
    let (mut l, mut r) = (vec![0.5f32; 480], vec![0.5f32; 480]);
    p.process(&mut l, &mut r, None, &[]);
    let t = view.transport();
    let end = 4.25 + 480.0 * 90.0 / 60.0 / 48_000.0;
    assert_eq!(t.playing_at, Some((end * ppq).floor() as i64));
    assert_eq!(t.ticks_per_second, 90.0 / 60.0 * ppq);
    assert_eq!(t.ticks_per_bar, 3 * cantus_core::PPQ);
    // A tempo of 0 is not believed; a host with no timeline gives no playhead.
    p.set_transport(Some(host(None, Some(0.0))));
    p.process(&mut l, &mut r, None, &[]);
    let t = view.transport();
    assert_eq!(t.playing_at, None);
    assert_eq!(t.ticks_per_second, 2.0 * ppq, "120 BPM, as the units play");
    // A host that stops sending a transport: back to the default.
    p.set_transport(None);
    p.process(&mut l, &mut r, None, &[]);
    assert_eq!(view.transport(), Transport::default());
}
