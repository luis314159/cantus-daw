//! The plug-ins' own editors — the DAW's, running in the host's window (ADR 0020).
//!
//! The editor is the one Cantus draws (`cantus_ui::views::fx::gate::editor`), written
//! against [`FxHost`] instead of the app. Here the host is [`PluginHost`]: the plug-in's
//! values, its own telemetry and analyser tap (lock-free, from the audio thread), and the
//! host's gestures through an [`EditSink`] the format layer provides. The window is egui
//! 0.36 in a child of the host's window, through baseview and egui-baseview (OpenGL).
//!
//! Porta's editor is the first; the other six show the host's generic interface until
//! their editors move onto [`FxHost`] the same way (the plan is in ADR 0020).

pub use crate::instance::Edit;
use crate::instance::{Shared, PAIRS};
use cantus_core::{EffectKind, EffectSpec};
use cantus_engine::Telemetry;
use cantus_ui::views::fx::gate;
use cantus_ui::views::fx::host::{FxHost, KeySource, KeyTarget, Transport};
use egui_baseview::baseview::dpi::{LogicalSize, Size};
use egui_baseview::baseview::{self, Window};
use egui_baseview::{EguiWindow, EguiWindowSettings, Frame};
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use std::collections::VecDeque;
use std::num::NonZeroIsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// The editor's size in logical pixels: the DAW's editor window's default.
pub const SIZE: (f64, f64) = (1060.0, 720.0);

/// Whether a kind has its own editor in the exported plug-in yet.
pub fn has_editor(kind: EffectKind) -> bool {
    kind == EffectKind::Gate
}

/// Where the editor's edits go: CLAP's output events, VST3's component handler.
pub trait EditSink: Send {
    fn edit(&mut self, edit: Edit);
}

/// The plug-in as the editor's host.
pub struct PluginHost {
    shared: Arc<Shared>,
    spec: EffectSpec,
    pairs: VecDeque<(f32, f32)>,
    rx: Option<rtrb::Consumer<(f32, f32)>>,
    sink: Box<dyn EditSink>,
    /// Parameters inside an open gesture.
    active: Vec<u32>,
}

impl PluginHost {
    pub fn new(shared: Arc<Shared>, sink: Box<dyn EditSink>) -> PluginHost {
        let rx = shared.take_pairs();
        PluginHost {
            spec: shared.spec(),
            shared,
            pairs: VecDeque::with_capacity(PAIRS),
            rx,
            sink,
            active: Vec::new(),
        }
    }

    /// Before each frame: the values as they are now, and what the tap brought.
    pub fn refresh(&mut self) {
        self.spec = self.shared.spec();
        if let Some(rx) = self.rx.as_mut() {
            while let Ok(p) = rx.pop() {
                if self.pairs.len() == PAIRS {
                    self.pairs.pop_front();
                }
                self.pairs.push_back(p);
            }
        }
    }

    fn index(&self, key: &str) -> Option<u32> {
        self.shared.table.by_key(key).map(|p| p.index as u32)
    }

    /// A value from the editor: shown at once, and sent to the host inside a gesture.
    fn edit(&mut self, index: u32, value: f32, gesture: bool) {
        let v = self.shared.show_value(index as usize, value as f64);
        if !self.active.contains(&index) {
            self.sink.edit(Edit::Begin(index));
            if gesture {
                self.active.push(index);
            }
        }
        self.sink.edit(Edit::Value(index, v));
        if !gesture && !self.active.contains(&index) {
            self.sink.edit(Edit::End(index));
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.end_gesture();
        if let Some(rx) = self.rx.take() {
            self.shared.return_pairs(rx);
        }
    }
}

impl FxHost for PluginHost {
    fn spec(&self) -> &EffectSpec {
        &self.spec
    }

    fn set(&mut self, key: &str, value: f32, gesture: Option<&str>) {
        if let Some(i) = self.index(key) {
            self.edit(i, value, gesture.is_some());
        }
    }

    fn set_many(&mut self, values: &[(&str, f32)], _label_key: &str) {
        for (k, v) in values {
            if let Some(i) = self.index(k) {
                self.edit(i, *v, false);
            }
        }
    }

