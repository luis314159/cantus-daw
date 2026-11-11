//! The sounds browser's detail card (round 37, finish; ADR 0103 §47): the chosen sound
//! under the list — its name, its format and length, a large waveform (a click plays it),
//! and its tags as chips: the key and tempo its name says, the families it is in. The
//! judge's "large empty lower half" is the user's sound, at a glance. // r37:finish

use super::samples_paint as paint;
use super::samples_thumbs::{self, Thumb, FAMILIES};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};
use std::path::Path;

/// The card's height at 100 %.
pub const HEIGHT: f32 = 118.0; // r39:lo-rest: name, place, a 38-high well, the keys (r37: 84)
/// The least body height that keeps a card (below it the list has the room).
pub const MIN_BODY: f32 = 330.0;

/// The chips of a sound: the key, the tempo, then each family it is in.
pub fn tags(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(k) = samples_thumbs::key_in(name) {
        out.push(k);
    }
    if let Some(b) = samples_thumbs::bpm_in(name) {
        out.push(format!("{b} BPM"));
    }
    for (i, key) in FAMILIES.iter().enumerate().skip(1) {
        if samples_thumbs::in_family(name, i) {
            out.push(t(key));
        }
    }
    out
}

/// What a sound is in numbers where its name and length are already shown (the preview
/// strip under the card, r39:lo-rest): "48 kHz · Stereo".
pub fn short_format(th: &Thumb) -> String {
    let khz = th.rate as f32 / 1000.0;
    let rate = if (khz - khz.round()).abs() < 0.05 {
        format!("{khz:.0} kHz")
    } else {
        format!("{khz:.1} kHz")
    };
    let ch = match th.channels {
        1 => t("samples.detail.mono"),
        2 => t("samples.detail.stereo"),
        n => format!("{n} ch"),
    };
    format!("{rate} \u{b7} {ch}")
}

/// The format line: "WAV · 48 kHz · Stereo · 2.6 s".
pub fn format_line(path: &Path, th: &Thumb) -> String {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    let khz = th.rate as f32 / 1000.0;
    let rate = if (khz - khz.round()).abs() < 0.05 {
        format!("{khz:.0} kHz")
    } else {
        format!("{khz:.1} kHz")
    };
    let ch = match th.channels {
        1 => t("samples.detail.mono"),
        2 => t("samples.detail.stereo"),
        n => format!("{n} ch"),
    };
    format!("{ext} \u{b7} {rate} \u{b7} {ch} \u{b7} {:.1} s", th.seconds)
}

/// Where a sound is: its pack or folder, then the folders down to it ("Cantus Starter
/// Drums › Kicks").
pub fn place(app: &mut CantusApp, path: &Path) -> String {
    let user = app.settings.sample_folders.clone();
    let roots = app.samples.roots(&user);
    let Some(r) = roots.iter().find(|r| path.starts_with(&r.path)) else {
        return path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
    };
    let mut out = r.label.clone();
    if let Some(rel) = path.parent().and_then(|d| d.strip_prefix(&r.path).ok()) {
        for part in rel.iter() {
            out.push_str(" \u{203a} ");
            out.push_str(&part.to_string_lossy());
        }
    }
    out
}

/// A waveform's columns scaled so its loudest reaches the top (at most 16 times up).
pub fn fitted(peaks: &[f32; samples_thumbs::COLUMNS]) -> [f32; samples_thumbs::COLUMNS] {
    let top = peaks.iter().copied().fold(0.0f32, f32::max);
    let gain = if top > 1e-6 {
        (1.0 / top).min(16.0)
    } else {
        1.0
    };
    peaks.map(|p| (p * gain).min(1.0))
}

/// The preview's parts in `rect` (des-views, *Preview*): the name and the star on the
/// first line, the place under it, the waveform's well, 38 high, then the keys' line.
pub fn card_rects(rect: Rect, k: f32) -> (Rect, Rect, Rect, Rect, Rect) {
    let inner = rect.shrink2(vec2(10.0 * k, 6.0 * k));
    let title = Rect::from_min_size(inner.min, vec2(inner.width(), 18.0 * k));
    let star = Rect::from_center_size(
        pos2(inner.right() - 8.0 * k, title.center().y),
        vec2(18.0, 18.0) * k,
    );
    let place = Rect::from_min_size(
        pos2(inner.left(), title.bottom()),
        vec2(inner.width(), 14.0 * k),
    );
    let well = Rect::from_min_size(
        pos2(inner.left(), place.bottom() + 6.0 * k),
        vec2(inner.width(), WELL * k),
    );
    let keys = Rect::from_min_max(pos2(inner.left(), well.bottom() + 6.0 * k), inner.max);
    (title, star, place, well, keys)
}

/// The waveform's well, at 100 %.
pub const WELL: f32 = 38.0;

