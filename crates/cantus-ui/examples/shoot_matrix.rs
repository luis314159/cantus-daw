//! Screenshots of the Routing tab and the inspector's `POST | PRE` switch (round 25,
//! ADR 0041) without a screen:
//! `cargo run --release -p cantus-ui --example shoot_matrix -- <out-dir> [scale] [locale]`.
//!
//! Every picture in Slate and Paper:
//! - `matrix`: the welcome song with returns, a bus, a pre-fader send, a send switched
//!   off, a doubled send, a sidechain key and notes into a gate, *All* strips, the pointer
//!   on a cell a send would close a loop through (hatched; the hint bar says why);
//! - `matrix-inuse`: the same with *In use* on;
//! - `matrix-64`: sixty-four strips, many routes, *In use* by default, scrolled;
//! - `insp-tap`: the inspector of the strip with the pre-fader send — the live switch.
//!
//! `scale` is the interface's zoom (0.75 … 2.0); the hint bar is drawn under the matrix.
//! `CANTUS_SHOOT_KINDS=matrix,insp-tap` picks pictures; `CANTUS_SHOOT_SIZE=w,h` the size.

use cantus_core::{Command, EffectKind, Send, SendTap, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::kit::{self, Style};
use cantus_ui::raster::shoot_at;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

/// Returns, a bus and routes worth reading in the welcome song. Returns (the strip with
/// the pre-fader send, the loop cell's row and column strips).
fn scene(app: &mut CantusApp, extra: usize) -> (StripId, (StripId, StripId)) {
    let tracks: Vec<StripId> = app
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.strip)
        .collect();
    let add = |app: &mut CantusApp, name: &str| {
        let s = app.bus.new_strip(name);
        let id = s.id;
        let index = app.project().strips.len();
        app.exec(Command::InsertStrip {
            index,
            strip: Box::new(s),
        });
        id
    };
    let reverb = add(app, "Reverb");
    let delay = add(app, "Delay");
    let bus = add(app, "Drum bus");
    for i in 0..extra {
        add(app, &format!("Ch {}", i + 1));
    }
    let send =
        |app: &mut CantusApp, from: StripId, to: StripId, level: f32, tap: SendTap, on: bool| {
            app.set_strip_send(
                from,
                Send {
                    level,
                    tap,
                    enabled: on,
                    ..Send::new(to)
                },
            );
            app.end_gesture();
        };
    let (a, b) = (tracks[0], tracks[1 % tracks.len()]);
    send(app, a, reverb, 0.5, SendTap::Post, true);
    send(app, b, reverb, 0.7, SendTap::Pre, true);
    send(app, a, delay, 0.3, SendTap::Post, false);
    if let Some(&c) = tracks.get(2) {
        app.replace_output(&[c], bus);
        // Parallel compression: a send where the output already goes (doubled).
        send(app, c, bus, 0.4, SendTap::Post, true);
    }
    send(app, bus, reverb, 0.25, SendTap::Post, true);
    // A key: a compressor on the bus keyed from the first track's strip.
    app.add_effect(bus, EffectKind::Compressor);
    let comp = app.project().strip(bus).unwrap().effects[0].id;
    app.key_sidechain(a, bus, comp);
    // Notes into a gate on the delay.
    app.add_effect(delay, EffectKind::Gate);
    let gate = app.project().strip(delay).unwrap().effects[0].id;
    let track = app.project().tracks[0].id;
    app.exec(Command::SetEffectMidiSource {
        strip: delay,
        effect: gate,
        source: Some(track),
    });
    for i in 0..extra.min(40) {
        let first = app.project().strips.len() - extra;
        let s = app.project().strips[first + i].id;
        let to = [reverb, delay, bus][i % 3];
        send(app, s, to, 0.2 + (i % 5) as f32 * 0.15, SendTap::Post, true);
        if i % 4 == 0 {
            app.replace_output(&[s], bus);
        }
    }
    (b, (reverb, bus))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let zoom: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let locale = match args.next().as_deref() {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(locale);
    let code = locale.code();
    let kinds: Vec<String> = std::env::var("CANTUS_SHOOT_KINDS")
        .map(|k| k.split(',').map(str::to_string).collect())
        .unwrap_or_else(|_| {
            ["matrix", "matrix-inuse", "matrix-64", "insp-tap"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        });
    for kind in kinds.iter().map(String::as_str) {
        for (theme_name, theme) in [
            ("slate", cantus_ui::theme::slate()),
            ("paper", cantus_ui::theme::paper()),
        ] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&theme, &ctx);
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            let have = app.project().strips.len() + 3;
            let extra = if kind == "matrix-64" {
                64usize.saturating_sub(have)
            } else {
                0
            };
            let (pre, (row, col)) = scene(&mut app, extra);
            let path = dir.join(format!("{kind}-{theme_name}-{code}-z{zoom}.png"));
            if kind == "insp-tap" {
                app.select_strip(pre);
                let size = egui::vec2(290.0, 1320.0);
                shoot_at(
                    &ctx,
                    size,
                    zoom,
                    6,
                    Some(&path),
                    |_| None,
                    |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(root, |ui| views::inspector::show(&mut app, ui));
                    },
                );
                println!("wrote {}", path.display());
                continue;
            }
            match kind {
                "matrix" => app.routing_view.set_in_use(false),
                "matrix-inuse" => app.routing_view.set_in_use(true),
                _ => {}
            }
            // `CANTUS_SHOOT_SIZE=w,h` (points) frames a corner of it, for a close look.
            let size = std::env::var("CANTUS_SHOOT_SIZE")
                .ok()
                .and_then(|v| {
                    let (w, h) = v.split_once(',')?;
                    Some(egui::vec2(w.parse().ok()?, h.parse().ok()?))
                })
                .unwrap_or(egui::vec2(1100.0, 640.0));
            // Where the loop cell is: published by each frame, pointed at by the next (one
            // run of frames, so the fonts the first frame uploads are still there).
            let target = std::cell::Cell::new(None::<egui::Pos2>);
            shoot_at(
                &ctx,
                size,
                zoom,
                8,
                Some(&path),
                |_| target.get(),
                |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| {
                            let full = ui.max_rect();
                            let hint_h = 40.0;
                            let top = egui::Rect::from_min_max(
                                full.min,
                                egui::pos2(full.right(), full.bottom() - hint_h),
                            );
                            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(top));
                            views::routing::show(&mut app, &mut child);
                            let s = Style::current();
                            let hint = egui::Rect::from_min_max(
                                egui::pos2(full.left() + 6.0, top.bottom() + 4.0),
                                egui::pos2(full.right() - 6.0, full.bottom() - 4.0),
                            );
                            kit::with_brush(ui.painter(), |b| {
                                views::hint::paint_panel(b, hint, &s, "Cantus", None)
                            });
                            let v = &app.routing_view;
                            if let (Some(g), Some(r), Some(c)) = (
                                v.geometry(),
                                v.grid().row_of(views::routing::Row::Strip(row)),
                                v.grid().col_of(views::routing::Col::Strip(col)),
                            ) {
                                target.set(Some(g.cell_rect(r, c).center()));
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
    }
}
