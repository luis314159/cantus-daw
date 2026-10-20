//! Plug-ins loaded into real helper processes through `BridgedBackend` (ADR 0063), against
//! the same plug-ins loaded in this process: the same bits, and every failure the test
//! plug-ins can make on purpose survived — the audio flowing (dry for an effect, silent for
//! an instrument), the crash said once, the audio thread never held past the deadline.

mod support;

use cantus_host::bridge::audio::deadline;
use cantus_host::{
    Backend, Format, HostError, Loaded, MainThread, NoteEvent, NoteKind, Notice, ParamChange,
};
use std::time::{Duration, Instant};
use support::*;

fn load_within(id: &str) -> Loaded {
    let info = info(id);
    cantus_host::backend_for(info.format)
        .unwrap()
        .load(&info, setup())
        .expect("loads in process")
}

fn load_isolated(
    pool: &std::sync::Arc<cantus_host::bridge::Sandboxes>,
    id: &str,
    group: &str,
) -> Loaded {
    let info = info(id);
    bridged(pool, info.format, group, false)
        .load(&info, setup())
        .expect("loads in the helper")
}

/// Loaded for an offline render: every block waited for, without a deadline — what makes
/// the comparison of bits independent of how busy the machine is.
fn load_offline(
    pool: &std::sync::Arc<cantus_host::bridge::Sandboxes>,
    id: &str,
    group: &str,
) -> Loaded {
    let info = info(id);
    bridged(pool, info.format, group, true)
        .load(&info, setup())
        .expect("loads in the helper")
}

