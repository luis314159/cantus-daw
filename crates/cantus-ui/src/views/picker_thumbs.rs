//! The thumbnails' cache (ADR 0029 §3): real-look pictures of the native devices, made
//! once per look and kept — in a texture atlas in memory (so the whole gallery draws its
//! pictures as one mesh a page) and as PNG files on disk (so the next start shows them at
//! once).
//!
//! A picture is asked for with [`Thumbs::get`]; until it is ready the tile shows its drawn
//! card. [`Thumbs::pump`] runs once a frame while the picker shows: it uploads what the
//! workers finished, and draws **one** capture at a time (the part that must run on the UI
//! thread, `picker_snap::capture`); the filling and shrinking run on a worker, which also
//! writes the PNG. A file on disk is decoded on a worker too.
//!
//! The key of a picture is the device, the look (every token of the theme, hashed), the
//! language (an editor has words in it) and [`REV`], bumped when the pictures' recipe
//! changes. A different look simply asks for other pictures.

use super::picker_snap::{self, Subject};
use egui::{ColorImage, Context, Rect, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The recipe's version: bump it when what a picture shows changes (2: the units' faces,
/// round 32 — pictures made plain before them are never read back; 3: Chorda's and
/// Tympanum's scenes waited for, round 40 — their pictures on disk showed an empty stage;
/// 4: the instruments at their own size, their words whole at every scale, fx/final).
pub const REV: u32 = 4;
/// A picture's size in pixels: 16:10, sharp on a large tile at 150 %.
pub const W: usize = 416;
pub const H: usize = 260;
/// Pixels per point the editor is drawn at before being shrunk (twice the final size).
const PPP: f32 = 0.8;
/// The corner radius baked into a picture, in pixels.
const RADIUS: f32 = 9.0;
/// An atlas page: four pictures across, seven down. One page holds what a gallery shows at
/// once; a picture not shown for the longest is let go for a new one (the store once held
/// 28 and dropped every later picture — 43 effects — asking for it again every frame), and
/// a second page is opened only when more than a page's worth is shown at once.
const COLS: usize = 4;
const ROWS: usize = 7;
const PER_PAGE: usize = COLS * ROWS;

/// r42:tuning-face: what a pass of the idle warm-up may cost the interface thread beyond a
/// plain pass, ms (a quarter of a 60 Hz frame): its editors are drawn on a worker.
pub const WARM_BUDGET_MS: f64 = 4.0;

/// What the cache has done, for tests and the frame budget.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Editors drawn off screen.
    pub captures: u32,
    /// Pictures read back from disk.
    pub disk_loads: u32,
    /// Pictures put into the atlas.
    pub uploads: u32,
    /// Pictures made ahead and dropped: the look changed while they were made.
    pub stale: u32,
}

enum Done {
    Image(Subject, u64, ColorImage),
    Failed(Subject, u64),
    /// r42 fix-ui: made while the theme changed under it — neither saved nor shown, and not
    /// a failure (the new look makes it again).
    Stale(Subject),
}

/// r42 fix-ui — when the warm-up's runner may make its next picture: the app sets it in the
/// frames it paints (opened at the instant the start-up and a quiet hand allow, closed
/// while the song plays or records and while the picker is open), and the runner waits on
/// it — so at rest, with no frame painted, it goes on by itself and the window is never
/// asked for a frame (round 38's rest).
#[derive(Debug, Default)]
pub struct Gate {
    /// The instant from which it may make the next picture; `None`: closed.
    open_at: Mutex<Option<Instant>>,
    /// The look changed: stop before the next picture.
    stop: AtomicBool,
    /// A tile is waiting (the picker open): ask for the frame a finished picture lands in.
    wake: AtomicBool,
    /// Pictures begun since the last pump.
    started: AtomicU32,
    /// The runner is still working.
    running: AtomicBool,
}

impl Gate {
    /// Open from `wait` seconds from now (0: at once).
    pub fn open_in(&self, wait: f64) {
        let at = Instant::now() + Duration::from_secs_f64(wait.max(0.0));
        if let Ok(mut g) = self.open_at.lock() {
            *g = Some(at);
        }
    }

    /// Closed until opened again.
    pub fn close(&self) {
        if let Ok(mut g) = self.open_at.lock() {
            *g = None;
        }
    }

