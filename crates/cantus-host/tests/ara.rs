//! ARA 2 hosting (ADR 0097), against the repository's ARA test plug-in (`cantus-test-ara`,
//! built from source with these tests): a bound clip renders its source to the bit, an edit
//! in the plug-in renders again, clip edits and undo reach the plug-in and keep its state, the
//! state is archived and restored, a copy is cloned, a missing plug-in leaves the source
//! playing with the reason — and **no ARA call rule is ever broken** (the plug-in counts).
//!
//! The plug-in's counters are process-wide, so the tests run one at a time (`LOCK`).

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::{Arc, Mutex, OnceLock};

use cantus_core::ara::{modification_persistent_id, AraClip};
use cantus_core::audio::AudioSource;
use cantus_core::model::{AudioSourceId, ClipId, Project, TrackId};
use cantus_core::{Command, CommandBus, Origin, SampleData, Ticks};
use cantus_host::ara::{plugin_ref, AraSession, AraStatus, SourceState};
use cantus_host::{Backend, PluginInfo};

static LOCK: Mutex<()> = Mutex::new(());

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn built_name() -> (&'static str, &'static str, &'static str) {
    if cfg!(target_os = "windows") {
        ("cantus_test_ara.dll", "x86_64-win", ".vst3")
    } else if cfg!(target_os = "macos") {
        ("libcantus_test_ara.dylib", "MacOS", "")
    } else {
        ("libcantus_test_ara.so", "x86_64-linux", ".so")
    }
}

/// The ARA test plug-in as a VST3 bundle, and the binary inside it.
fn bundle() -> &'static (PathBuf, PathBuf) {
    static B: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    B.get_or_init(|| {
        let (built, arch, ext) = built_name();
        let target = workspace().join("target").join("cantus-test-plugins");
        let beside = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join(built)))
            .filter(|p| p.is_file());
        let source = beside.unwrap_or_else(|| {
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let ok = Process::new(cargo)
                .current_dir(workspace())
                .args(["build", "--quiet", "-p", "cantus-test-ara", "--target-dir"])
                .arg(&target)
                .status()
                .expect("cargo builds the ARA test plug-in");
            assert!(ok.success());
            target.join("debug").join(built)
        });
        let bundle = target.join("bundles-ara").join("CantusTestARA.vst3");
        let dir = bundle.join("Contents").join(arch);
        std::fs::create_dir_all(&dir).expect("bundle folders");
        let binary = dir.join(format!("CantusTestARA{ext}"));
        std::fs::copy(&source, &binary).expect("copy the plug-in into its bundle");
        (bundle, binary)
    })
}

fn info() -> PluginInfo {
    cantus_host::vst3::Vst3Backend::new()
        .scan(&bundle().0)
        .expect("scan")
        .into_iter()
        .find(|i| i.name == "Cantus Test ARA")
        .expect("the processor is in the bundle")
}

/// The plug-in's test exports (the same module the host loaded: the system shares it).
struct Probe(libloading::Library);

impl Probe {
    fn new() -> Probe {
        // SAFETY: our own test plug-in.
        Probe(unsafe { libloading::Library::new(&bundle().1) }.expect("open the plug-in"))
    }

    fn violations(&self) -> i32 {
        // SAFETY: exported with this signature.
        unsafe {
            (self
                .0
                .get::<unsafe extern "C" fn() -> i32>(b"cantus_test_ara_violations\0")
                .unwrap())()
        }
    }

    /// [controllers, sources, modifications, active, regions, sequences, bound, tempo entries]
    fn counts(&self) -> [i32; 8] {
        let mut v = [0i32; 8];
        // SAFETY: writes eight i32.
        unsafe {
            (self
                .0
                .get::<unsafe extern "C" fn(*mut i32)>(b"cantus_test_ara_counts\0")
                .unwrap())(v.as_mut_ptr())
        };
        v
    }

    fn set_gain(&self, clip: ClipId, gain: f32) -> bool {
        let pid = CString::new(modification_persistent_id(clip)).unwrap();
        // SAFETY: exported with this signature.
        unsafe {
            (self
                .0
                .get::<unsafe extern "C" fn(*const std::ffi::c_char, f32) -> i32>(
                    b"cantus_test_ara_set_gain\0",
                )
                .unwrap())(pid.as_ptr(), gain)
                == 1
        }
    }

    fn gain(&self, clip: ClipId) -> f32 {
        let pid = CString::new(modification_persistent_id(clip)).unwrap();
        // SAFETY: exported with this signature.
        unsafe {
            (self
                .0
                .get::<unsafe extern "C" fn(*const std::ffi::c_char) -> f32>(
                    b"cantus_test_ara_gain\0",
                )
                .unwrap())(pid.as_ptr())
        }
    }
}

