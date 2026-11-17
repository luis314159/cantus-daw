//! fxc:comparatio — Comparatio exported (ADR 0146, phase B): its references are files by
//! path in its state with the size and hash they were loaded with, never their audio; a state
//! loaded reads the one compared at the host's rate on a thread of the plug-in's own and the
//! processor hears it, loudness-matched, once it is in; a missing file is reported, never
//! skipped silently; a host's offline render hears the mix, to the bit; and the mix heard is
//! the input to the bit.

use cantus_core::fx::comparatio::{Reference, References};
use cantus_core::{EffectKind, IrFile};
use cantus_engine::fx::comparatio_store::{self as store, RefStatus};
use cantus_plugins::instance::{HostTransport, Processor, Shared};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SR: f32 = 48_000.0;

fn shared() -> Arc<Shared> {
    Shared::new(cantus_plugins::desc::of(EffectKind::Reference).expect("Comparatio is exported"))
}

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Steady noise, low-passed by `a`, `gain` after: `frames` long.
fn steady(seed: u32, a: f32, gain: f32, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut s = seed;
    let mut lp = 0.0f32;
    let l: Vec<f32> = (0..frames)
        .map(|_| {
            lp += a * (noise(&mut s) - lp);
            gain * lp
        })
        .collect();
    let r = l.clone();
    (l, r)
}

