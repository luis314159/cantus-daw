//! The plug-ins' own editors — the DAW's, running in the host's window (ADR 0020).
//!
//! The editor is the one Cantus draws (`cantus_ui::views::fx::Editor`: the effect's
//! editor in its device frame, ADR 0024 §8), written against [`FxHost`] instead of the
//! app. Here the host is [`PluginHost`]: the plug-in's values, its own telemetry and
//! analyser tap (lock-free, from the audio thread), the host's gestures through an
//! [`EditSink`] the format layer provides, the standards' bypass for the frame's lamp, and
//! A/B kept here. The window is egui 0.36 in a child of the host's window, through
//! baseview and egui-baseview (OpenGL).
//!
//! A kind whose editor has not moved onto [`FxHost`] shows the host's generic interface
//! (the plan is in ADR 0020 §8).

pub use crate::instance::Edit;
use crate::instance::{Shared, PAIRS};
use cantus_core::{EffectKind, EffectSpec};
use cantus_engine::Telemetry;
use cantus_ui::views::fx::host::{FxHost, KeySource, KeyTarget, StereoTake, Transport};
use cantus_ui::views::fx::Editor as FxEditor;
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

/// The editor's size in logical pixels: the device frame's designed size for the kind
/// (ADR 0024 §8), as in the DAW at 100 %.
pub fn size(kind: EffectKind) -> (f64, f64) {
    let s = cantus_ui::views::fx::frame::designed_size(kind);
    (s.x as f64, s.y as f64)
}

/// Whether a kind has its own editor in the exported plug-in yet.
pub fn has_editor(kind: EffectKind) -> bool {
    FxEditor::of(kind).is_some()
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
    /// Frames the window's queue let go before Speculum took them (review 1).
    stereo_lost: u64,
    sink: Box<dyn EditSink>,
    /// Parameters inside an open gesture.
    active: Vec<u32>,
    /// Bumped whenever a value changes (the editor's own edits, the host's automation, a
    /// state load): the frame keeps what it formats until then.
    revision: u64,
    /// The other side of A/B (the plug-in's values), and whether B is the side showing.
    other: Option<Vec<f32>>,
    on_b: bool,
}

impl PluginHost {
    pub fn new(shared: Arc<Shared>, sink: Box<dyn EditSink>) -> PluginHost {
        let rx = shared.take_pairs();
        PluginHost {
            spec: shared.spec(),
            shared,
            pairs: VecDeque::with_capacity(PAIRS),
            rx,
            stereo_lost: 0,
            sink,
            active: Vec::new(),
            revision: 0,
            other: None,
            on_b: false,
        }
    }