const RATE: u32 = 48_000;

/// 1.3 s of a stereo signal no two frames alike, at 48 kHz.
fn signal() -> Arc<SampleData> {
    let frames = 62_401usize;
    let mut v = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let x = i as f32;
        v.push(((x * 0.013).sin() * 0.5) + x * 1e-7);
        v.push(((x * 0.007).cos() * 0.4) - x * 1e-7);
    }
    Arc::new(SampleData {
        rate: RATE,
        channels: 2,
        frames: v.into(),
        stream: None,
    })
}

struct Rig {
    bus: CommandBus,
    track: TrackId,
    clip: ClipId,
    source: AudioSourceId,
    audio: Arc<SampleData>,
    info: PluginInfo,
    session: AraSession,
}

impl Rig {
    /// One track, one audio clip (beat 1, three beats long, slipped half a beat into its
    /// source) bound to the test plug-in. 120 BPM.
    fn new() -> Rig {
        let info = info();
        let mut bus = CommandBus::new(Project::new("ara"));
        let mut track = bus.new_track("Vocal");
        track.clips.clear();
        let source = AudioSourceId(bus.alloc_id());
        let audio = signal();
        let src = AudioSource {
            id: source,
            name: "take".into(),
            path: "take.wav".into(),
            sample_rate: RATE,
            channels: 2,
            frames: (audio.frames.len() / 2) as u64,
            sha256: None,
            recorded: false,
        };
        let mut clip = bus.new_clip("take", Ticks::beats(1), Ticks::beats(3) - Ticks(7));
        clip.notes.clear();
        let mut region = cantus_core::AudioRegion::of(source);
        region.ara = Some(Box::new(AraClip::new(plugin_ref(
            &info,
            "org.cantus.test.ara.factory",
        ))));
        clip.audio = Some(region);
        clip.offset = Ticks::beats(1) / 2;
        let (t, c) = (track.id, clip.id);
        bus.execute(
            Command::InsertTrack {
                index: 0,
                track: Box::new(track),
            },
            Origin::User,
        )
        .unwrap();
        bus.execute(Command::InsertAudioSource { source: src }, Origin::User)
            .unwrap();
        bus.execute(
            Command::InsertClip {
                track: t,
                clip: Box::new(clip),
            },
            Origin::User,
        )
        .unwrap();
        Rig {
            bus,
            track: t,
            clip: c,
            source,
            audio,
            info,
            session: AraSession::new(),
        }
    }

    /// One frame of the window: sync, the plug-in's edits run as the window runs them
    /// (merged per gesture), the renders waited for and taken in.
    fn sync(&mut self) -> bool {
        self.session.set_debounce(std::time::Duration::ZERO);
        let a = self.sync_once();
        for (cmd, key) in self.session.take_edits() {
            let _ = self.bus.execute_merging(
                cmd,
                Origin::User,
                cantus_core::MergeKey::of(&("ara", key)),
            );
        }
        let b = self.session.wait(std::time::Duration::from_secs(20));
        let c = self.sync_once();
        a || b || c
    }

    fn sync_once(&mut self) -> bool {
        let info = self.info.clone();
        let audio = self.audio.clone();
        let source = self.source;
        self.session.sync(
            self.bus.project(),
            &move |p| p.id.eq_ignore_ascii_case(&info.id).then(|| info.clone()),
            &move |s| {
                if s.id == source {
                    SourceState::Ready(audio.clone())
                } else {
                    SourceState::Missing
                }
            },
            RATE,
        )
    }

    fn run(&mut self, c: Command) {
        self.bus.execute(c, Origin::User).unwrap();
    }

    /// What the clip plays, as the whole source at its rate: the source, the render's
    /// stretch laid where it begins.
    fn render(&self, clip: ClipId) -> Vec<f32> {
        let r = self.session.renders().get(&clip).expect("a render");
        let mut out = self.audio.frames.to_vec();
        let (first, end) = r.source_frames;
        out[first as usize * 2..end as usize * 2].copy_from_slice(&r.at_source_rate);
        out
    }
}

/// The frames of the clip's window in its source at 120 BPM: (start, len) in frames.
fn window(offset: Ticks, length: Ticks) -> (usize, usize) {
    let f = |t: Ticks| (t.0 as f64 / 960.0 * 0.5 * f64::from(RATE)).round() as usize;
    (f(offset), f(length))
}

