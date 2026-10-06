//! Reproduce (and check) an ASIO driver running output and input together, with cpal
//! alone — no Cantus engine in the way. Plays **silence**, counts both directions'
//! callbacks every quarter second and says when either stops.
//!
//! ```text
//! cargo run -p cantus-engine --features asio --example asio_duplex -- "<driver>" [mode] [seconds]
//! ```
//!
//! `mode`: `out` (output alone), `duplex` (output built and started, then the input — what
//! Cantus did up to round 20), `in-first` (input, then output), `build-both` (both built,
//! then both played). Never name a driver whose interface is unplugged (loading the UMC
//! driver with no UMC attached crashed the process in round 19).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let _ = env_logger::builder().try_init();
    let args: Vec<String> = std::env::args().collect();
    let name = args.get(1).cloned().unwrap_or_else(|| "ASIO4ALL v2".into());
    let mode = args.get(2).cloned().unwrap_or_else(|| "duplex".into());
    let secs: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
    // `pump`: dispatch this thread's window messages while waiting, as a GUI thread does
    // (an apartment-threaded ASIO driver may run on the loading thread's messages).
    let pump = args.get(4).is_some_and(|a| a == "pump");

    let host = cpal::host_from_id(cpal::HostId::Asio).expect("the ASIO host");
    // Only the driver named is loaded (the patched cpal's `device_by_id`), never the
    // others — an unplugged interface's driver must not be touched.
    let device = host
        .device_by_id(&cpal::DeviceId::new(cpal::HostId::Asio, &name))
        .unwrap_or_else(|| panic!("no ASIO driver named {name} (or it did not load)"));
    let oc = device.default_output_config().expect("an output config");
    println!("{name}: output {oc:?}");
    let ic = device.default_input_config();
    println!("{name}: input {ic:?}");

    let outs = Arc::new(AtomicU64::new(0));
    let ins = Arc::new(AtomicU64::new(0));
    let errors = Arc::new(AtomicU64::new(0));

    let build_out = || {
        let n = outs.clone();
        let e = errors.clone();
        device
            .build_output_stream_raw(
                oc.config(),
                oc.sample_format(),
                move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {
                    // Silence in every format ASIO speaks (signed integers and floats).
                    data.bytes_mut().fill(0);
                    n.fetch_add(1, Ordering::Relaxed);
                },
                move |err| {
                    e.fetch_add(1, Ordering::Relaxed);
                    eprintln!("output error: {err} ({:?})", err.kind());
                },
                None,
            )
            .expect("the output stream")
    };
    let build_in = || {
        let ic = ic.as_ref().expect("an input config").clone();
        let n = ins.clone();
        let e = errors.clone();
        device
            .build_input_stream_raw(
                ic.config(),
                ic.sample_format(),
                move |_: &cpal::Data, _: &cpal::InputCallbackInfo| {
                    n.fetch_add(1, Ordering::Relaxed);
                },
                move |err| {
                    e.fetch_add(1, Ordering::Relaxed);
                    eprintln!("input error: {err} ({:?})", err.kind());
                },
                None,
            )
            .expect("the input stream")
    };

    let t0 = Instant::now();
    let mut streams = Vec::new();
    match mode.as_str() {
        "out" => {
            let s = build_out();
            s.play().unwrap();
            streams.push(s);
        }
        "duplex" => {
            let s = build_out();
            s.play().unwrap();
            streams.push(s);
            std::thread::sleep(Duration::from_millis(200));
            let s = build_in();
            s.play().unwrap();
            streams.push(s);
        }
        "in-first" => {
            let s = build_in();
            s.play().unwrap();
            streams.push(s);
            let s = build_out();
            s.play().unwrap();
            streams.push(s);
        }
        "build-both" => {
            let o = build_out();
            let i = build_in();
            o.play().unwrap();
            i.play().unwrap();
            streams.push(o);
            streams.push(i);
        }
        other => panic!("unknown mode {other}"),
    }
    println!("built in {:?}; buffer {:?}", t0.elapsed(), streams[0].buffer_size());

    let t0 = Instant::now();
    let (mut last_out, mut last_in) = (0, 0);
    let (mut still_out, mut still_in) = (Duration::ZERO, Duration::ZERO);
    let (mut worst_out, mut worst_in) = (Duration::ZERO, Duration::ZERO);
    let step = Duration::from_millis(250);
    while t0.elapsed() < Duration::from_secs(secs) {
        if pump {
            let until = Instant::now() + step;
            while Instant::now() < until {
                win::pump();
                std::thread::sleep(Duration::from_millis(2));
            }
        } else {
            std::thread::sleep(step);
        }
        let (o, i) = (outs.load(Ordering::Relaxed), ins.load(Ordering::Relaxed));
        // Gaps count once a direction has started (ASIO4ALL takes a second to start).
        still_out = if o == last_out && o > 0 { still_out + step } else { Duration::ZERO };
        still_in = if i == last_in && i > 0 { still_in + step } else { Duration::ZERO };
        worst_out = worst_out.max(still_out);
        worst_in = worst_in.max(still_in);
        if t0.elapsed().as_millis() % 1000 < 250 || still_out >= Duration::from_millis(500) {
            println!(
                "{:>5.2} s  out {o:>6}  in {i:>6}{}",
                t0.elapsed().as_secs_f64(),
                if still_out >= Duration::from_millis(500) { "  << output stalled" } else { "" }
            );
        }
        (last_out, last_in) = (o, i);
    }
    let stalled = worst_out >= Duration::from_millis(500)
        || outs.load(Ordering::Relaxed) == 0
        || (streams.len() > 1
            && (worst_in >= Duration::from_millis(500) || ins.load(Ordering::Relaxed) == 0));
    println!(
        "RESULT {name} {mode}: out {} in {} callbacks in {secs} s, longest silence out {:?} in {:?}, errors {} -> {}",
        outs.load(Ordering::Relaxed),
        ins.load(Ordering::Relaxed),
        worst_out,
        worst_in,
        errors.load(Ordering::Relaxed),
        if stalled { "STALLED" } else { "ok" }
    );
    drop(streams);
}

/// The calling thread's window messages, dispatched (what a GUI thread's loop does).
mod win {
    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: [i32; 2],
        private: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn PeekMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
    }
    pub fn pump() {
        const PM_REMOVE: u32 = 1;
        let mut m = std::mem::MaybeUninit::<Msg>::zeroed();
        // SAFETY: plain Win32 calls on this thread's queue with a zeroed MSG.
        unsafe {
            while PeekMessageW(m.as_mut_ptr(), 0, 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(m.as_ptr());
                DispatchMessageW(m.as_ptr());
            }
        }
    }
}
