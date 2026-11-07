//! The sounds browser's row details (round 37, finish; ADR 0103 §30, §38): a waveform
//! thumbnail and the length of each audio file, and its tempo and key where its name says
//! them ("Drum Loop 124 BPM", "Chord Stab Am"). One worker thread reads the thumbnails, one
//! file at a time, the newest asked first (a fast scroll leaves the rows it passed behind),
//! each in pieces ([`super::samples_peaks`]); they are kept for the session, at most
//! [`CACHE`] of them (the least recently shown let go first), and read again when the
//! file's size or time changes. The worker asks the window to repaint when one lands. The
//! browser never waits on a disk. // r37:finish

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

pub use super::samples_peaks::{Thumb, COLUMNS};

/// The most thumbnails kept.
pub const CACHE: usize = 4096;
/// The most files waiting to be read: past it the oldest asks are dropped (they are asked
/// again if their rows come back into view).
pub const QUEUE: usize = 256;
/// How often a kept thumbnail's file is looked at again for a change.
const RECHECK: Duration = Duration::from_secs(2);

#[derive(Clone)]
enum Slot {
    Waiting,
    Done(Option<Arc<Thumb>>),
}

struct Entry {
    slot: Slot,
    /// The file's size and time when it was read (or asked).
    stamp: Option<(u64, SystemTime)>,
    checked: Instant,
    used: u64,
}

#[derive(Default)]
struct State {
    map: HashMap<PathBuf, Entry>,
    queue: VecDeque<PathBuf>,
    clock: u64,
    /// No worker could be started: every ask answers "no thumbnail".
    no_worker: bool,
    repaint: Option<egui::Context>,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

fn shared() -> &'static Shared {
    static S: OnceLock<Shared> = OnceLock::new();
    static WORKER: OnceLock<()> = OnceLock::new();
    let s = S.get_or_init(|| Shared {
        state: Mutex::new(State::default()),
        wake: Condvar::new(),
    });
    WORKER.get_or_init(|| {
        let started = std::thread::Builder::new()
            .name("cantus-thumbs".into())
            .spawn(move || work(s));
        if started.is_err() {
            if let Ok(mut st) = s.state.lock() {
                st.no_worker = true;
            }
        }
    });
    s
}

fn stamp_of(path: &Path) -> Option<(u64, SystemTime)> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
}

/// The window the worker asks to repaint when a thumbnail lands (the browser gives it).
pub fn attach(ctx: &egui::Context) {
    if let Ok(mut st) = shared().state.lock() {
        if st.repaint.is_none() {
            st.repaint = Some(ctx.clone());
        }
    }
}

/// `path`'s thumbnail when it has been read; asks for it the first time (and when its file
/// changed), `None` meanwhile and for a file it cannot read.
pub fn thumb(path: &Path) -> Option<Arc<Thumb>> {
    let sh = shared();
    let mut st = sh.state.lock().ok()?;
    if st.no_worker {
        return None;
    }
    st.clock += 1;
    let now = st.clock;
    let mut ask = false;
    let out = match st.map.get_mut(path) {
        Some(e) => {
            e.used = now;
            // A kept thumbnail's file looked at again now and then: changed, it is read again.
            if e.checked.elapsed() >= RECHECK {
                e.checked = Instant::now();
                let fresh = stamp_of(path);
                if fresh != e.stamp {
                    e.stamp = fresh;
                    e.slot = Slot::Waiting;
                    ask = true;
                }
            }
            match &e.slot {
                Slot::Done(t) => t.clone(),
                Slot::Waiting => None,
            }
        }
        None => {
            st.map.insert(
                path.to_path_buf(),
                Entry {
                    slot: Slot::Waiting,
                    stamp: None,
                    checked: Instant::now(),
                    used: now,
                },
            );
            ask = true;
            None
        }
    };
    if ask {
        st.queue.retain(|p| p != path);
        st.queue.push_back(path.to_path_buf());
        while st.queue.len() > QUEUE {
            // The oldest ask let go: its row was scrolled past; asked again if it returns.
            if let Some(old) = st.queue.pop_front() {
                st.map.remove(&old);
            }
        }
        if st.map.len() > CACHE {
            evict(&mut st);
        }
        sh.wake.notify_one();
    }
    out
}

