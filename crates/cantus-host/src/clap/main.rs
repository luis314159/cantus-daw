//! The main-thread half of a CLAP plug-in: parameters, state, latency, the editor, the
//! plug-in's requests, and restarting it.

use super::audio::ClapAudio;
use super::events::{InEvents, OutEvents};
use super::host::{flag, AudioScope};
use super::instance::{owner, Instance, Ports};
use crate::backend::{
    AudioThread, EditorSize, HostError, MainThread, Notice, ParamChange, ParamInfo, ParamOut,
    PluginInfo,
};
use clap_sys::ext::gui::{
    clap_window, clap_window_handle, CLAP_WINDOW_API_WIN32, CLAP_WINDOW_API_X11,
};
use clap_sys::ext::params::{
    clap_param_info, CLAP_PARAM_IS_AUTOMATABLE, CLAP_PARAM_IS_HIDDEN, CLAP_PARAM_IS_READONLY,
    CLAP_PARAM_IS_STEPPED,
};
use clap_sys::stream::{clap_istream, clap_ostream};
use std::ffi::{c_char, c_void};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Text in a fixed C buffer, up to its first NUL.
pub(crate) fn c_text(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Editor {
    Closed,
    Embedded,
    Floating,
}

pub struct ClapMain {
    /// `Some` for the whole life of the half; an `Option` only so a restart can take it
    /// out and put it back with `&mut` access.
    inst: Option<Arc<Instance>>,
    info: PluginInfo,
    ports: Ports,
    editor: Editor,
    /// The latency read after the last activation.
    latency: usize,
    events: Box<InEvents>,
    out_events: Box<OutEvents>,
}

impl ClapMain {
    pub(crate) fn new(inst: Arc<Instance>, info: PluginInfo, ports: Ports) -> ClapMain {
        let latency = inst.latency();
        ClapMain {
            inst: Some(inst),
            info,
            ports,
            editor: Editor::Closed,
            latency,
            events: InEvents::new(),
            out_events: OutEvents::new(),
        }
    }

    fn inst(&self) -> &Instance {
        self.inst
            .as_ref()
            .expect("the instance is only taken out during a restart")
    }

    fn plugin(&self) -> *const clap_sys::plugin::clap_plugin {
        self.inst().plugin()
    }

    fn param_info_at(&self, index: u32) -> Option<clap_param_info> {
        let params = unsafe { self.inst().ext.params.as_ref() }?;
        let get = params.get_info?;
        // SAFETY: zeroed is valid for this plain C struct; the params extension on the
        // main thread.
        let mut info: clap_param_info = unsafe { std::mem::zeroed() };
        unsafe { get(self.plugin(), index, &mut info) }.then_some(info)
    }

    fn gui(&self) -> Option<&clap_sys::ext::gui::clap_plugin_gui> {
        unsafe { self.inst().ext.gui.as_ref() }
    }
}

/// The display scale of a window: its DPI over 96.
#[cfg(windows)]
fn window_scale(hwnd: *mut c_void) -> f64 {
    // SAFETY: a window handle the host owns; an invalid one returns 0.
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    if dpi == 0 {
        1.0
    } else {
        f64::from(dpi) / 96.0
    }
}

#[cfg(not(windows))]
fn window_scale(_window: *mut c_void) -> f64 {
    1.0
}

/// Where `state.save` writes.
unsafe extern "C" fn ostream_write(
    stream: *const clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    let out = &mut *((*stream).ctx as *mut Vec<u8>);
    if buffer.is_null() {
        return -1;
    }
    let bytes = std::slice::from_raw_parts(buffer as *const u8, size as usize);
    out.extend_from_slice(bytes);
    size as i64
}

/// Where `state.load` reads from: the bytes and how far it has read.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

unsafe extern "C" fn istream_read(
    stream: *const clap_istream,
    buffer: *mut c_void,
    size: u64,
) -> i64 {
    let r = &mut *((*stream).ctx as *mut Reader<'_>);
    if buffer.is_null() {
        return -1;
    }
    let n = (r.data.len() - r.at).min(size as usize);
    std::ptr::copy_nonoverlapping(r.data[r.at..].as_ptr(), buffer as *mut u8, n);
    r.at += n;
    n as i64
}

impl MainThread for ClapMain {
    fn expression_support(&self) -> crate::backend::ExprSupport {
        crate::backend::ExprSupport {
            note_expressions: self.ports.note_dialects.clap,
            midi: self.ports.note_dialects.midi,
        }
    }

    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn params(&self) -> Vec<ParamInfo> {
        let Some(count) = (unsafe { self.inst().ext.params.as_ref() }).and_then(|p| p.count) else {
            return Vec::new();
        };
        // SAFETY: the params extension, on the main thread.
        let n = unsafe { count(self.plugin()) };
        (0..n)
            .filter_map(|i| self.param_info_at(i))
            .map(|p| ParamInfo {
                id: p.id,
                name: c_text(&p.name),
                group: c_text(&p.module),
                min: p.min_value,
                max: p.max_value,
                default: p.default_value,
                stepped: p.flags & CLAP_PARAM_IS_STEPPED != 0,
                automatable: p.flags & CLAP_PARAM_IS_AUTOMATABLE != 0,
                hidden: p.flags & CLAP_PARAM_IS_HIDDEN != 0,
                read_only: p.flags & CLAP_PARAM_IS_READONLY != 0,
                // A stepped CLAP parameter's steps are its whole units.
                steps: if p.flags & CLAP_PARAM_IS_STEPPED != 0 {
                    let n = (p.max_value - p.min_value).abs().round();
                    if n.is_finite() && n < f64::from(u32::MAX) {
                        n as u32
                    } else {
                        0
                    }
                } else {
                    0
                },
            })
            .collect()
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        let get = unsafe { self.inst().ext.params.as_ref() }?.get_value?;
        let mut v = 0.0;
        // SAFETY: the params extension, on the main thread.
        unsafe { get(self.plugin(), id, &mut v) }.then_some(v)
    }

    fn param_text(&self, id: u32, value: f64) -> Option<String> {
        let to_text = unsafe { self.inst().ext.params.as_ref() }?.value_to_text?;
        let mut buf = [0 as c_char; 256];
        // SAFETY: the params extension, on the main thread, with a buffer of the size given.
        let ok = unsafe { to_text(self.plugin(), id, value, buf.as_mut_ptr(), buf.len() as u32) };
        ok.then(|| c_text(&buf))
    }

    fn save_state(&mut self) -> Result<Vec<u8>, HostError> {
        let save = unsafe { self.inst().ext.state.as_ref() }
            .and_then(|s| s.save)
            .ok_or(HostError::Unsupported("state"))?;
        let mut bytes: Vec<u8> = Vec::new();
        let stream = clap_ostream {
            ctx: (&mut bytes as *mut Vec<u8>).cast(),
            write: Some(ostream_write),
        };
        // SAFETY: the state extension, on the main thread; `bytes` outlives the call.
        if unsafe { save(self.plugin(), &stream) } {
            Ok(bytes)
        } else {
            Err(HostError::State(
                "the plug-in could not save its state".into(),
            ))
        }
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), HostError> {
        let load = unsafe { self.inst().ext.state.as_ref() }
            .and_then(|s| s.load)
            .ok_or(HostError::Unsupported("state"))?;
        let mut reader = Reader { data, at: 0 };
        let stream = clap_istream {
            ctx: (&mut reader as *mut Reader<'_>).cast(),
            read: Some(istream_read),
        };
        // SAFETY: as for `save_state`.
        if unsafe { load(self.plugin(), &stream) } {
            Ok(())
        } else {
            Err(HostError::State("the plug-in refused the state".into()))
        }
    }

    fn latency(&self) -> usize {
        self.latency
    }

    fn has_editor(&self) -> bool {
        self.info.editor
    }

    fn open_editor(
        &mut self,
        parent: raw_window_handle::RawWindowHandle,
    ) -> Result<EditorSize, HostError> {
        if self.editor != Editor::Closed {
            self.close_editor();
        }
        let gui = *self.gui().ok_or(HostError::Unsupported("editor"))?;
        let plugin = self.plugin();
        // Win32 on Windows; X11 on Linux (the window of `window/x11.rs`, ADR 0065); the
        // parent `NSView` on macOS (the `cocoa` member of the same pointer-sized union, ADR
        // 0066).
        let (api, specific, win32) = match parent {
            raw_window_handle::RawWindowHandle::Win32(h) => {
                let hwnd = h.hwnd.get() as *mut c_void;
                (
                    CLAP_WINDOW_API_WIN32.as_ptr(),
                    clap_window_handle { win32: hwnd },
                    Some(hwnd),
                )
            }
            raw_window_handle::RawWindowHandle::Xlib(h) => (
                CLAP_WINDOW_API_X11.as_ptr(),
                clap_window_handle { x11: h.window },
                None,
            ),
            raw_window_handle::RawWindowHandle::AppKit(h) => (
                clap_sys::ext::gui::CLAP_WINDOW_API_COCOA.as_ptr(),
                clap_window_handle {
                    cocoa: h.ns_view.as_ptr(),
                },
                None,
            ),
            _ => return Err(HostError::Unsupported("editor on this windowing system")),
        };
        let window = clap_window { api, specific };
        // SAFETY: the gui extension, on the main thread, in the order `gui.h` gives.
        unsafe {
            let supported = |floating: bool| {
                gui.is_api_supported
                    .is_some_and(|f| f(plugin, api, floating))
            };
            let create = gui.create.ok_or(HostError::Unsupported("editor"))?;
            if supported(false) {
                if !create(plugin, api, false) {
                    return Err(HostError::Editor("create".into()));
                }
                // Win32 sizes are physical pixels: tell the editor the window's scale
                // before it says how big it is. (X11 editors read the screen's scale
                // themselves; Cocoa's sizes are points: CLAP leaves `set_scale` to them.)
                if let (Some(f), Some(hwnd)) = (gui.set_scale, win32) {
                    f(plugin, window_scale(hwnd));
                }
                let (mut w, mut h) = (0u32, 0u32);
                if let Some(f) = gui.get_size {
                    f(plugin, &mut w, &mut h);
                }
                if !gui.set_parent.is_some_and(|f| f(plugin, &window)) {
                    if let Some(f) = gui.destroy {
                        f(plugin);
                    }
                    return Err(HostError::Editor("set_parent".into()));
                }
                if let Some(f) = gui.show {
                    f(plugin);
                }
                self.editor = Editor::Embedded;
                Ok(EditorSize {
                    width: w,
                    height: h,
                })
            } else if supported(true) {
                if !create(plugin, api, true) {
                    return Err(HostError::Editor("create".into()));
                }
                if let Some(f) = gui.set_transient {
                    f(plugin, &window);
                }
                if let Some(f) = gui.suggest_title {
                    if let Ok(title) = std::ffi::CString::new(self.info.name.as_str()) {
                        f(plugin, title.as_ptr());
                    }
                }
                if let Some(f) = gui.show {
                    f(plugin);
                }
                self.editor = Editor::Floating;
                Ok(EditorSize {
                    width: 0,
                    height: 0,
                })
            } else {
                Err(HostError::Unsupported("editor for this window system"))
            }
        }
    }

    fn close_editor(&mut self) {
        if self.editor == Editor::Closed {
            return;
        }
        if let Some(gui) = self.gui().copied() {
            let plugin = self.plugin();
            // SAFETY: the gui extension, on the main thread, after `create`.
            unsafe {
                if let Some(f) = gui.hide {
                    f(plugin);
                }
                if let Some(f) = gui.destroy {
                    f(plugin);
                }
            }
        }
        self.editor = Editor::Closed;
    }

    fn idle(&mut self) -> Vec<Notice> {
        super::instance::collect_garbage();
        #[cfg(unix)]
        self.serve_run_loop();
        let mut notices = Vec::new();
        // Everything is taken but a flush request, which `flush` serves (the host calls it
        // every frame).
        let flags = self
            .inst()
            .host()
            .flags
            .fetch_and(flag::REQUEST_FLUSH, Ordering::AcqRel);
        if flags & flag::CALLBACK != 0 {
            let plugin = self.plugin();
            // SAFETY: `on_main_thread` is what `request_callback` asked for; main thread.
            unsafe {
                if let Some(f) = (*plugin).on_main_thread {
                    f(plugin);
                }
            }
        }
        if flags & flag::RESTART != 0 {
            notices.push(Notice::RestartNeeded);
        }
        if flags & flag::PARAM_INFO != 0 {
            notices.push(Notice::ParamsChanged);
        } else if flags & flag::PARAM_VALUES != 0 {
            // New values without new parameters: a preset the plug-in loaded itself. The
            // project stores the new state and values.
            notices.push(Notice::StateDirty);
        }
        if flags & flag::STATE_DIRTY != 0 && !notices.contains(&Notice::StateDirty) {
            notices.push(Notice::StateDirty);
        }
        if flags & flag::GUI_RESIZE != 0 {
            let s = self.inst().host().gui_size.load(Ordering::Acquire);
            notices.push(Notice::EditorResized(EditorSize {
                width: (s >> 32) as u32,
                height: s as u32,
            }));
        }
        if flags & flag::GUI_CLOSED != 0 {
            self.editor = Editor::Closed;
            notices.push(Notice::EditorClosed);
        }
        notices
    }

    fn restart(&mut self) -> Result<Option<Box<dyn AudioThread>>, HostError> {
        if self.inst().audio_alive.load(Ordering::Acquire) {
            return Ok(None);
        }
        // Both halves hold the `Arc`; with the audio half gone this is the only one, so
        // the instance can be deactivated and activated through it.
        let mut arc = self.inst.take().expect("instance present");
        let result = match Arc::get_mut(&mut arc) {
            Some(inst) => {
                inst.deactivate();
                inst.activate()
            }
            None => Err(HostError::Activate("the instance is still shared".into())),
        };
        let ports = arc.ports();
        self.inst = Some(arc);
        result?;
        let old = self.latency;
        self.latency = self.inst().latency();
        if self.latency != old {
            log::info!(
                "{}: latency {} → {} samples",
                self.info.name,
                old,
                self.latency
            );
        }
        self.ports = ports.clone();
        let inst = self.inst.clone().expect("instance present");
        Ok(Some(Box::new(ClapAudio::new(inst, ports))))
    }

    fn flush(&mut self, params: &[ParamChange], out: &mut ParamOut) -> bool {
        let host = self.inst().host();
        let asked = host.flags.load(Ordering::Acquire) & flag::REQUEST_FLUSH != 0;
        if params.is_empty() && !asked {
            return true; // nothing to deliver and nothing asked for
        }
        let Some(flush) = (unsafe { self.inst().ext.params.as_ref() }).and_then(|p| p.flush) else {
            return false;
        };
        let inst = self.inst.clone().expect("instance present");
        if inst
            .owner
            .compare_exchange(
                owner::NONE,
                owner::MAIN,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return false; // processing: the changes go with the next block
        }
        host.take_bits(flag::REQUEST_FLUSH);
        self.events.fill(params, &[], 1, Default::default(), 0);
        self.out_events.bind(out);
        {
            // An active plug-in is flushed from its audio thread; with nothing processing,
            // this thread holds that role for the call.
            let _scope = AudioScope::enter();
            // SAFETY: nothing else calls the processing side (the owner flag says so).
            unsafe { flush(inst.plugin(), self.events.raw(), self.out_events.raw()) };
        }
        self.out_events.unbind();
        inst.owner.store(owner::NONE, Ordering::Release);
        true
    }

    fn editor_is_floating(&self) -> bool {
        self.editor == Editor::Floating
    }

    fn resize_editor(&mut self, size: EditorSize) -> Option<EditorSize> {
        if self.editor != Editor::Embedded {
            return None;
        }
        let gui = *self.gui()?;
        let plugin = self.plugin();
        let (mut w, mut h) = (size.width, size.height);
        // SAFETY: the gui extension, on the main thread, while created.
        unsafe {
            if !gui.can_resize.is_some_and(|f| f(plugin)) {
                return None;
            }
            if let Some(f) = gui.adjust_size {
                f(plugin, &mut w, &mut h);
            }
            gui.set_size
                .is_some_and(|f| f(plugin, w, h))
                .then_some(EditorSize {
                    width: w,
                    height: h,
                })
        }
    }

    fn editor_can_resize(&self) -> bool {
        if self.editor != Editor::Embedded {
            return false;
        }
        let Some(gui) = self.gui().copied() else {
            return false;
        };
        // SAFETY: the gui extension, on the main thread, while created.
        unsafe { gui.can_resize.is_some_and(|f| f(self.plugin())) }
    }
}

impl Drop for ClapMain {
    fn drop(&mut self) {
        self.close_editor();
    }
}

#[cfg(unix)]
impl ClapMain {
    /// The plug-in's timers that are due and descriptors that are ready, called back
    /// (`timer-support`, `posix-fd-support`; ADR 0065). The run loop is not locked while
    /// the plug-in runs: it may register and unregister from inside its callback.
    fn serve_run_loop(&mut self) {
        use crate::runloop::{serve, Fired};
        let plugin = self.plugin();
        let ext = &self.inst().ext;
        // SAFETY: the plug-in's own extensions, on the main thread.
        let on_timer = unsafe { ext.timer_support.as_ref() }.and_then(|t| t.on_timer);
        let on_fd = unsafe { ext.posix_fd_support.as_ref() }.and_then(|f| f.on_fd);
        serve(
            &self.inst().host().run_loop,
            std::time::Instant::now(),
            // SAFETY: the plug-in's callbacks, on the main thread, the run loop unlocked.
            |fired| unsafe {
                match fired {
                    Fired::Timer(id, ()) => {
                        if let Some(f) = on_timer {
                            f(plugin, id);
                        }
                    }
                    Fired::Fd(fd, flags, ()) => {
                        if let Some(f) = on_fd {
                            f(plugin, fd, flags);
                        }
                    }
                }
            },
        );
    }
}