#[test]
fn a_bound_clip_renders_its_source_to_the_bit_and_breaks_no_rule() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    assert!(rig.sync(), "a first render");
    assert_eq!(rig.session.status(rig.clip), Some(&AraStatus::Live));
    let r = rig.render(rig.clip);

    assert_eq!(
        &r[..],
        &rig.audio.frames[..],
        "gain 1 renders the source to the bit"
    );
    let c = probe.counts();
    assert_eq!(
        &c[..7],
        &[1, 1, 1, 1, 1, 1, 1],
        "one of each, bound once: {c:?}"
    );
    assert!(c[7] >= 2, "the plug-in read the tempo: {c:?}");
    assert!(!rig.sync(), "nothing changed: no render");
    assert_eq!(probe.violations(), v0);
}

#[test]
fn an_edit_in_the_plugin_renders_the_window_again() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    assert!(probe.set_gain(rig.clip, 0.5));
    assert!(rig.sync(), "the plug-in said its content changed");
    let r = rig.render(rig.clip);
    let (s, n) = window(Ticks::beats(1) / 2, Ticks::beats(3) - Ticks(7));
    for (i, (a, b)) in r.chunks(2).zip(rig.audio.frames.chunks(2)).enumerate() {
        let want = if (s..s + n).contains(&i) {
            [b[0] * 0.5, b[1] * 0.5]
        } else {
            [b[0], b[1]]
        };
        assert_eq!([a[0], a[1]], want, "frame {i}");
    }
    assert_eq!(probe.violations(), v0);
}

#[test]
fn clip_edits_and_undo_reach_the_plugin_and_keep_its_state() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    probe.set_gain(rig.clip, 0.25);
    rig.sync();
    let first = rig.render(rig.clip);
    let (t, c) = (rig.track, rig.clip);
    rig.run(Command::SetClipBounds {
        track: t,
        clip: c,
        start: Ticks::beats(2),
        length: Ticks::beats(1),
    });
    assert!(rig.sync(), "a trimmed region renders again");
    let (s, n) = window(Ticks::beats(1) / 2, Ticks::beats(1));
    let r = rig.render(c);
    assert_eq!(
        r[(s + n) * 2],
        rig.audio.frames[(s + n) * 2],
        "past the new window: the source"
    );
    assert_eq!(r[(s + n - 1) * 2], rig.audio.frames[(s + n - 1) * 2] * 0.25);
    // Another bound clip keeps the document open.
    let mut other = clip_of(rig.bus.project(), c).clone();
    other.id = ClipId(rig.bus.alloc_id());
    other.start = Ticks::beats(12);
    rig.run(Command::InsertClip {
        track: t,
        clip: Box::new(other),
    });
    rig.sync();
    // Deleted: the region goes, the modification is kept for undo (deactivated).
    rig.run(Command::RemoveClip { track: t, clip: c });
    rig.sync();
    assert!(!rig.session.renders().contains_key(&c));
    let k = probe.counts();
    assert_eq!(
        (k[2], k[3], k[4], k[6]),
        (2, 1, 1, 1),
        "kept, inactive, no region, no instance: {k:?}"
    );
    rig.bus.undo().unwrap();
    rig.bus.undo().unwrap();
    rig.bus.undo().unwrap();
    rig.sync();
    assert_eq!(
        probe.gain(c),
        0.25,
        "the plug-in's edit came back with the clip"
    );
    assert_eq!(&rig.render(c)[..], &first[..], "undo plays what it played");
    assert_eq!(probe.violations(), v0);
}

#[test]
fn the_plugins_state_is_archived_and_restored_with_the_project() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    probe.set_gain(rig.clip, 0.75);
    rig.sync();
    let played = rig.render(rig.clip);
    let mut saved = rig.bus.project().clone();
    rig.session.archive_into(&mut saved);
    let a = clip_of(&saved, rig.clip)
        .audio
        .as_ref()
        .unwrap()
        .ara
        .as_ref()
        .unwrap()
        .archive
        .clone()
        .expect("an archive");
    assert_eq!(a.document_archive_id, "org.cantus.test.ara.document.v1");
    assert_eq!(
        a.data.bytes(),
        format!("{}=0.75\n", modification_persistent_id(rig.clip)).as_bytes()
    );
    // The project opened again: a new session, a new document, the state restored.
    rig.session = AraSession::new();
    assert_eq!(probe.counts()[0], 0, "the old document is gone");
    rig.bus = CommandBus::new(saved);
    rig.sync();
    assert_eq!(probe.gain(rig.clip), 0.75);
    assert_eq!(&rig.render(rig.clip)[..], &played[..]);
    assert_eq!(probe.violations(), v0);
}

