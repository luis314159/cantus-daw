//! The channel names of the interface in use, where every input menu can read them
//! (round 20): "Guitar" instead of "In 3" in a track's header, the inspector, the mixer.
//!
//! The UI is drawn on one thread; the table is that thread's, refreshed by
//! `audio_setup::tick` when the interface or its names change.

use std::cell::RefCell;

thread_local! {
    static INPUTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Replace the table (the interface in use, or none).
pub fn set_inputs(names: &[String]) {
    INPUTS.with(|n| {
        let mut n = n.borrow_mut();
        if n.as_slice() != names {
            n.clear();
            n.extend(names.iter().cloned());
        }
    });
}

/// Write input channel `first` (and `first + 1` for a pair) by name into `b`, when named.
/// A pair whose two channels are named alike ("Keys L", "Keys R") or only one is named
/// shows that name; two different names show both ("Mic / Guitar"). Returns whether it
/// wrote anything.
pub fn write_input(first: usize, stereo: bool, b: &mut String) -> bool {
    INPUTS.with(|n| {
        let n = n.borrow();
        let get = |c: usize| n.get(c).map(|s| s.trim()).filter(|s| !s.is_empty());
        match (get(first), stereo.then(|| get(first + 1)).flatten()) {
            (Some(a), None) => {
                b.push_str(a);
                true
            }
            (None, Some(c)) => {
                b.push_str(c);
                true
            }
            (Some(a), Some(c)) => {
                let stem = |s: &str| {
                    let mut x = s.to_string();
                    for suffix in [" Left", " Right", " L", " R", " left", " right"] {
                        if let Some(t) = s.strip_suffix(suffix) {
                            x = t.trim_end().to_string();
                            break;
                        }
                    }
                    x
                };
                if a == c || (!stem(a).is_empty() && stem(a) == stem(c)) {
                    b.push_str(&stem(a));
                } else {
                    b.push_str(a);
                    b.push_str(" / ");
                    b.push_str(c);
                }
                true
            }
            (None, None) => false,
        }
    })
}
