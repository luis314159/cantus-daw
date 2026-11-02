//! An imported file as a [`Character`]: its clips mapped onto our moves (ADR 0089 §4).
//!
//! Every format is a [`Source`]: clips it can draw at a time, in its own units. This turns
//! a [`Pose`] into (clip, time) — the move's clip (or a mood's in Idle's place), its time
//! from the pose's phase (the song's beat) or, at *its own speed*, from the song's seconds —
//! and places the source's box on the character's space: its bottom centre at the feet,
//! one unit tall.

use super::mapping::{Clip, Mapping};
use super::raster::{self, Filter, Region, Tint};
use crate::character::{Bounds, Character, CharacterInfo};
use crate::pose::Pose;
use crate::render::Canvas;
use cantus_core::inst::dancer::{Mood, Move};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tiny_skia::{Pixmap, Transform};

/// What a format draws.
pub trait Source: Send + Sync {
    /// The format's name, as the card says it ("Lottie", "Aseprite"…).
    fn format(&self) -> &'static str;
    fn clips(&self) -> &[Clip];
    /// The box of everything it draws, in its own units (y down).
    fn bounds(&self) -> Bounds;
    /// Draw clip `clip` at `t` seconds into it (0 ≤ t < its length) onto `canvas`, whose
    /// transform maps the source's units onto the pixels. `filter` is the person's choice
    /// for images.
    fn draw(&self, clip: usize, t: f32, filter: Filter, canvas: &mut Canvas);
    /// Whether frames are worth caching (a heavy vector file): decided from the file's
    /// contents, never from a timing, so every machine draws the same pixels.
    fn heavy(&self) -> bool {
        false
    }
    /// The file's own frame rate (a cached frame is one of these).
    fn frame_rate(&self) -> f32;
}

/// How a person set an import up (kept in `character.json`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub mapping: Mapping,
    pub filter: Filter,
    /// Play the file's frames at its own pace instead of spanning each move's beats.
    #[serde(default)]
    pub own_speed: bool,
}

/// The frames of a heavy file drawn once at the sizes shown (review of the fix rounds):
/// every frame its own entry, evicted by recency within one byte budget — never a whole
/// size at once. Sizes are quantised to an eighth of an octave, so a dancer being resized
/// reuses its frames; a size drawn in the last second is protected from a size drawn less
/// often (an export evicts its own frames before the screen's). Frames are drawn at hue 0;
/// the hue is turned as a frame is placed.
struct FrameCache {
    entries: Vec<Entry>,
    bytes: usize,
    tick: u64,
    /// When each size was last drawn.
    seen: Vec<(i32, Instant)>,
}

struct Entry {
    scale: i32,
    key: (usize, u32),
    frame: Arc<Pixmap>,
    used: u64,
}

/// At most this much of drawn frames for each character, all sizes together.
pub const CACHE_BYTES: usize = 64 << 20;
/// A size drawn this recently is protected from sizes drawn less often.
pub const LIVE: Duration = Duration::from_secs(1);
/// Steps of a size key per octave.
pub const SCALE_STEPS: f32 = 8.0;

/// The size key of `ppu` (pixels per source unit) and the scale it stands for.
pub fn scale_key(ppu: f32) -> (i32, f32) {
    let ppu = if ppu.is_finite() {
        ppu.clamp(1e-4, 1e5)
    } else {
        1.0
    };
    let q = (cantus_dsp::math::log2(ppu) * SCALE_STEPS).round() as i32;
    (q, cantus_dsp::math::exp2(q as f32 / SCALE_STEPS))
}

impl FrameCache {
    fn live(&self, scale: i32, now: Instant) -> bool {
        self.seen
            .iter()
            .any(|(s, at)| *s == scale && now.duration_since(*at) < LIVE)
    }

    fn get(&mut self, scale: i32, key: (usize, u32)) -> Option<Arc<Pixmap>> {
        let now = Instant::now();
        match self.seen.iter_mut().find(|(s, _)| *s == scale) {
            Some(s) => s.1 = now,
            None => self.seen.push((scale, now)),
        }
        self.seen
            .retain(|(_, at)| now.duration_since(*at) < Duration::from_secs(60));
        self.tick += 1;
        let tick = self.tick;
        let e = self
            .entries
            .iter_mut()
            .find(|e| e.scale == scale && e.key == key)?;
        e.used = tick;
        Some(e.frame.clone())
    }

