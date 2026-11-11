//! r39:el-arrange — the track headers in Slate (`des-arrange/README.md`, *The track
//! header*; ADR 0111): a quiet body, the user's colour a 3-px stripe, the number, the name
//! and one line under it (the instrument, or the input with its meter, or "Frozen"); the
//! keys M S ● A as letters at rest, gathered into one well under the pointer, only the
//! engaged one lit; a slim meter at the right; a level on a 64-px row, the inserts on a
//! 96-px one. Folders' rows, the automation lanes' headers and the add-track row are drawn
//! here too. Everything is laid out by [`rects`], so the painter and the input agree.

use super::slate::{say, Ink, W};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::{Brush, Fill, Style, Tok};
use cantus_core::Track;
use egui::{pos2, vec2, Align2, Color32, Rect};

/// The header column's widths (logical px): narrow, normal (the default), wide; the
/// splitter drags it from 120 to 360.
pub const NARROW: f32 = 136.0;
pub const NORMAL: f32 = 200.0;
pub const WIDE: f32 = 280.0;
pub const MIN_W: f32 = 120.0;
pub const MAX_W: f32 = 360.0;
/// The row heights a menu offers (logical px at the default zoom): small, compact,
/// normal (the default), large, tall.
pub const HEIGHTS: [(&str, f32); 5] = [
    ("arrange.height.small", 24.0),
    ("arrange.height.compact", 32.0),
    ("arrange.height.normal", 44.0),
    ("arrange.height.large", 64.0),
    ("arrange.height.tall", 96.0),
];
/// The default row: what a track's height of 1 is.
pub const ROW: f32 = 44.0;
/// A folder's own row, open; closed it is a normal row with its summary line.
pub const FOLDER_OPEN: f32 = 30.0;
/// The add-track row under the last track.
pub const ADD_ROW: f32 = 28.0;

/// How wide the column is drawn.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Width {
    Narrow,
    Normal,
    Wide,
}

impl Width {
    pub fn of(w: f32, k: f32) -> Width {
        if w < 170.0 * k {
            Width::Narrow
        } else if w >= 260.0 * k {
            Width::Wide
        } else {
            Width::Normal
        }
    }
}

/// A header's keys, in their order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Mute,
    Solo,
    Arm,
    Auto,
}

pub const KEYS: [Key; 4] = [Key::Mute, Key::Solo, Key::Arm, Key::Auto];

/// Where a header's parts go.
#[derive(Copy, Clone, Debug)]
pub struct Rects {
    pub row: Rect,
    /// The number (or a folder's chevron), right-aligned at x 24.
    pub number: Rect,
    /// The user's own icon before the name (an empty slot when none: a click there picks).
    pub glyph: Rect,
    /// The name's line.
    pub name: Rect,
    /// The second line (instrument, input, frozen), when the row is tall enough.
    pub line2: Option<Rect>,
    /// The type icon at the start of the second line.
    pub kind: Rect,
    /// The keys, joined (`n` of them: 4, or 2 on a narrow header and a folder).
    pub keys: [Rect; 4],
    pub n: usize,
    /// The well the keys gather into under the pointer, and "more" left of it.
    pub well: Rect,
    pub more: Rect,
    /// The meter at the right edge.
    pub meter: Rect,
    /// The level slider and its value (a row of 60 px or more).
    pub level: Option<Rect>,
    /// The inserts in words (a row of 88 px or more).
    pub inserts: Option<Rect>,
}

