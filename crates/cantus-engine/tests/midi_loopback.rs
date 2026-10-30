//! MIDI clock out through a real loopback (round 33, ADR 0079): on Linux an ALSA virtual
//! input port stands for the receiving device; the engine's MIDI-out thread opens it by
//! name like any port and sends a clock every 20.8 ms; the port's callback stamps what
//! arrives. The intervals' spread is the jitter at the port. Where ALSA's sequencer is not
//! there (a container, WSL without `snd-seq`, CI's runners), the test prints
//! `SKIPPED …` on stderr and passes — it cannot fail there, so the loopback is owner-run
//! (`docs/beta-checklist.md`); Windows needs a loopback driver (loopMIDI, Windows MIDI
//! Services).

#[cfg(target_os = "linux")]
#[test]
fn clock_out_through_an_alsa_loopback() {
    use cantus_engine::transport::sync::{self, out, HostLine, OutKind, Stamped};
    use midir::os::unix::VirtualInput;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    let Ok(input) = midir::MidiInput::new("cantus-loopback-test") else {
        eprintln!("SKIPPED clock_out_through_an_alsa_loopback: no ALSA sequencer here (the loopback is owner-run)");
        return;
    };
    type Arrivals = Vec<(i64, Vec<u8>)>;
    let got: Arc<Mutex<Arrivals>> = Arc::default();
    let g2 = got.clone();
    let mut input = input;
    input.ignore(midir::Ignore::ActiveSense);
    let Ok(_conn) = input.create_virtual(
        "cantus-loopback",
        move |_stamp, bytes, _| {
            g2.lock()
                .unwrap()
                .push((sync::host_micros(), bytes.to_vec()));
        },
        (),
    ) else {
        eprintln!("SKIPPED clock_out_through_an_alsa_loopback: no virtual ports here (the loopback is owner-run)");
        return;
    };
    let name = midir::MidiOutput::new("cantus-loopback-scan")
        .ok()
        .and_then(|o| {
            o.ports()
                .iter()
                .filter_map(|p| o.port_name(p).ok())
                .find(|n| n.contains("cantus-loopback"))
        });
    let Some(name) = name else {
        eprintln!("SKIPPED clock_out_through_an_alsa_loopback: the virtual port is not listed (the loopback is owner-run)");
        return;
    };
    let (mut port, mut handle) = sync::pair();
    let t0 = sync::host_micros() + 100_000;
    handle.shared.host_line.publish(&HostLine {
        s0: 0,
        a: t0 as f64,
        b: 1e6 / 48_000.0,
        latency_us: 0.0,
    });
    let thread = out::MidiOut::start(
        handle.out.take().unwrap(),
        handle.shared.clone(),
        cantus_engine::midi::pool::OutPool::new(out::connect_system),
    );
    thread.set_ports(vec![out::PortChoice {
        name,
        clock: true,
        mtc: false,
        offset_ms: 0.0,
    }]);
    std::thread::sleep(Duration::from_millis(50));
    for k in 0..240u64 {
        port.out
            .push(Stamped::new(k * 1_000, OutKind::Clock, &[0xF8]))
            .unwrap();
    }
    std::thread::sleep(Duration::from_millis(5_400));
    drop(thread);
    let got = got.lock().unwrap().clone();
    assert_eq!(got.len(), 240, "every clock arrived");
    let mut worst = 0.0f64;
    for (k, (at, msg)) in got.iter().enumerate() {
        assert_eq!(msg, &vec![0xF8]);
        let due = t0 as f64 + k as f64 * 1_000.0 * 1e6 / 48_000.0;
        worst = worst.max((*at as f64 - due).abs());
    }
    println!(
        "ALSA loopback: 240 clocks, worst deviation from the schedule {:.3} ms",
        worst / 1e3
    );
    assert!(worst < 2_000.0, "{worst} µs");
}
