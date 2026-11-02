//! The heavy-frame cache (the integrated review): a frame is rasterised outside the cache's
//! lock, so two threads drawing one character (a video export's workers, the screen) never
//! wait on each other's rasterising and draw the same pixels; each size shown keeps its own
//! budget, so an export at another size never evicts the screen's frames.

use cantus_dance::character::{Bounds, CharacterInfo, Origin};
use cantus_dance::import::{Clip, Filter, Imported, Mapping, Settings, Source};
use cantus_dance::render::{self, Background, Canvas, Stage};
use cantus_dance::{pose_at, DanceFrame, Licence, Pose};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_skia::{Paint, PathBuilder};

type Meet = (Sender<()>, Receiver<()>);

/// A heavy source whose drawings can be made to meet each other: each draw takes a pair,
/// says it is in, and waits (at most 3 s) for the other's word — which only comes if the
/// other thread could start drawing meanwhile.
struct Meeting {
    clips: Vec<Clip>,
    meet: Mutex<Vec<Meet>>,
    /// How many frames it has rasterised.
    draws: Arc<AtomicUsize>,
}

impl Source for Meeting {
    fn format(&self) -> &'static str {
        "Test"
    }
    fn clips(&self) -> &[Clip] {
        &self.clips
    }
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -100.0,
            top: -200.0,
            right: 100.0,
            bottom: 0.0,
        }
    }
    fn draw(&self, _clip: usize, t: f32, _f: Filter, canvas: &mut Canvas) {
        self.draws.fetch_add(1, Ordering::SeqCst);
        let pair = self.meet.lock().unwrap().pop();
        if let Some((tx, rx)) = pair {
            let _ = tx.send(());
            assert!(
                rx.recv_timeout(Duration::from_secs(3)).is_ok(),
                "the other thread could not draw meanwhile"
            );
        }
        let mut paint = Paint::default();
        paint.set_color_rgba8(200, (t * 40.0) as u8, 60, 255);
        if let Some(p) = PathBuilder::from_circle(0.0, -100.0, 60.0 + t * 10.0) {
            canvas.pixmap.fill_path(
                &p,
                &paint,
                tiny_skia::FillRule::Winding,
                canvas.transform,
                None,
            );
        }
    }
    fn heavy(&self) -> bool {
        true
    }
    fn frame_rate(&self) -> f32 {
        24.0
    }
}

fn character(meet: Vec<Meet>) -> Imported {
    character_counted(meet, Arc::new(AtomicUsize::new(0)))
}

fn character_counted(meet: Vec<Meet>, draws: Arc<AtomicUsize>) -> Imported {
    let clips = vec![Clip {
        name: "idle".into(),
        seconds: 4.0,
        frames: 96,
    }];
    let info = CharacterInfo {
        id: "file:test".into(),
        name: "test".into(),
        author: String::new(),
        origin: Origin::Imported { format: "Test" },
        licence: Licence::Unknown,
    };
    let settings = Settings {
        mapping: Mapping::guess(&clips),
        filter: Filter::Bilinear,
        own_speed: true,
    };
    let source = Meeting {
        clips,
        meet: Mutex::new(meet),
        draws,
    };
    Imported::new(info, Box::new(source), settings)
}

fn at(ch: &Imported, seconds: f64) -> Pose {
    let f = DanceFrame {
        playing: true,
        beats: seconds * 2.0,
        seconds,
        ..DanceFrame::default()
    };
    pose_at(&f, ch, 0.0)
}

fn drawn(ch: &Imported, seconds: f64, size: u32) -> Vec<u8> {
    let stage = Stage::new(size, size, Background::Transparent);
    render::render(&stage, &[(ch, at(ch, seconds))])
        .unwrap()
        .data()
        .to_vec()
}

#[test]
fn two_threads_draw_one_character_without_waiting_on_each_other() {
    let alone = character(Vec::new());
    let (want_a, want_b) = (drawn(&alone, 0.5, 200), drawn(&alone, 1.5, 200));
    // Two crossed pairs: each thread's draw waits for the other's to have started.
    let (a_tx, a_rx) = channel();
    let (b_tx, b_rx) = channel();
    let shared = Arc::new(character(vec![(a_tx, b_rx), (b_tx, a_rx)]));
    let other = shared.clone();
    let handle = std::thread::spawn(move || drawn(&other, 1.5, 200));
    let mine = drawn(&shared, 0.5, 200);
    let theirs = handle.join().expect("the other thread drew");
    assert!(!shared.is_broken(), "neither draw waited out the other");
    assert_eq!(mine, want_a, "the same pixels as drawn alone");
    assert_eq!(theirs, want_b);
}

#[test]
fn another_size_never_evicts_the_screens_frames() {
    let ch = character(Vec::new());
    let first = drawn(&ch, 0.2, 300);
    // An export at a much bigger size draws many frames, more than the budget holds, while
    // the screen goes on showing its frame.
    for i in 0..60 {
        let _ = drawn(&ch, 0.3 + f64::from(i) * 0.05, 2000);
        if i % 5 == 0 {
            assert_eq!(drawn(&ch, 0.2, 300), first);
        }
    }
    let scales = ch.cached_scales();
    assert!(scales.len() >= 2, "{scales:?}");
    assert_eq!(drawn(&ch, 0.2, 300), first);
}

/// Five consumers at five sizes (a docked dancer, a floating one, the companion, the
/// dancer window's card, an export) drawing in turn: once each has drawn its frames, none
/// is drawn again — no size is evicted whole.
#[test]
fn five_consumers_drawing_in_turn_each_keep_their_frames() {
    let draws = Arc::new(AtomicUsize::new(0));
    let ch = character_counted(Vec::new(), draws.clone());
    let sizes = [180u32, 260, 360, 480, 640];
    let moments = [0.1f64, 0.6, 1.1, 1.6];
    for _round in 0..4 {
        for s in sizes {
            for m in moments {
                let _ = drawn(&ch, m, s);
            }
        }
    }
    assert_eq!(
        draws.load(Ordering::SeqCst),
        sizes.len() * moments.len(),
        "each frame rasterised once: {:?}",
        ch.cached_scales()
    );
}

/// A floating dancer being resized a pixel at a time reuses its frames (sizes are kept an
/// eighth of an octave apart).
#[test]
fn a_live_resize_reuses_frames() {
    let draws = Arc::new(AtomicUsize::new(0));
    let ch = character_counted(Vec::new(), draws.clone());
    for size in (300..=330).step_by(2) {
        let _ = drawn(&ch, 0.4, size);
    }
    let n = draws.load(Ordering::SeqCst);
    assert!(n <= 2, "16 sizes, {n} rasterisations");
}
