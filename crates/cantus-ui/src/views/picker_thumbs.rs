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
use std::sync::mpsc::{channel, Receiver, Sender};

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

/// What the cache has done, for tests and the frame budget.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Editors drawn off screen.
    pub captures: u32,
    /// Pictures read back from disk.
    pub disk_loads: u32,
    /// Pictures put into the atlas.
    pub uploads: u32,
}

enum Done {
    Image(Subject, u64, ColorImage),
    Failed(Subject, u64),
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
            stats: Stats::default(),
        }
    }
}

/// The look a picture is made in: every token of the installed theme, the language, and
/// the units' faces — the switch and every sheet (`skin::sheets_digest`; round 32), so a
/// picture made plain, or before a sheet changed, is never shown for a face.
pub fn look_key() -> u64 {
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
        crate::i18n::locale().code(),
        crate::kit::skin::enabled(),
        crate::kit::skin::sheets_digest(),
        REV,
    ))
}

impl Thumbs {
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

    /// Whether every picture asked for is in.
    pub fn idle(&self) -> bool {
        self.wanted.is_empty() && self.busy.is_empty() && self.capturing.is_none()
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
        }
        while let Ok(done) = self.rx.try_recv() {
            match done {
                Done::Image(s, l, img) => {
                    self.busy.remove(&s);
                    if l == self.look {
                        self.upload(ctx, s, &img);
                    }
                }
                Done::Failed(s, l) => {
                    self.busy.remove(&s);
                    if l == self.look {
                        self.failed.insert(s);
                    }
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
        if !self.idle() {
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
        let mut th = Thumbs::default();
        th.enabled = true;
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