#[test]
fn a_copy_of_a_bound_clip_is_cloned_with_the_plugins_edits() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    probe.set_gain(rig.clip, 0.125);
    rig.sync();
    // A paste: the clip's data with a new id, its archive naming the original's modification.
    let mut saved = rig.bus.project().clone();
    rig.session.archive_into(&mut saved);
    let mut copy = clip_of(&saved, rig.clip).clone();
    copy.id = ClipId(rig.bus.alloc_id());
    copy.start = Ticks::beats(8);
    let id = copy.id;
    let t = rig.track;
    rig.run(Command::InsertClip {
        track: t,
        clip: Box::new(copy),
    });
    rig.sync();
    assert_eq!(probe.gain(id), 0.125);
    assert_eq!(probe.counts()[2], 2);
    assert_eq!(probe.violations(), v0);
}

#[test]
fn a_missing_plugin_leaves_the_source_playing_and_says_why() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut rig = Rig::new();
    let changed = rig.session.sync(
        rig.bus.project(),
        &|_| None,
        &|_| SourceState::Missing,
        RATE,
    );
    assert!(!changed);
    assert!(rig.session.renders().is_empty());
    assert!(matches!(
        rig.session.status(rig.clip),
        Some(AraStatus::NotInstalled)
    ));
    // The binding stays in the project.
    assert!(clip_of(rig.bus.project(), rig.clip)
        .audio
        .as_ref()
        .unwrap()
        .ara
        .is_some());
}

#[test]
fn letting_go_of_the_last_clip_closes_the_document() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    let (t, c) = (rig.track, rig.clip);
    probe.set_gain(c, 0.5);
    rig.sync();
    let edited = rig.render(c);
    rig.run(Command::SetClipAra {
        track: t,
        clip: c,
        ara: None,
    });
    assert!(rig.sync(), "the render goes");
    assert!(rig.session.renders().is_empty());
    assert!(!rig.session.active());
    assert_eq!(probe.counts()[..7], [0, 0, 0, 0, 0, 0, 0]);
    // Undo binds it again, with what the plug-in did to it (kept when the document closed).
    rig.bus.undo().unwrap();
    rig.sync();
    assert_eq!(probe.gain(c), 0.5);
    assert_eq!(&rig.render(c)[..], &edited[..]);
    assert_eq!(probe.violations(), v0);
}

#[test]
fn a_tempo_map_that_moves_is_published_to_the_plugin() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    assert_eq!(probe.counts()[7], 2, "one tempo: two entries");
    let mut p = rig.bus.project().clone();
    p.tempo_points = vec![
        cantus_core::tempo::TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: true,
        },
        cantus_core::tempo::TempoPoint {
            tick: Ticks::beats(8),
            bpm: 90.0,
            ramp: false,
        },
    ];
    rig.bus = CommandBus::new(p);
    rig.sync();
    assert!(
        probe.counts()[7] > 2,
        "a ramp is followed quarter by quarter: {:?}",
        probe.counts()
    );
    assert_eq!(probe.violations(), v0);
}

fn clip_of(p: &Project, c: ClipId) -> &cantus_core::model::Clip {
    p.tracks.iter().find_map(|t| t.clip(c)).expect("the clip")
}

/// Celemony's own reference plug-in (`ARATestPlugIn`, Apache-2.0), when it has been built
/// with `tools/ara/build-test-plugin.sh` (or `CANTUS_ARA_TEST_PLUGIN` names its bundle):
/// bound, rendered, archived and restored. Skipped, saying why, when it is not there — the
/// repository's own test plug-in above covers every path on every machine and in CI.
#[test]
fn celemonys_reference_plugin_when_built() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = std::env::var_os("CANTUS_ARA_TEST_PLUGIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("../downloads/ara/build/ARATestPlugIn.vst3"));
    if !path.exists() {
        eprintln!(
            "skipped: ARATestPlugIn not built ({}); run tools/ara/build-test-plugin.sh or set CANTUS_ARA_TEST_PLUGIN",
            path.display()
        );
        return;
    }
    let infos = cantus_host::vst3::Vst3Backend::new()
        .scan_loading(&path)
        .expect("scan ARATestPlugIn");
    let info = infos.into_iter().next().expect("ARATestPlugIn's processor");
    let mut rig = Rig::new();
    rig.info = info.clone();
    let (t, c) = (rig.track, rig.clip);
    let factory = cantus_host::ara::factory_id(&info).expect("an ARA factory");
    let binding = AraClip::new(plugin_ref(&info, &factory));
    rig.run(Command::SetClipAra {
        track: t,
        clip: c,
        ara: Some(Box::new(binding)),
    });
    assert!(rig.sync(), "rendered");
    assert_eq!(rig.session.status(c), Some(&AraStatus::Live));
    let r = rig.render(c);
    let worst = r
        .iter()
        .zip(rig.audio.frames.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        worst < 1e-6,
        "ARATestPlugIn renders the source: worst {worst}"
    );
    let mut saved = rig.bus.project().clone();
    rig.session.archive_into(&mut saved);
    assert!(clip_of(&saved, c)
        .audio
        .as_ref()
        .unwrap()
        .ara
        .as_ref()
        .unwrap()
        .archive
        .is_some());
    rig.session = AraSession::new();
    rig.bus = CommandBus::new(saved);
    assert!(rig.sync(), "restored and rendered again");
}

