//! CANTUS PATCH (2026-11-17, Cantus's fx side round, ADR 0146 phase B): files dragged onto
//! the window reach egui.
//!
//! baseview passes the system's drag-and-drop to the window handler (`DragEntered`,
//! `DragMoved`, `DragLeft`, `DragDropped`), and on Windows the OLE drop target accepts the
//! drop only if the handler answers `EventStatus::AcceptDrop`. egui-baseview 0.7.2 answered
//! every one of them as handled without passing them on, so no file could ever be dropped
//! on an egui-baseview window. [`input`] turns them into what egui-winit gives egui: the
//! files hovering (`RawInput::hovered_files`), the files dropped (`RawInput::dropped_files`)
//! and the pointer where they are; `EguiWindow::on_event` calls it first and answers the
//! drop target with what it returns.

use baseview::dpi::{LogicalPosition, PhysicalPosition};
use baseview::{DropData, DropEffect, Event, EventStatus, MouseEvent};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A file dropped on the window, by its path (what egui-winit hands egui on the desktop).
#[derive(Debug)]
pub struct DroppedPath(pub PathBuf);

impl egui::DroppedFile for DroppedPath {
    fn path(&self) -> &Path {
        &self.0
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

/// A drag-and-drop event into `input` (`scale`: physical pixels per logical point, the
/// window's scale times egui's zoom). `None` for any other event; otherwise what to answer
/// the system with, and where the pointer is in logical points.
pub fn input(
    event: &Event,
    scale: f64,
    input: &mut egui::RawInput,
) -> Option<(EventStatus, Option<egui::Pos2>)> {
    let Event::Mouse(m) = event else {
        return None;
    };
    let at = |p: &PhysicalPosition<f64>| {
        let l: LogicalPosition<f32> = p.to_logical(scale);
        egui::pos2(l.x, l.y)
    };
    let files = |data: &DropData| match data {
        DropData::Files(f) if !f.is_empty() => Some(f.clone()),
        _ => None,
    };
    match m {
        MouseEvent::DragEntered { position, data, .. }
        | MouseEvent::DragMoved { position, data, .. } => {
            let p = at(position);
            input.events.push(egui::Event::PointerMoved(p));
            match files(data) {
                Some(f) => {
                    input.hovered_files = f
                        .into_iter()
                        .map(|path| egui::HoveredFile {
                            path: Some(path),
                            mime: String::new(),
                        })
                        .collect();
                    Some((EventStatus::AcceptDrop(DropEffect::Copy), Some(p)))
                }
                None => Some((EventStatus::Ignored, Some(p))),
            }
        }
        MouseEvent::DragLeft => {
            input.hovered_files.clear();
            Some((EventStatus::Captured, None))
        }
        MouseEvent::DragDropped { position, data, .. } => {
            let p = at(position);
            input.events.push(egui::Event::PointerMoved(p));
            input.hovered_files.clear();
            match files(data) {
                Some(f) => {
                    input.dropped_files.extend(
                        f.into_iter()
                            .map(|path| Arc::new(DroppedPath(path)) as egui::DroppedFileHandle),
                    );
                    Some((EventStatus::AcceptDrop(DropEffect::Copy), Some(p)))
                }
                None => Some((EventStatus::Ignored, Some(p))),
            }
        }
        _ => None,
    }
}