fn wav(dir: &Path, name: &str, l: &[f32], r: &[f32], rate: u32) -> PathBuf {
    let path = dir.join(name);
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for (a, b) in l.iter().zip(r) {
        w.write_sample(*a).unwrap();
        w.write_sample(*b).unwrap();
    }
    w.finalize().unwrap();
    path
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantus-plugin-comparatio-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Integrated loudness (EBU R128, Terminus's meter), LUFS.
fn lufs(l: &[f32], r: &[f32]) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(SR);
    m.process(l, r);
    m.integrated().expect("loud enough")
}

/// Run `frames` of `(l, r)` through `p` in blocks of 480; the output.
fn run(p: &mut Processor, l: &[f32], r: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let (mut ol, mut or) = (Vec::new(), Vec::new());
    for (a, b) in l.chunks(480).zip(r.chunks(480)) {
        let (mut x, mut y) = (a.to_vec(), b.to_vec());
        // The host's song is playing (a free reference plays while the song does).
        p.set_transport(Some(HostTransport {
            playing: true,
            tempo: Some(120.0),
            ..Default::default()
        }));
        p.process(&mut x, &mut y, None, &[]);
        ol.extend_from_slice(&x);
        or.extend_from_slice(&y);
    }
    (ol, or)
}

#[test]
fn references_live_in_the_state_by_path_size_and_hash_never_their_audio() {
    let dir = temp("state");
    let (rl, rr) = steady(0x0BAD_5EED, 0.5, 0.3, 48_000);
    let file = wav(&dir, "Finished master.wav", &rl, &rr, 44_100);
    let path = file.to_string_lossy().to_string();
    let (bytes, hash) = store::fingerprint(&file).unwrap();
    let one = shared();
    let refs = References {
        list: vec![
            Reference {
                path: path.clone(),
                bytes,
                hash: hash.clone(),
            },
            Reference::new(dir.join("Album master.wav").to_string_lossy().to_string()),
        ],
        eq: None,
    };
    one.comparatio.set(Some(refs.clone()), Some(IrFile::new(path.clone())));
    let state = one.save_state();
    let text = String::from_utf8(state.clone()).unwrap();
    assert!(text.contains("\"refs\"") && text.contains(&hash), "{text}");
    assert!(state.len() < 2_000, "no audio in the state: {} bytes", state.len());
    // Another instance (another session) loads it: the list, the one compared, their prints.
    let two = shared();
    two.load_state(&state).unwrap();
    let (got, ir) = two.comparatio.get();
    assert_eq!(got.as_ref(), Some(&refs));
    assert_eq!(ir.map(|f| f.path), Some(path.clone()));
    let spec = two.spec();
    assert_eq!(spec.refs.as_deref(), Some(&refs), "the editor's spec carries them");
    // A hand-edited state is repaired as a project is: a path twice dropped, the one
    // compared listed.
    let edited = text.replace("Album master.wav", "Finished master.wav");
    let three = shared();
    three.load_state(edited.as_bytes()).unwrap();
    assert_eq!(three.comparatio.get().0.unwrap().list.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_reference_loaded_with_the_state_is_heard_matched_and_the_mix_is_untouched() {
    let dir = temp("heard");
    // The reference 12 dB louder than the mix, in a file at another rate (resampled).
    // 20 s: a free reference plays once from the song's start, so it outlasts the test.
    let (rl, rr) = steady(0x0BAD_5EED, 0.5, 0.4, 44_100 * 20);
    let file = wav(&dir, "Loud master.wav", &rl, &rr, 44_100);
    let path = file.to_string_lossy().to_string();
    let one = shared();
    one.comparatio.set(
        Some(References {
            list: vec![Reference::new(path.clone())],
            eq: None,
        }),
        Some(IrFile::new(path.clone())),
    );
    let state = one.save_state();
    let host = shared();
    host.load_state(&state).unwrap();
    let mut p = Processor::new(host.clone(), SR);
    let (ml, mr) = steady(0x5EED_0001, 0.5, 0.1, SR as usize * 4);
    // Your mix heard: the input, to the bit — the reference being read meanwhile.
    let (ol, or) = run(&mut p, &ml, &mr);
    assert_eq!(ol, ml, "your mix, untouched");
    assert_eq!(or, mr);
    // Wait (as a host's next blocks would) for the store to have read it.
    let key = cantus_engine::fx::aula_store::file_key(&path);
    assert!(
        store::wait_for(key, SR as u32, std::time::Duration::from_secs(30)).is_some(),
        "read at the host's rate on the plug-in's own thread: {:?}",
        store::status(&path)
    );
    // A few more seconds of the mix: the unit has its reference now and measures it beside.
    let (ol, _) = run(&mut p, &ml, &mr);
    assert_eq!(ol, ml, "still your mix, untouched");
    // The reference heard: the host sets listen (a parameter like any other).
    let listen = host.table.by_key("listen").unwrap().index;
    host.set_value(listen, 1.0);
    let heard = run(&mut p, &ml, &mr);
    // From half a second on (the crossfade and the match's glide done).
    let h = SR as usize / 2;
    let tail = &heard.0[h..];
    let mix_db = lufs(&ml[h..], &mr[h..]);
    let heard_db = lufs(tail, &heard.1[h..]);
    eprintln!("mix {mix_db:.2} LUFS, heard {heard_db:.2} LUFS");
    assert!(
        (heard_db - mix_db).abs() < 0.2,
        "heard at the mix's loudness: {heard_db:.2} against {mix_db:.2}"
    );
    assert_ne!(tail, &ml[SR as usize / 2..], "it is the reference that is heard");
    // A host's offline render hears the mix, to the bit, whatever the panel says.
    host.comparatio.set_offline(true);
    let mut p2 = Processor::new(host.clone(), SR);
    let mut out = (Vec::new(), Vec::new());
    for _ in 0..3 {
        let (a, b) = run(&mut p2, &ml, &mr);
        out.0.extend(a);
        out.1.extend(b);
    }
    let n = ml.len();
    assert_eq!(&out.0[2 * n..], &ml[..], "an offline render: the mix");
    assert_eq!(&out.1[2 * n..], &mr[..]);
    host.comparatio.set_offline(false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_is_reported_not_skipped() {
    let dir = temp("missing");
    let gone = dir.join("Gone master.wav").to_string_lossy().to_string();
    let host = shared();
    host.comparatio.set(
        Some(References {
            list: vec![Reference {
                path: gone.clone(),
                bytes: 1_234,
                hash: "0123456789abcdef".into(),
            }],
            eq: None,
        }),
        Some(IrFile::new(gone.clone())),
    );
    let mut p = Processor::new(host.clone(), SR);
    let (ml, mr) = steady(7, 0.5, 0.1, 4_800);
    let (ol, _) = run(&mut p, &ml, &mr);
    assert_eq!(ol, ml, "the mix plays meanwhile");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        let s = store::status(&gone);
        if matches!(s, RefStatus::Failed(_)) || std::time::Instant::now() > until {
            break s;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    match status {
        RefStatus::Failed(e) => assert!(e.contains("not there"), "{e}"),
        other => panic!("a missing file must be reported: {other:?}"),
    }
    // The state still names it (with its print), for when the file comes back.
    let text = String::from_utf8(host.save_state()).unwrap();
    assert!(text.contains("Gone master.wav") && text.contains("0123456789abcdef"));
    let _ = std::fs::remove_dir_all(&dir);
}
