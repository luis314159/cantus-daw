//! Devices (ADR 0025, round 19): enumeration, sample conversion into the input ring, and
//! the loopback correlator. The tests marked `#[ignore]` open the machine's real audio
//! devices: `cargo test -p cantus-engine --test devices -- --ignored --nocapture`.

use cantus_engine::devices::{self, InputSample, OutputSample};
use cantus_engine::loopback::{self, Probe};
use cantus_engine::{AudioConfig, EngineHandle, LoopbackError};

#[test]
fn enumeration_never_panics() {
    let drivers = devices::drivers();
    // The default driver, when there is one, comes first and is the only default.
    assert!(drivers.iter().filter(|d| d.is_default).count() <= 1);
    if let Some(first) = drivers.first() {
        assert!(first.is_default);
    }
    let _ = devices::input_devices(None);
    let _ = devices::output_devices_on(None);
    // A driver this machine does not have falls back to the default, quietly.
    let _ = devices::input_devices(Some("no such driver"));
    let _ = devices::output_devices_on(Some("no such driver"));
    for d in &drivers {
        let inputs = devices::input_devices(Some(&d.name));
        assert!(inputs.iter().filter(|i| i.is_default).count() <= 1);
    }
}

#[test]
fn integer_formats_convert_exactly() {
    // i16: scaled by 1/32768, the ends and the middle land exactly.
    assert_eq!(i16::MIN.to_f32(), -1.0);
    assert_eq!(0i16.to_f32(), 0.0);
    assert_eq!(16_384i16.to_f32(), 0.5);
    assert_eq!(i16::MAX.to_f32(), 32_767.0 / 32_768.0);
    for v in [-32_768i16, -12_345, -1, 0, 1, 777, 32_767] {
        assert_eq!(v.to_f32(), v as f32 / 32_768.0);
    }
    // i32: 1/2^31.
    assert_eq!(i32::MIN.to_f32(), -1.0);
    assert_eq!((1i32 << 30).to_f32(), 0.5);
    assert_eq!((-(1i32 << 29)).to_f32(), -0.25);
    assert_eq!(0i32.to_f32(), 0.0);
    // u16: offset to the midpoint, then 1/32768.
    assert_eq!(0u16.to_f32(), -1.0);
    assert_eq!(32_768u16.to_f32(), 0.0);
    assert_eq!(49_152u16.to_f32(), 0.5);
    assert_eq!(u16::MAX.to_f32(), 32_767.0 / 32_768.0);
    // The rest of the family.
    assert_eq!(0u8.to_f32(), -1.0);
    assert_eq!(128u8.to_f32(), 0.0);
    assert_eq!((-64i8).to_f32(), -0.5);
    assert_eq!(cpal::I24::new(1 << 22).unwrap().to_f32(), 0.5);
    assert_eq!(cpal::I24::new(-(1 << 23)).unwrap().to_f32(), -1.0);
    assert_eq!(cpal::U24::new(1 << 23).unwrap().to_f32(), 0.0);
    assert_eq!((1u32 << 31).to_f32(), 0.0);
    assert_eq!(0.25f64.to_f32(), 0.25);
}

#[test]
fn output_formats_round_trip_every_step() {
    // Every i16 survives f32 and back, and so does u16.
    for v in i16::MIN..=i16::MAX {
        assert_eq!(i16::from_f32(v.to_f32()), v);
    }
    for v in 0..=u16::MAX {
        assert_eq!(u16::from_f32(v.to_f32()), v);
    }
    for v in [i32::MIN, -(1 << 30), 0, 1 << 30] {
        assert_eq!(i32::from_f32(v.to_f32()), v);
    }
    // Out of range clamps; not-a-number is silence.
    assert_eq!(i16::from_f32(2.0), i16::MAX);
    assert_eq!(i16::from_f32(-2.0), i16::MIN);
    assert_eq!(i16::from_f32(f32::NAN), 0);
    assert_eq!(i32::from_f32(1.0), i32::MAX);
}

#[test]
fn the_ring_takes_converted_samples_and_drops_what_does_not_fit() {
    let (mut tx, mut rx) = rtrb::RingBuffer::<f32>::new(6);
    let data: [i16; 4] = [-32_768, -16_384, 0, 16_384];
    assert_eq!(devices::push_converted(&mut tx, &data), 0);
    let wide: [u16; 4] = [0, 32_768, 49_152, 65_535];
    // Two fit, two are dropped (and counted), never blocking.
    assert_eq!(devices::push_converted(&mut tx, &wide), 2);
    let got: Vec<f32> = std::iter::from_fn(|| rx.pop().ok()).collect();
    assert_eq!(got, vec![-1.0, -0.5, 0.0, 0.5, -1.0, 0.0]);
    let ints: [i32; 2] = [1 << 30, i32::MIN];
    assert_eq!(devices::push_converted(&mut tx, &ints), 0);
    assert_eq!(rx.pop(), Ok(0.5));
    assert_eq!(rx.pop(), Ok(-1.0));
}

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