#[test]
fn a_scan_marks_an_ara_plugin_and_only_it() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let i = info();
    assert!(
        i.features.iter().any(|f| f == cantus_host::db::ARA_FEATURE),
        "{:?}",
        i.features
    );
    let mut db = cantus_host::PluginDb::default();
    db.entries.push(cantus_host::DbEntry {
        path: bundle().0.clone(),
        format: cantus_host::Format::Vst3,
        modified: 1,
        size: 1,
        status: cantus_host::ScanStatus::Ok,
        plugins: vec![i.clone()],
    });
    assert_eq!(db.ara_plugins().len(), 1);
    // A database written before ARA plug-ins were marked is rescanned once for them.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.json");
    db.save(&path).unwrap();
    let back = cantus_host::PluginDb::load(&path);
    assert!(back.ara_marked);
    assert_eq!(back.entries[0].modified, 0, "looked at again");
}

// ---- slice 2 essentials and the review's findings -------------------------------------

impl Probe {
    fn sym<T>(&self, name: &[u8]) -> libloading::Symbol<'_, T> {
        // SAFETY: our own exports, with the signatures the callers name.
        unsafe { self.0.get::<T>(name).unwrap() }
    }

    fn set_gain_range(&self, clip: ClipId, gain: f32, a: f64, b: f64) {
        let pid = CString::new(modification_persistent_id(clip)).unwrap();
        let f = self.sym::<unsafe extern "C" fn(*const std::ffi::c_char, f32, f64, f64) -> i32>(
            b"cantus_test_ara_set_gain_range\0",
        );
        assert_eq!(unsafe { f(pid.as_ptr(), gain, a, b) }, 1);
    }

    fn set_latency(&self, n: u32) {
        unsafe { self.sym::<unsafe extern "C" fn(u32)>(b"cantus_test_ara_set_latency\0")(n) }
    }

    fn set_head_tail(&self, h: f64, t: f64) {
        unsafe {
            self.sym::<unsafe extern "C" fn(f64, f64)>(b"cantus_test_ara_set_head_tail\0")(h, t)
        }
    }

    fn crash_next_render(&self) {
        unsafe { self.sym::<unsafe extern "C" fn()>(b"cantus_test_ara_crash_next_render\0")() }
    }

    fn hang_next_render(&self, ms: u32) {
        unsafe { self.sym::<unsafe extern "C" fn(u32)>(b"cantus_test_ara_hang_next_render\0")(ms) }
    }

    fn selections(&self) -> i32 {
        unsafe { self.sym::<unsafe extern "C" fn() -> i32>(b"cantus_test_ara_selections\0")() }
    }

    fn request_play(&self, at: f64) -> i32 {
        unsafe {
            self.sym::<unsafe extern "C" fn(f64) -> i32>(b"cantus_test_ara_request_play\0")(at)
        }
    }
}

fn labels(bus: &CommandBus) -> Vec<String> {
    bus.history().iter().map(|h| h.label_key.clone()).collect()
}

#[test]
fn an_edit_in_the_plugin_is_one_undo_step_and_undo_puts_it_back_into_the_plugin() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let v0 = probe.violations();
    let mut rig = Rig::new();
    rig.sync();
    let steps = labels(&rig.bus).len();
    // Two edits within one gesture: one step.
    probe.set_gain(rig.clip, 0.5);
    rig.sync();
    probe.set_gain(rig.clip, 0.25);
    rig.sync();
    let after = labels(&rig.bus);
    assert_eq!(after.len(), steps + 1, "{after:?}");
    assert_eq!(after.last().map(String::as_str), Some("cmd.ara_edit"));
    let edited = rig.render(rig.clip);
    // Undo: the plug-in is put back where it was, and the clip plays the source again.
    rig.bus.undo().unwrap();
    rig.sync();
    assert_eq!(probe.gain(rig.clip), 1.0);
    assert_eq!(&rig.render(rig.clip)[..], &rig.audio.frames[..]);
    assert_eq!(
        labels(&rig.bus).len(),
        steps,
        "putting it back is no new step"
    );
    // Redo: the edit again.
    rig.bus.redo().unwrap();
    rig.sync();
    assert_eq!(probe.gain(rig.clip), 0.25);
    assert_eq!(&rig.render(rig.clip)[..], &edited[..]);
    assert_eq!(probe.violations(), v0);
}