/// Waits until the helper's mirror shows parameter `id` at `value`: the block that set it
/// has run (it may have been late on a busy machine).
fn arrived(main: &mut dyn MainThread, id: u32, value: f64) {
    let start = Instant::now();
    while main.param_value(id) != Some(value) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "parameter {id} never arrived"
        );
        main.idle();
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Polls `main.idle()` until it says the plug-in crashed, or `limit` passes.
fn crashed_within(main: &mut dyn MainThread, limit: Duration) -> Option<String> {
    let start = Instant::now();
    while start.elapsed() < limit {
        for n in main.idle() {
            if let Notice::Crashed { reason, .. } = n {
                return Some(reason);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn an_isolated_gain_renders_the_same_bits_as_in_process_at_every_block_size() {
    let _g = serial();
    let pool = pool();
    for (id, gain_id, gain) in [(GAIN_ID, P_GAIN, -3.0), (VST3_GAIN_ID, V_GAIN, -3.0)] {
        let Loaded {
            main: mut m_in,
            audio: mut a_in,
        } = load_within(id);
        let Loaded {
            main: mut m_out,
            audio: mut a_out,
        } = load_offline(&pool, id, "together");
        // The instance really lives in another process.
        let pid = m_out
            .params()
            .iter()
            .find(|p| p.name == "Process")
            .and_then(|p| m_out.param_value(p.id))
            .expect("the Process parameter");
        assert_ne!(
            pid.round() as u32,
            std::process::id(),
            "{id} ran in this process"
        );
        assert_eq!(Some(pid.round() as u32), m_out.isolated_in());
        let change = [ParamChange {
            offset: 0,
            id: gain_id,
            value: gain,
        }];
        let _ = (&mut m_in, &mut m_out);
        for block in [1usize, 37, 64, 512] {
            let (l0, r0) = (noise(4_000, 1), noise(4_000, 2));
            let (mut l1, mut r1) = (l0.clone(), r0.clone());
            let (mut l2, mut r2) = (l0.clone(), r0.clone());
            run(a_in.as_mut(), &mut l1, &mut r1, block, &change);
            run(a_out.as_mut(), &mut l2, &mut r2, block, &change);
            assert!(!same_bits(&l1, &l0), "{id}: the gain did something");
            assert_eq!(differing(&l1, &l2), 0, "{id} at {block}: left differs");
            assert_eq!(differing(&r1, &r2), 0, "{id} at {block}: right differs");
        }
        a_in.stop();
        a_out.stop();
    }
}

/// Notes at their samples, spread over the blocks — some inside later blocks — at block
/// sizes 1, 37, 64 and 512 (review #24).
#[test]
fn an_isolated_instrument_plays_its_notes_at_their_samples_as_in_process() {
    let _g = serial();
    let pool = pool();
    let note = |at: usize, key, kind| {
        (
            at,
            NoteEvent {
                offset: 0,
                note_id: i32::from(key),
                channel: 0,
                key,
                kind,
            },
        )
    };
    let score = [
        note(3, 60, NoteKind::On { velocity: 0.8 }),
        note(40, 64, NoteKind::On { velocity: 0.5 }),
        note(100, 60, NoteKind::Off { velocity: 0.0 }),
        note(700, 67, NoteKind::On { velocity: 0.7 }),
        note(1500, 64, NoteKind::Off { velocity: 0.0 }),
        note(2101, 72, NoteKind::On { velocity: 0.9 }),
        note(2900, 67, NoteKind::Off { velocity: 0.0 }),
    ];
    let frames = 4_000;
    for id in [SYNTH_ID, VST3_PULSE_ID] {
        for block in [1usize, 37, 64, 512] {
            let Loaded {
                main: _m_in,
                audio: mut a_in,
            } = load_within(id);
            let Loaded {
                main: _m_out,
                audio: mut a_out,
            } = load_offline(&pool, id, "together");
            let mut outs = Vec::new();
            for audio in [a_in.as_mut(), a_out.as_mut()] {
                let mut all = Vec::new();
                let mut at = 0;
                while at < frames {
                    let n = block.min(frames - at);
                    let notes: Vec<NoteEvent> = score
                        .iter()
                        .filter(|(t, _)| (at..at + n).contains(t))
                        .map(|(t, e)| NoteEvent {
                            offset: (t - at) as u32,
                            ..*e
                        })
                        .collect();
                    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
                    call(audio, &mut l, &mut r, &[], &notes);
                    all.extend(l);
                    all.extend(r);
                    at += n;
                }
                outs.push(all);
            }
            assert!(peak(&outs[0]) > 0.0, "{id} sounded");
            assert_eq!(
                differing(&outs[0], &outs[1]),
                0,
                "{id} at {block}: isolated differs"
            );
        }
    }
}

/// The sidechain reaches an isolated plug-in bit for bit (review #29).
#[test]
fn an_isolated_gain_hears_its_sidechain_as_in_process() {
    let _g = serial();
    let pool = pool();
    for block in [1usize, 37, 64, 512] {
        let Loaded {
            main: _m_in,
            audio: mut a_in,
        } = load_within(GAIN_ID);
        let Loaded {
            main: _m_out,
            audio: mut a_out,
        } = load_offline(&pool, GAIN_ID, "together");
        let (l0, r0) = (noise(3_000, 21), noise(3_000, 22));
        let (kl, kr) = (noise(3_000, 23), noise(3_000, 24));
        let on = [
            ParamChange {
                offset: 0,
                id: P_KEY,
                value: 1.0,
            },
            ParamChange {
                offset: 0,
                id: P_GAIN,
                value: -9.0,
            },
        ];
        let mut outs = Vec::new();
        for audio in [a_in.as_mut(), a_out.as_mut()] {
            let (mut l, mut r) = (l0.clone(), r0.clone());
            let mut at = 0;
            while at < l.len() {
                let end = (at + block).min(l.len());
                let p: &[ParamChange] = if at == 0 { &on } else { &[] };
                let mut out = cantus_host::ParamOut::new();
                audio.process(
                    cantus_host::Audio {
                        left: &mut l[at..end],
                        right: &mut r[at..end],
                        key: Some((&kl[at..end], &kr[at..end])),
                        transport: None,
                    },
                    p,
                    &[],
                    &mut out,
                );
                at = end;
            }
            outs.push((l, r));
        }
        let gained: Vec<f32> = l0.iter().map(|x| x * 10f32.powf(-9.0 / 20.0)).collect();
        assert_ne!(differing(&outs[0].0, &gained), 0, "the key was heard");
        assert_eq!(differing(&outs[0].0, &outs[1].0), 0, "left at {block}");
        assert_eq!(differing(&outs[0].1, &outs[1].1), 0, "right at {block}");
    }
}

#[test]
fn a_plugin_that_aborts_in_process_leaves_the_block_dry_and_says_so_once() {
    let _g = serial();
    let pool = pool();
    for (id, fault_id) in [(GAIN_ID, P_FAULT), (VST3_GAIN_ID, V_FAULT)] {
        let Loaded {
            mut main,
            mut audio,
        } = load_isolated(&pool, id, &format!("own-abort-{id}"));
        // −6 dB first, so a block it processed differs from one that passed dry (#21).
        let (l0, r0) = (noise(256, 3), noise(256, 4));
        let (mut l, mut r) = (l0.clone(), r0.clone());
        let gain_id = if id == GAIN_ID { P_GAIN } else { V_GAIN };
        call(
            audio.as_mut(),
            &mut l,
            &mut r,
            &[ParamChange {
                offset: 0,
                id: gain_id,
                value: -6.0,
            }],
            &[],
        );
        assert!(!same_bits(&l, &l0), "{id}: processed at −6 dB");
        // Each block its own input, so a stale output cannot pass for a dry one.
        let (l0, r0) = (noise(256, 30), noise(256, 31));
        let t = Instant::now();
        let (mut l, mut r) = (l0.clone(), r0.clone());
        call(
            audio.as_mut(),
            &mut l,
            &mut r,
            &fault(fault_id, ABORT_PROCESS),
            &[],
        );
        assert!(
            t.elapsed()
                < deadline(256, SR, cantus_host::bridge::audio::DEADLINE_FLOOR)
                    + Duration::from_millis(50),
            "{id}: the audio thread waited {:?}",
            t.elapsed()
        );
        assert!(same_bits(&l, &l0) && same_bits(&r, &r0), "{id}: dry");
        // Every later block passes dry at once.
        for k in 0..10 {
            let t = Instant::now();
            let (l0, r0) = (noise(256, 40 + k), noise(256, 60 + k));
            let (mut l, mut r) = (l0.clone(), r0.clone());
            call(audio.as_mut(), &mut l, &mut r, &[], &[]);
            assert!(
                same_bits(&l, &l0) && same_bits(&r, &r0),
                "{id}: dry after the crash"
            );
            assert!(t.elapsed() < Duration::from_millis(5), "{:?}", t.elapsed());
        }
        let reason = crashed_within(main.as_mut(), Duration::from_secs(5)).expect("told");
        assert!(!reason.is_empty());
        assert!(
            crashed_within(main.as_mut(), Duration::from_millis(100)).is_none(),
            "{id}: said once"
        );
        // Gone: saving answers with what it said last, never an error.
        assert!(main.save_state().is_ok(), "{id}: the last-good state");
    }
}

/// A plug-in with latency that crashes: the block passes its input delayed by the latency
/// — where the plug-in's output would have been — from the crashing block on (#21).
#[test]
fn a_crash_with_latency_plays_the_input_delayed_by_the_latency() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load_isolated(&pool, GAIN_ID, "own-abort-latency");
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &[ParamChange {
            offset: 0,
            id: P_LATENCY,
            value: 32.0,
        }],
        &[],
    );
    // It asks for a restart (its latency changed): the host restarts it.
    let asked = Instant::now();
    while !main
        .idle()
        .iter()
        .any(|n| matches!(n, Notice::RestartNeeded | Notice::LatencyChanged(_)))
    {
        assert!(asked.elapsed() < Duration::from_secs(5), "never asked");
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(audio);
    let mut audio = None;
    for _ in 0..100 {
        if let Ok(Some(a)) = main.restart() {
            audio = Some(a);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut audio = audio.expect("restarted with its latency");
    assert_eq!(main.latency(), 32);
    let n = 256;
    let (la, ra) = (noise(n, 70), noise(n, 71));
    let (mut l, mut r) = (la.clone(), ra.clone());
    call(audio.as_mut(), &mut l, &mut r, &[], &[]);
    let (lb, rb) = (noise(n, 72), noise(n, 73));
    let (mut l, mut r) = (lb.clone(), rb.clone());
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, ABORT_PROCESS),
        &[],
    );
    let delayed = |a: &[f32], b: &[f32]| -> Vec<f32> {
        a[n - 32..].iter().chain(&b[..n - 32]).copied().collect()
    };
    assert_eq!(differing(&l, &delayed(&la, &lb)), 0, "left, delayed");
    assert_eq!(differing(&r, &delayed(&ra, &rb)), 0, "right, delayed");
    let (lc, rc) = (noise(n, 74), noise(n, 75));
    let (mut l, mut r) = (lc.clone(), rc.clone());
    call(audio.as_mut(), &mut l, &mut r, &[], &[]);
    assert_eq!(differing(&l, &delayed(&lb, &lc)), 0, "and the block after");
    assert_eq!(differing(&r, &delayed(&rb, &rc)), 0);
}

#[test]
fn a_plugin_that_hangs_never_holds_the_audio_thread_past_the_deadline() {
    let _g = serial();
    let pool = pool();
    pool.set_hang_limit(Duration::from_millis(500));
    let Loaded {
        mut main,
        mut audio,
    } = load_isolated(&pool, GAIN_ID, "own-hang");
    let (l0, r0) = (noise(128, 5), noise(128, 6));
    let mut worst = Duration::ZERO;
    let mut after_late = Vec::new();
    let mut dry = 0;
    let started = Instant::now();
    for k in 0..200 {
        let (mut l, mut r) = (l0.clone(), r0.clone());
        let p: &[ParamChange] = if k == 2 {
            &fault(P_FAULT, HANG_PROCESS)
        } else {
            &[]
        };
        let t = Instant::now();
        call(audio.as_mut(), &mut l, &mut r, p, &[]);
        worst = worst.max(t.elapsed());
        if k > 2 {
            after_late.push(t.elapsed());
        }
        if k > 2 {
            assert!(same_bits(&l, &l0), "dry while it hangs");
            dry += 1;
        }
        // The interface's frame: never blocked by the hang.
        let t = Instant::now();
        let n = main.idle();
        assert!(
            t.elapsed() < Duration::from_millis(50),
            "idle took {:?}",
            t.elapsed()
        );
        if n.iter().any(|n| matches!(n, Notice::Crashed { .. })) {
            break;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(dry > 0);
    let floor = cantus_host::bridge::audio::DEADLINE_FLOOR;
    assert!(
        worst < deadline(128, SR, floor) + Duration::from_millis(5),
        "the audio thread waited {worst:?}"
    );
    // The late job is never waited for again: every block after it passes at once (#20),
    // and only that job counts as late (#35).
    after_late.sort();
    let median = after_late[after_late.len() / 2];
    assert!(
        median < Duration::from_millis(1),
        "after the late one: {median:?}"
    );
    assert!(
        after_late.iter().all(|t| *t < deadline(128, SR, floor)),
        "a block waited out a deadline again: {after_late:?}"
    );
    assert_eq!(main.late_blocks(), 1);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the hang was ended: {:?}",
        started.elapsed()
    );
}

#[test]
fn a_crash_while_saving_keeps_the_last_good_state() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load_isolated(&pool, GAIN_ID, "own-save");
    let mut out = cantus_host::ParamOut::new();
    main.flush(
        &[ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -6.0,
        }],
        &mut out,
    );
    let good = main.save_state().expect("a state");
    assert_eq!(main.param_value(P_GAIN), Some(-6.0));
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, ABORT_SAVE),
        &[],
    );
    arrived(main.as_mut(), P_FAULT, ABORT_SAVE);
    let saved = main
        .save_state()
        .expect("the last-good state, not an error");
    assert_eq!(saved, good);
    assert!(crashed_within(main.as_mut(), Duration::from_secs(5)).is_some());
}

