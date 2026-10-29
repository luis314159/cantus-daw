//! The thumbnails' cache (ADR 0029 §3): real-look pictures of the native devices, made
//! once per look and kept — in one texture atlas in memory (so the whole gallery draws
//! its pictures as one mesh) and as PNG files on disk (so the next start shows them at
//! once).
//!
//! A picture is asked for with [`Thumbs::get`]; until it is ready the tile shows its drawn
//! card. [`Thumbs::pump`] runs once a frame while the picker shows: it uploads what the
//! workers finished, and starts **one** capture a frame (the part that must run on the UI
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
/// round 32 — pictures made plain before them are never read back).
pub const REV: u32 = 2;
/// A picture's size in pixels: 16:10, sharp on a large tile at 150 %.
pub const W: usize = 416;
pub const H: usize = 260;
/// Pixels per point the editor is drawn at before being shrunk (twice the final size).
const PPP: f32 = 0.8;
/// The corner radius baked into a picture, in pixels.
const RADIUS: f32 = 9.0;
/// The atlas: four pictures across, seven down.
const COLS: usize = 4;
const ROWS: usize = 7;

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
    atlas: Option<TextureHandle>,
    /// Where each picture of the current look sits in the atlas.
    slots: HashMap<Subject, usize>,
    look: u64,
    wanted: VecDeque<Subject>,
    busy: HashSet<Subject>,
    failed: HashSet<Subject>,
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
            atlas: None,
            slots: HashMap::new(),
            look: 0,
            wanted: VecDeque::new(),
            busy: HashSet::new(),
            failed: HashSet::new(),
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
        if let (Some(&slot), Some(atlas)) = (self.slots.get(&s), &self.atlas) {
            return Some((atlas.id(), uv(slot)));
        }
        if !self.busy.contains(&s) && !self.failed.contains(&s) && !self.wanted.contains(&s) {
            self.wanted.push_back(s);
        }
        None
    }

    /// Whether every picture asked for is in.
    pub fn idle(&self) -> bool {
        self.wanted.is_empty() && self.busy.is_empty()
    }

    /// Once a frame: take in what the workers finished, start what is wanted.
    pub fn pump(&mut self, ctx: &Context) {
        if !self.enabled {
            return;
        }
        let look = look_key();
        if look != self.look {
            // Another theme or language: other pictures. The atlas is reused.
            self.look = look;
            self.slots.clear();
            self.failed.clear();
            self.wanted.clear();
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
        // One capture a frame at most: each costs a few frames of one editor.
        let mut captured = false;
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
            match picker_snap::capture(ctx, s, PPP) {
                Some(frame) => {
                    self.busy.insert(s);
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
                None => {
                    self.failed.insert(s);
                }
            }
        }
        if !self.idle() {
            crate::views::workspace_ticker::moving(
                ctx,
                crate::views::workspace_ticker::Reason::Background,
            );
        }
    }

    fn upload(&mut self, ctx: &Context, s: Subject, img: &ColorImage) {
        let used: HashSet<usize> = self.slots.values().copied().collect();
        let Some(slot) = (0..COLS * ROWS).find(|i| !used.contains(i)) else {
            return;
        };
        let img = fit(img);
        let atlas = self.atlas.get_or_insert_with(|| {
            let blank = ColorImage::filled([COLS * W, ROWS * H], egui::Color32::TRANSPARENT);
            ctx.load_texture("picker-thumbs", blank, TextureOptions::LINEAR)
        });
        atlas.set_partial(
            [(slot % COLS) * W, (slot / COLS) * H],
            img,
            TextureOptions::LINEAR,
        );
        self.slots.insert(s, slot);
        self.stats.uploads += 1;
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
    let (aw, ah) = ((COLS * W) as f32, (ROWS * H) as f32);
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
}