    fn insert(&mut self, scale: i32, key: (usize, u32), frame: Arc<Pixmap>) {
        if self
            .entries
            .iter()
            .any(|e| e.scale == scale && e.key == key)
        {
            return;
        }
        let size = frame.data().len();
        let now = Instant::now();
        while self.bytes + size > CACHE_BYTES && !self.entries.is_empty() {
            // First a size nobody drew in the last second, then this size's own, then
            // the least recently used of all.
            let victim = {
                let pick = |f: &dyn Fn(&Entry) -> bool| {
                    self.entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| f(e))
                        .min_by_key(|(_, e)| e.used)
                        .map(|(i, _)| i)
                };
                pick(&|e| !self.live(e.scale, now))
                    .or_else(|| pick(&|e| e.scale == scale))
                    .or_else(|| pick(&|_| true))
            };
            let Some(i) = victim else { break };
            let old = self.entries.swap_remove(i);
            self.bytes -= old.frame.data().len();
        }
        self.tick += 1;
        self.bytes += size;
        self.entries.push(Entry {
            scale,
            key,
            frame,
            used: self.tick,
        });
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

pub struct Imported {
    info: CharacterInfo,
    source: Box<dyn Source>,
    settings: Settings,
    /// The source's units onto the character's (feet at the origin, one unit tall).
    to_units: Transform,
    bounds: Bounds,
    routine: Vec<Move>,
    cache: Mutex<FrameCache>,
    /// A drawing that panicked once: never drawn again (the frame goes on without it).
    broken: AtomicBool,
}

impl Imported {
    pub fn new(info: CharacterInfo, source: Box<dyn Source>, settings: Settings) -> Imported {
        let clips = source.clips().len();
        let settings = Settings {
            mapping: settings.mapping.sanitised(clips),
            ..settings
        };
        let b = source.bounds();
        let h = b.height().max(1e-3);
        let s = 1.0 / h;
        let cx = (b.left + b.right) * 0.5;
        let to_units = Transform::from_scale(s, s).pre_translate(-cx, -b.bottom);
        let bounds = Bounds {
            left: (b.left - cx) * s,
            top: -1.0,
            right: (b.right - cx) * s,
            bottom: 0.0,
        };
        let routine = routine_of(&settings.mapping, clips);
        Imported {
            info,
            source,
            settings,
            to_units,
            bounds,
            routine,
            broken: AtomicBool::new(false),
            cache: Mutex::new(FrameCache {
                entries: Vec::new(),
                bytes: 0,
                tick: 0,
                seen: Vec::new(),
            }),
        }
    }

    pub fn source(&self) -> &dyn Source {
        self.source.as_ref()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Choose again (the card's mapping, its pace, its sampling): the routine follows and
    /// the cached frames go.
    pub fn set_settings(&mut self, settings: Settings) {
        let clips = self.source.clips().len();
        self.settings = Settings {
            mapping: settings.mapping.sanitised(clips),
            ..settings
        };
        self.routine = routine_of(&self.settings.mapping, clips);
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        cache.entries.clear();
        cache.bytes = 0;
    }

    /// How many drawn frames of a heavy file are kept now.
    pub fn cached_frames(&self) -> usize {
        self.lock_cache().len()
    }

    /// The sizes kept now (keys of [`scale_key`]) and how many frames each holds.
    pub fn cached_scales(&self) -> Vec<(i32, usize)> {
        let cache = self.lock_cache();
        let mut out: Vec<(i32, usize)> = Vec::new();
        for e in &cache.entries {
            match out.iter_mut().find(|(s, _)| *s == e.scale) {
                Some(s) => s.1 += 1,
                None => out.push((e.scale, 1)),
            }
        }
        out.sort();
        out
    }

    fn lock_cache(&self) -> std::sync::MutexGuard<'_, FrameCache> {
        match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        }
    }

