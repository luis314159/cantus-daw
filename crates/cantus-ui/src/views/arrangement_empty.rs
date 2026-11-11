//! r39:el-arrange — the empty song (`des-arrange`, *The empty song*): a new project's canvas
//! says how to begin — three starters (an instrument track, a drum kit, an audio track; the
//! first ringed as the default), a drop zone, and a demo song to open.

use super::slate::{say, Ink, W};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::{self, Fill, Style};
use egui::{pos2, vec2, Align2, Id, Rect, Sense};

/// Where the starters were drawn last frame (tests aim with it): instrument, drums, audio,
/// and the demo link.
#[derive(Copy, Clone, Debug)]
pub struct Drawn {
    pub starters: [Rect; 3],
    pub demo: Rect,
}

impl Default for Drawn {
    fn default() -> Self {
        Drawn {
            starters: [Rect::NOTHING; 3],
            demo: Rect::NOTHING,
        }
    }
}

const KEYS: [(&str, &str); 3] = [
    ("arrange.empty.instrument", "arrange.empty.instrument_line"),
    ("arrange.empty.drums", "arrange.empty.drums_line"),
    ("arrange.empty.audio", "arrange.empty.audio_line"),
];

/// Paint and handle the empty song in the canvas `rect`.
pub(super) fn show(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect, s: &Style) {
    let ink = Ink::of(s);
    let k = s.k;
    let c = rect.center();
    // The drop zone takes what room is left (up to 132), and goes when too little is.
    let zone_h = (rect.height() - 246.0 * k).clamp(0.0, 132.0 * k);
    let zone_h = if zone_h < 40.0 * k { 0.0 } else { zone_h };
    let block_h = 156.0 * k + zone_h + 40.0 * k;
    let top = (c.y - block_h * 0.5).max(rect.top() + 16.0 * k);
    let well_w = 236.0 * k;
    let gap = 16.0 * k;
    let row_w = well_w * 3.0 + gap * 2.0;
    let left = c.x - row_w * 0.5;
    let wells: [Rect; 3] = std::array::from_fn(|i| {
        Rect::from_min_size(
            pos2(left + (well_w + gap) * i as f32, top + 70.0 * k),
            vec2(well_w, 64.0 * k),
        )
    });
    let zone = Rect::from_min_size(pos2(left, top + 156.0 * k), vec2(row_w, zone_h));
    let below = if zone_h > 0.0 {
        zone.bottom()
    } else {
        wells[0].bottom()
    };
    let demo = Rect::from_center_size(pos2(c.x, below + 24.0 * k), vec2(row_w, 18.0 * k));
    let resps: [egui::Response; 3] =
        std::array::from_fn(|i| ui.interact(wells[i], Id::new(("arr-starter", i)), Sense::click()));
    let demo_r = ui.interact(demo, Id::new("arr-demo-link"), Sense::click());
    let keys = [
        String::new(),
        String::new(),
        app.keymap.display_for(crate::keymap::Action::AddAudioTrack),
    ];
    kit::with_brush(&ui.painter().with_clip_rect(rect), |b| {
        let title = Rect::from_center_size(pos2(c.x, top + 12.0 * k), vec2(row_w, 24.0 * k));
        let _ = say(
            b,
            title,
            &t("arrange.empty.title"),
            W::Medium,
            17.0,
            ink.ink1,
            Align2::CENTER_CENTER,
            s,
        );
        let line = Rect::from_center_size(pos2(c.x, top + 38.0 * k), vec2(row_w, 18.0 * k));
        let _ = say(
            b,
            line,
            &t("arrange.empty.line"),
            W::Regular,
            12.0,
            ink.ink3,
            Align2::CENTER_CENTER,
            s,
        );
        for (i, w) in wells.iter().enumerate() {
            let hover = resps[i].hovered();
            b.rrect(*w, s.r(6.0), Fill::Solid(ink.edge));
            let fill = if hover { ink.well_hi } else { ink.well };
            b.rrect(w.shrink(b.px()), s.r(6.0), Fill::Solid(fill));
            if i == 0 {
                b.rrect_stroke(*w, s.r(6.0), 1.0 * k, ink.accent);
            }
            let tr = Rect::from_min_max(
                pos2(w.left() + 38.0 * k, w.top() + 14.0 * k),
                pos2(w.right() - 60.0 * k, w.top() + 32.0 * k),
            );
            let _ = say(
                b,
                tr,
                &t(KEYS[i].0),
                W::Medium,
                13.0,
                ink.ink1,
                Align2::LEFT_CENTER,
                s,
            );
            let lr = Rect::from_min_max(
                pos2(w.left() + 38.0 * k, w.top() + 34.0 * k),
                pos2(w.right() - 12.0 * k, w.top() + 50.0 * k),
            );
            let _ = say(
                b,
                lr,
                &t(KEYS[i].1),
                W::Regular,
                11.0,
                ink.ink3,
                Align2::LEFT_CENTER,
                s,
            );
            if !keys[i].is_empty() {
                let kw = super::slate::width(b.ctx(), &keys[i], W::Medium, 9.0, s) + 10.0 * k;
                let kr = Rect::from_min_size(
                    pos2(w.right() - 12.0 * k - kw, w.top() + 14.0 * k),
                    vec2(kw, 15.0 * k),
                );
                b.rrect_stroke(kr, s.r(3.0), b.px(), ink.ink4);
                let _ = say(
                    b,
                    kr,
                    &keys[i],
                    W::Medium,
                    9.0,
                    ink.ink3,
                    Align2::CENTER_CENTER,
                    s,
                );
            }
        }
        // The drop zone: dashed.
        let pts = [
            zone.left_top(),
            zone.right_top(),
            zone.right_bottom(),
            zone.left_bottom(),
            zone.left_top(),
        ];
        if zone.height() > 0.0 {
            b.dashed(&pts, b.px(), ink.ink4, 5.0 * k, 4.0 * k);
            let _ = say(
                b,
                zone,
                &t("arrange.empty.drop"),
                W::Regular,
                12.0,
                ink.ink3,
                Align2::CENTER_CENTER,
                s,
            );
        }
        let dc = if demo_r.hovered() {
            ink.ink1
        } else {
            ink.accent
        };
        let _ = say(
            b,
            demo,
            &t("arrange.empty.demo"),
            W::Medium,
            12.0,
            dc,
            Align2::CENTER_CENTER,
            s,
        );
    });
    // The starters' icons over the mesh.
    for (i, w) in wells.iter().enumerate() {
        let icon = [
            cantus_core::TrackIcon::Keys,
            cantus_core::TrackIcon::Drums,
            cantus_core::TrackIcon::Vocal,
        ][i];
        let r = Rect::from_center_size(
            pos2(w.left() + 22.0 * k, w.top() + 23.0 * k),
            vec2(12.0 * k, 12.0 * k),
        );
        crate::icons::paint_track_icon(ui.painter(), r, icon, ink.ink2);
    }
    for (i, r) in resps.iter().enumerate() {
        if r.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &t(KEYS[i].0),
                &t(KEYS[i].1),
                crate::kit::hint::Caps::NONE,
            );
        }
    }
    if demo_r.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("arrange.empty.demo"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    if resps[0].clicked() {
        app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
    }
    if resps[1].clicked() {
        app.add_track();
        if let Some(id) = app.selected_track {
            app.exec(cantus_core::Command::SetInstrument {
                track: id,
                spec: Box::new(cantus_core::InstrumentSpec::Pulsus(Default::default())),
            });
        }
    }
    if resps[2].clicked() {
        app.add_audio_track();
    }
    if demo_r.clicked() {
        crate::views::demos::show(app);
    }
    app.arrangement.empty = Drawn {
        starters: wells,
        demo,
    };
}
