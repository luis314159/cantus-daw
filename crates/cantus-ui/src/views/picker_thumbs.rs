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

/// The recipe's version: bump it when what a picture shows changes.
pub const REV: u32 = 1;
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
            stats: Stats::default(),
        }
    }
}

/// The look a picture is made in: every token of the installed theme and the language.
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
            crate::views::workspace_ticker::moving(ctx, crate::views::workspace_ticker::Reason::Background);
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