#[test]
fn a_crash_while_loading_a_state_or_activating_is_an_error_and_a_crash() {
    let _g = serial();
    let pool = pool();
    for (fault_value, what) in [(ABORT_LOAD, "load"), (ABORT_ACTIVATE, "activate")] {
        let Loaded {
            mut main,
            mut audio,
        } = load_isolated(&pool, GAIN_ID, &format!("own-{what}"));
        let state = main.save_state().unwrap();
        let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
        call(
            audio.as_mut(),
            &mut l,
            &mut r,
            &fault(P_FAULT, fault_value),
            &[],
        );
        arrived(main.as_mut(), P_FAULT, fault_value);
        let failed = if what == "load" {
            main.load_state(&state).is_err()
        } else {
            // A latency change asks for a restart: deactivate, activate.
            call(
                audio.as_mut(),
                &mut l,
                &mut r,
                &[ParamChange {
                    offset: 0,
                    id: P_LATENCY,
                    value: 32.0,
                }],
                &[],
            );
            drop(audio);
            let mut failed = false;
            for _ in 0..50 {
                match main.restart() {
                    Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    Ok(Some(_)) => break,
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            failed
        };
        assert!(failed, "{what}: an error, not a hang");
        assert!(
            crashed_within(main.as_mut(), Duration::from_secs(5)).is_some(),
            "{what}: told"
        );
    }
}

#[cfg(windows)]
#[test]
fn a_crash_when_the_editor_opens_is_an_error_and_a_crash() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        mut main,
        mut audio,
    } = load_isolated(&pool, GAIN_ID, "own-editor");
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(
        audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, ABORT_EDITOR),
        &[],
    );
    arrived(main.as_mut(), P_FAULT, ABORT_EDITOR);
    let parent = raw_window_handle::RawWindowHandle::Win32(
        raw_window_handle::Win32WindowHandle::new(std::num::NonZeroIsize::new(1).unwrap()),
    );
    assert!(main.open_editor(parent).is_err());
    assert!(crashed_within(main.as_mut(), Duration::from_secs(5)).is_some());
}