#[test]
fn renders_run_on_the_render_thread_show_progress_and_the_last_one_plays_meanwhile() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    let first = rig.render(rig.clip);
    probe.hang_next_render(300);
    probe.set_gain(rig.clip, 0.5);
    let t = std::time::Instant::now();
    rig.sync_once();
    assert!(
        t.elapsed() < std::time::Duration::from_millis(250),
        "sync never waits for a render"
    );
    for (cmd, key) in rig.session.take_edits() {
        rig.bus
            .execute_merging(cmd, Origin::User, cantus_core::MergeKey::of(&("ara", key)))
            .unwrap();
    }
    assert!(
        rig.session.progress(rig.clip).is_some(),
        "its progress is shown"
    );
    assert_eq!(
        &rig.render(rig.clip)[..],
        &first[..],
        "the last render plays until the new one"
    );
    rig.session.wait(std::time::Duration::from_secs(10));
    rig.sync_once();
    assert!(rig.session.progress(rig.clip).is_none());
    assert_ne!(&rig.render(rig.clip)[..], &first[..]);
}

#[test]
fn an_edit_of_a_part_renders_only_that_part() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    // The clip shows the source from 0.25 s; the plug-in says 0.4 s to 0.5 s changed.
    probe.set_gain_range(rig.clip, 0.5, 0.4, 0.5);
    rig.sync();
    let r = rig.render(rig.clip);
    let (a, b) = (
        (0.4 * RATE as f64) as usize,
        (0.5 * RATE as f64) as usize + 1,
    );
    for (i, (x, y)) in r.chunks(2).zip(rig.audio.frames.chunks(2)).enumerate() {
        let want = if (a..b).contains(&i) {
            y[0] * 0.5
        } else {
            y[0]
        };
        assert_eq!(x[0], want, "frame {i}");
    }
}

#[test]
fn a_plugin_with_latency_renders_in_place() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    probe.set_latency(333);
    let mut rig = Rig::new();
    rig.sync();
    probe.set_latency(0);
    assert_eq!(
        &rig.render(rig.clip)[..],
        &rig.audio.frames[..],
        "pre-rolled and dropped: no offset"
    );
}

#[test]
fn a_plugins_head_and_tail_are_rendered() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    probe.set_head_tail(0.01, 0.02);
    let mut rig = Rig::new();
    rig.sync();
    probe.set_head_tail(0.0, 0.0);
    // The test plug-in plays nothing outside the region: its head (the 480 frames before the
    // window at 0.25 s) and its tail (960 after its end) are its silence, not the source.
    let r = rig.render(rig.clip);
    let start = (0.25 * RATE as f64) as usize;
    assert!(
        r[(start - 480) * 2..start * 2].iter().all(|x| *x == 0.0),
        "head"
    );
    assert_ne!(r[(start - 481) * 2], 0.0, "before the head: the source");
    assert_eq!(
        r[start * 2],
        rig.audio.frames[start * 2],
        "the region: the source"
    );
}

#[test]
fn the_plugins_editor_is_shown_the_clip() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    let before = probe.selections();
    // The test plug-in has no editor window: the clip is selected in its editor view all
    // the same, and the window says why none opens.
    let r = rig.session.open_editor(rig.clip, "test", None);
    assert!(r.is_err());
    assert_eq!(probe.selections(), before + 1);
}

#[test]
fn the_plugins_transport_wishes_reach_the_window() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    rig.session.take_playback_requests();
    assert_eq!(probe.request_play(2.0), 1);
    rig.sync_once();
    use cantus_ara::callbacks::PlaybackRequest as P;
    assert_eq!(
        rig.session.take_playback_requests(),
        vec![P::Start, P::Position(2.0)]
    );
}

#[test]
fn a_fresh_bind_starts_clean_and_previous_edits_come_back_on_request() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    probe.set_gain(rig.clip, 0.5);
    rig.sync();
    let (t, c) = (rig.track, rig.clip);
    rig.run(Command::SetClipAra {
        track: t,
        clip: c,
        ara: None,
    });
    rig.sync();
    let previous = rig.session.previous_binding(c).cloned().expect("kept");
    // Bound afresh (a new binding number): clean.
    let mut fresh = AraClip::new(plugin_ref(&rig.info, "org.cantus.test.ara.factory"));
    fresh.binding = 77;
    rig.run(Command::SetClipAra {
        track: t,
        clip: c,
        ara: Some(Box::new(fresh)),
    });
    rig.sync();
    assert_eq!(probe.gain(c), 1.0, "a fresh bind is clean");
    // Bound again with the previous edits.
    rig.run(Command::SetClipAra {
        track: t,
        clip: c,
        ara: Some(Box::new(previous)),
    });
    rig.sync();
    assert_eq!(probe.gain(c), 0.5);
}