    fn end_gesture(&mut self) {
        for i in std::mem::take(&mut self.active) {
            self.sink.edit(Edit::End(i));
        }
    }

    fn telemetry(&self) -> Telemetry {
        self.shared.telemetry()
    }

    fn pairs(&self) -> &VecDeque<(f32, f32)> {
        &self.pairs
    }

    fn transport(&self) -> Transport {
        Transport::default()
    }

    fn key_source(&self) -> Option<KeySource> {
        Some(KeySource::Switch(
            self.shared.value(self.shared.table.sidechain) >= 0.5,
        ))
    }

    fn set_key_source(&mut self, target: KeyTarget) {
        if let KeyTarget::Switch(on) = target {
            let i = self.shared.table.sidechain as u32;
            self.edit(i, if on { 1.0 } else { 0.0 }, false);
        }
    }
}

/// The editor inside egui-baseview's window.
struct App {
    host: PluginHost,
    state: gate::State,
}

impl egui_baseview::App for App {
    fn build(
        &mut self,
        ctx: egui::Context,
        frame: &mut Frame,
    ) -> Result<(), baseview::HandlerError> {
        // The editor speaks the language the parameters do (`crate::text`).
        cantus_ui::i18n::set_locale(match crate::text::lang() {
            crate::text::Lang::En => cantus_ui::i18n::Locale::En,
            crate::text::Lang::Es => cantus_ui::i18n::Locale::Es,
        });
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
        let pal = cantus_ui::theme::p();
        frame.set_clear_color(pal.panel.into());
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        self.host.refresh();
        let pal = cantus_ui::theme::p();
        egui::Frame::new()
            .fill(pal.panel)
            .inner_margin(cantus_ui::design::space::S3)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                gate::editor(&mut self.host, &mut self.state, ui);
            });
        // The meters and history move with the audio: about 30 frames a second.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
}

/// The host's window, as raw-window-handle hands it to baseview.
struct Parent(RawWindowHandle);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the host keeps its window alive while ours is its child (the standards'
        // contract for embedded editors), and ours is destroyed first.
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}

/// A raw window handle from a Win32 `HWND`.
pub fn win32_parent(hwnd: *mut std::ffi::c_void) -> Option<RawWindowHandle> {
    NonZeroIsize::new(hwnd as isize).map(|h| RawWindowHandle::Win32(Win32WindowHandle::new(h)))
}

/// An open editor. Dropping it closes the window.
pub struct Editor {
    window: Window,
    shared: Arc<Shared>,
}

impl Editor {
    /// Open the editor as a child of `parent`. Main thread.
    pub fn open(
        shared: Arc<Shared>,
        sink: Box<dyn EditSink>,
        parent: RawWindowHandle,
    ) -> Result<Editor, String> {
        if !has_editor(shared.desc.kind) {
            return Err("no editor".into());
        }
        let settings = EguiWindowSettings::new()
            .with_title(shared.desc.full_name)
            .with_size(Size::Logical(LogicalSize {
                width: SIZE.0,
                height: SIZE.1,
            }))
            .with_resizable(false)
            .with_parent(&Parent(parent));
        shared.editor_open.store(true, Ordering::Release);
        let app = App {
            host: PluginHost::new(shared.clone(), sink),
            state: gate::State::default(),
        };
        match EguiWindow::create(settings, app) {
            Ok(window) => {
                let _ = window.show();
                Ok(Editor { window, shared })
            }
            Err(e) => {
                shared.editor_open.store(false, Ordering::Release);
                Err(format!("{e:?}"))
            }
        }
    }

    /// The window's size in physical pixels.
    pub fn physical_size(&self) -> (u32, u32) {
        let s = self.window.size();
        (s.physical.width, s.physical.height)
    }

    pub fn show(&self) -> bool {
        self.window.show().is_ok()
    }

    pub fn hide(&self) -> bool {
        self.window.hide().is_ok()
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.shared.editor_open.store(false, Ordering::Release);
    }
}
