//! The picker's details (ADR 0029 §2): the chosen device large — its picture, its name and
//! what it is, a star, label | value rows (vendor, format, shelves; for a plug-in its
//! version, audio and editor), a description from the device's documentation, and its
//! factory presets. Bitwig's file info column, in the inspector's row grammar.

use super::picker::PickerView;
use super::picker_keys::Action;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cache, with_brush, Fill, Style, Tok};
use crate::picker::PickChoice;
use crate::picker_catalog::{presets, Source};
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

/// Draw the pane in `r`. Returns an action when a preset row was double-clicked (it
/// chooses the device).
pub fn show(v: &mut PickerView, ui: &mut Ui, r: Rect, s: &Style, _verb: &str) -> Option<Action> {
    let pad = s.s(16.0);
    let painter = ui.painter().with_clip_rect(r);
    with_brush(&painter, |b| {
        b.register(r, "picker-details", &[Tok::Panel], &[]);
        b.rect(r, Fill::Solid(s.t.panel));
    });
    let d = v.selection()?.clone();
    let inner = r.shrink(pad);
    // The picture.
    let pic = Rect::from_min_size(
        inner.min,
        vec2(inner.width(), inner.width() / super::picker_snap::ASPECT),
    );
    let thumb = super::picker_grid::subject(&d).and_then(|x| v.thumbs.get(x));
    with_brush(&painter, |b| match thumb {
        Some(_) => b.rrect(pic, s.r(8.0), Fill::Solid(s.t.well)),
        None => super::picker_cover::card(b, pic, s.r(8.0), s, &d),
    });
    if let Some((tex, uv)) = thumb {
        let mut m = egui::Mesh::with_texture(tex);
        m.add_rect_with_uv(pic, uv, egui::Color32::WHITE);
        painter.add(egui::Shape::mesh(m));
    }
    // Name, what it is, and the star.
    let star = Rect::from_min_size(
        pos2(inner.right() - s.s(28.0), pic.bottom() + s.s(12.0)),
        vec2(s.s(28.0), s.s(28.0)),
    );
    v.geo.star = star;
    let fav = v.prefs.is_favourite(&d.key);
    let sr = ui.interact(star, Id::new("picker-details-star"), Sense::click());
    let sr = sr.on_hover_text(t(if fav {
        "picker.favourite.remove"
    } else {
        "picker.favourite.add"
    }));
    if sr.clicked() {
        v.toggle_favourite(&d.key);
    }
    let fav = v.prefs.is_favourite(&d.key);
    let name_r = Rect::from_min_max(
        pos2(inner.left(), pic.bottom() + s.s(10.0)),
        pos2(star.left() - s.s(6.0), pic.bottom() + s.s(36.0)),
    );
    let what_r = name_r.translate(vec2(0.0, s.s(24.0)));
    // The description, wrapped to the pane.
    let about = d.slug.map(|slug| t(&format!("picker.dev.{slug}.about")));
    let wrap = inner.width();
    let about_g = about.as_ref().map(|a| {
        let key = cache::key((
            "picker-about",
            a,
            wrap.to_bits(),
            s.tk().to_bits(),
            cache::ckey(s.t.text_dim),
        ));
        cache::with(ui.ctx(), |c| {
            c.galley(ui.ctx(), key, || {
                let mut j = LayoutJob::default();
                j.wrap.max_width = wrap;
                j.append(
                    a,
                    0.0,
                    TextFormat {
                        font_id: Role::Body.font(s.tk()),
                        color: s.t.text_dim,
                        line_height: Some(s.s(17.0)),
                        ..Default::default()
                    },
                );
                j
            })
        })
    });
    let mut y = what_r.bottom() + s.s(12.0);
    let rows = facts(&d);
    let label_w = s.s(84.0);
    let list = presets(&d.choice);
    with_brush(&painter, |b| {
        let nt = Text::new(&d.name, Role::Title, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, name_r, &nt, s.tk() * 1.55);
        let wt = Text::new(&d.what, Role::Body, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, what_r, &wt, s.tk());
        let hov = sr.hovered();
        b.circle(
            star.center(),
            star.width() * 0.5,
            Fill::Solid(if hov {
                shade(s.t.panel, 0.06)
            } else {
                shade(s.t.panel, 0.02)
            }),
        );
        let (g, c) = if fav {
            (Glyph::StarFilled, s.t.brand_bright)
        } else {
            (Glyph::Star, if hov { s.t.text } else { s.t.text_dim })
        };
        glyph::paint(b, star.shrink(s.s(6.0)), g, c);
        // Label | value rows.
        for (label, value) in &rows {
            let row = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), s.s(22.0)));
            let lr = Rect::from_min_size(row.min, vec2(label_w, row.height()));
            let _ = text::paint(
                b,
                lr,
                &Text::new(label, Role::Label, s.t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let vr = Rect::from_min_max(pos2(lr.right() + s.s(6.0), row.top()), row.max);
            let _ = text::paint(
                b,
                vr,
                &Text::new(value, Role::Body, s.t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
            y += s.s(22.0);
        }
        y += s.s(10.0);
        if let Some(g) = &about_g {
            b.text(pos2(inner.left(), y), g.clone(), s.t.text_dim, 0.0);
            y += g.rect.height() + s.s(14.0);
        }
        // The presets, as many as fit.
        if !list.is_empty() && y + s.s(48.0) < inner.bottom() {
            let band = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), s.s(20.0)));
            let title = format!("{} · {}", t("picker.details.presets"), list.len());
            let _ = text::paint(
                b,
                band,
                &Text::new(&title, Role::Section, s.t.label).align(Align2::LEFT_CENTER),
                s.tk(),
            );
            b.hline(
                inner.left(),
                inner.right(),
                b.snap_mid(band.bottom()),
                b.px(),
                with_alpha(s.t.edge, 0.4),
            );
            y = band.bottom() + s.s(6.0);
            let row_h = s.s(21.0);
            let room = ((inner.bottom() - y) / row_h).floor().max(0.0) as usize;
            let show = if list.len() > room {
                room.saturating_sub(1)
            } else {
                list.len()
            };
            for (i, p) in list.iter().take(show).enumerate() {
                let row = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), row_h));
                if i % 2 == 0 {
                    b.rrect(row, s.r(3.0), Fill::Solid(mix(s.t.panel, s.t.strip, 0.45)));
                }
                let dot = pos2(row.left() + s.s(9.0), row.center().y);
                b.circle(dot, s.s(2.2), Fill::Solid(s.t.hue(d.hue())));
                let tr = Rect::from_min_max(
                    pos2(row.left() + s.s(18.0), row.top()),
                    pos2(row.right() - s.s(6.0), row.bottom()),
                );
                let _ = text::paint(
                    b,
                    tr,
                    &Text::new(p, Role::Body, s.t.text)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
                y += row_h;
            }
            if show < list.len() {
                let more = tf("picker.details.more", &[("n", &(list.len() - show))]);
                let row = Rect::from_min_size(
                    pos2(inner.left() + s.s(18.0), y),
                    vec2(inner.width(), row_h),
                );
                let _ = text::paint(
                    b,
                    row,
                    &Text::new(&more, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER),
                    s.tk(),
                );
            }
        }
    });
    None
}

