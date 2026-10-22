//! A minimal editor, so the host's plug-in window can be tested: on Windows, a `STATIC`
//! child window of a fixed 320 × 200 physical pixels, made a child of whatever window the
//! host gives it. Nothing is drawn beyond what the control draws itself. On Linux, an X11
//! child window of the same size (`x11gui.rs`).

use clap_sys::ext::gui::{
    clap_plugin_gui, clap_window, CLAP_WINDOW_API_COCOA, CLAP_WINDOW_API_WIN32, CLAP_WINDOW_API_X11,
};
use clap_sys::plugin::clap_plugin;
use std::ffi::{c_char, CStr};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 200;
/// The editor's text (macOS's label; Windows' `STATIC` says the same).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const TITLE: &str = "Cantus Test Gain";

pub static GUI_EXT: clap_plugin_gui = clap_plugin_gui {
    is_api_supported: Some(is_api_supported),
    get_preferred_api: None,
    create: Some(create),
    destroy: Some(destroy),
    set_scale: Some(set_scale),
    get_size: Some(get_size),
    can_resize: Some(can_resize),
    get_resize_hints: None,
    adjust_size: None,
    set_size: Some(set_size),
    set_parent: Some(set_parent),
    set_transient: None,
    suggest_title: None,
    show: Some(show),
    hide: Some(hide),
};

unsafe extern "C" fn is_api_supported(
    _p: *const clap_plugin,
    api: *const c_char,
    floating: bool,
) -> bool {
    if floating || api.is_null() {
        return false;
    }
    let api = CStr::from_ptr(api);
    (cfg!(windows) && api == CLAP_WINDOW_API_WIN32)
        || (cfg!(target_os = "linux") && api == CLAP_WINDOW_API_X11)
        || (cfg!(target_os = "macos") && api == CLAP_WINDOW_API_COCOA)
}

unsafe extern "C" fn create(p: *const clap_plugin, api: *const c_char, floating: bool) -> bool {
    let me = crate::plugin::me(p);
    me.fault.fire(crate::fault::ABORT_EDITOR);
    if !is_api_supported(p, api, floating) || me.editor_created {
        return false;
    }
    me.editor_created = true;
    true
}

unsafe extern "C" fn set_scale(_p: *const clap_plugin, _scale: f64) -> bool {
    true
}

unsafe extern "C" fn get_size(_p: *const clap_plugin, w: *mut u32, h: *mut u32) -> bool {
    *w = WIDTH;
    *h = HEIGHT;
    true
}

unsafe extern "C" fn can_resize(_p: *const clap_plugin) -> bool {
    false
}

unsafe extern "C" fn set_size(_p: *const clap_plugin, w: u32, h: u32) -> bool {
    w == WIDTH && h == HEIGHT
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::plugin::me;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SetTimer, ShowWindow, SW_HIDE, SW_SHOW, WS_CHILD,
        WS_VISIBLE,
    };

    /// The editor's timer: runs the nested loop the Fault parameter asked for (12), from
    /// inside the window's own message — as a plug-in's file dialog does.
    unsafe extern "system" fn on_timer(_hwnd: HWND, _msg: u32, id: usize, _time: u32) {
        // The timer's id is its instance (its window, and so the timer, goes before it).
        let fault = &(*(id as *const crate::plugin::Plugin)).fault;
        let secs = fault.take_window_pump();
        if secs > 0 {
            crate::fault::pump(std::time::Duration::from_secs(u64::from(secs)));
        }
        if fault.take_window_segv() {
            // SAFETY: none — a write to an address no process maps, on purpose.
            unsafe { std::ptr::write_volatile(std::ptr::without_provenance_mut::<u8>(16), 1) }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub unsafe extern "C" fn set_parent(p: *const clap_plugin, window: *const clap_window) -> bool {
        let me = me(p);
        if !me.editor_created || me.editor != 0 || window.is_null() {
            return false;
        }
        let parent = (*window).specific.win32 as HWND;
        let class = wide("STATIC");
        let text = wide("Cantus Test Gain");
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            WIDTH as i32,
            HEIGHT as i32,
            parent,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );
        me.editor = hwnd as isize;
        me.editor != 0
    }

    pub unsafe extern "C" fn show(p: *const clap_plugin) -> bool {
        let me = me(p);
        if me.editor == 0 {
            return false;
        }
        ShowWindow(me.editor as HWND, SW_SHOW);
        SetTimer(
            me.editor as HWND,
            me as *const crate::plugin::Plugin as usize,
            20,
            Some(on_timer),
        );
        true
    }

    pub unsafe extern "C" fn hide(p: *const clap_plugin) -> bool {
        let me = me(p);
        if me.editor == 0 {
            return false;
        }
        ShowWindow(me.editor as HWND, SW_HIDE);
        true
    }

    pub unsafe extern "C" fn destroy(p: *const clap_plugin) {
        let me = me(p);
        if me.editor != 0 {
            DestroyWindow(me.editor as HWND);
        }
        me.editor = 0;
        me.editor_created = false;
    }
}

#[cfg(windows)]
use win::{destroy, hide, set_parent, show};

/// Linux: an X11 child window (`x11gui.rs`, ADR 0065).
#[cfg(target_os = "linux")]
mod x11 {
    use super::*;
    use crate::plugin::me;

    pub unsafe extern "C" fn set_parent(p: *const clap_plugin, window: *const clap_window) -> bool {
        let me = me(p);
        if !me.editor_created || me.editor != 0 || window.is_null() {
            return false;
        }
        me.editor = crate::x11gui::open(me.host, window, "Cantus Test Gain");
        me.editor != 0
    }

    pub unsafe extern "C" fn show(p: *const clap_plugin) -> bool {
        crate::x11gui::show(me(p).editor, true)
    }

    pub unsafe extern "C" fn hide(p: *const clap_plugin) -> bool {
        crate::x11gui::show(me(p).editor, false)
    }

    pub unsafe extern "C" fn destroy(p: *const clap_plugin) {
        let me = me(p);
        crate::x11gui::close(me.host, me.editor);
        me.editor = 0;
        me.editor_created = false;
    }
}

#[cfg(target_os = "linux")]
use x11::{destroy, hide, set_parent, show};

#[cfg(target_os = "macos")]
#[path = "gui_cocoa.rs"]
mod cocoa;
#[cfg(target_os = "macos")]
use cocoa::{destroy, hide, set_parent, show};

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
unsafe extern "C" fn set_parent(_p: *const clap_plugin, _w: *const clap_window) -> bool {
    false
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
unsafe extern "C" fn show(_p: *const clap_plugin) -> bool {
    false
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
unsafe extern "C" fn hide(_p: *const clap_plugin) -> bool {
    false
}
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
unsafe extern "C" fn destroy(p: *const clap_plugin) {
    crate::plugin::me(p).editor_created = false;
}