/// Child processes for the tests that leave a process unusable: a crash, a hang.
#[test]
#[ignore = "run by its parent test in a child process"]
fn ara_child() {
    let dir = PathBuf::from(std::env::var("CANTUS_ARA_MARKER").expect("marker dir"));
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.session = AraSession::with_marker(dir);
    match std::env::var("CANTUS_ARA_CHILD").as_deref() {
        Ok("crash") => {
            probe.crash_next_render();
            rig.sync(); // aborts the process
            std::process::exit(3);
        }
        Ok("hang") => {
            rig.session
                .set_hang_limit(std::time::Duration::from_millis(150));
            probe.hang_next_render(5_000);
            let t = std::time::Instant::now();
            rig.sync_once();
            std::thread::sleep(std::time::Duration::from_millis(400));
            rig.sync_once();
            let stopped = matches!(rig.session.status(rig.clip), Some(AraStatus::Stopped(_)));
            let quick = t.elapsed() < std::time::Duration::from_secs(2);
            // Exit at once: the hung render thread is never joined.
            std::process::exit(if stopped && quick { 0 } else { 4 });
        }
        _ => {}
    }
}

fn child(kind: &str, dir: &Path) -> std::process::ExitStatus {
    Process::new(std::env::current_exe().unwrap())
        .args([
            "ara_child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CANTUS_ARA_CHILD", kind)
        .env("CANTUS_ARA_MARKER", dir)
        .status()
        .expect("the child runs")
}

#[test]
fn a_plugin_that_crashes_cantus_is_stopped_at_the_next_start_until_retry() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let status = child("crash", dir.path());
    assert!(!status.success(), "the child crashed");
    assert!(
        dir.path().join(cantus_host::ara::MARKER_FILE).exists(),
        "the marker stayed"
    );
    let mut rig = Rig::new();
    rig.session = AraSession::with_marker(dir.path().to_path_buf());
    rig.sync();
    assert!(
        matches!(rig.session.status(rig.clip), Some(AraStatus::Stopped(_))),
        "{:?}",
        rig.session.status(rig.clip)
    );
    assert!(
        rig.session.renders().is_empty(),
        "its clips play their sources"
    );
    rig.session.retry(&rig.info.id);
    rig.sync();
    assert_eq!(rig.session.status(rig.clip), Some(&AraStatus::Live));
    drop(rig);
    assert!(
        !dir.path().join(cantus_host::ara::MARKER_FILE).exists(),
        "cleared after"
    );
}

#[test]
fn a_plugin_that_hangs_is_stopped_and_the_window_never_waits() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    assert!(child("hang", dir.path()).success());
}

#[test]
fn ara_plugins_off_play_every_source() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut rig = Rig::new();
    rig.sync();
    rig.session.set_enabled(false);
    rig.sync();
    assert_eq!(rig.session.status(rig.clip), Some(&AraStatus::Off));
    assert!(rig.session.renders().is_empty() && !rig.session.active());
    rig.session.set_enabled(true);
    rig.sync();
    assert_eq!(rig.session.status(rig.clip), Some(&AraStatus::Live));
}

