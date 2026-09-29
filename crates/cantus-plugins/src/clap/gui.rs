//! CLAP's GUI extension, and what an open editor sends the host.
//!
//! The editor (`crate::editor`, feature `editor`) runs on the main thread and hands each
//! edit — gesture begin, value, gesture end — to a lock-free ring. The audio thread
//! drains it at the top of `process` (or in `params.flush` when the host is not
//! processing, which the editor asks for with `request_flush`): each value goes into the
//! effect and out to the host as a CLAP event, inside its gesture, so the host records it
//! as automation and one undo step.

use super::Instance;
use crate::instance::{Edit, ParamEvent, Processor};
use clap_sys::events::*;
use clap_sys::plugin::clap_plugin;
use std::ffi::{c_void, CStr};
use std::ptr;

/// Edits a ring holds between two blocks: far more than a person makes.
pub(crate) const EDITS: usize = 1024;

/// An open editor, when there is one.
#[derive(Default)]
pub(crate) struct Gui {
    #[cfg(all(feature = "editor", windows))]
    editor: Option<crate::editor::Editor>,
}

impl std::fmt::Debug for Gui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gui")
    }
}

/// The editor side of the edit ring, shared by every editor an instance opens.
pub(crate) type EditTx = std::sync::Arc<std::sync::Mutex<rtrb::Producer<Edit>>>;

pub(crate) fn extension(plugin: *const clap_plugin, id: &CStr) -> *const c_void {
    #[cfg(all(feature = "editor", windows))]
    {
        // SAFETY: the host's pointer to one of our instances.
        let has = unsafe { Instance::from(plugin) }
            .is_some_and(|i| crate::editor::has_editor(i.shared.desc.kind));
        if has && id == clap_sys::ext::gui::CLAP_EXT_GUI {
            return &imp::GUI as *const _ as *const c_void;
        }
    }
    let _ = (plugin, id);
    ptr::null()
}

/// Close the editor before the instance goes.
pub(crate) fn gui_destroy_for(inst: &Instance) {
    // SAFETY: main thread (`destroy`).
    unsafe { *inst.gui.get() = None };
}

pub(crate) fn gui_on_main_thread(_inst: &Instance) {}

/// Take what the editor did since the last call: into the effect, and out to the host.
/// Audio thread (or the main thread in an inactive `flush`, with no processor).
///
/// # Safety
/// `out` must be null or the host's valid output list for this call.
pub(crate) unsafe fn drain_edits(
    inst: &Instance,
    mut proc_: Option<&mut Processor>,
    out: *const clap_output_events,
) {
    let rx = &mut *inst.edits_rx.get();
    let push = |ev: *const clap_event_header| {
        if !out.is_null() {
            if let Some(try_push) = (*out).try_push {
                try_push(out, ev);
            }
        }
    };
    let table = inst.shared.table;
    while let Ok(edit) = rx.pop() {
        let header = |type_: u16, size: usize| clap_event_header {
            size: size as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_,
            flags: CLAP_EVENT_IS_LIVE,
        };
        match edit {
            Edit::Begin(i) | Edit::End(i) => {
                let Some(p) = table.params.get(i as usize) else {
                    continue;
                };
                let type_ = if matches!(edit, Edit::Begin(_)) {
                    CLAP_EVENT_PARAM_GESTURE_BEGIN
                } else {
                    CLAP_EVENT_PARAM_GESTURE_END
                };
                let ev = clap_event_param_gesture {
                    header: header(type_, std::mem::size_of::<clap_event_param_gesture>()),
                    param_id: p.id,
                };
                push(&ev.header);
            }
            Edit::Value(i, v) => {
                let Some(p) = table.params.get(i as usize) else {
                    continue;
                };
                match proc_.as_deref_mut() {
                    Some(proc_) => proc_.flush(&[ParamEvent {
                        offset: 0,
                        index: i,
                        value: v,
                        modulation: false,
                    }]),
                    None => {
                        inst.shared.set_value(i as usize, v as f64);
                    }
                }
                let ev = clap_event_param_value {
                    header: header(
                        CLAP_EVENT_PARAM_VALUE,
                        std::mem::size_of::<clap_event_param_value>(),
                    ),
                    param_id: p.id,
                    cookie: (p.index + 1) as *mut c_void,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: p.to_clap(v),
                };
                push(&ev.header);
            }
        }
    }
}

#[cfg(all(feature = "editor", windows))]
mod imp {
    use super::super::Instance;
    use super::{EditTx, Gui};
    use crate::editor::{self, Edit, EditSink, Editor};
    use clap_sys::ext::gui::*;
    use clap_sys::ext::params::{clap_host_params, CLAP_EXT_PARAMS};
    use clap_sys::host::clap_host;
    use clap_sys::plugin::clap_plugin;
    use std::ffi::{c_char, CStr};

    pub(super) static GUI: clap_plugin_gui = clap_plugin_gui {
        is_api_supported: Some(is_api_supported),
        get_preferred_api: Some(get_preferred_api),
        create: Some(create),
        destroy: Some(destroy),
        set_scale: Some(set_scale),
        get_size: Some(get_size),
        can_resize: Some(can_resize),
        get_resize_hints: Some(get_resize_hints),
        adjust_size: Some(adjust_size),
        set_size: Some(set_size),
        set_parent: Some(set_parent),
        set_transient: Some(set_transient),
        suggest_title: Some(suggest_title),
        show: Some(show),
        hide: Some(hide),
    };

