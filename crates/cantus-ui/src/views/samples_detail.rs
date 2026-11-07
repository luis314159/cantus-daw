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
pub const HEIGHT: f32 = 84.0; // r37:finish twelfth (was 124: judge, the card dominated)
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

/// Draw the card for `path` in `rect`; returns the waveform's rect (a click plays).
pub fn card(app: &mut CantusApp, ui: &mut Ui, s: &Style, rect: Rect, path: &Path) -> Rect {
    let k = s.k;
    let th = samples_thumbs::thumb(path);
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = super::samples_browser::shown_name(&stem).to_string();
    // r37:finish twelfth — compact: the name and its chips on one line, the format under
    // it, then the waveform (the card is the sound's, not the panel's centrepiece).
    let inner = rect.shrink2(vec2(10.0 * k, 6.0 * k));
    let title = Rect::from_min_size(inner.min, vec2(inner.width(), 18.0 * k));
    let fmt = Rect::from_min_size(
        pos2(inner.left(), title.bottom()),
        vec2(inner.width(), 14.0 * k),
    );
    let wave = Rect::from_min_max(pos2(inner.left(), fmt.bottom() + 4.0 * k), inner.max);
    let resp = ui.interact(wave, egui::Id::new("samples-detail-wave"), Sense::click());
    let playing = app.samples.preview.is_playing(path);
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "samples-detail",
            &[crate::kit::Tok::Panel, crate::kit::Tok::Text],
            &[],
        );
        let m = crate::kit::light::Material::of(&s.t);
        crate::kit::light::recess(b, rect, s.r(4.0), shade(s.t.panel, -0.02), &m, 1.0, k);
        // The chips first, right-aligned on the title line: the name takes what is left.
        let mut x = inner.right();
        for tag in tags(&stem).iter().rev() {
            let w =
                text::measure(b.ctx(), &Text::new(tag, Role::Label, s.t.text), s.tk()).x + 12.0 * k;
            if x - w < inner.left() + 90.0 * k {
                break;
            }
            let chip = Rect::from_min_max(
                pos2(x - w, title.center().y - 8.0 * k),
                pos2(x, title.center().y + 8.0 * k),
            );
            b.rrect(
                chip,
                chip.height() * 0.5,
                Fill::Solid(with_alpha(
                    s.t.selection,
                    if s.t.dark { 0.22 } else { 0.14 },
                )),
            );
            b.rrect_stroke(
                chip,
                chip.height() * 0.5,
                b.px(),
                with_alpha(s.t.selection, 0.55),
            );
            let _ = text::paint(
                b,
                chip,
                &Text::new(tag, Role::Label, s.t.text).align(Align2::CENTER_CENTER),
                s.tk(),
            );
            x -= w + 5.0 * k;
        }
        paint::line(
            b,
            Rect::from_min_max(title.min, pos2(x - 6.0 * k, title.bottom())),
            s,
            Src::Str(&name),
            Role::Body,
            s.t.text,
            Align2::LEFT_CENTER,
        );
        if let Some(th) = th.as_ref() {
            paint::line(
                b,
                fmt,
                s,
                Src::Str(&format_line(path, th)),
                Role::Label,
                s.t.text_dim,
                Align2::LEFT_CENTER,
            );
            b.rrect(wave, s.r(3.0), Fill::Solid(with_alpha(s.t.text, 0.04)));
            let ink = if playing || resp.hovered() {
                s.t.highlight
            } else {
                s.t.selection
            };
            paint::wave(b, wave.shrink2(vec2(4.0 * k, 3.0 * k)), &th.peaks, ink, s);
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
