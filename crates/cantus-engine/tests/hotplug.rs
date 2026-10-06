//! The device watcher (round 20, audio setup): it lists the devices off the UI thread and
//! reports only changes; the UI's poll never blocks.

use cantus_engine::hotplug::{DeviceList, DeviceWatch};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn list(outputs: &[&str], inputs: &[&str]) -> DeviceList {
    DeviceList {
        outputs: outputs.iter().map(|s| s.to_string()).collect(),
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
        default_output: outputs.first().map(|s| s.to_string()),
        default_input: inputs.first().map(|s| s.to_string()),
    }
}

/// Poll until something arrives (or a second passes).
fn wait(w: &DeviceWatch) -> Option<DeviceList> {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(2) {
        if let Some(l) = w.poll() {
            return Some(l);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

#[test]
fn the_watcher_reports_the_first_list_then_only_changes() {
    let laptop = list(&["Speakers (Realtek(R) Audio)"], &["Microphone (Realtek(R) Audio)"]);
    let plugged = list(
        &["Speakers (Realtek(R) Audio)", "Speakers (UMC204HD 192k)"],
        &["Microphone (Realtek(R) Audio)", "Line (UMC204HD 192k)"],
    );
    let now = Arc::new(Mutex::new(laptop.clone()));
    let source = now.clone();
    let w = DeviceWatch::with_source(Duration::from_millis(10), move || {
        source.lock().unwrap().clone()
    });
    assert_eq!(wait(&w), Some(laptop.clone()), "the first list at once");
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(w.poll(), None, "no change, nothing sent");
    *now.lock().unwrap() = plugged.clone();
    let got = wait(&w).expect("the change arrives");
    assert_eq!(got, plugged);
    assert_eq!(
        got.added_since(&laptop),
        vec![
            "Speakers (UMC204HD 192k)".to_string(),
            "Line (UMC204HD 192k)".to_string()
        ]
    );
    *now.lock().unwrap() = laptop.clone();
    let back = wait(&w).expect("the removal arrives");
    assert_eq!(back.removed_since(&plugged).len(), 2);
    assert!(back.added_since(&plugged).is_empty());
    assert!(back.has("Microphone (Realtek(R) Audio)"));
    assert!(!back.has("Line (UMC204HD 192k)"));
}

#[test]
fn polling_never_waits_for_a_slow_scan() {
    // A scan that takes 300 ms (WASAPI can take tens): the poll returns at once.
    let w = DeviceWatch::with_source(Duration::from_millis(10), || {
        std::thread::sleep(Duration::from_millis(300));
        DeviceList::default()
    });
    let t0 = Instant::now();
    for _ in 0..100 {
        let _ = w.poll();
    }
    assert!(t0.elapsed() < Duration::from_millis(50));
    // Dropping the watch does not wait for the scan either.
    let t0 = Instant::now();
    drop(w);
    assert!(t0.elapsed() < Duration::from_millis(50));
}
