//! Sectio's card in Exemplar's Play view (round 37, ADR 0099): the loop drawn whole with its
//! slices on it, and its editor —
//!
//! - **the points**: drag one to move it (it lands on the nearest zero crossing), right-click
//!   one to remove it, double-click the waveform to add one — each one undo step, the zones
//!   rebuilt from the points;
//! - **slice again**: *Markers*, *Transients* (with its sensitivity) or *Grid* (with its
//!   steps a beat), previewed on the loop as they are turned and kept with *Apply* (one undo
//!   step) or dropped with *Cancel*;
//! - **the gestures**: *Dump to piano roll*, *Random*, *Stutter*, *Quantize*, one undo step
//!   each.
//!
//! One loop dropped on the card is sliced into this Exemplar (the drop is the card's alone).

use super::parts;
use super::wave;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band::CardLook;
use crate::kit::text::{Elide, Policy, Role, Src};
use crate::kit::{with_brush, Style};
use crate::sectio_ui::styles;
use crate::sectio_ui::Gesture;
use cantus_core::inst::sampler::Zone;
use cantus_core::sectio::{SliceMode, Slicing};
use cantus_core::{ClipId, NativeSpec, Ticks, TrackId};
use egui::{pos2, vec2, Align2, Id, Pos2, Rect, Sense};

/// The editor's state between frames (egui's memory, per track): a point being dragged, and
/// the slice-again preview.
#[derive(Clone, Debug, Default)]
struct Edit {
    drag: Option<(usize, u32)>,
    mode: usize,
    sens: f32,
    grid: usize,
    preview: Option<Slicing>,
    asked: Option<(usize, u32, usize)>,
}

const GRIDS: [u8; 4] = [1, 2, 4, 8];

fn mode_of(i: usize) -> SliceMode {
    [SliceMode::Markers, SliceMode::Transients, SliceMode::Grid][i.min(2)]
}

fn index_of(m: SliceMode) -> usize {
    match m {
        SliceMode::Markers | SliceMode::Manual => 0,
        SliceMode::Transients => 1,
        SliceMode::Grid => 2,
    }
}

/// The clip the gestures act on: the track's last clip that plays its slices.
fn slice_clip(app: &CantusApp, track: TrackId) -> Option<ClipId> {
    let tr = app.project().track(track)?;
    tr.clips
        .iter()
        .rev()
        .find(|c| {
            c.notes
                .iter()
                .any(|n| n.pitch >= cantus_core::sectio::FIRST_KEY)
        })
        .map(|c| c.id)
}

/// One audio file dropped from the system on `rect`, taken from the frame so no other
/// handler imports it too (review 2 #10).
fn take_drop(ui: &egui::Ui, rect: Rect) -> Option<std::path::PathBuf> {
    let (files, at) = ui.ctx().input(|i| {
        (
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>(),
            i.pointer.latest_pos(),
        )
    });
    if files.len() != 1 || !at.is_some_and(|p| rect.contains(p)) || !super::is_audio(&files[0]) {
        return None;
    }
    ui.ctx().input_mut(|i| i.raw.dropped_files.clear());
    files.into_iter().next()
}