/// Down to three quarters of [`CACHE`], the least recently shown first (never one waiting).
fn evict(st: &mut State) {
    let mut used: Vec<(u64, PathBuf)> = st
        .map
        .iter()
        .filter(|(_, e)| matches!(e.slot, Slot::Done(_)))
        .map(|(p, e)| (e.used, p.clone()))
        .collect();
    used.sort_unstable_by_key(|(u, _)| *u);
    let excess = st.map.len().saturating_sub(CACHE * 3 / 4);
    for (_, p) in used.into_iter().take(excess) {
        st.map.remove(&p);
    }
}

/// How many thumbnails are kept, and how many files wait to be read (tests).
pub fn counts() -> (usize, usize) {
    shared()
        .state
        .lock()
        .map(|st| (st.map.len(), st.queue.len()))
        .unwrap_or((0, 0))
}

/// The worker: the newest ask first, one file at a time.
fn work(sh: &'static Shared) {
    loop {
        let path = {
            let Ok(mut st) = sh.state.lock() else {
                return;
            };
            loop {
                if let Some(p) = st.queue.pop_back() {
                    break p;
                }
                st = match sh.wake.wait(st) {
                    Ok(s) => s,
                    Err(_) => return,
                };
            }
        };
        let stamp = stamp_of(&path);
        let t = read(&path).map(Arc::new);
        let repaint = sh.state.lock().ok().and_then(|mut st| {
            if let Some(e) = st.map.get_mut(&path) {
                e.slot = Slot::Done(t);
                e.stamp = stamp;
                e.checked = Instant::now();
            }
            st.repaint.clone()
        });
        if let Some(ctx) = repaint {
            ctx.request_repaint();
        }
    }
}

/// A WAV file's thumbnail, read in pieces.
fn read(path: &Path) -> Option<Thumb> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if ext != "wav" {
        return None;
    }
    let f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    super::samples_peaks::read(&mut std::io::BufReader::new(f), len)
}

/// Peaks and length from a RIFF WAVE file's bytes.
pub fn parse_wav(b: &[u8]) -> Option<Thumb> {
    super::samples_peaks::parse_wav(b)
}

