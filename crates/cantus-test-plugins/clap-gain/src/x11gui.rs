//! Linux (ADR 0065): the test editor as an X11 child window of 320 × 200 in whatever
//! window the host gives (CLAP `x11`), drawn on its own connection, which it hands the
//! host's run loop (`posix-fd-support`) so its events are read when they arrive, and with a
//! timer (`timer-support`) that runs the Fault parameter's editor faults, as the Win32
//! editor's `SetTimer` does. Both leave a mark the tests can read from the server: the
//! child window's `_CANTUS_TEST_TICKS` (timer calls) and `_CANTUS_TEST_EXPOSED` (expose
//! events drawn), and its name is the plug-in's.
//!
//! Shared by both CLAP test plug-ins (`#[path]` from the synth).

use clap_sys::ext::gui::clap_window;
use clap_sys::ext::posix_fd_support::{
    clap_host_posix_fd_support, clap_plugin_posix_fd_support, clap_posix_fd_flags,
    CLAP_EXT_POSIX_FD_SUPPORT, CLAP_POSIX_FD_READ,
};
use clap_sys::ext::timer_support::{
    clap_host_timer_support, clap_plugin_timer_support, CLAP_EXT_TIMER_SUPPORT,
};
use clap_sys::host::clap_host;
use clap_sys::id::clap_id;
use std::os::fd::AsRawFd;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeGCAux, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, PropMode,
    Rectangle, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub const WIDTH: u16 = 320;
pub const HEIGHT: u16 = 200;

/// One open editor.
pub struct Editor {
    conn: RustConnection,
    window: u32,
    gc: u32,
    ticks_atom: u32,
    exposed_atom: u32,
    ticks: u32,
    exposed: u32,
    timer: Option<clap_id>,
    fd: Option<i32>,
    name: &'static str,
}

pub static TIMER_EXT: clap_plugin_timer_support = clap_plugin_timer_support {
    on_timer: Some(on_timer),
};

pub static FD_EXT: clap_plugin_posix_fd_support =
    clap_plugin_posix_fd_support { on_fd: Some(on_fd) };

/// The extensions this file answers for, when `id` is one.
pub fn extension(id: &std::ffi::CStr) -> Option<*const std::ffi::c_void> {
    if id == CLAP_EXT_TIMER_SUPPORT {
        Some((&TIMER_EXT as *const clap_plugin_timer_support).cast())
    } else if id == CLAP_EXT_POSIX_FD_SUPPORT {
        Some((&FD_EXT as *const clap_plugin_posix_fd_support).cast())
    } else {
        None
    }
}

unsafe fn host_ext<T>(host: *const clap_host, id: &std::ffi::CStr) -> Option<&'static T> {
    let get = (*host).get_extension?;
    (get(host, id.as_ptr()) as *const T).as_ref()
}

/// The editor behind `editor` (the plug-in's `editor` field), if one is open.
///
/// # Safety
/// `editor` is 0 or what [`open`] returned and [`close`] has not freed.
unsafe fn of<'a>(editor: isize) -> Option<&'a mut Editor> {
    (editor as *mut Editor).as_mut()
}

/// Make the editor's window a child of the host's, register its descriptor and timer.
/// Returns the editor, as an `isize` for the plug-in's `editor` field; 0 when it failed.
///
/// # Safety
/// On the main thread; `window` is the host's `clap_window` for the `x11` API.
pub unsafe fn open(
    host: *const clap_host,
    window: *const clap_window,
    name: &'static str,
) -> isize {
    let parent = (*window).specific.x11 as u32;
    let Ok((conn, _)) = x11rb::connect(None) else {
        return 0;
    };
    let Some(made) = (|| -> Option<Editor> {
        let window = conn.generate_id().ok()?;
        let aux = CreateWindowAux::new()
            .background_pixel(0x0020_2830)
            .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY);
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            parent,
            0,
            0,
            WIDTH,
            HEIGHT,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &aux,
        )
        .ok()?;
        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            name.as_bytes(),
        )
        .ok()?;
        let gc = conn.generate_id().ok()?;
        conn.create_gc(gc, window, &CreateGCAux::new().foreground(0x008d_ef11))
            .ok()?;
        let atom =
            |n: &[u8]| -> Option<u32> { Some(conn.intern_atom(false, n).ok()?.reply().ok()?.atom) };
        let ticks_atom = atom(b"_CANTUS_TEST_TICKS")?;
        let exposed_atom = atom(b"_CANTUS_TEST_EXPOSED")?;
        conn.flush().ok()?;
        Some(Editor {
            conn,
            window,
            gc,
            ticks_atom,
            exposed_atom,
            ticks: 0,
            exposed: 0,
            timer: None,
            fd: None,
            name,
        })
    })() else {
        return 0;
    };
    let mut editor = Box::new(made);
    if let Some(fds) = host_ext::<clap_host_posix_fd_support>(host, CLAP_EXT_POSIX_FD_SUPPORT) {
        let fd = editor.conn.stream().as_raw_fd();
        if fds
            .register_fd
            .is_some_and(|f| f(host, fd, CLAP_POSIX_FD_READ))
        {
            editor.fd = Some(fd);
        }
    }
    if let Some(timers) = host_ext::<clap_host_timer_support>(host, CLAP_EXT_TIMER_SUPPORT) {
        let mut id: clap_id = 0;
        if timers.register_timer.is_some_and(|f| f(host, 20, &mut id)) {
            editor.timer = Some(id);
        }
    }
    Box::into_raw(editor) as isize
}