/// Measured, and written in `docs/round36/ara.md`: 20 comp clips over a 10-minute stereo
/// take hold their windows only; an edit of a part of a 3-minute clip is heard (its render
/// taken in) within a frame or two.
#[test]
fn memory_of_twenty_comp_clips_and_the_time_from_edit_to_heard() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    let frames = 10 * 60 * RATE as usize;
    let long: Vec<f32> = (0..frames * 2)
        .map(|i| ((i as f32) * 0.001).sin() * 0.3)
        .collect();
    rig.audio = Arc::new(SampleData {
        rate: RATE,
        channels: 2,
        frames: long.into(),
        stream: None,
    });
    // 20 windows of 30 s each across the take (120 BPM: 60 beats = 30 s).
    let t = rig.track;
    let first = clip_of(rig.bus.project(), rig.clip).clone();
    rig.run(Command::RemoveClip {
        track: t,
        clip: rig.clip,
    });
    for k in 0..20i64 {
        let mut c = first.clone();
        c.id = ClipId(rig.bus.alloc_id());
        c.start = Ticks::beats(60 * k);
        c.length = Ticks::beats(60);
        c.offset = Ticks::ZERO;
        rig.run(Command::InsertClip {
            track: t,
            clip: Box::new(c),
        });
    }
    let t0 = std::time::Instant::now();
    rig.sync();
    let all = t0.elapsed();
    let bytes = rig.session.render_bytes();
    let source = frames * 2 * 4;
    eprintln!("ARA memory: 20 comp clips of a 10-min take: renders {} MB (source {} MB, a copy per clip would be {} MB); rendered in {:?}", bytes / 1_000_000, source / 1_000_000, 20 * source / 1_000_000, all);
    assert!(bytes < source + source / 5, "{bytes} bytes for the renders");
    // A 3-minute clip, an edit of 1 s of it: from the plug-in's word to the render taken in.
    let mut c = first.clone();
    c.id = ClipId(rig.bus.alloc_id());
    c.start = Ticks::beats(1200);
    c.length = Ticks::beats(360);
    let three = c.id;
    rig.run(Command::InsertClip {
        track: t,
        clip: Box::new(c),
    });
    rig.sync();
    probe.set_gain_range(three, 0.5, 60.0, 61.0);
    let t1 = std::time::Instant::now();
    rig.sync();
    let heard = t1.elapsed();
    eprintln!(
        "ARA edit to heard: 1 s edited in a 3-min clip: {heard:?} (last render {:?})",
        rig.session.last_render()
    );
    assert!(heard < std::time::Duration::from_secs(2));
}

// ---- the fix round's findings -----------------------------------------------------------

/// A part said changed while the clip's stretch changed too (the plug-in's head grew): the
/// last render no longer covers the stretch, so the whole region is rendered — never the
/// part alone over the raw source, which would leave the rest unprocessed.
#[test]
fn a_part_over_a_render_of_another_stretch_renders_the_whole_region() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    probe.set_head_tail(0.05, 0.0);
    probe.set_gain_range(rig.clip, 0.5, 0.4, 0.45);
    rig.sync();
    probe.set_head_tail(0.0, 0.0);
    let r = rig.render(rig.clip);
    // The region shows the source from 0.25 s to its end (1.3 s): all of it at the new gain.
    let (a, b) = ((0.26 * RATE as f64) as usize, (1.29 * RATE as f64) as usize);
    for i in a..b {
        assert_eq!(r[i * 2], rig.audio.frames[i * 2] * 0.5, "frame {i}");
    }
}

/// A part said changed while the whole region's render is still on its way: the new job
/// covers the whole region too (it cancels the old one), so nothing stale is left.
#[test]
fn a_part_never_replaces_a_whole_render_in_flight() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    rig.session.set_debounce(std::time::Duration::ZERO);
    // A whole re-render (an edit everywhere), held up in the plug-in…
    probe.hang_next_render(300);
    probe.set_gain(rig.clip, 0.5);
    rig.sync_once();
    for (cmd, key) in rig.session.take_edits() {
        rig.bus
            .execute_merging(cmd, Origin::User, cantus_core::MergeKey::of(&("ara", key)))
            .unwrap();
    }
    assert!(
        rig.session.progress(rig.clip).is_some(),
        "the whole render is on its way"
    );
    // …and a part said changed meanwhile.
    probe.set_gain_range(rig.clip, 0.25, 0.4, 0.45);
    rig.sync();
    let r = rig.render(rig.clip);
    let (a, b) = ((0.26 * RATE as f64) as usize, (1.29 * RATE as f64) as usize);
    for i in a..b {
        assert_eq!(
            r[i * 2],
            rig.audio.frames[i * 2] * 0.25,
            "frame {i}: no stale render"
        );
    }
}

/// An offline render waits until every bound clip plays its current render.
#[test]
fn the_session_is_settled_only_when_no_render_is_on_its_way() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let probe = Probe::new();
    let mut rig = Rig::new();
    rig.sync();
    assert!(rig.session.settled());
    probe.hang_next_render(200);
    probe.set_gain(rig.clip, 0.5);
    rig.sync_once();
    assert!(
        !rig.session.settled(),
        "an edit not yet in the project, a render on its way"
    );
    for (cmd, key) in rig.session.take_edits() {
        rig.bus
            .execute_merging(cmd, Origin::User, cantus_core::MergeKey::of(&("ara", key)))
            .unwrap();
    }
    assert!(!rig.session.settled(), "a render on its way");
    let t = std::time::Instant::now();
    while rig.session.pending() > 0 && t.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(!rig.session.settled(), "made, not yet taken in");
    assert!(rig.session.settled());
}