/// Draw the card for `path` in `rect`; returns the waveform's rect (a click plays).
pub fn card(app: &mut CantusApp, ui: &mut Ui, s: &Style, rect: Rect, path: &Path) -> Rect {
    let k = s.k;
    let room = super::room::Room::of(&s.t);
    let th = samples_thumbs::thumb(path);
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = super::samples_browser::shown_name(&stem).to_string();
    let where_ = place(app, path);
    let (title, star, place_r, wave, _) = card_rects(rect, k);
    let resp = ui.interact(wave, egui::Id::new("samples-detail-wave"), Sense::click());
    let star_resp = ui.interact(star, egui::Id::new("samples-detail-star"), Sense::click());
    let starred = app.is_sample_favourite(path);
    let playing = app.samples.preview.is_playing(path);
    // How much of it has been heard, while it plays.
    let heard = app
        .samples
        .preview
        .playing
        .as_ref()
        .filter(|(p, at, len)| p == path && at.elapsed() < *len)
        .map(|(_, at, len)| (at.elapsed().as_secs_f32() / len.as_secs_f32().max(1e-3)).min(1.0));
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "samples-detail",
            &[crate::kit::Tok::Panel, crate::kit::Tok::Text],
            &[],
        );
        let m = crate::kit::light::Material::of(&s.t);
        crate::kit::light::recess(b, rect, s.r(4.0), shade(s.t.panel, -0.02), &m, 1.0, k);
        // The star: outlined in ink 3, filled in `solo` once starred.
        let (g, ink) = if starred {
            (crate::kit::glyph_browser::Glyph::StarFilled, room.solo)
        } else if star_resp.hovered() {
            (crate::kit::glyph_browser::Glyph::Star, room.ink1)
        } else {
            (crate::kit::glyph_browser::Glyph::Star, room.ink3)
        };
        crate::kit::glyph_browser::paint(b, star.shrink(1.0 * k), g, ink);
        paint::line(
            b,
            Rect::from_min_max(title.min, pos2(star.left() - 6.0 * k, title.bottom())),
            s,
            Src::Str(&name),
            Role::Title,
            room.ink1,
            Align2::LEFT_CENTER,
        );
        // The place, and the sound's tags as chips at the line's right where they fit.
        let mut x = place_r.right();
        for tag in tags(&stem).iter().rev() {
            let tx = Text::new(tag, Role::Label, room.ink2).caps(false);
            let w = text::measure(b.ctx(), &tx, s.tk() * 10.0 / 11.0).x + 10.0 * k;
            if x - w < place_r.left() + 120.0 * k {
                break;
            }
            let chip = Rect::from_min_max(
                pos2(x - w, place_r.center().y - 7.0 * k),
                pos2(x, place_r.center().y + 7.0 * k),
            );
            b.rrect(
                chip,
                chip.height() * 0.5,
                Fill::Solid(with_alpha(room.accent, 0.10)),
            );
            b.rrect_stroke(
                chip,
                chip.height() * 0.5,
                b.px(),
                with_alpha(room.accent, 0.35),
            );
            let _ = text::paint(
                b,
                chip,
                &tx.align(Align2::CENTER_CENTER),
                s.tk() * 10.0 / 11.0,
            );
            x -= w + 4.0 * k;
        }
        paint::line(
            b,
            Rect::from_min_max(place_r.min, pos2(x - 6.0 * k, place_r.bottom())),
            s,
            Src::Str(&where_),
            Role::Label,
            room.ink3,
            Align2::LEFT_CENTER,
        );
        // The well: `well` with its 1-px edge; the heard part in the accent, the rest
        // well → ink 2 at 80 %, a 1-px ink 1 line where it plays.
        let w = b.snap_rect(wave);
        b.rrect(w, s.r(3.0), Fill::Solid(room.edge));
        let fill = if resp.hovered() {
            room.well_hi
        } else {
            room.well
        };
        b.rrect(w.shrink(b.px()), s.r(3.0), Fill::Solid(fill));
        if let Some(th) = th.as_ref() {
            let body = w.shrink2(vec2(4.0 * k, 3.0 * k));
            let rest = crate::kit::color::mix(room.well, room.ink2, 0.80);
            // Drawn to the sound's own loudest column (a quiet sound reads, not a flat
            // line), at most 16 times up.
            let peaks = fitted(&th.peaks);
            let th = Thumb { peaks, ..**th };
            let n = th.peaks.len();
            match heard.filter(|_| n >= 4) {
                Some(f) => {
                    // The columns heard, and the rest from the same column on.
                    let i = ((f * (n - 1) as f32).round() as usize).clamp(1, n - 2);
                    let x = body.left() + body.width() * i as f32 / (n - 1) as f32;
                    let a = Rect::from_min_max(body.min, pos2(x, body.bottom()));
                    let z = Rect::from_min_max(pos2(x, body.top()), body.max);
                    paint::wave(b, a, &th.peaks[..=i], room.accent, s);
                    paint::wave(b, z, &th.peaks[i..], rest, s);
                    let at = b.snap(body.left() + body.width() * f);
                    b.vline(at, w.top() + b.px(), w.bottom() - b.px(), b.px(), room.ink1);
                }
                None => paint::wave(b, body, &th.peaks, room.accent, s),
            }
        }
    });
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("samples.preview.play"),
            &name,
            crate::kit::hint::Caps::NONE,
        );
    }
    if star_resp.hovered() {
        let tip = if starred {
            t("samples.unstar")
        } else {
            t("samples.star")
        };
        crate::kit::hint::set_text(ui.ctx(), &tip, &name, crate::kit::hint::Caps::NONE);
    }
    if star_resp.clicked() {
        app.toggle_sample_favourite(path);
    }
    if resp.clicked() {
        if playing {
            app.stop_audition();
        } else {
            app.audition_sample(path);
        }
    }
    wave
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sounds_tags_and_format_read_from_its_name_and_file() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        assert_eq!(tags("Drum Loop 124 BPM"), ["124 BPM", "Drums", "Loops"]);
        assert_eq!(tags("Pad Swell F#m"), ["F#m", "Melodic"]);
        let th = Thumb {
            peaks: [0.5; samples_thumbs::COLUMNS],
            seconds: 2.6,
            rate: 48_000,
            channels: 2,
        };
        assert_eq!(
            format_line(Path::new("a/Pad.wav"), &th),
            "WAV \u{b7} 48 kHz \u{b7} Stereo \u{b7} 2.6 s"
        );
    }
}