#[test]
fn together_shares_one_process_and_one_crash_takes_both_individually_does_not() {
    let _g = serial();
    let pool = pool();
    let Loaded {
        main: mut a_main,
        audio: mut a_audio,
    } = load_isolated(&pool, GAIN_ID, "together-test");
    let Loaded {
        main: mut b_main,
        audio: _b_audio,
    } = load_isolated(&pool, GAIN_ID, "together-test");
    let Loaded {
        main: mut c_main,
        audio: mut c_audio,
    } = load_isolated(&pool, GAIN_ID, "own-c");
    assert_eq!(a_main.isolated_in(), b_main.isolated_in());
    assert_ne!(a_main.isolated_in(), c_main.isolated_in());
    let (mut l, mut r) = (vec![0.0; 64], vec![0.0; 64]);
    call(
        a_audio.as_mut(),
        &mut l,
        &mut r,
        &fault(P_FAULT, ABORT_PROCESS),
        &[],
    );
    assert!(crashed_within(a_main.as_mut(), Duration::from_secs(5)).is_some());
    assert!(crashed_within(b_main.as_mut(), Duration::from_secs(5)).is_some());
    // The other process plays on.
    let l0 = noise(64, 9);
    let (mut l, mut r) = (l0.clone(), l0.clone());
    call(
        c_audio.as_mut(),
        &mut l,
        &mut r,
        &[ParamChange {
            offset: 0,
            id: P_GAIN,
            value: -6.0,
        }],
        &[],
    );
    assert!(!same_bits(&l, &l0), "the separate one still processes");
    assert!(crashed_within(c_main.as_mut(), Duration::from_millis(200)).is_none());
}

#[test]
fn a_missing_helper_is_an_error_not_a_panic() {
    let _g = serial();
    let pool = cantus_host::bridge::Sandboxes::new("no-such-cantus-sandbox.exe".into());
    let info = info(GAIN_ID);
    let r = bridged(&pool, Format::Clap, "together", false).load(&info, setup());
    assert!(matches!(r, Err(HostError::Load(_))));
}
