//! The preset browser's rows and its player (round 37, ADR 0101); the window is
//! `views::preset_browser`.

use super::preset_browser::{across, heard_on, what_for, Ask};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role as TextRole, Src, Text};
use crate::kit::widgets::LedButton;
use crate::kit::{with_brush, Brush, Fill, State, Style};
use egui::{pos2, vec2, Align2, Color32, Id, Rect, Sense, Ui, Vec2};

const ROW: f32 = 44.0;
const WAVE_W: f32 = 96.0;

/// The rows, culled to the view and scrolled; a click on ▶ hears, a click chooses, a
/// double-click loads.
pub(super) fn rows(app: &mut CantusApp, ui: &mut Ui, view: Rect, s: &Style) -> Ask {
    let k = s.k;
    let row = ROW * k;
    let shown: Vec<usize> = app.presets.query.run(&app.presets.entries);
    let content = shown.len() as f32 * row + 8.0 * k;
    let b = &mut app.presets;
    if b.reveal {
        b.reveal = false;
        if let Some(i) = b
            .selected
            .as_ref()
            .and_then(|id| shown.iter().position(|&j| &b.entries[j].id == id))
        {
            let top = i as f32 * row;
            if top < b.scroll {
                b.scroll = top;
            } else if top + row > b.scroll + view.height() {
                b.scroll = top + row - view.height();
            }
        }
    }
    if ui.rect_contains_pointer(view) {
        b.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    b.scroll = b.scroll.clamp(0.0, (content - view.height()).max(0.0));
    let off = view.top() + 4.0 * k - b.scroll;
    let painter = ui.painter().with_clip_rect(view);
    let mut ask = Ask::None;
    b.rows.clear();
    if shown.is_empty() {
        with_brush(&painter, |br| {
            let _ = text::paint(
                br,
                view.shrink(16.0 * k),
                &Text::key("preset.browser.none", TextRole::Body, s.t.text_dim)
                    .align(Align2::CENTER_CENTER),
                s.tk(),
            );
        });
    }
    for (n, &i) in shown.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(view.left() + 6.0 * k, off + n as f32 * row),
            vec2(view.width() - 12.0 * k, row - 2.0 * k),
        );
        if !r.intersects(view) {
            continue;
        }
        let e = &b.entries[i];
        let id = e.id.clone();
        let play = Rect::from_center_size(
            pos2(r.left() + 18.0 * k, r.center().y),
            Vec2::splat(26.0 * k),
        );
        let resp = ui.interact(
            r.intersect(view),
            Id::new(("preset-row", &id)),
            Sense::click(),
        );
        resp.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                true,
                b.selected.as_deref() == Some(id.as_str()),
                e.name.clone(),
            )
        });
        let pr = ui.interact(
            play.intersect(view),
            Id::new(("preset-play", &id)),
            Sense::click(),
        );
        let hear_words = if b.audition.is_on(&id) {
            t("preset.browser.stop")
        } else {
            t("preset.browser.play")
        };
        pr.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                e.unit.audible(),
                format!("{hear_words}: {}", e.name),
            )
        });
        if pr.clicked() {
            ask = Ask::Hear(id.clone());
        } else if resp.double_clicked() {
            ask = Ask::Load(id.clone());
        } else if resp.clicked() {
            b.selected = Some(id.clone());
        }
        let sel = b.selected.as_deref() == Some(id.as_str());
        let on = b.audition.is_on(&id);
        let applied = b.applied.as_deref() == Some(id.as_str());
        let peaks = b.audition.peaks.get(&id).cloned();
        let rendering = on && b.audition.rendering();
        with_brush(&painter, |br| {
            if sel || resp.hovered() {
                let base = if sel {
                    mix(s.t.ink, s.t.selection, 0.20)
                } else {
                    shade(s.t.ink, if s.t.dark { 0.05 } else { -0.04 })
                };
                br.rrect(r, s.r(5.0), Fill::Solid(base));
            }
            play_key(
                br,
                play,
                s,
                State::of(&pr, on),
                on,
                rendering,
                e.unit.audible(),
            );
            let tx = play.right() + 10.0 * k;
            let right = r.right() - WAVE_W * k - 14.0 * k;
            let name = Rect::from_min_max(
                pos2(tx, r.top() + 4.0 * k),
                pos2(right, r.center().y + 1.0 * k),
            );
            let _ = text::paint(
                br,
                name,
                &Text::new(&e.name, TextRole::Body, s.t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let what =
                Rect::from_min_max(pos2(tx, r.center().y), pos2(right, r.bottom() - 4.0 * k));
            let _ = text::paint(
                br,
                what,
                &Text::new(&what_for(e), TextRole::Label, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let wr = Rect::from_min_max(
                pos2(r.right() - WAVE_W * k - 6.0 * k, r.top() + 8.0 * k),
                pos2(r.right() - 6.0 * k, r.bottom() - 8.0 * k),
            );
            match &peaks {
                Some(p) => wave(
                    br,
                    wr,
                    p,
                    if on {
                        s.t.brand_bright
                    } else {
                        with_alpha(s.t.text_dim, 0.8)
                    },
                    None,
                ),
                None => br.hline(
                    wr.left(),
                    wr.right(),
                    br.snap_mid(wr.center().y),
                    br.px(),
                    with_alpha(s.t.text_dim, 0.35),
                ),
            }
            if applied {
                br.dot(
                    pos2(r.right() - WAVE_W * k - 18.0 * k, r.center().y),
                    3.0 * k,
                    s.t.brand,
                );
            }
        });
        b.rows.push((id, r));
    }
    ask
}

/// The ▶ key of a row: a round key with a play triangle, a stop square while heard, an
/// arc while it renders.
fn play_key(
    b: &mut Brush,
    r: Rect,
    s: &Style,
    st: State,
    on: bool,
    rendering: bool,
    audible: bool,
) {
    let face = if on {
        mix(s.t.panel, s.t.brand, 0.55)
    } else if st.hovered {
        shade(s.t.panel, if s.t.dark { 0.14 } else { -0.10 })
    } else {
        shade(s.t.panel, if s.t.dark { 0.06 } else { -0.05 })
    };
    b.circle(r.center(), r.width() * 0.5, Fill::Solid(face));
    b.circle_stroke(
        r.center(),
        r.width() * 0.5,
        b.px(),
        with_alpha(s.t.edge, 0.8),
    );
    let g = Rect::from_center_size(r.center(), r.size() * 0.5);
    let ink = if audible {
        s.t.text
    } else {
        with_alpha(s.t.text_dim, 0.4)
    };
    if rendering {
        b.arc(
            r.center(),
            r.width() * 0.32,
            0.0,
            4.2,
            1.6 * s.k,
            s.t.brand_bright,
            true,
        );
    } else {
        glyph::paint(b, g, if on { Glyph::Stop } else { Glyph::Play }, ink);
    }
}

/// A waveform outline (0…1 per column) mirrored about the middle of `r`, with the part
/// left of `played` (0…1) brighter.
fn wave(b: &mut Brush, r: Rect, peaks: &[f32], color: Color32, played: Option<f32>) {
    if peaks.is_empty() {
        return;
    }
    let n = peaks.len();
    let mid = r.center().y;
    let half = r.height() * 0.5;
    // On a decibel scale over [`FLOOR_DB`]: a preset at −30 dBFS still has a shape, and the
    // outlines of two presets compare as their levels do.
    let peaks: Vec<f32> = peaks.iter().map(|p| shown(*p)).collect();
    let x = |i: usize| r.left() + r.width() * i as f32 / (n - 1).max(1) as f32;
    let top: Vec<_> = peaks
        .iter()
        .enumerate()
        .map(|(i, p)| pos2(x(i), mid - p * half))
        .collect();
    let bottom: Vec<_> = peaks
        .iter()
        .enumerate()
        .map(|(i, p)| pos2(x(i), mid + p * half))
        .collect();
    b.between(&top, &bottom, with_alpha(color, 0.55));
    if let Some(f) = played {
        let cut = ((n as f32 * f.clamp(0.0, 1.0)) as usize).min(n);
        if cut > 1 {
            b.between(&top[..cut], &bottom[..cut], color);
        }
    }
}

/// The player at the foot: what was heard (or is chosen), on what, its large waveform with
/// the playhead, the count, and *Load*.
pub(super) fn player(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) -> Ask {
    let k = s.k;
    let pad = 12.0 * k;
    let b = &app.presets;
    let shown = b.query.run(&b.entries).len();
    let focus = b
        .audition
        .heard
        .clone()
        .or_else(|| b.selected.clone())
        .and_then(|id| b.entry(&id).cloned());
    let load_w = 88.0 * k;
    let line1 = Rect::from_min_size(
        pos2(r.left() + pad, r.top() + 6.0 * k),
        vec2(r.width() - 3.0 * pad - load_w, 18.0 * k),
    );
    let line2 = line1.translate(vec2(0.0, 18.0 * k));
    let wave_r = Rect::from_min_max(
        pos2(r.left() + pad, line2.bottom() + 4.0 * k),
        pos2(line1.right(), r.bottom() - 8.0 * k),
    );
    let count = tf(
        "preset.browser.count",
        &[("shown", &shown), ("all", &b.entries.len())],
    );
    let (name, on_what) = match &focus {
        Some(e) => (e.name.clone(), heard_on(app, e)),
        None => (count.clone(), t("preset.browser.help")),
    };
    let playing = b.audition.until.map(|u| {
        let left = u
            .saturating_duration_since(std::time::Instant::now())
            .as_secs_f32();
        let secs = b
            .audition
            .last
            .as_ref()
            .map(|l| l.len_frames() as f32 / l.rate.max(1) as f32)
            .unwrap_or(1.0);
        1.0 - left / secs.max(0.001)
    });
    let failed = b.audition.failed.clone();
    let wave_peaks = b.audition.wave.clone();
    let heard_is_focus = b.audition.heard.is_some();
    with_brush(ui.painter(), |br| {
        br.rect(
            r,
            Fill::Solid(shade(s.t.panel, if s.t.dark { -0.03 } else { 0.02 })),
        );
        let _ = text::paint(
            br,
            line1,
            &Text::new(&name, TextRole::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let sub = match &failed {
            Some(why) => tf("preset.browser.failed", &[("why", why)]),
            None if focus.is_some() => format!("{on_what}  ·  {count}"),
            None => on_what.clone(),
        };
        let _ = text::paint(
            br,
            line2,
            &Text::new(&sub, TextRole::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        br.rrect(wave_r, s.r(4.0), Fill::Solid(s.t.meter_well));
        if heard_is_focus && !wave_peaks.is_empty() {
            wave(
                br,
                wave_r.shrink(3.0 * k),
                &wave_peaks,
                s.t.brand_bright,
                playing,
            );
            if let Some(f) = playing {
                let x = across(wave_r, f);
                br.vline(
                    br.snap_mid(x),
                    wave_r.top(),
                    wave_r.bottom(),
                    1.5 * k,
                    s.t.playhead,
                );
            }
        } else {
            br.hline(
                wave_r.left() + 4.0 * k,
                wave_r.right() - 4.0 * k,
                br.snap_mid(wave_r.center().y),
                br.px(),
                with_alpha(s.t.text_dim, 0.4),
            );
        }
    });
    let load = Rect::from_min_size(
        pos2(r.right() - pad - load_w, r.top() + 10.0 * k),
        vec2(load_w, 24.0 * k),
    );
    app.presets.load = Some(load);
    // A frozen target refuses a load (ADR 0064): the key's place says so instead.
    if crate::preset_browser::load_refused(app) {
        app.presets.load = None;
        with_brush(ui.painter(), |br| {
            let _ = text::paint(
                br,
                load.expand2(vec2(40.0 * k, 0.0))
                    .translate(vec2(-40.0 * k, 0.0)),
                &Text::key("preset.browser.frozen", TextRole::Label, s.t.text_dim)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
        return Ask::None;
    }
    let can = app.presets.selected.is_some();
    let resp = LedButton::new(false, Src::Key("preset.browser.load"))
        .momentary(true)
        .enabled(can)
        .id(Id::new("preset-browser-load"))
        .caps(crate::kit::hint::Caps::NONE)
        .show_at(ui, load);
    if playing.is_some() || app.presets.audition.rendering() {
        ui.ctx().request_repaint();
    }
    match (&app.presets.selected, resp.clicked()) {
        (Some(id), true) => Ask::Load(id.clone()),
        _ => Ask::None,
    }
}

/// How far down a waveform's outline reaches, dBFS (its bottom; 0 dBFS at the top).
const FLOOR_DB: f32 = 48.0;

/// A peak (0…1, linear) as the height an outline shows (0…1, decibels over `FLOOR_DB`).
fn shown(peak: f32) -> f32 {
    if peak <= 0.0 {
        return 0.0;
    }
    ((20.0 * peak.log10() + FLOOR_DB) / FLOOR_DB).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_outline_reads_in_decibels() {
        assert_eq!(super::shown(0.0), 0.0);
        assert!((super::shown(1.0) - 1.0).abs() < 1e-6);
        assert!((super::shown(0.5) - (1.0 - 6.0206 / 48.0)).abs() < 1e-3);
        assert_eq!(super::shown(1e-4), 0.0, "−80 dBFS is under the floor");
    }
}