    /// Whether a finished picture asks for a frame (a tile waits for it).
    pub fn set_wake(&self, on: bool) {
        self.wake.store(on, Ordering::Release);
    }

    /// Wait (on the runner) until open; `false` when told to stop.
    fn wait_open(&self) -> bool {
        loop {
            if self.stop.load(Ordering::Acquire) {
                return false;
            }
            let open = self
                .open_at
                .lock()
                .map(|g| g.is_some_and(|at| Instant::now() >= at))
                .unwrap_or(false);
            if open {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// See the module docs.
pub struct Thumbs {
    /// Off: no pictures are made (tests, and until the app turns it on); tiles show cards.
    pub enabled: bool,
    disk: Option<PathBuf>,
    /// The atlas pages (most often one).
    pages: Vec<TextureHandle>,
    /// Where each picture of the current look sits: its page and slot.
    slots: HashMap<Subject, (usize, usize)>,
    /// The frame each picture was last asked for ([`Thumbs::get`]), to let the oldest go.
    seen: HashMap<Subject, u64>,
    /// Frames pumped.
    frame: u64,
    look: u64,
    wanted: VecDeque<Subject>,
    busy: HashSet<Subject>,
    failed: HashSet<Subject>,
    /// The editor being drawn while its scene's picture comes (one at a time).
    capturing: Option<picker_snap::Capture>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// The look whose old neighbours on disk were last pruned.
    pruned: u64,
    /// r42:tuning-face — the pictures made ahead, while the app is idle (BACKLOG §6j): every
    /// device in the order asked ([`Thumbs::warm`], the most used first), and those still to
    /// make in this look. Lower than anything a tile asks for: one a pump, after them.
    warm_all: Vec<Subject>,
    warming: VecDeque<Subject>,
    /// Pictures made ahead only to the disk: the atlas's page was full (it keeps the most
    /// used; the rest come back from their files in a frame).
    disk_only: HashSet<Subject>,
    /// The pictures the runner has still to make ahead (r42 fix-ui: one runner walks the
    /// list; it was one worker a pump).
    ahead: HashSet<Subject>,
    /// The warm-up's runner, while it works (r42 fix-ui).
    runner: Option<std::sync::Arc<Gate>>,
    /// What a worker needs of the interface thread's look to draw as it draws.
    worker_look: WorkerLook,
    pub stats: Stats,
}

impl Default for Thumbs {
    fn default() -> Self {
        let (tx, rx) = channel();
        Thumbs {
            enabled: false,
            disk: None,
            pages: Vec::new(),
            slots: HashMap::new(),
            seen: HashMap::new(),
            frame: 0,
            look: 0,
            wanted: VecDeque::new(),
            busy: HashSet::new(),
            failed: HashSet::new(),
            capturing: None,
            tx,
            rx,
            pruned: 0,
            warm_all: Vec::new(),
            warming: VecDeque::new(),
            disk_only: HashSet::new(),
            ahead: HashSet::new(),
            runner: None,
            worker_look: WorkerLook::default(),
            stats: Stats::default(),
        }
    }
}

/// r42:tuning-face: what a worker wears to draw a picture as the interface thread draws
/// it: the thread-local parts of the look (the units' faces switch, their colourways and
/// views) and the language.
#[derive(Clone, Debug)]
pub struct WorkerLook {
    pub faces: bool,
    pub colourways: Vec<(String, String)>,
    pub views: Vec<(String, String)>,
    pub locale: crate::i18n::Locale,
}

impl Default for WorkerLook {
    fn default() -> Self {
        WorkerLook {
            faces: crate::kit::skin::enabled(),
            colourways: Vec::new(),
            views: Vec::new(),
            locale: crate::i18n::locale(),
        }
    }
}

impl WorkerLook {
    /// Wear it on this thread (a worker's).
    pub fn wear(&self) {
        crate::i18n::set_thread_locale(self.locale);
        crate::kit::skin::set_enabled(self.faces);
        crate::kit::skin::set_colourways(
            self.colourways
                .iter()
                .map(|(u, id)| (u.as_str(), id.as_str())),
        );
        crate::views::fx::unit_view::set_views(
            self.views.iter().map(|(u, id)| (u.as_str(), id.as_str())),
        );
    }
}

/// The look a picture is made in: every token of the installed theme, the language, and
/// the units' faces — the switch and every sheet (`skin::sheets_digest`; round 32), so a
/// picture made plain, or before a sheet changed, is never shown for a face.
pub fn look_key() -> u64 {
    use crate::kit::cache;
    cache::key((
        theme_key(),
        crate::i18n::locale().code(),
        crate::kit::skin::enabled(),
        crate::kit::skin::sheets_digest(),
        REV,
    ))
}

/// The installed theme's part of [`look_key`]: every token. Process-wide (the tokens are
/// global and live), so a worker reads the same value the interface thread does — it
/// checks it after a capture to know the theme did not change under it.
pub fn theme_key() -> u64 {
    use crate::kit::{cache, Tok};
    let t = crate::kit::tokens::current();
    let colours: Vec<[u8; 4]> = Tok::ALL.iter().map(|k| t.get(*k).to_array()).collect();
    cache::key((
        colours,
        t.dark,
        t.radius.to_bits(),
        t.round.to_bits(),
        t.depth.to_bits(),
        t.density.to_bits(),
        t.text_scale.to_bits(),
    ))
}

impl Thumbs {
    /// r42:tuning-face: what the warm-up's worker must wear to draw as this thread draws
    /// (the units' faces, their colourways and views, the language).
    pub fn set_worker_look(&mut self, look: WorkerLook) {
        self.worker_look = look;
    }

    /// Turn pictures on, kept under `disk` (a folder; `None` keeps them in memory only).
    pub fn enable(&mut self, disk: Option<PathBuf>) {
        self.enabled = true;
        self.disk = disk;
    }

    fn file(&self, s: Subject, look: u64) -> Option<PathBuf> {
        let name = format!("{s:?}").to_lowercase().replace(['(', ')'], "-");
        self.disk
            .as_ref()
            .map(|d| d.join(format!("{name}{look:016x}-{W}.png")))
    }

    /// The atlas and the picture's place in it (as UV), if `s`'s picture is ready; else
    /// ask for it.
    pub fn get(&mut self, s: Subject) -> Option<(egui::TextureId, Rect)> {
        if !self.enabled {
            return None;
        }
        self.seen.insert(s, self.frame);
        if let Some(&(page, slot)) = self.slots.get(&s) {
            if let Some(atlas) = self.pages.get(page) {
                return Some((atlas.id(), uv(slot)));
            }
        }
        if !self.busy.contains(&s) && !self.failed.contains(&s) && !self.wanted.contains(&s) {
            self.wanted.push_back(s);
        }
        None
    }

    /// Whether a picture is being made ahead now.
    pub fn making_ahead(&self) -> bool {
        !self.ahead.is_empty()
    }

    /// Whether every picture asked for is in.
    pub fn idle(&self) -> bool {
        self.wanted.is_empty() && self.busy.is_empty() && self.capturing.is_none()
    }

    /// r42:tuning-face — make `subjects`' pictures ahead (BACKLOG §6j), in this order (the
    /// most used first: the atlas keeps one page of them, the rest go to the disk only),
    /// after anything a tile asks for, one a pump. Again in a new look.
    pub fn warm(&mut self, subjects: impl IntoIterator<Item = Subject>) {
        for s in subjects {
            if !self.warm_all.contains(&s) {
                self.warm_all.push(s);
                self.warming.push_back(s);
            }
        }
    }

    /// How many pictures the warm-up has still to make or read in this look.
    pub fn warm_left(&self) -> usize {
        self.warming.len() + self.ahead.len()
    }

    /// Whether `s`'s picture is ready to show at once: in the atlas, or made on disk (read
    /// back in a frame, no editor drawn).
    pub fn ready(&self, s: Subject) -> bool {
        self.slots.contains_key(&s) || self.file(s, self.look).is_some_and(|p| p.exists())
    }

    /// r42 fix-ui — the warm-up's gate, while its runner works through the list (the app
    /// opens and closes it each frame it paints; nothing here asks for a frame).
    pub fn gate(&self) -> Option<std::sync::Arc<Gate>> {
        self.runner.clone()
    }

    /// r42 fix-ui — start the runner on what is left to make ahead in this look, if it is
    /// not running: one background thread that makes the pictures one after another — the
    /// editor staged in a scratch application, drawn, tessellated, filled, shrunk and
    /// written there (round 42: drawn on the interface thread, a step of a heavy editor
    /// stalled the window 0.9 s) — each only while its [`Gate`] is open. Pictures already
    /// on disk are left there (a tile reads its file back in a frame). The runner never asks
    /// the window for a frame (round 38's rest, `idle_repaint_e2e`): what it finishes is
    /// taken in by the next frame the app paints anyway.
    pub fn warm_start(&mut self, ctx: &Context) {
        if self.runner.is_some() || self.warming.is_empty() || !self.enabled {
            return;
        }
        let look = self.look;
        let mut jobs: Vec<(Subject, Option<PathBuf>)> = Vec::new();
        while let Some(s) = self.warming.pop_front() {
            let taken =
                self.slots.contains_key(&s) || self.busy.contains(&s) || self.failed.contains(&s);
            if taken || self.file(s, look).is_some_and(|p| p.exists()) {
                continue;
            }
            // The atlas keeps the most used (a page); the rest go to the disk only.
            if self.slots.len() + self.busy.len() >= PER_PAGE {
                self.disk_only.insert(s);
            }
            self.busy.insert(s);
            self.ahead.insert(s);
            jobs.push((s, self.file(s, look)));
        }
        if jobs.is_empty() {
            return;
        }
        let gate = std::sync::Arc::new(Gate::default());
        gate.running.store(true, Ordering::Release);
        self.runner = Some(gate.clone());
        let (tx, ctx2, wear) = (self.tx.clone(), ctx.clone(), self.worker_look.clone());
        // r42 fix-ui (review ui#11): a picture is saved under this look's file only if it
        // was made in this look — the language and faces the runner wears are this
        // thread's, and the theme (global, live) did not change during its capture.
        let theme = theme_key();
        let worn =
            wear.locale == crate::i18n::locale() && wear.faces == crate::kit::skin::enabled();
        let spawned = std::thread::Builder::new()
            .name("cantus-picker-ahead".into())
            .spawn(move || {
                // Background priority (review ui#12): below the interface, never MMCSS.
                let _ = cantus_rt::thread::lower_background_thread();
                wear.wear();
                for (s, path) in jobs {
                    if !gate.wait_open() {
                        break;
                    }
                    gate.started.fetch_add(1, Ordering::AcqRel);
                    let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        picker_snap::capture(&ctx2, s, PPP)
                            .map(|frame| picker_snap::finish(frame, W, RADIUS))
                    }));
                    let same_look = worn && theme_key() == theme;
                    let msg = match made {
                        Ok(Some(_)) if !same_look => Done::Stale(s),
                        Ok(Some(img)) => {
                            if let Some(p) = path {
                                save_png(&img, &p);
                            }
                            Done::Image(s, look, img)
                        }
                        _ => Done::Failed(s, look),
                    };
                    let _ = tx.send(msg);
                    // A frame only when a tile is waiting for it (the picker open).
                    if gate.wake.load(Ordering::Acquire) {
                        ctx2.request_repaint();
                    }
                }
                gate.running.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            // No thread: nothing made ahead; the picker makes them on demand.
            self.busy.retain(|s| !self.ahead.contains(s));
            self.ahead.clear();
            self.runner = None;
        }
    }

    /// Once a frame: take in what the workers finished, start what is wanted.
    pub fn pump(&mut self, ctx: &Context) {
        if !self.enabled {
            return;
        }
        self.frame += 1;
        let look = look_key();
        if look != self.look {
            // Another theme or language: other pictures. The atlas is reused.
            self.look = look;
            self.slots.clear();
            self.seen.clear();
            self.failed.clear();
            self.wanted.clear();
            // A capture in the old look is dropped (its editor has the old words).
            if let Some(c) = self.capturing.take() {
                self.busy.remove(&c.subject());
            }
            // r42:tuning-face — the warm-up begins again in the new look (r42 fix-ui: the
            // runner in the old one stops before its next picture).
            if let Some(g) = self.runner.take() {
                g.stop.store(true, Ordering::Release);
            }
            self.busy.retain(|s| !self.ahead.contains(s));
            self.ahead.clear();
            self.warming = self.warm_all.iter().copied().collect();
            self.disk_only.clear();
        }
        // r42 fix-ui — what the runner began since the last pump, and its end.
        if let Some(g) = &self.runner {
            self.stats.captures += g.started.swap(0, Ordering::AcqRel);
            if !g.running.load(Ordering::Acquire) && self.ahead.is_empty() {
                self.runner = None;
            }
        }
        while let Ok(done) = self.rx.try_recv() {
            match done {
                Done::Image(s, l, img) => {
                    self.busy.remove(&s);
                    self.ahead.remove(&s);
                    // r42:tuning-face — a picture made ahead past the atlas's page stays on
                    // disk, unless a tile asked for it meanwhile.
                    let ahead_only = self.disk_only.remove(&s) && !self.seen.contains_key(&s);
                    if l == self.look && !ahead_only {
                        self.upload(ctx, s, &img);
                    }
                }
                Done::Failed(s, l) => {
                    self.busy.remove(&s);
                    self.ahead.remove(&s);
                    if l == self.look {
                        self.failed.insert(s);
                    }
                }
                // Made in another look than its file's: dropped; a tile that asks makes it
                // on demand, and a new look warms it again.
                Done::Stale(s) => {
                    self.busy.remove(&s);
                    self.disk_only.remove(&s);
                    self.ahead.remove(&s);
                    self.stats.stale += 1;
                }
            }
        }
        // The capture going on: a frame of it a frame while its scene's picture comes.
        if let Some(c) = self.capturing.as_mut() {
            if let Some(got) = c.step() {
                let s = c.subject();
                self.capturing = None;
                self.captured(ctx, s, look, got);
            }
        }
        // One capture at a time: each costs a few frames of one editor.
        let mut captured = self.capturing.is_some();
        while let Some(s) = self.wanted.pop_front() {
            if let Some(path) = self.file(s, look).filter(|p| p.exists()) {
                self.busy.insert(s);
                self.stats.disk_loads += 1;
                let (tx, ctx2) = (self.tx.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let msg = match load_png(&path) {
                        Some(img) => Done::Image(s, look, img),
                        None => {
                            let _ = std::fs::remove_file(&path);
                            Done::Failed(s, look)
                        }
                    };
                    let _ = tx.send(msg);
                    ctx2.request_repaint();
                });
                continue;
            }
            if captured {
                self.wanted.push_front(s);
                break;
            }
            captured = true;
            self.stats.captures += 1;
            // The first picture written in a look: the folder keeps that look and the few
            // last used before it (switching back is instant), and forgets the rest — so
            // it never grows by a set per launch or per sheet change.
            if self.pruned != look {
                self.pruned = look;
                if let Some(dir) = self.disk.clone() {
                    std::thread::spawn(move || prune(&dir, look, KEEP_LOOKS));
                }
            }
            self.busy.insert(s);
            let mut c = picker_snap::Capture::start(ctx, s, PPP);
            match c.step() {
                Some(got) => self.captured(ctx, s, look, got),
                None => self.capturing = Some(c),
            }
        }
        // The pictures made ahead are the runner's (`warm_start`): the window is not kept
        // repainting while it works (r42 fix-ui: round 38's rest, `idle_repaint_e2e`).
        let only_ahead = self.wanted.is_empty()
            && self.capturing.is_none()
            && self.busy.iter().all(|b| self.ahead.contains(b));
        if !self.idle() && !only_ahead {
            crate::views::workspace_ticker::moving(
                ctx,
                crate::views::workspace_ticker::Reason::Background,
            );
        }
    }

    /// A capture done: its picture filled, shrunk and written on a worker.
    fn captured(&mut self, ctx: &Context, s: Subject, look: u64, got: Option<picker_snap::Frame>) {
        let Some(frame) = got else {
            self.busy.remove(&s);
            self.failed.insert(s);
            return;
        };
        let (tx, ctx2) = (self.tx.clone(), ctx.clone());
        let path = self.file(s, look);
        std::thread::spawn(move || {
            let img = picker_snap::finish(frame, W, RADIUS);
            if let Some(p) = path {
                save_png(&img, &p);
            }
            let _ = tx.send(Done::Image(s, look, img));
            ctx2.request_repaint();
        });
    }

    fn upload(&mut self, ctx: &Context, s: Subject, img: &ColorImage) {
        let (page, slot) = self.place(s);
        let img = fit(img);
        while self.pages.len() <= page {
            let blank = ColorImage::filled([COLS * W, ROWS * H], egui::Color32::TRANSPARENT);
            let name = format!("picker-thumbs-{}", self.pages.len());
            self.pages
                .push(ctx.load_texture(name, blank, TextureOptions::LINEAR));
        }
        self.pages[page].set_partial(
            [(slot % COLS) * W, (slot / COLS) * H],
            img,
            TextureOptions::LINEAR,
        );
        self.slots.insert(s, (page, slot));
        self.stats.uploads += 1;
    }

    /// Where `s`'s picture goes: its own place if it has one, else a free slot, else the
    /// slot of the picture asked for longest ago — never one asked for in this frame or the
    /// last (it is on screen); with every slot on screen, a slot of a new page.
    fn place(&mut self, s: Subject) -> (usize, usize) {
        if let Some(&at) = self.slots.get(&s) {
            return at;
        }
        let used: HashSet<(usize, usize)> = self.slots.values().copied().collect();
        for page in 0..self.pages.len().max(1) {
            if let Some(i) = (0..PER_PAGE).find(|i| !used.contains(&(page, *i))) {
                return (page, i);
            }
        }
        let recent = self.frame.saturating_sub(1);
        let oldest = self
            .slots
            .keys()
            .copied()
            .filter(|k| self.seen.get(k).copied().unwrap_or(0) < recent)
            .min_by_key(|k| (self.seen.get(k).copied().unwrap_or(0), format!("{k:?}")));
        match oldest {
            Some(k) => self.slots.remove(&k).expect("a placed picture"),
            None => (self.pages.len(), 0),
        }
    }

    /// The pictures held now, and the atlas pages (for tests and the frame budget).
    pub fn held(&self) -> (usize, usize) {
        (self.slots.len(), self.pages.len())
    }
}

/// A picture at exactly `W × H` (a file of another size is padded or cut).
fn fit(img: &ColorImage) -> ColorImage {
    if img.size == [W, H] {
        return img.clone();
    }
    let mut out = ColorImage::filled([W, H], egui::Color32::TRANSPARENT);
    for y in 0..H.min(img.size[1]) {
        for x in 0..W.min(img.size[0]) {
            out.pixels[y * W + x] = img.pixels[y * img.size[0] + x];
        }
    }
    out
}

/// The UV rect of atlas slot `i`.
fn uv(i: usize) -> Rect {
    let (aw, ah) = ((COLS * W) as f32, (ROWS * H) as f32); // one page
    let (x, y) = (((i % COLS) * W) as f32, ((i / COLS) * H) as f32);
    Rect::from_min_size(
        egui::pos2(x / aw, y / ah),
        egui::vec2(W as f32 / aw, H as f32 / ah),
    )
}

/// How many looks besides the current one the folder keeps pictures of.
pub const KEEP_LOOKS: usize = 3;

/// Remove from `dir` the pictures of every look but `keep` and the `others` most recently
/// written other looks. A picture's look is the 16 hex digits before `-{W}.png`; other
/// files are left alone.
pub fn prune(dir: &std::path::Path, keep: u64, others: usize) {
    let suffix = format!("-{W}.png");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(u64, std::time::SystemTime, PathBuf)> = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(&suffix) else {
            continue;
        };
        let Some(hex) = stem.get(stem.len().saturating_sub(16)..) else {
            continue;
        };
        let Ok(look) = u64::from_str_radix(hex, 16) else {
            continue;
        };
        let when = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        files.push((look, when, e.path()));
    }
    // The other looks, newest first by their newest picture.
    let mut looks: Vec<(u64, std::time::SystemTime)> = Vec::new();
    for (l, w, _) in &files {
        if *l == keep {
            continue;
        }
        match looks.iter_mut().find(|(x, _)| x == l) {
            Some(e) => e.1 = e.1.max(*w),
            None => looks.push((*l, *w)),
        }
    }
    looks.sort_by_key(|l| std::cmp::Reverse(l.1));
    let kept: Vec<u64> = looks.iter().take(others).map(|(l, _)| *l).collect();
    for (l, _, p) in files {
        if l != keep && !kept.contains(&l) {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn save_png(img: &ColorImage, path: &std::path::Path) {
    let [w, h] = img.size;
    let Some(mut pm) = tiny_skia::Pixmap::new(w as u32, h as u32) else {
        return;
    };
    let data = pm.data_mut();
    for (i, c) in img.pixels.iter().enumerate() {
        data[i * 4..i * 4 + 4].copy_from_slice(&c.to_array());
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("png.tmp");
    if pm.save_png(&tmp).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn load_png(path: &std::path::Path) -> Option<ColorImage> {
    let pm = tiny_skia::Pixmap::load_png(path).ok()?;
    // Both are premultiplied RGBA, in the same order.
    Some(ColorImage::from_rgba_premultiplied(
        [pm.width() as usize, pm.height() as usize],
        pm.data(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::skin::{self, Material, Skin};

    /// The integrator's review: nothing pruned the folder, and every look (a theme, a
    /// language, a sheet) left a full set of pictures behind. `prune` keeps the current look
    /// and the few last used, and leaves other files alone.
    #[test]
    fn the_folder_keeps_the_current_look_and_the_last_few() {
        let dir = std::env::temp_dir().join(format!("cantus-thumbs-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let t0 = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        for look in 1u64..=6 {
            for unit in ["effect-eq-", "effect-space-"] {
                let p = dir.join(format!("{unit}{look:016x}-{W}.png"));
                let f = std::fs::File::create(&p).expect("a file");
                f.set_modified(t0 + std::time::Duration::from_secs(look * 60))
                    .expect("a time");
            }
        }
        std::fs::write(dir.join("notes.txt"), b"keep me").expect("a file");
        prune(&dir, 2, 3);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .expect("the folder")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        let _ = std::fs::remove_dir_all(&dir);
        let suffix = format!("-{W}.png");
        let looks: std::collections::BTreeSet<u64> = left
            .iter()
            .filter_map(|n| n.strip_suffix(suffix.as_str()))
            .filter_map(|s| u64::from_str_radix(&s[s.len() - 16..], 16).ok())
            .collect();
        // The current look (2) and the three newest others (6, 5, 4).
        assert_eq!(
            looks.into_iter().collect::<Vec<_>>(),
            vec![2, 4, 5, 6],
            "{left:?}"
        );
        assert!(left.contains(&"notes.txt".to_owned()));
    }

    /// before the switch was turned or a sheet changed, must never be shown for a face.
    #[test]
    fn a_picture_s_look_changes_with_the_switch_and_with_every_sheet() {
        static WOOD: Skin = Skin {
            material: Material::Wood,
            ..Skin::plain("test.thumb.wood")
        };
        static GLASS: Skin = Skin {
            material: Material::Glass,
            ..Skin::plain("test.thumb.glass")
        };
        let on = look_key();
        skin::set_enabled(false);
        let off = look_key();
        skin::set_enabled(true);
        assert_ne!(on, off, "the switch changes the pictures");
        skin::set_override(Some(&WOOD));
        let wood = look_key();
        skin::set_override(Some(&GLASS));
        let glass = look_key();
        skin::set_override(None);
        assert_ne!(wood, glass, "a sheet that changes changes the pictures");
        assert_ne!(wood, on);
        assert_eq!(look_key(), on, "and the same faces find the same pictures");
    }

    /// The store held 28 pictures and dropped every later one — and asked for it again every
    /// frame. Now the one asked for longest ago is let go; what is on screen is never let
    /// go, and more than a page on screen opens a second page.
    #[test]
    fn more_pictures_than_a_page_let_the_oldest_go_and_never_what_is_shown() {
        let ctx = egui::Context::default();
        let mut th = Thumbs {
            enabled: true,
            ..Default::default()
        };
        let subjects: Vec<Subject> = cantus_core::EffectKind::ALL
            .iter()
            .map(|k| Subject::Effect(*k))
            .collect();
        assert!(subjects.len() > PER_PAGE, "more effects than a page");
        let img = ColorImage::filled([W, H], egui::Color32::WHITE);
        // A gallery scrolled a few at a time: each frame shows 12 and takes their pictures.
        for start in (0..subjects.len() - 12).step_by(4) {
            th.frame += 1;
            for s in &subjects[start..start + 12] {
                if th.get(*s).is_none() {
                    th.upload(&ctx, *s, &img);
                }
            }
            for s in &subjects[start..start + 12] {
                assert!(th.get(*s).is_some(), "{s:?} is shown with its picture");
            }
            assert!(th.held().0 <= PER_PAGE);
        }
        assert_eq!(th.held().1, 1, "one page held every view");
        // Everything shown at once: a second page, and every picture in.
        th.frame += 1;
        for s in &subjects {
            if th.get(*s).is_none() {
                th.upload(&ctx, *s, &img);
            }
        }
        for s in &subjects {
            assert!(th.get(*s).is_some(), "{s:?}");
        }
        assert_eq!(
            th.held(),
            (subjects.len(), subjects.len().div_ceil(PER_PAGE))
        );
    }
}
