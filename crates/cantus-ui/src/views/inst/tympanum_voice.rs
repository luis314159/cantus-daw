//! A modelled kit's voice, under its row in the kits list (round 40, `drums-hyb`; ADR 0124's
//! third amendment): *Generated · Hybrid*, and beside it what the hybrid needs — *Get ·
//! size* while its pack is not on disk (choosing *Hybrid* starts the download too), the
//! progress with *Cancel*, then the pack's credit. Until the pack is in, a hybrid kit plays
//! generated (never silence). One command (`voice`), one undo step.

use super::iu::{self, words};
use super::kits::size_text;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph_browser::Glyph as Gb;
use crate::kit::text::Src;
use crate::kit::{with_brush, Fill, Style};
use crate::views::mixer::look::{Pal, W};
use cantus_content::{Phase, Status};
use cantus_core::inst::tympanum::voice::{hybrid_pack, set_voice, Voice};
use cantus_core::{NativeSpec, TrackId};
use cantus_engine::inst::tympanum_hybrid as hybrid;
use egui::{pos2, vec2, Align2, Rect};

/// The row's height (unscaled).
pub const ROW: f32 = 62.0;

/// The voice row for the modelled kit `kit` (it has a hybrid pack) in `r`.
pub fn row(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    kit: &str,
    s: &Style,
    r: Rect,
) {
    let Some(pack) = hybrid_pack(kit) else {
        return;
    };
    let p = Pal::of(s);
    let voice = Voice::of(spec);
    let (status, size, licence) = {
        let m = app.samples.content();
        let found = m.manifest().pack(pack).cloned();
        (
            m.status(pack),
            found
                .as_ref()
                .map_or(0, |p| p.download_size().max(p.installed_size)),
            found.map(|p| p.licence).unwrap_or_default(),
        )
    };
    let there = matches!(status, Status::Installed { .. } | Status::Update { .. });
    let busy = match &status {
        Status::Busy(ph) => Some(ph.clone()),
        _ => None,
    };
    // A pack just in: the engine reads it now and the song hears it.
    if there
        && voice == Voice::Hybrid
        && hybrid::residual(kit).is_none()
        && hybrid::warm(kit).is_some()
    {
        app.resync_engine();
    }
    // Generated · Hybrid, at the left, under the kit's name.
    let x0 = r.left() + s.s(14.0);
    let sw = (r.right() - s.s(8.0) - x0).max(s.s(40.0));
    let sr = Rect::from_min_size(pos2(x0, r.top() + s.s(2.0)), vec2(sw, s.s(24.0)));
    let labels = [
        Src::Key(Voice::Generated.label_key()),
        Src::Key(Voice::Hybrid.label_key()),
    ];
    let now = usize::from(voice == Voice::Hybrid);
    let (resp, chosen) = crate::kit::widgets::Segmented::new(now, &labels)
        .id(ui.id().with(("tymp-voice", track.0)))
        .name(&t("tympanum.voice"))
        .style(*s)
        .show_at(ui, sr);
    let _ = iu::hint(resp, || t("tympanum.voice.tip"));
    if let Some(i) = chosen {
        let want = Voice::ALL[i.min(1)];
        if let Some(cmd) = set_voice(app.project(), track, want) {
            app.exec(cmd);
        }
        // Hybrid wanted and its pack not here: fetch it (the kit plays generated meanwhile).
        if want == Voice::Hybrid && !there && busy.is_none() {
            app.samples.content().install(pack);
        }
    }
    // The line under it: Get · size, the progress, or the pack's licence.
    let ly = sr.bottom() + s.s(6.0);
    let rx = x0;
    let rw = sw;
    if let Some(ph) = busy {
        let cr = Rect::from_min_size(
            pos2(r.right() - s.s(8.0) - s.s(24.0), ly),
            vec2(s.s(24.0), s.s(24.0)),
        );
        let cresp = iu::button(
            ui,
            s,
            cr,
            ui.id().with(("tymp-voice-cancel", track.0)),
            "",
            iu::Lead::Glyph(Gb::Clear),
            false,
            true,
        );
        let cresp = iu::hint(cresp, || t("tympanum.kits.cancel"));
        if cresp.clicked() {
            app.samples.content().cancel(pack);
        }
        let f = crate::views::samples_content::fraction(ph.as_ref());
        let (done, total) = match ph {
            Some(Phase::Downloading { done, total }) => (done, total),
            _ => ((size as f64 * f64::from(f)) as u64, size),
        };
        let bw = (cr.left() - s.s(8.0) - rx).max(s.s(10.0));
        with_brush(ui.painter(), |b| {
            let bar = Rect::from_min_size(pos2(rx, ly + s.s(4.0)), vec2(bw, s.s(3.0)));
            b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
            let fill =
                Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), bar.height()));
            b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.accent));
            let pct = format!("{:.0}", f * 100.0);
            let line = tf(
                "tympanum.kits.progress",
                &[
                    ("pct", &pct),
                    ("done", &size_text(done)),
                    ("total", &size_text(total)),
                ],
            );
            let wr = Rect::from_min_size(pos2(rx, bar.bottom() + s.s(3.0)), vec2(bw, s.s(13.0)));
            words(
                b,
                s,
                &line,
                wr,
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        });
    } else if !there {
        let label = tf("tympanum.kits.get", &[("size", &size_text(size))]);
        let cw = (iu::width(ui, s, &label, 12.0, W::Medium) + s.s(36.0)).min(rw);
        let cr = Rect::from_min_size(pos2(rx, ly), vec2(cw, s.s(24.0)));
        let cresp = iu::button(
            ui,
            s,
            cr,
            ui.id().with(("tymp-voice-get", track.0)),
            &label,
            iu::Lead::Glyph(Gb::Download),
            false,
            true,
        );
        let cresp = iu::hint(cresp, || t("tympanum.voice.tip_get"));
        if cresp.clicked() {
            app.samples.content().install(pack);
        }
    } else {
        // On disk: what it is, and its licence.
        let line = tf("tympanum.voice.on_disk", &[("licence", &licence)]);
        with_brush(ui.painter(), |b| {
            let wr = Rect::from_min_size(pos2(rx, ly), vec2(rw, s.s(24.0)));
            words(
                b,
                s,
                &line,
                wr,
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        });
    }
}