/// # Safety
/// As [`of`].
pub unsafe fn show(editor: isize, on: bool) -> bool {
    let Some(e) = of(editor) else {
        return false;
    };
    let _ = if on {
        e.conn.map_window(e.window)
    } else {
        e.conn.unmap_window(e.window)
    };
    e.conn.flush().is_ok()
}

/// Unregister, destroy the window, free the editor.
///
/// # Safety
/// As [`of`]; `editor` is not used again.
pub unsafe fn close(host: *const clap_host, editor: isize) {
    let Some(e) = of(editor) else {
        return;
    };
    if let Some(fd) = e.fd.take() {
        if let Some(fds) = host_ext::<clap_host_posix_fd_support>(host, CLAP_EXT_POSIX_FD_SUPPORT) {
            if let Some(f) = fds.unregister_fd {
                f(host, fd);
            }
        }
    }
    if let Some(id) = e.timer.take() {
        if let Some(t) = host_ext::<clap_host_timer_support>(host, CLAP_EXT_TIMER_SUPPORT) {
            if let Some(f) = t.unregister_timer {
                f(host, id);
            }
        }
    }
    let _ = e.conn.destroy_window(e.window);
    let _ = e.conn.flush();
    drop(Box::from_raw(editor as *mut Editor));
}

impl Editor {
    fn mark(&self, atom: u32, value: u32) {
        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            self.window,
            atom,
            AtomEnum::CARDINAL,
            &[value],
        );
        let _ = self.conn.flush();
    }

    fn draw(&mut self) {
        let _ = self
            .conn
            .change_gc(self.gc, &ChangeGCAux::new().foreground(0x008d_ef11));
        let bar = Rectangle {
            x: 16,
            y: 16,
            width: WIDTH - 32,
            height: 8,
        };
        let _ = self.conn.poly_fill_rectangle(self.window, self.gc, &[bar]);
        let _ = self
            .conn
            .image_text8(self.window, self.gc, 16, 48, self.name.as_bytes());
        self.exposed += 1;
        self.mark(self.exposed_atom, self.exposed);
    }
}

unsafe extern "C" fn on_timer(p: *const clap_sys::plugin::clap_plugin, _id: clap_id) {
    let me = crate::plugin::me(p);
    if let Some(e) = of(me.editor) {
        e.ticks += 1;
        e.mark(e.ticks_atom, e.ticks);
    }
    let secs = me.fault.take_window_pump();
    if secs > 0 {
        crate::fault::pump(std::time::Duration::from_secs(u64::from(secs)));
    }
    if me.fault.take_window_segv() {
        // SAFETY: none — a write to an address no process maps, on purpose.
        unsafe { std::ptr::write_volatile(std::ptr::without_provenance_mut::<u8>(16), 1) }
    }
}

unsafe extern "C" fn on_fd(
    p: *const clap_sys::plugin::clap_plugin,
    _fd: i32,
    _flags: clap_posix_fd_flags,
) {
    let me = crate::plugin::me(p);
    let Some(e) = of(me.editor) else {
        return;
    };
    let mut expose = false;
    while let Ok(Some(event)) = e.conn.poll_for_event() {
        if let Event::Expose(x) = event {
            expose |= x.count == 0;
        }
    }
    if expose {
        e.draw();
    }
}