    /// The editor's edits into the ring, and a nudge to the host to flush them if it is
    /// not processing.
    struct Sink {
        tx: EditTx,
        host: *const clap_host,
        params: *const clap_host_params,
    }

    // SAFETY: the sink lives in the editor, on the host's main thread; the host pointers
    // are the host's own, valid while the instance lives, and `request_flush` may be
    // called from any thread but the audio thread.
    unsafe impl Send for Sink {}

    impl EditSink for Sink {
        fn edit(&mut self, edit: Edit) {
            if let Ok(mut tx) = self.tx.lock() {
                let _ = tx.push(edit);
            }
            // SAFETY: see above.
            unsafe {
                if !self.params.is_null() {
                    if let Some(flush) = (*self.params).request_flush {
                        flush(self.host);
                    }
                }
            }
        }
    }

    /// The system's scale (Windows reports physical pixels to CLAP).
    fn scale() -> f64 {
        // SAFETY: a plain query.
        let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() };
        if dpi == 0 {
            1.0
        } else {
            dpi as f64 / 96.0
        }
    }

    fn supported(api: *const c_char, is_floating: bool) -> bool {
        // SAFETY: the host's C string.
        !is_floating && !api.is_null() && unsafe { CStr::from_ptr(api) } == CLAP_WINDOW_API_WIN32
    }

    unsafe extern "C" fn is_api_supported(
        _p: *const clap_plugin,
        api: *const c_char,
        is_floating: bool,
    ) -> bool {
        supported(api, is_floating)
    }

    unsafe extern "C" fn get_preferred_api(
        _p: *const clap_plugin,
        api: *mut *const c_char,
        is_floating: *mut bool,
    ) -> bool {
        if api.is_null() || is_floating.is_null() {
            return false;
        }
        *api = CLAP_WINDOW_API_WIN32.as_ptr();
        *is_floating = false;
        true
    }

    unsafe extern "C" fn create(
        plugin: *const clap_plugin,
        api: *const c_char,
        is_floating: bool,
    ) -> bool {
        let Some(inst) = Instance::from(plugin) else {
            return false;
        };
        if !supported(api, is_floating) {
            return false;
        }
        *inst.gui.get() = Some(Gui::default());
        true
    }

    unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
        if let Some(inst) = Instance::from(plugin) {
            *inst.gui.get() = None;
        }
    }

    unsafe extern "C" fn set_scale(_p: *const clap_plugin, _scale: f64) -> bool {
        // The window follows the system's scale itself (baseview reads the monitor's).
        false
    }

    unsafe extern "C" fn get_size(plugin: *const clap_plugin, w: *mut u32, h: *mut u32) -> bool {
        let Some(inst) = Instance::from(plugin) else {
            return false;
        };
        if w.is_null() || h.is_null() {
            return false;
        }
        let open = (*inst.gui.get())
            .as_ref()
            .and_then(|g| g.editor.as_ref())
            .map(Editor::physical_size);
        let (pw, ph) = open.unwrap_or_else(|| {
            let s = scale();
            let (w, h) = editor::size(inst.shared.desc.kind);
            ((w * s).round() as u32, (h * s).round() as u32)
        });
        *w = pw;
        *h = ph;
        true
    }

    unsafe extern "C" fn can_resize(_p: *const clap_plugin) -> bool {
        false
    }

    unsafe extern "C" fn get_resize_hints(
        _p: *const clap_plugin,
        _hints: *mut clap_gui_resize_hints,
    ) -> bool {
        false
    }

    unsafe extern "C" fn adjust_size(_p: *const clap_plugin, _w: *mut u32, _h: *mut u32) -> bool {
        false
    }

    unsafe extern "C" fn set_size(_p: *const clap_plugin, _w: u32, _h: u32) -> bool {
        false
    }

    unsafe extern "C" fn set_parent(
        plugin: *const clap_plugin,
        window: *const clap_window,
    ) -> bool {
        let Some(inst) = Instance::from(plugin) else {
            return false;
        };
        if window.is_null() || !supported((*window).api, false) {
            return false;
        }
        let Some(parent) = editor::win32_parent((*window).specific.win32) else {
            return false;
        };
        let Some(gui) = (*inst.gui.get()).as_mut() else {
            return false;
        };
        let sink = Sink {
            tx: inst.edits_tx.clone(),
            host: inst.host,
            params: inst.host_extension::<clap_host_params>(CLAP_EXT_PARAMS),
        };
        match Editor::open(inst.shared.clone(), Box::new(sink), parent) {
            Ok(e) => {
                gui.editor = Some(e);
                true
            }
            Err(_) => false,
        }
    }

    unsafe extern "C" fn set_transient(_p: *const clap_plugin, _w: *const clap_window) -> bool {
        false
    }

    unsafe extern "C" fn suggest_title(_p: *const clap_plugin, _title: *const c_char) {}

    unsafe extern "C" fn show(plugin: *const clap_plugin) -> bool {
        Instance::from(plugin)
            .and_then(|i| (*i.gui.get()).as_ref())
            .and_then(|g| g.editor.as_ref())
            .is_some_and(Editor::show)
    }

    unsafe extern "C" fn hide(plugin: *const clap_plugin) -> bool {
        Instance::from(plugin)
            .and_then(|i| (*i.gui.get()).as_ref())
            .and_then(|g| g.editor.as_ref())
            .is_some_and(Editor::hide)
    }
}