/// Lay out a header in `row` (its depth in folders, how many keys it has).
pub fn rects(row: Rect, depth: usize, n: usize, glyph: bool, s: &Style) -> Rects {
    let k = s.k;
    let x0 = row.left();
    let indent = 12.0 * k * depth as f32;
    let h = row.height();
    let single = h < 40.0 * k;
    // Line 1's band: centred on a short row, from y+5 on a tall one.
    let l1 = if single {
        Rect::from_center_size(
            pos2(row.center().x, row.top() + h * 0.5),
            vec2(row.width(), (16.0 * k).min(h)),
        )
    } else {
        Rect::from_min_max(
            pos2(row.left(), row.top() + 5.0 * k),
            pos2(row.right(), row.top() + 21.0 * k),
        )
    };
    let meter = Rect::from_min_max(
        pos2(row.right() - 7.0 * k, row.top() + 5.0 * k),
        pos2(row.right() - 4.0 * k, row.bottom() - 6.0 * k),
    );
    let key_w = 18.0 * k;
    let keys_right = meter.left() - 4.0 * k;
    let mut keys = [Rect::NOTHING; 4];
    for (i, r) in keys.iter_mut().enumerate().take(n) {
        let right = keys_right - key_w * (n - 1 - i) as f32;
        *r = Rect::from_min_max(
            pos2(right - key_w, l1.center().y - 8.0 * k),
            pos2(right, l1.center().y + 8.0 * k),
        );
    }
    let well = Rect::from_min_max(keys[0].min, keys[n - 1].max);
    let more = Rect::from_center_size(
        pos2(well.left() - 9.0 * k, well.center().y),
        vec2(12.0 * k, 16.0 * k),
    );
    let number = Rect::from_min_max(
        pos2(x0 + 4.0 * k + indent, l1.top()),
        pos2(x0 + 24.0 * k + indent, l1.bottom()),
    );
    let name_x = x0 + 32.0 * k + indent;
    let glyph_r = Rect::from_min_size(
        pos2(name_x, l1.center().y - 6.0 * k),
        vec2(12.0 * k, 12.0 * k),
    );
    let name_left = if glyph {
        glyph_r.right() + 4.0 * k
    } else {
        name_x
    };
    let name = Rect::from_min_max(
        pos2(name_left, l1.top()),
        pos2(more.left() - 2.0 * k, l1.bottom()),
    );
    let line2 = (!single).then(|| {
        Rect::from_min_max(
            pos2(x0 + 50.0 * k + indent, row.top() + 24.0 * k),
            pos2(meter.left() - 6.0 * k, row.top() + 38.0 * k),
        )
    });
    let kind = Rect::from_center_size(
        pos2(x0 + 40.0 * k + indent, row.top() + 31.0 * k),
        vec2(9.0 * k, 9.0 * k),
    );
    let level = (h >= 60.0 * k).then(|| {
        Rect::from_min_max(
            pos2(x0 + 32.0 * k + indent, row.top() + 43.0 * k),
            pos2(meter.left() - 6.0 * k, row.top() + 57.0 * k),
        )
    });
    let inserts = (h >= 88.0 * k).then(|| {
        Rect::from_min_max(
            pos2(x0 + 32.0 * k + indent, row.top() + 67.0 * k),
            pos2(meter.left() - 6.0 * k, row.top() + 81.0 * k),
        )
    });
    Rects {
        row,
        number,
        glyph: glyph_r,
        name,
        line2,
        kind,
        keys,
        n,
        well,
        more,
        meter,
        level,
        inserts,
    }
}

/// The words a header shows besides the name, rebuilt only when the song or the language
/// changes (a steady frame allocates nothing).
#[derive(Debug, Default)]
pub struct Words {
    rev: u64,
    locale: Option<crate::i18n::Locale>,
    /// Per track (by index): line 2, the level, the inserts, what a frozen one froze.
    pub line2: Vec<String>,
    pub level: Vec<String>,
    pub inserts: Vec<String>,
    /// Per folder (by index): its summary line.
    pub folder: Vec<String>,
    /// "7 tracks", "64 bars · 2:03".
    pub count: String,
    pub foot: String,
    pub frozen: String,
    pub add: String,
    pub no_tracks: String,
    /// "Sections": the lane's header in the corner.
    pub sections_word: String,
    /// The drop hint below the last track.
    pub drop: String,
    /// Each section's length (by index): "8 bars".
    pub sections: Vec<String>,
}

impl Words {
    pub fn refresh(&mut self, app: &CantusApp) {
        let rev = app.bus.revision();
        let now = crate::i18n::locale();
        let p = app.project();
        if rev == self.rev && self.locale == Some(now) && self.line2.len() == p.tracks.len() {
            return;
        }
        self.rev = rev;
        self.locale = Some(now);
        self.frozen = t("arrange.frozen");
        self.add = t("arrange.add_track");
        self.no_tracks = t("arrange.no_tracks");
        self.sections_word = t("arrange.sections");
        self.drop = t("arrange.drop_hint");
        self.sections.clear();
        for sec in &p.sections {
            let bars = crate::views::bars::bar_position(p, sec.end())
                - crate::views::bars::bar_position(p, sec.start);
            let n = if (bars - bars.round()).abs() < 1e-3 {
                format!("{}", bars.round() as i64)
            } else {
                format!("{bars:.1}")
            };
            self.sections.push(tf("arrange.section.bars", &[("n", &n)]));
        }
        self.line2.clear();
        self.level.clear();
        self.inserts.clear();
        for tr in &p.tracks {
            self.line2.push(line2(app, tr));
            self.level.push(db(tr.gain_db));
            self.inserts.push(inserts(app, tr));
        }
        self.folder.clear();
        for f in &p.folders {
            let n = p.folder_members(f.id).count();
            let mut s = tf("arrange.folder.line", &[("n", &n)]);
            // Where its tracks go, when they all go to one strip that is not the master.
            let outs: Vec<_> = p.folder_members(f.id).map(|t| t.strip).collect();
            let dest = outs
                .first()
                .copied()
                .flatten()
                .and_then(|s0| p.strip(s0))
                .and_then(|s0| s0.output)
                .filter(|_| outs.windows(2).all(|w| w[0] == w[1]))
                .and_then(|o| p.strip(o));
            if let Some(d) = dest {
                s.push_str(" \u{2192} ");
                s.push_str(&strip_name(app, d.id));
            }
            self.folder.push(s);
        }
        self.count = tf("arrange.tracks_count", &[("n", &p.tracks.len())]);
        self.foot = foot(app);
    }
}