#[test]
fn the_correlator_finds_a_known_delay_to_the_sample() {
    let rate = 48_000.0;
    let signal = loopback::probe_signal(rate);
    let mut seed = 99u32;
    for delay in [0usize, 1, 37, 480, 4_801, 30_000] {
        // Quieter, inverted (a polarity flip in the cable) and buried in noise.
        let mut rec = vec![0.0f32; signal.len() + 48_000];
        for (i, x) in signal.iter().enumerate() {
            rec[i + delay] = -0.2 * x;
        }
        for x in rec.iter_mut() {
            *x += 0.02 * noise(&mut seed);
        }
        let (lag, confidence) = loopback::find_delay(&signal, &rec, 48_000).unwrap();
        assert_eq!(lag, delay, "delay {delay}");
        assert!(
            confidence > loopback::MIN_CONFIDENCE,
            "confidence {confidence}"
        );
        // Every burst is heard on its own at that lag, and none a sample off (white noise).
        assert_eq!(loopback::bursts_agreeing(&signal, &rec, lag, rate), 8);
        assert_eq!(loopback::bursts_agreeing(&signal, &rec, lag + 1, rate), 0);
    }
    // Noise alone stands nowhere near.
    let rec: Vec<f32> = (0..signal.len() + 48_000)
        .map(|_| 0.1 * noise(&mut seed))
        .collect();
    let (_, confidence) = loopback::find_delay(&signal, &rec, 48_000).unwrap();
    assert!(
        confidence < loopback::MIN_CONFIDENCE,
        "noise read {confidence}"
    );
    for lag in [0, 100, 4_000] {
        assert!(loopback::bursts_agreeing(&signal, &rec, lag, rate) < loopback::MIN_BURSTS);
    }
    // Silence is no answer at all.
    assert!(loopback::find_delay(&signal, &vec![0.0; 1000], 500).is_none());
}

/// Drive a probe as the engine's callbacks would, through a simulated cable: the output
/// plays in blocks of `block`; the input device hands over every frame captured more
/// than `jitter` frames ago, and what it captures is the output `delay` frames earlier.
fn simulate(block: usize, delay: usize, jitter: usize, in_channels: usize) -> (Probe, usize) {
    let rate = 48_000.0;
    let mut probe = Probe::new(rate, in_channels);
    let (mut tx, mut rx) = rtrb::RingBuffer::<f32>::new(1 << 20);
    let mut played: Vec<f32> = Vec::new();
    let mut delivered = 0usize;
    let mut pos = 0usize;
    let mut first = None;
    let mut out = vec![0.0f32; block * 2];
    loop {
        // The input that has arrived by this callback.
        let due = pos.saturating_sub(jitter);
        while delivered < due {
            let v = delivered
                .checked_sub(delay)
                .and_then(|i| played.get(i).copied())
                .unwrap_or(0.0);
            for ch in 0..in_channels {
                // The cable is on the last channel; the others hear nothing.
                tx.push(if ch + 1 == in_channels { 0.5 * v } else { 0.0 })
                    .unwrap();
            }
            delivered += 1;
        }
        if first.is_none() && delivered > 0 {
            first = Some(pos);
        }
        let done = probe.run(&mut out, 2, &mut rx);
        played.extend(out.chunks(2).map(|f| f[0]));
        pos += block;
        if done {
            break;
        }
    }
    (probe, first.unwrap())
}