/// The card: `rect` in the Play view of a sliced Exemplar.
pub fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(crate::kit::Function::Source);
    let k = s.k;
    let Some(sl) = Slicing::of(spec) else {
        return;
    };
    let id = Id::new(("sectio-edit", track.0));
    let mut ed: Edit = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_else(|| Edit {
        mode: index_of(sl.mode),
        sens: sl.sens,
        grid: GRIDS.iter().position(|g| *g == sl.grid).unwrap_or(2),
        ..Edit::default()
    });
    let shown = ed.preview.clone().unwrap_or_else(|| sl.clone());
    let info = match &ed.preview {
        Some(p) => tf("slicer.preview", &[("n", &p.points.len())]),
        None => tf(
            "slicer.info",
            &[
                ("n", &shown.points.len()),
                ("bpm", &format!("{:.1}", shown.bpm)),
                ("mode", &t(shown.mode.name_key())),
            ],
        ),
    };
    let row_h = 26.0 * k;
    let disp = rect.height() / k - crate::kit::band::TITLE - 1.0 - 2.0 * row_h / k - 12.0;
    let look = CardLook {
        title: Src::Key("slicer.title"),
        switchable: false,
        selector: None,
        menu: false,
        display: Some(disp.max(40.0)),
    };
    let cr = parts::card(ui, rect, s, &look, true);
    parts::words(
        ui,
        cr.title,
        s,
        Src::Str(&info),
        Role::Micro,
        s.t.text_dim,
        Align2::RIGHT_CENTER,
        Policy::Elide(Elide::End),
    );
    let Some(d) = cr.display else {
        return;
    };
    crate::samples::drop::publish(
        ui.ctx(),
        rect,
        crate::samples::drop::Target::Slicer { track },
    );
    if let Some(path) = take_drop(ui, rect) {
        app.slice_into_track(track, &path, SliceMode::Markers);
        ui.ctx().data_mut(|m| m.remove::<Edit>(id));
        return;
    }
    let sd = &crate::views::inst::face::on_display(s);
    let zone = Zone::of(spec, 0);
    let audio = zone.as_ref().and_then(|z| wave::audio_of(app, z));
    let len = sl.len.max(1) as f32;
    let x_of = |f: u32| d.left() + f as f32 / len * d.width();
    let f_of = |x: f32| (((x - d.left()) / d.width()).clamp(0.0, 1.0) * len) as u32;

    // The points: handles to drag and right-click, the waveform to double-click.
    let back = ui.interact(d, id.with("wave"), Sense::click());
    if back.double_clicked() && ed.preview.is_none() {
        if let Some(p) = back.interact_pointer_pos() {
            let f = snap(&audio, f_of(p.x), sl.rate);
            app.add_slice(track, f);
        }
    }
    let mut moved: Option<(usize, u32)> = None;
    let mut removed: Option<usize> = None;
    if ed.preview.is_none() {
        for (i, p) in sl.points.iter().enumerate().skip(1) {
            let x = x_of(*p);
            let h = Rect::from_min_max(pos2(x - 4.0 * k, d.top()), pos2(x + 4.0 * k, d.bottom()));
            let r = ui.interact(h, id.with(("point", i)), Sense::click_and_drag());
            if r.hovered() || r.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                crate::kit::hint::set_text(
                    ui.ctx(),
                    &t("slicer.point"),
                    &t("slicer.hint.point"),
                    crate::kit::hint::Caps::NONE,
                );
            }
            if r.dragged() {
                if let Some(pos) = r.interact_pointer_pos() {
                    ed.drag = Some((i, f_of(pos.x)));
                }
            }
            if r.drag_stopped() {
                if let Some((j, f)) = ed.drag.take() {
                    moved = Some((j, snap(&audio, f, sl.rate)));
                }
            }
            if r.secondary_clicked() {
                removed = Some(i);
            }
        }
    }
    let painter = ui.painter_at(d);
    with_brush(&painter, |b| {
        let mid = d.center().y;
        if let Some(a) = &audio {
            let frames = a.frames();
            let cols = d.width().max(1.0) as usize;
            for c in 0..cols {
                let f0 = c * frames / cols;
                let f1 = ((c + 1) * frames / cols).max(f0 + 1).min(frames);
                let (mut lo, mut hi) = (0.0f32, 0.0f32);
                for f in f0..f1 {
                    let v = a.data[f * a.channels];
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
                let x = d.left() + c as f32;
                let h = d.height() * 0.46;
                b.segment(
                    Pos2::new(x, mid - hi * h),
                    Pos2::new(x, mid - lo * h),
                    1.0,
                    sd.t.text_dim,
                    false,
                );
            }
        }
        for (i, p) in shown.points.iter().enumerate() {
            let f = match ed.drag {
                Some((j, f)) if j == i => f,
                _ => *p,
            };
            let x = x_of(f);
            b.segment(
                Pos2::new(x, d.top()),
                Pos2::new(x, d.bottom()),
                1.2 * k,
                sd.hue,
                false,
            );
        }
    });
    // The slices' numbers where they fit (dense slices keep their lines, not every label).
    let mut last_right = f32::MIN;
    for (i, p) in shown.points.iter().enumerate() {
        let x = x_of(*p) + 2.0 * k;
        let w = 16.0 * k;
        if x < last_right || x + w > d.right() {
            continue;
        }
        last_right = x + w;
        let label = format!("{}", i + 1);
        let r = Rect::from_min_size(pos2(x, d.top() + 1.0 * k), vec2(w, 12.0 * k));
        parts::words(
            ui,
            r,
            sd,
            Src::Str(&label),
            Role::Micro,
            sd.hue,
            Align2::LEFT_TOP,
            Policy::Elide(Elide::End),
        );
    }

    // Slice again: the mode, its number, the preview, Apply / Cancel.
    let gap = 6.0 * k;
    let row1 = Rect::from_min_size(
        pos2(rect.left() + 8.0 * k, rect.bottom() - 2.0 * row_h - 8.0 * k),
        vec2(rect.width() - 16.0 * k, row_h),
    );
    let unit = (row1.width() - 3.0 * gap) / 6.0;
    let cell = |at: f32, w: f32| {
        Rect::from_min_size(
            pos2(
                row1.left() + at * unit + (at.floor()) * gap / 2.0,
                row1.top(),
            ),
            vec2(w * unit, row_h),
        )
    };
    let labels = [
        Src::Key("slicer.mode.markers"),
        Src::Key("slicer.mode.transients"),
        Src::Key("slicer.mode.grid"),
    ];
    let mut changed = false;
    if let Some(m) = parts::seg(ui, cell(0.0, 2.6), s, ed.mode, &labels, &t("slicer.mode")) {
        changed |= m != ed.mode;
        ed.mode = m;
    }
    let num = cell(2.75, 1.2);
    if ed.mode == 2 {
        let grids = [
            Src::Str("1/4"),
            Src::Str("1/8"),
            Src::Str("1/16"),
            Src::Str("1/32"),
        ];
        if let Some(g) = parts::seg(ui, num, s, ed.grid, &grids, &t("slicer.grid")) {
            changed |= g != ed.grid;
            ed.grid = g;
        }
    } else if ed.mode == 1 {
        let r = parts::drag_num(
            ui,
            num,
            s,
            &mut ed.sens,
            0.0..=1.0,
            0.5,
            false,
            &t("slicer.sens"),
            |v, out| {
                use std::fmt::Write;
                let _ = write!(out, "{:.0} %", v * 100.0);
            },
        );
        changed |= r.drag_stopped() || r.double_clicked();
    }
    let asked = (ed.mode, (ed.sens * 1000.0) as u32, ed.grid);
    if changed && ed.asked != Some(asked) {
        ed.asked = Some(asked);
        let song = app.project().tempo_bpm;
        ed.preview = styles::preview(spec, mode_of(ed.mode), ed.sens, GRIDS[ed.grid], song);
    }
    if ed.preview.is_some() {
        if parts::press(
            ui,
            cell(4.05, 0.95),
            s,
            Src::Key("slicer.apply"),
            &t("slicer.apply"),
        )
        .clicked()
        {
            app.reslice(track, mode_of(ed.mode), ed.sens, GRIDS[ed.grid]);
            ed.preview = None;
        }
        if parts::press(
            ui,
            cell(5.05, 0.95),
            s,
            Src::Key("slicer.cancel"),
            &t("slicer.cancel"),
        )
        .clicked()
        {
            ed.preview = None;
            ed.asked = None;
        }
    }

    // The gestures.
    let row = Rect::from_min_size(
        pos2(rect.left() + 8.0 * k, rect.bottom() - row_h - 4.0 * k),
        vec2(rect.width() - 16.0 * k, row_h),
    );
    let names = [
        "slicer.dump",
        "slicer.random",
        "slicer.stutter",
        "slicer.quantize",
    ];
    // The dump is the first gesture and the longest name: the widest press.
    let weights = [1.7f32, 1.0, 1.0, 1.0];
    let unit = (row.width() - 3.0 * gap) / weights.iter().sum::<f32>();
    let mut x = row.left();
    for (i, key) in names.iter().enumerate() {
        let w = unit * weights[i];
        let r = Rect::from_min_size(pos2(x, row.top()), vec2(w, row.height()));
        x += w + gap;
        if !parts::press(ui, r, s, Src::Key(key), &t(key)).clicked() {
            continue;
        }
        let seed = app.bus.revision() ^ 0x5EC7;
        match i {
            0 => {
                let at = crate::views::bars::bar_start(app.project(), app.engine.position())
                    .max(Ticks::ZERO);
                app.dump_slices(track, at);
            }
            _ => {
                if let Some(clip) = slice_clip(app, track) {
                    let g = match i {
                        1 => Gesture::Random(seed),
                        2 => Gesture::Stutter {
                            amount: 0.35,
                            repeats: 4,
                            seed,
                        },
                        _ => Gesture::Quantize {
                            grid: Ticks::QUARTER.0 / 4,
                            swing: 0.0,
                        },
                    };
                    app.slice_gesture(track, clip, g);
                }
            }
        }
    }
    if let Some((i, f)) = moved {
        app.move_slice(track, i, f);
    }
    if let Some(i) = removed {
        app.remove_slice(track, i);
    }
    ui.ctx().data_mut(|m| m.insert_temp(id, ed));
}

/// `f` on the loop's nearest zero crossing (within 2 ms).
fn snap(audio: &Option<wave::Audio>, f: u32, rate: u32) -> u32 {
    match audio {
        Some(a) => styles::zero_crossing(&a.data, a.channels, f, (rate / 500).max(1)),
        None => f,
    }
}