/// The label | value rows for `d`.
fn facts(d: &crate::picker_catalog::Device) -> Vec<(String, String)> {
    let mut rows = vec![
        (t("picker.details.vendor"), d.vendor.clone()),
        (
            t("picker.details.format"),
            match d.source {
                Source::Native => t("picker.format.native"),
                s => t(s.key()),
            },
        ),
        (
            t("picker.details.shelves"),
            d.categories
                .iter()
                .map(|c| t(c.key()))
                .collect::<Vec<_>>()
                .join(" · "),
        ),
    ];
    if let PickChoice::Sound(s) = &d.choice {
        let lang = crate::i18n::locale().code();
        rows.push((t("samples.sound.pack"), s.0.pack_name.get(lang).to_string()));
        let licence = cantus_content::Licence::from_spdx(&s.0.licence)
            .map_or(s.0.licence.clone(), |l| l.name().to_string());
        rows.push((t("samples.detail.licence"), licence));
    }
    if let PickChoice::Hosted(info) = &d.choice {
        if !info.version.is_empty() {
            rows.push((t("picker.details.version"), info.version.clone()));
        }
        let mut io = tf(
            "picker.details.io_value",
            &[("in", &info.main_inputs), ("out", &info.main_outputs)],
        );
        if info.sidechain {
            io.push_str(&format!(" · {}", t("picker.details.sidechain")));
        }
        rows.push((t("picker.details.io"), io));
        rows.push((
            t("picker.details.kind"),
            t(if info.editor {
                "picker.details.editor"
            } else {
                "picker.details.generic"
            }),
        ));
        rows.push((t("picker.details.path"), info.path.display().to_string()));
    }
    rows
}