#[test]
fn the_probe_measures_the_round_trip_the_processor_sees() {
    for (block, delay, jitter, channels) in [
        (256, 300, 0, 2),
        (256, 1_234, 100, 1),
        (64, 4_000, 1_000, 4),
        (1024, 17, 512, 2),
    ] {
        let (probe, first_pop) = simulate(block, delay, jitter, channels);
        // The input device reports its buffer and the jitter (capture → callback of a
        // buffer's first frame); the output reports nothing (the cable is the rest).
        let reported = (block + jitter) as u32;
        let r = loopback::evaluate(&probe, 48_000.0, reported).unwrap();
        // The frame captured at input index k is processed at output frame first + k.
        assert_eq!(
            r.round_trip as usize,
            first_pop + delay,
            "{block} {delay} {jitter}"
        );
        // The ring's wait as the processor would count it (no clock: the block stands
        // for the input's buffer, just arrived).
        assert_eq!(
            r.queue,
            first_pop as i64 - jitter as i64 - block as i64,
            "{block} {delay} {jitter}"
        );
        assert_eq!(r.offset, r.round_trip as i64 - reported as i64 - r.queue);
        // What the drivers report plus the ring's wait is capture → placement exactly,
        // so the offset left over is the cable alone.
        assert_eq!(r.offset, delay as i64, "{block} {delay} {jitter}");
        assert_eq!(r.channel, channels - 1);
        assert!(r.confidence > loopback::MIN_CONFIDENCE);
        assert_eq!(r.bursts, 8);
    }
}

#[test]
fn a_probe_with_no_cable_says_so() {
    let rate = 48_000.0;
    let mut probe = Probe::new(rate, 2);
    let (mut tx, mut rx) = rtrb::RingBuffer::<f32>::new(1 << 20);
    let mut out = vec![0.0f32; 512];
    let mut seed = 5u32;
    loop {
        for _ in 0..512 {
            tx.push(0.01 * noise(&mut seed)).unwrap();
        }
        if probe.run(&mut out, 2, &mut rx) {
            break;
        }
    }
    assert_eq!(
        loopback::evaluate(&probe, rate, 0),
        Err(LoopbackError::NotFound)
    );
    // And one that heard nothing at all.
    let mut probe = Probe::new(rate, 2);
    let (_tx, mut rx) = rtrb::RingBuffer::<f32>::new(16);
    while !probe.run(&mut out, 2, &mut rx) {}
    assert_eq!(
        loopback::evaluate(&probe, rate, 0),
        Err(LoopbackError::NoInputData)
    );
}

#[test]
fn a_handle_without_a_device_refuses_to_measure() {
    let mut h = EngineHandle::dummy();
    assert_eq!(h.begin_loopback(), Err(LoopbackError::NoDevice));
    assert!(!h.is_measuring());
    assert!(h.poll_loopback().is_none());
    assert_eq!(h.driver_name(), "none");
    assert_eq!(h.device_latency(), Default::default());
    assert!(h.input_error().is_none());
    assert_eq!(h.input_channel_count(), 0);
    assert!(!h.output_stalled());
    assert_eq!(h.device_callbacks(), (0, 0));
}

// --- the machine's own devices -------------------------------------------------------

/// The configuration the live tests open: the default, or the driver and devices named by
/// `CANTUS_TEST_DRIVER`, `CANTUS_TEST_OUTPUT` and `CANTUS_TEST_INPUT`.
fn live_config() -> AudioConfig {
    let _ = env_logger::builder().is_test(true).try_init();
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    AudioConfig {
        driver: var("CANTUS_TEST_DRIVER"),
        device_name: var("CANTUS_TEST_OUTPUT"),
        // `none` opens no input.
        input_device_name: var("CANTUS_TEST_INPUT").map(|v| {
            if v == "none" {
                String::new()
            } else {
                v
            }
        }),
        ..Default::default()
    }
}