/// The tempo a name says ("124 BPM", "124bpm", "_124_"), in 40…240.
pub fn bpm_in(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for (i, w) in words.iter().enumerate() {
        let num = w.trim_end_matches("bpm");
        if let Ok(n) = num.parse::<u32>() {
            let said = w.ends_with("bpm") || words.get(i + 1).is_some_and(|x| *x == "bpm");
            if said && (40..=240).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

/// The key a name says ("Am", "C#m", "F maj", "Ebmin"), as written short ("Am", "C#").
pub fn key_in(name: &str) -> Option<String> {
    for w in name.split([' ', '_', '-', '.']) {
        let mut ch = w.chars();
        let Some(root) = ch.next().filter(|c| ('A'..='G').contains(c)) else {
            continue;
        };
        let rest: String = ch.collect();
        let (acc, tail) = match rest.chars().next() {
            Some(c @ ('#' | 'b')) => (Some(c), rest[1..].to_string()),
            _ => (None, rest.clone()),
        };
        let minor = match tail.as_str() {
            "m" | "min" | "minor" => true,
            "" | "maj" | "major" => false,
            _ => continue,
        };
        // A lone capital in a name ("C" in "Bass Stab C") is a key only with its letter
        // alone in the word.
        let mut out = String::new();
        out.push(root);
        if let Some(a) = acc {
            out.push(a);
        }
        if minor {
            out.push('m');
        }
        return Some(out);
    }
    None
}

/// The browser's category tabs, in order (their locale keys): every sound, then the
/// families a name says.
pub const FAMILIES: [&str; 5] = [
    "samples.family.all",
    "samples.family.drums",
    "samples.family.loops",
    "samples.family.melodic",
    "samples.family.fx",
];

/// Whether a sound named `name` is in tab `family` (an index of [`FAMILIES`]): drums by
/// their parts' names, loops by "loop" or a tempo, melodic by a key or an instrument's
/// name, effects by theirs. A sound can be in two ("Drum Loop 124 BPM").
pub fn in_family(name: &str, family: usize) -> bool {
    const DRUMS: &[&str] = &[
        "kick",
        "snare",
        "hat",
        "hihat",
        "hh",
        "clap",
        "drum",
        "drums",
        "perc",
        "tom",
        "cymbal",
        "crash",
        "ride",
        "rim",
        "shaker",
        "808",
        "909",
        "snap",
        "conga",
        "bongo",
        "tambourine",
        "cowbell",
        "kit",
    ];
    const MELODIC: &[&str] = &[
        "pad", "bass", "lead", "chord", "chords", "stab", "synth", "piano", "keys", "pluck", "arp",
        "melody", "string", "strings", "bell", "organ", "guitar", "vocal", "vox",
    ];
    const FX: &[&str] = &[
        "fx",
        "sfx",
        "riser",
        "rise",
        "sweep",
        "impact",
        "noise",
        "whoosh",
        "downlifter",
        "uplifter",
        "transition",
        "reverse",
        "texture",
        "atmo",
        "drone",
    ];
    let words = name_words(name);
    let has = |set: &[&str]| {
        words
            .iter()
            .any(|w| set.contains(&w.as_str()) || singular(w).is_some_and(|one| set.contains(&one)))
    };
    match family {
        1 => has(DRUMS),
        2 => has(&["loop", "loops", "groove", "break"]) || bpm_in(name).is_some(),
        3 => has(MELODIC) || key_in(name).is_some(),
        4 => has(FX),
        _ => true,
    }
}

/// A name's words, lower case, letters parted from digits ("808Kick" → "808", "kick";
/// "Kick01" → "kick", "01"). // r37:finish
fn name_words(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut digit = false;
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let d = c.is_ascii_digit();
        if !cur.is_empty() && d != digit {
            out.push(std::mem::take(&mut cur));
        }
        digit = d;
        cur.push(c.to_ascii_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A plural's singular ("kicks" → "kick", "toms" → "tom"); not "bass", not "fx".
fn singular(w: &str) -> Option<&str> {
    (w.len() > 3 && w.ends_with('s') && !w.ends_with("ss")).then(|| &w[..w.len() - 1])
}

/// The preview's level, 0…1 of the audition's (a session's choice, not the project's).
static VOLUME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000);

/// The preview's level, 0…1 (1 is the audition's −3 dB).
pub fn preview_volume() -> f32 {
    f32::from_bits(VOLUME.load(std::sync::atomic::Ordering::Relaxed))
}

/// Sets the preview's level, kept in 0…1.
pub fn set_preview_volume(v: f32) {
    VOLUME.store(
        v.clamp(0.0, 1.0).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_names_tempo_and_key_are_read() {
        assert_eq!(bpm_in("Drum Loop 124 BPM"), Some(124));
        assert_eq!(bpm_in("loop_90bpm_dry"), Some(90));
        assert_eq!(bpm_in("Kick 808 Deep"), None);
        assert_eq!(key_in("Chord Stab Am").as_deref(), Some("Am"));
        assert_eq!(key_in("Bass Stab C").as_deref(), Some("C"));
        assert_eq!(key_in("Pad F#m warm").as_deref(), Some("F#m"));
        assert_eq!(key_in("Hat Closed"), None);
    }

    /// Names as packs write them: numbers glued on, plurals, both cases.
    #[test]
    fn glued_numbers_and_plurals_are_read() {
        for (name, family) in [
            ("Kick01", 1),
            ("Hat2", 1),
            ("808Kick", 1),
            ("Clap3", 1),
            ("Kicks", 1),
            ("Toms", 1),
            ("SNARE_07", 1),
            ("Loop120", 2),
            ("Loops Dry", 2),
            ("Pads", 3),
            ("Bass12", 3),
            ("Risers", 4),
            ("FX03", 4),
        ] {
            assert!(in_family(name, family), "{name} in {}", FAMILIES[family]);
        }
        assert!(!in_family("Kicks", 3) && !in_family("Bass12", 1) && !in_family("Glass", 1));
        assert_eq!(name_words("808Kick-01b"), ["808", "kick", "01", "b"]);
    }

    #[test]
    fn a_names_family_is_read() {
        assert!(in_family("Kick Deep", 1) && !in_family("Kick Deep", 2));
        assert!(in_family("Drum Loop 124 BPM", 1) && in_family("Drum Loop 124 BPM", 2));
        assert!(in_family("Chord Stab Am", 3) && !in_family("Chord Stab Am", 1));
        assert!(in_family("Riser White Noise", 4));
        assert!(FAMILIES.len() == 5 && in_family("anything", 0));
    }

    fn hound_wav(path: &Path, seconds: f32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8_000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..(8_000.0 * seconds) as i32 {
            w.write_sample((i % 100 - 50) * 1000).unwrap();
        }
        w.finalize().unwrap();
    }

    fn wait_for(path: &Path, ok: impl Fn(&Thumb) -> bool) -> Option<Arc<Thumb>> {
        for _ in 0..1000 {
            if let Some(t) = thumb(path).filter(|t| ok(t)) {
                return Some(t);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    /// Asks past the queue's bound let the oldest go: a fast scroll through thousands of
    /// files never queues more than [`QUEUE`] reads, and one worker reads them.
    #[test]
    fn asks_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..QUEUE * 4 {
            let _ = thumb(&dir.path().join(format!("missing {i}.wav")));
            assert!(counts().1 <= QUEUE, "{:?}", counts());
        }
        assert!(counts().0 <= CACHE);
    }

    /// A thumbnail lands with a repaint asked of the window, and a file changed on disk is
    /// read again.
    #[test]
    fn a_landing_repaints_and_a_changed_file_is_read_again() {
        let ctx = egui::Context::default();
        attach(&ctx);
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("take.wav");
        hound_wav(&p, 0.1);
        let t = wait_for(&p, |_| true).expect("read");
        assert!((t.seconds - 0.1).abs() < 1e-3, "{}", t.seconds);
        assert!(
            t.peaks.iter().all(|x| *x > 0.001),
            "a 24-bit extensible file reads true: {:?}",
            t.peaks
        );
        if shared().state.lock().unwrap().repaint.as_ref() == Some(&ctx) {
            assert!(ctx.has_requested_repaint(), "the worker asked for a frame");
        }
        hound_wav(&p, 0.3);
        std::thread::sleep(RECHECK + Duration::from_millis(50));
        let t = wait_for(&p, |t| (t.seconds - 0.3).abs() < 1e-3).expect("read again");
        assert!((t.seconds - 0.3).abs() < 1e-3);
    }

    #[test]
    fn a_wav_reads_as_peaks_and_a_length() {
        // A second of 16-bit mono at 1 kHz: a ramp to full scale.
        let rate = 1000u32;
        let n = 1000usize;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + 2 * n as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(2 * n as u32).to_le_bytes());
        for i in 0..n {
            let v = (i as f32 / n as f32 * 32_000.0) as i16;
            b.extend_from_slice(&v.to_le_bytes());
        }
        let t = parse_wav(&b).expect("a thumbnail");
        assert!((t.seconds - 1.0).abs() < 1e-3);
        assert!(
            t.peaks[0] < 0.1 && t.peaks[COLUMNS - 1] > 0.9,
            "{:?}",
            t.peaks
        );
    }
}