    /// Whether its drawing failed (a panic in its format's code) and it is left out.
    pub fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Relaxed)
    }

    pub fn info_mut(&mut self) -> &mut CharacterInfo {
        &mut self.info
    }

    /// The clip and the time in it that `pose` shows.
    pub fn clip_at(&self, pose: &Pose) -> Option<(usize, f32)> {
        let clips = self.source.clips();
        let n = clips.len();
        let map = &self.settings.mapping;
        let mood_clip = (pose.mv == Move::Idle && pose.mood != Mood::Neutral)
            .then(|| map.mood_clip(pose.mood, n))
            .flatten();
        let ci = mood_clip.or_else(|| map.clip_for(pose.mv, n))?;
        let len = clips[ci].seconds.max(1e-3);
        let t = if self.settings.own_speed || mood_clip.is_some() {
            (pose.seconds.rem_euclid(f64::from(len))) as f32
        } else {
            pose.phase.clamp(0.0, 1.0) * len
        };
        Some((ci, t.clamp(0.0, len * 0.999_999)))
    }

    fn draw_cached(&self, ci: usize, t: f32, canvas: &mut Canvas) {
        let b = self.source.bounds();
        let fr = self.source.frame_rate().clamp(1.0, 240.0);
        let frame = (t * fr) as u32;
        // The scale the frame is drawn at: pixels per source unit, an eighth of an octave
        // at a time (a resize reuses frames; the blit below fits it to the size shown).
        let ppu = canvas.px_per_unit * self.to_units.sx.abs();
        let (key_scale, scale) = scale_key(ppu);
        let key = (ci, frame);
        let (w, h) = (
            ((b.width() * scale).ceil() as u32).clamp(1, 4096),
            ((b.height() * scale).ceil() as u32).clamp(1, 4096),
        );
        // Looked up under the lock; drawn outside it (another thread drawing the same
        // character — a video export's workers, the screen — never waits for this one's
        // rasterising; two may draw the same frame, and the first kept is the one used).
        let cached = self.lock_cache().get(key_scale, key);
        let pm = match cached {
            Some(pm) => pm,
            None => {
                let Some(mut pm) = Pixmap::new(w, h) else {
                    return;
                };
                {
                    let mut c = Canvas {
                        pixmap: &mut pm,
                        transform: Transform::from_scale(scale, scale)
                            .pre_translate(-b.left, -b.top),
                        px_per_unit: scale,
                        hue: 0.0,
                        shadow: true,
                    };
                    self.source
                        .draw(ci, frame as f32 / fr, self.settings.filter, &mut c);
                }
                let pm = Arc::new(pm);
                let mut cache = self.lock_cache();
                cache.insert(key_scale, key, pm.clone());
                cache.get(key_scale, key).unwrap_or(pm)
            }
        };
        // The frame's pixels → source units → the canvas.
        let t = canvas
            .transform
            .pre_concat(self.to_units)
            .pre_translate(b.left, b.top)
            .pre_scale(1.0 / scale, 1.0 / scale);
        raster::image(
            canvas.pixmap,
            &pm,
            Region::of(&pm),
            t,
            Filter::Bilinear,
            Tint::NONE.hue(canvas.hue),
        );
    }
}

impl Character for Imported {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }

    fn has_move(&self, m: Move) -> bool {
        let n = self.source.clips().len();
        self.settings.mapping.has(m, n) || (m == Move::Idle && n > 0)
    }

    fn move_beats(&self, m: Move) -> f32 {
        self.settings.mapping.beats_of(m)
    }

    fn routine(&self) -> &[Move] {
        &self.routine
    }

    fn bounds(&self) -> Bounds {
        self.bounds
    }

    fn draw(&self, pose: &Pose, canvas: &mut Canvas) {
        if self.is_broken() {
            return;
        }
        let Some((ci, t)) = self.clip_at(pose) else {
            return;
        };
        let saved = (canvas.transform, canvas.px_per_unit);
        let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if self.source.heavy() {
                self.draw_cached(ci, t, canvas);
                return;
            }
            canvas.transform = canvas.transform.pre_concat(self.to_units);
            canvas.px_per_unit *= self.to_units.sx.abs();
            self.source.draw(ci, t, self.settings.filter, canvas);
        }));
        (canvas.transform, canvas.px_per_unit) = saved;
        if drawn.is_err() {
            self.broken.store(true, Ordering::Relaxed);
        }
    }
}

/// An import's routine: the usual one (the trait's) where the file has those moves; else
/// every move it has, in our order (not Idle or Freeze — they do not dance); else Idle.
pub fn routine_of(mapping: &Mapping, clips: usize) -> Vec<Move> {
    let usual: Vec<Move> = ROUTINE
        .iter()
        .copied()
        .filter(|m| mapping.has(*m, clips))
        .collect();
    if usual.len() >= 4 {
        return usual;
    }
    let all: Vec<Move> = Move::ALL
        .iter()
        .copied()
        .filter(|m| !matches!(m, Move::Idle | Move::Freeze) && mapping.has(*m, clips))
        .collect();
    if all.is_empty() {
        vec![if clips > 0 && !mapping.has(Move::Idle, clips) {
            Move::Bounce
        } else {
            Move::Idle
        }]
    } else {
        all
    }
}

const ROUTINE: [Move; 8] = [
    Move::Bounce,
    Move::Bounce,
    Move::StepLeft,
    Move::StepRight,
    Move::Clap,
    Move::Sway,
    Move::ArmsUp,
    Move::Spin,
];