#[test]
#[ignore = "opens the machine's audio devices"]
fn live_streams_report_their_latencies() {
    let t0 = std::time::Instant::now();
    let mut h = EngineHandle::start(&live_config()).expect("an output device");
    println!("started in {:?}", t0.elapsed());
    let t0 = std::time::Instant::now();
    while t0.elapsed() < std::time::Duration::from_millis(800) {
        h.poll();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let l = h.device_latency();
    println!(
        "driver {} · out {} @ {} Hz · in {:?} ({} ch) · latency in {} out {} frames · input error {:?} · dropped {}",
        h.driver_name(),
        h.device_name,
        h.sample_rate,
        h.input_device_name(),
        h.input_channel_count(),
        l.input,
        l.output,
        h.input_error(),
        h.input_dropped(),
    );
    println!(
        "callbacks (in, out) after 0.8 s: {:?}",
        h.device_callbacks()
    );
    assert!(h.device_callbacks().1 > 0, "the output runs");
    assert!(l.output > 0, "the output reports a latency");
    if h.input_channel_count() > 0 {
        // Some ASIO drivers report an input latency of 0 (Realtek's does).
        // What a take is moved by follows the driver (read a moment apart).
        h.poll();
        let (now, rec) = (h.device_latency().input, h.record_latency().input);
        assert!(now.abs_diff(rec) <= 2, "{now} against {rec}");
    }
    for d in devices::drivers() {
        println!("driver {} default {}", d.name, d.is_default);
        for i in devices::input_devices(Some(&d.name)) {
            println!(
                "  in  {} · {} ch · default {}",
                i.name, i.channels, i.is_default
            );
        }
        for o in devices::output_devices_on(Some(&d.name)) {
            println!("  out {} · default {}", o.name, o.is_default);
        }
    }
}

#[test]
#[ignore = "plays bursts on the machine's output and listens on its input"]
fn live_loopback() {
    let mut h = EngineHandle::start(&live_config()).expect("an output device");
    std::thread::sleep(std::time::Duration::from_millis(1000));
    h.poll();
    println!("callbacks (in, out) before: {:?}", h.device_callbacks());
    if let Err(e) = h.begin_loopback() {
        println!("loopback not started: {e}");
        return;
    }
    let probe = loop {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.poll();
        if let Some(r) = h.take_loopback_probe() {
            println!("callbacks (in, out) after: {:?}", h.device_callbacks());
            match r {
                Ok(p) => break p,
                Err(e) => {
                    println!(
                        "no probe back: {e} (output stalled: {})",
                        h.output_stalled()
                    );
                    return;
                }
            }
        }
    };
    let reported = h.device_latency().round_trip();
    println!(
        "loopback: {:?}",
        loopback::evaluate(&probe, h.sample_rate, reported)
    );
    // `CANTUS_LOOPBACK_DUMP=<dir>`: the probe and what came back, as WAV files.
    if let Ok(dir) = std::env::var("CANTUS_LOOPBACK_DUMP") {
        let (rec, ch, first) = probe.recorded();
        println!("first pop at output frame {first:?}, {ch} channels");
        let spec = |channels| hound::WavSpec {
            channels,
            sample_rate: h.sample_rate as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let dir = std::path::PathBuf::from(dir);
        let mut w = hound::WavWriter::create(dir.join("signal.wav"), spec(1)).unwrap();
        for x in probe.signal() {
            w.write_sample(*x).unwrap();
        }
        w.finalize().unwrap();
        let mut w = hound::WavWriter::create(dir.join("recorded.wav"), spec(ch as u16)).unwrap();
        for x in rec {
            w.write_sample(*x).unwrap();
        }
        w.finalize().unwrap();
    }
}

/// Runs the configured devices for `CANTUS_TEST_SECONDS` (60 by default) and fails when
/// either direction stops calling back for half a second once it has started, or when the
/// device is reported lost. Plays nothing (the engine's output is silence without a
/// project). `CANTUS_TEST_INPUT=none` runs the output alone.
#[test]
#[ignore = "opens the machine's audio devices for a minute"]
fn live_streams_run_without_a_stall() {
    use std::time::{Duration, Instant};
    let secs: u64 = std::env::var("CANTUS_TEST_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let t0 = Instant::now();
    let mut h = EngineHandle::start(&live_config()).expect("an output device");
    println!(
        "started {} {} in {:?}: {} Hz, buffer {:?} (asked {:?}), input {:?} ({} ch), error {:?}",
        h.driver_name(),
        h.device_name,
        t0.elapsed(),
        h.sample_rate,
        h.device_buffer(),
        h.requested_buffer(),
        h.input_device_name(),
        h.input_channel_count(),
        h.input_error(),
    );
    let duplex = h.input_channel_count() > 0;
    let t0 = Instant::now();
    let (mut seen, mut since) = ((0u64, 0u64), (t0, t0));
    let (mut worst_out, mut worst_in) = (Duration::ZERO, Duration::ZERO);
    let mut first = (None, None);
    let mut last_print = t0;
    while t0.elapsed() < Duration::from_secs(secs) {
        std::thread::sleep(Duration::from_millis(20));
        h.poll();
        let (i, o) = h.device_callbacks();
        let now = Instant::now();
        if o != seen.1 {
            if first.1.is_none() {
                first.1 = Some(t0.elapsed());
            } else {
                worst_out = worst_out.max(now - since.1);
            }
            since.1 = now;
        }
        if i != seen.0 {
            if first.0.is_none() {
                first.0 = Some(t0.elapsed());
            } else {
                worst_in = worst_in.max(now - since.0);
            }
            since.0 = now;
        }
        seen = (i, o);
        if now - last_print >= Duration::from_secs(5) {
            last_print = now;
            println!(
                "{:>5.1} s: callbacks in {i} out {o}, state {:?}",
                t0.elapsed().as_secs_f64(),
                h.device_state()
            );
        }
    }
    // A direction that stopped and never came back counts up to the end.
    let now = Instant::now();
    worst_out = worst_out.max(now - since.1);
    if duplex {
        worst_in = worst_in.max(now - since.0);
    }
    println!(
        "first callback in {:?} out {:?}; longest gap in {worst_in:?} out {worst_out:?}; \
         callbacks {:?}; xruns {}; state {:?}; buffer {:?}",
        first.0,
        first.1,
        h.device_callbacks(),
        h.xruns(),
        h.device_state(),
        h.device_buffer(),
    );
    assert!(first.1.is_some(), "the output never called back");
    assert!(worst_out < Duration::from_millis(500), "the output stalled for {worst_out:?}");
    if duplex {
        assert!(first.0.is_some(), "the input never called back");
        assert!(worst_in < Duration::from_millis(500), "the input stalled for {worst_in:?}");
    }
    assert!(!h.device_lost(), "the device was reported lost");
}

// --- the device's state (round 21): stalled is not lost -----------------------------------

#[test]
fn a_stalled_driver_is_stalled_not_lost() {
    use devices::{classify, DeviceState as S, START_GRACE, STALL_AFTER};
    use std::time::Duration;
    let ms = Duration::from_millis;
    let open = ms(10_000);
    // Running: both directions called back a moment ago.
    assert_eq!(classify(false, false, open, Some(ms(5)), Some(Some(ms(5)))), S::Running);
    // The output stopped calling back while the device is still there: stalled — never
    // lost, which is only what a stream says when its device is gone.
    let stalled = classify(false, false, open, Some(STALL_AFTER + ms(1)), None);
    assert_eq!(stalled, S::Stalled);
    assert!(stalled.is_silent());
    // The input stopped while the output runs (the duplex stall of round 19): stalled too.
    assert_eq!(classify(false, false, open, Some(ms(5)), Some(Some(ms(1_500)))), S::Stalled);
    // An input that never called back, past the grace.
    assert_eq!(classify(false, false, open, Some(ms(5)), Some(None)), S::Stalled);
    // Slow to start (ASIO4ALL takes a second): starting, then stalled after the grace.
    assert_eq!(classify(false, false, ms(1_200), None, Some(None)), S::Starting);
    assert_eq!(classify(false, false, START_GRACE, None, None), S::Stalled);
    // The driver asked to be reset: a restart, not a lost device.
    assert_eq!(classify(false, true, open, Some(ms(5)), None), S::RestartRequested);
    // Gone is gone, whatever else.
    assert_eq!(classify(true, true, open, Some(ms(5)), None), S::Lost);
}

#[test]
fn the_beat_says_how_long_ago_a_callback_ran() {
    let beat = devices::Beat::default();
    assert_eq!(beat.since(), None);
    beat.tick();
    let since = beat.since().expect("a callback ran");
    assert!(since < std::time::Duration::from_secs(1));
}

#[test]
fn a_handle_without_a_device_is_off() {
    let h = EngineHandle::dummy();
    assert_eq!(h.device_state(), devices::DeviceState::Off);
    assert!(!h.device_lost());
    assert!(!h.output_stalled());
    assert_eq!(h.device_buffer(), None);
}

#[test]
fn a_reset_request_asks_for_a_restart_and_an_unplug_is_lost() {
    use std::sync::atomic::Ordering;
    let t = devices::DeviceTelemetry::default();
    // What cpal's ASIO host sends for a driver's reset/resync request or a rate it
    // changed, and WASAPI for invalidated resources: the device is still there.
    let reset = cpal::Error::with_message(
        cpal::ErrorKind::StreamInvalidated,
        "Stream reset was requested by the ASIO driver",
    );
    devices::stream_error(&t, &reset, "output");
    assert!(t.restart_requested.load(Ordering::Acquire));
    assert!(!t.stream_lost.load(Ordering::Acquire));
    // A dropout is counted, nothing else.
    devices::stream_error(&t, &cpal::Error::new(cpal::ErrorKind::Xrun), "output");
    assert_eq!(t.xruns.load(Ordering::Relaxed), 1);
    assert!(!t.stream_lost.load(Ordering::Acquire));
    // The device went away: lost.
    devices::stream_error(&t, &cpal::Error::new(cpal::ErrorKind::DeviceNotAvailable), "input");
    assert!(t.stream_lost.load(Ordering::Acquire));
}