    /// Before each frame: the values as they are now, and what the tap brought. The spec
    /// holds every key, so a changed value is a write into its entry — nothing allocates.
    pub fn refresh(&mut self) {
        let table = self.shared.table;
        let mut changed = false;
        for p in &table.params[..table.spec_count] {
            let v = self.shared.value(p.index);
            match self.spec.params.get_mut(p.key) {
                Some(old) if *old != v => {
                    *old = v;
                    changed = true;
                }
                Some(_) => {}
                None => {
                    self.spec.params.insert(p.key.to_string(), v);
                    changed = true;
                }
            }
        }
        if changed {
            self.revision += 1;
        }
        if let Some(rx) = self.rx.as_mut() {
            while let Ok(p) = rx.pop() {
                if self.pairs.len() == PAIRS {
                    self.pairs.pop_front();
                    // Speculum reads every frame: one gone is a gap it reports.
                    self.stereo_lost += 1;
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
        self.revision += 1;
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

    /// Several keys at once; inside a gesture (a drag of several bands) each stays open
    /// until the pointer lets go, so the host records one gesture per parameter. A cleared
    /// key goes back to its default.
    fn apply(&mut self, edits: &[(&str, Option<f32>)], _label_key: &str, gesture: Option<u64>) {
        for (k, v) in edits {
            let Some(p) = self.shared.table.by_key(k) else {
                continue;
            };
            let value = v.unwrap_or(p.default);
            self.edit(p.index as u32, value, gesture.is_some());
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
        // The host's transport as the audio thread last saw it (ADR 0055); before the
        // first block, the default (120 BPM in 4/4, stopped).
        let Some(h) = self.shared.host_transport() else {
            return Transport::default();
        };
        let ppq = cantus_core::PPQ as f64;
        let (num, den) = h.time_sig;
        Transport {
            playing_at: h.playing.then_some((h.beats * ppq).floor() as i64),
            ticks_per_second: h.tempo / 60.0 * ppq,
            ticks_per_bar: (f64::from(num.max(1)) * 4.0 / f64::from(den.max(1)) * ppq).round()
                as i64,
        }
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

    fn bypassed(&self) -> Option<bool> {
        Some(self.shared.value(self.shared.table.bypass) >= 0.5)
    }

    fn set_bypassed(&mut self, bypassed: bool) {
        let i = self.shared.table.bypass as u32;
        self.edit(i, if bypassed { 1.0 } else { 0.0 }, false);
    }

    fn ab(&self) -> Option<bool> {
        Some(self.on_b)
    }

    /// Switch sides: the showing values are kept, the other side's sent to the host, each
    /// changed parameter as a gesture of its own.
    fn toggle_ab(&mut self) {
        let table = self.shared.table;
        let now: Vec<f32> = (0..table.spec_count)
            .map(|i| self.shared.value(i))
            .collect();
        let other = self.other.take().unwrap_or_else(|| now.clone());
        for (i, (a, b)) in now.iter().zip(&other).enumerate() {
            if a != b {
                self.edit(table.params[i].index as u32, *b, false);
            }
        }
        self.other = Some(now);
        self.on_b = !self.on_b;
    }

    fn copy_ab(&mut self) {
        let table = self.shared.table;
        self.other = Some(
            (0..table.spec_count)
                .map(|i| self.shared.value(i))
                .collect(),
        );
    }

    fn sample_rate(&self) -> f32 {
        let sr = self.shared.sample_rate();
        if sr > 1_000.0 {
            sr
        } else {
            48_000.0
        }
    }

    fn revision(&self) -> u64 {
        self.revision
    }

    fn take_pairs(&mut self, out: &mut Vec<(f32, f32)>) {
        out.extend(self.pairs.drain(..));
    }

    /// Speculum's tap carries its own frames, left and right (ADR 0056); the song's
    /// position is the host's transport as the audio thread last saw it.
    fn take_stereo(&mut self, out: &mut Vec<(f32, f32)>) -> StereoTake {
        if self.shared.desc.kind != EffectKind::Analyzer {
            return StereoTake::default();
        }
        out.extend(self.pairs.drain(..));
        let clock = self
            .shared
            .host_transport()
            .filter(|h| h.playing && h.tempo > 0.0)
            .map(|h| (h.last_beats, h.tempo));
        StereoTake {
            stream: true,
            clock,
            lost: self.shared.scope_lost.swap(0, Ordering::Relaxed)
                + std::mem::take(&mut self.stereo_lost),
        }
    }
}

/// The editor inside egui-baseview's window.
struct App {
    host: PluginHost,
    editor: FxEditor,
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
        // The device frame fills the window: its header is the plug-in's title bar.
        let _ = self.editor.show(&mut self.host, ui);
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
        let Some(editor) = FxEditor::of(shared.desc.kind) else {
            return Err("no editor".into());
        };
        let (w, h) = size(shared.desc.kind);
        let settings = EguiWindowSettings::new()
            .with_title(shared.desc.full_name)
            .with_size(Size::Logical(LogicalSize {
                width: w,
                height: h,
            }))
            .with_resizable(false)
            .with_parent(&Parent(parent));
        shared.editor_open.store(true, Ordering::Release);
        let app = App {
            host: PluginHost::new(shared.clone(), sink),
            editor,
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