fn strip_name(app: &CantusApp, id: cantus_core::StripId) -> String {
    let p = app.project();
    match p.strip(id) {
        Some(s) if !s.name.is_empty() => s.name.clone(),
        _ => p
            .tracks
            .iter()
            .find(|t| t.strip == Some(id))
            .map(|t| t.name.clone())
            .unwrap_or_default(),
    }
}

/// "−3.5 dB".
fn db(v: f32) -> String {
    if v.abs() < 0.05 {
        "0.0 dB".into()
    } else if v < 0.0 {
        format!("\u{2212}{:.1} dB", -v)
    } else {
        format!("+{v:.1} dB")
    }
}

/// Whether a track is an audio track: it takes an input, or every clip it has is audio.
pub fn is_audio(tr: &Track) -> bool {
    tr.input.is_some() || (!tr.clips.is_empty() && tr.clips.iter().all(|c| c.audio.is_some()))
}

/// Line 2: "Calor" (the instrument), "Audio · Perc loop 124" (an audio track and its first
/// clip), the input of an armed track ("In 1 · Mic" is the input's own name).
fn line2(app: &CantusApp, tr: &Track) -> String {
    if tr.armed {
        if let Some(i) = &tr.input {
            let mut s = String::new();
            crate::record_session::input_label(i, &mut s);
            return s;
        }
    }
    if tr.frozen.is_some() {
        let ins = inserts(app, tr);
        return if ins.is_empty() {
            crate::swap::instrument_name(&tr.instrument)
        } else {
            ins
        };
    }
    if is_audio(tr) {
        let first = tr
            .clips
            .iter()
            .find(|c| c.audio.is_some() && !c.name.is_empty())
            .map(|c| c.name.as_str());
        return match first {
            Some(n) => format!("{} \u{b7} {n}", t("arrange.kind.audio")),
            None => t("arrange.kind.audio"),
        };
    }
    match &tr.instrument {
        cantus_core::InstrumentSpec::External(x) => x.name.clone(),
        spec => crate::swap::instrument_name(spec),
    }
}

/// The inserts on a track's strip, in words.
fn inserts(app: &CantusApp, tr: &Track) -> String {
    let p = app.project();
    let Some(strip) = tr.strip.and_then(|s| p.strip(s)) else {
        return String::new();
    };
    let names: Vec<String> = strip
        .effects
        .iter()
        .map(|e| match e.spec.external.as_deref() {
            Some(x) => x.name.clone(),
            None => t(e.spec.kind.name_key()),
        })
        .collect();
    names.join(", ")
}

/// "64 bars · 2:03": the song's length.
fn foot(app: &CantusApp) -> String {
    let p = app.project();
    let end = p.content_end();
    let bars = crate::views::bars::bar_position(p, end).ceil().max(0.0) as i64;
    let secs = crate::views::bars::map(p).seconds_at(end.0 as f64).max(0.0);
    let (m, sec) = ((secs / 60.0).floor() as i64, (secs % 60.0).floor() as i64);
    tf(
        "arrange.foot",
        &[("bars", &bars), ("time", &format!("{m}:{sec:02}"))],
    )
}

/// The register's tokens for the headers.
pub const TOKENS: &[Tok] = &[
    Tok::Toolbar,
    Tok::Panel,
    Tok::Text,
    Tok::TextDim,
    Tok::Placeholder,
    Tok::Highlight,
    Tok::Danger,
    Tok::MeterHigh,
];

/// A body: `surface` (a folder's child a step back), tinted when selected, lifted under the
/// pointer, a 1-px foot.
pub fn body(b: &mut Brush, row: Rect, ink: &Ink, hue: Option<Color32>, child: bool, hover: bool) {
    let mut c = if child {
        Ink::to(ink.surface, ink.base, 0.30)
    } else {
        ink.surface
    };
    if let Some(h) = hue {
        c = ink.head_ground(c, h);
    }
    if hover {
        c = Ink::to(c, ink.ink1, 0.035);
    }
    b.rect(b.snap_rect(row), Fill::Solid(c));
    b.hline(
        row.left(),
        row.right(),
        row.bottom() - 0.5 * b.px(),
        b.px(),
        Ink::to(ink.surface, ink.gutter, 0.75),
    );
}

/// The stripe at x 0, full height.
pub fn stripe(b: &mut Brush, row: Rect, c: Color32, k: f32) {
    let r = Rect::from_min_max(row.min, pos2(row.left() + 3.0 * k, row.bottom()));
    b.rect(b.snap_rect(r), Fill::Solid(c));
}

/// A text at a size and weight (the sheets' measures).
#[allow(clippy::too_many_arguments)]
pub fn words(
    b: &mut Brush,
    r: Rect,
    s_: &str,
    w: W,
    size: f32,
    c: Color32,
    align: Align2,
    s: &Style,
) -> Option<Rect> {
    say(b, r, s_, w, size, c, align, s).map(|p| p.rect)
}

#[path = "arrangement_head_paint.rs"]
mod paint;
pub use paint::*;
