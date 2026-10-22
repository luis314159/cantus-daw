//! Linux: a top-level X11 window of ours that the plug-in's view embeds into (CLAP `x11`,
//! VST3 `X11EmbedWindowID`), ADR 0065. Its own connection to the X server (pure Rust,
//! `x11rb`); the plug-in makes its view a child of this window through its own Xlib or XCB
//! connection, so the window is created and synced before the editor is opened.
//!
//! A plug-in cannot embed into a Wayland surface: when Cantus runs on Wayland the window
//! goes through XWayland, and Cantus's own window does too (`cantus-ui` asks winit for X11
//! unless `CANTUS_WAYLAND=1`), so the editor can be transient for it and stay above it.
//!
//! Nothing dispatches this connection's events for us (on Windows the message loop does):
//! [`PluginWindow::closed`] and [`PluginWindow::take_resize`], which the frame loop asks
//! every frame, read them first.

use super::*;
use std::cell::{Cell, RefCell};
use x11rb::connection::Connection;
use x11rb::properties::WmSizeHints;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux,
    EventMask, MapState, PropMode, StackMode, Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        WM_STATE,
        UTF8_STRING,
        _NET_WM_NAME,
        _NET_WM_STATE,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_ABOVE,
        _NET_ACTIVE_WINDOW,
        _NET_CLIENT_LIST_STACKING,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_PID,
    }
}

/// `WM_STATE`'s value for a window minimised to the taskbar (ICCCM 4.1.3.1).
const ICONIC_STATE: u32 = 3;

fn failed(step: &str, e: impl std::fmt::Display) -> HostError {
    HostError::Editor(format!("X11 {step}: {e}"))
}

pub struct PluginWindow {
    conn: RustConnection,
    root: Window,
    window: Window,
    atoms: Atoms,
    /// The user pressed the window's close box (`WM_DELETE_WINDOW`), or it was destroyed.
    closed: Cell<bool>,
    /// A size the user dragged the window to, not yet taken.
    resize: Cell<Option<(u32, u32)>>,
    /// The size the window has now, as the server last said or we last set.
    size: Cell<(u32, u32)>,
    /// Sizes we asked for whose `ConfigureNotify` has not come back yet: those are not the
    /// user's and are not offered back to the editor.
    fitting: RefCell<Vec<(u32, u32)>>,
    /// The editor cannot be resized: the window's minimum and maximum are its size.
    fixed: Cell<bool>,
    /// The plug-in floats a window of its own; ours stays unmapped.
    floating: bool,
}

impl PluginWindow {
    /// Ours for a plug-in whose editor floats a window of its own that opened later than
    /// asked: never mapped; it stands for theirs, which closing ours closes.
    pub fn floating(title: &str) -> Option<PluginWindow> {
        let mut w = Self::create(title, None).ok()?;
        w.floating = true;
        Some(w)
    }

    /// Create the window, embed the editor in it, size it to the editor, show it.
    pub fn open(
        title: &str,
        owner: Option<RawWindowHandle>,
        main: &mut dyn MainThread,
    ) -> Result<PluginWindow, HostError> {
        let mut window = Self::create(title, owner)?;
        let parent = RawWindowHandle::Xlib(raw_window_handle::XlibWindowHandle::new(
            window.window as std::ffi::c_ulong,
        ));
        let size = main.open_editor(parent)?;
        if main.editor_is_floating() {
            window.floating = true;
            return Ok(window);
        }
        window.fixed.set(!main.editor_can_resize());
        window.fit(size);
        window
            .conn
            .map_window(window.window)
            .map_err(|e| failed("map", e))?;
        window.conn.flush().map_err(|e| failed("flush", e))?;
        Ok(window)
    }

    /// The window, unmapped, with no editor in it yet — known to the server when this
    /// returns, so the plug-in's own connection can parent its view to it at once.
    fn create(title: &str, owner: Option<RawWindowHandle>) -> Result<PluginWindow, HostError> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| failed("connect", e))?;
        let screen = conn.setup().roots[screen].clone();
        let atoms = Atoms::new(&conn)
            .map_err(|e| failed("atoms", e))?
            .reply()
            .map_err(|e| failed("atoms", e))?;
        let window = conn.generate_id().map_err(|e| failed("id", e))?;
        let (width, height) = (640u16, 480u16);
        let aux = CreateWindowAux::new()
            .background_pixel(screen.black_pixel)
            .event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE);
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &aux,
        )
        .map_err(|e| failed("create", e))?;
        let set = || -> Result<(), x11rb::errors::ConnectionError> {
            conn.change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                title.as_bytes(),
            )?;
            conn.change_property8(
                PropMode::REPLACE,
                window,
                atoms._NET_WM_NAME,
                atoms.UTF8_STRING,
                title.as_bytes(),
            )?;
            conn.change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                b"cantus-plugin\0Cantus\0",
            )?;
            conn.change_property32(
                PropMode::REPLACE,
                window,
                atoms.WM_PROTOCOLS,
                AtomEnum::ATOM,
                &[atoms.WM_DELETE_WINDOW],
            )?;
            conn.change_property32(
                PropMode::REPLACE,
                window,
                atoms._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                &[atoms._NET_WM_WINDOW_TYPE_NORMAL],
            )?;
            conn.change_property32(
                PropMode::REPLACE,
                window,
                atoms._NET_WM_PID,
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )?;
            // Above Cantus's window, minimised and restored with it.
            if let Some(owner) = owner_window(owner) {
                conn.change_property32(
                    PropMode::REPLACE,
                    window,
                    AtomEnum::WM_TRANSIENT_FOR,
                    AtomEnum::WINDOW,
                    &[owner],
                )?;
            }
            Ok(())
        };
        set().map_err(|e| failed("properties", e))?;
        // A round trip: the window exists on the server before the plug-in is told of it.
        conn.get_input_focus()
            .map_err(|e| failed("sync", e))?
            .reply()
            .map_err(|e| failed("sync", e))?;
        Ok(PluginWindow {
            root: screen.root,
            conn,
            window,
            atoms,
            closed: Cell::new(false),
            resize: Cell::new(None),
            size: Cell::new((width.into(), height.into())),
            fitting: RefCell::new(Vec::new()),
            fixed: Cell::new(false),
            floating: false,
        })
    }

    /// Size the window so its inside is `size` (X11 windows have no frame of their own: the
    /// window manager's goes around it).
    pub fn fit(&mut self, size: EditorSize) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        let (w, h) = (size.width.min(16_384), size.height.min(16_384));
        // The size asked of the window manager too (it decides a top-level's size): the
        // program's size, and min = max for an editor that cannot be resized.
        let mut hints = WmSizeHints::new();
        hints.size = Some((
            x11rb::properties::WmSizeHintsSpecification::ProgramSpecified,
            w as i32,
            h as i32,
        ));
        if self.fixed.get() {
            hints.min_size = Some((w as i32, h as i32));
            hints.max_size = Some((w as i32, h as i32));
        }
        if let Ok(c) = hints.set_normal_hints(&self.conn, self.window) {
            c.ignore_error();
        }
        if self.size.get() == (w, h) {
            return;
        }
        self.fitting.borrow_mut().push((w, h));
        let aux = ConfigureWindowAux::new().width(w).height(h);
        if let Ok(c) = self.conn.configure_window(self.window, &aux) {
            c.ignore_error();
        }
        self.size.set((w, h));
        let _ = self.conn.flush();
    }

    /// Bring the window to the front: mapped (restored if it was minimised), raised, and
    /// made the active window through the window manager (EWMH `_NET_ACTIVE_WINDOW`, from
    /// an application). A window kept unmapped for a plug-in that floats its own is left.
    pub fn focus(&self) {
        if self.floating {
            return;
        }
        let _ = self.conn.map_window(self.window);
        let _ = self.conn.configure_window(
            self.window,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        );
        let event = ClientMessageEvent::new(
            32,
            self.window,
            self.atoms._NET_ACTIVE_WINDOW,
            [1u32, 0, 0, 0, 0],
        );
        let _ = self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        let _ = self.conn.flush();
    }

    /// Whether the window is minimised (ICCCM `WM_STATE` iconic, or EWMH hidden).
    pub fn is_minimised(&self) -> bool {
        if self.floating {
            return false;
        }
        let iconic = self
            .property32(self.window, self.atoms.WM_STATE, self.atoms.WM_STATE)
            .first()
            == Some(&ICONIC_STATE);
        iconic || self.has_state(self.window, self.atoms._NET_WM_STATE_HIDDEN)
    }

    /// The plug-in floats a window of its own (ours stays unmapped).
    pub fn is_floating(&self) -> bool {
        self.floating
    }

    /// Whether the window is in front as the user sees it: mapped, not minimised, and no
    /// other window above it in the window manager's stacking order covers its middle —
    /// windows transient for ours (the plug-in's own dialogs) and always-on-top ones do not
    /// count. A window manager without `_NET_CLIENT_LIST_STACKING` gives no order: in front.
    pub fn is_frontmost(&self) -> bool {
        if self.floating {
            return true;
        }
        if self.is_minimised() || !self.viewable(self.window) {
            return false;
        }
        let Some((x, y, w, h)) = self.rect(self.window) else {
            return true;
        };
        let middle = (x + w as i32 / 2, y + h as i32 / 2);
        let stacking = self.property32(
            self.root,
            self.atoms._NET_CLIENT_LIST_STACKING,
            AtomEnum::WINDOW.into(),
        );
        let Some(at) = stacking.iter().position(|&c| c == self.window) else {
            return true;
        };
        !stacking[at + 1..].iter().any(|&other| {
            let transient = self.property32(
                other,
                AtomEnum::WM_TRANSIENT_FOR.into(),
                AtomEnum::WINDOW.into(),
            );
            if transient.first() == Some(&self.window) {
                return false;
            }
            if self.has_state(other, self.atoms._NET_WM_STATE_ABOVE)
                || self.has_state(other, self.atoms._NET_WM_STATE_HIDDEN)
                || !self.viewable(other)
            {
                return false;
            }
            self.rect(other).is_some_and(|(ox, oy, ow, oh)| {
                middle.0 >= ox
                    && middle.0 < ox + ow as i32
                    && middle.1 >= oy
                    && middle.1 < oy + oh as i32
            })
        })
    }

    /// The user closed the window (its close box), once the events so far are read.
    pub fn closed(&self) -> bool {
        self.pump();
        self.closed.get()
    }

    /// A size the user dragged the window to, once.
    pub fn take_resize(&mut self) -> Option<EditorSize> {
        self.pump();
        self.resize
            .take()
            .map(|(width, height)| EditorSize { width, height })
    }

    /// The native window (the X11 window id), for tests and diagnostics.
    pub fn handle(&self) -> Option<isize> {
        Some(self.window as isize)
    }

    /// Read the events that arrived: the close box, sizes, the window going away.
    fn pump(&self) {
        while let Ok(Some(event)) = self.conn.poll_for_event() {
            match event {
                Event::ClientMessage(e)
                    if e.window == self.window
                        && e.type_ == self.atoms.WM_PROTOCOLS
                        && e.data.as_data32()[0] == self.atoms.WM_DELETE_WINDOW =>
                {
                    self.closed.set(true);
                }
                Event::DestroyNotify(e) if e.window == self.window => self.closed.set(true),
                Event::ConfigureNotify(e) if e.window == self.window => {
                    let size = (u32::from(e.width), u32::from(e.height));
                    let ours = {
                        let mut fitting = self.fitting.borrow_mut();
                        match fitting.iter().position(|s| *s == size) {
                            Some(i) => {
                                fitting.drain(..=i);
                                true
                            }
                            None => false,
                        }
                    };
                    if !ours && size != self.size.get() && size.0 > 0 && size.1 > 0 {
                        self.resize.set(Some(size));
                    }
                    self.size.set(size);
                }
                _ => {}
            }
        }
    }

    fn property32(&self, window: Window, property: u32, kind: u32) -> Vec<u32> {
        self.conn
            .get_property(false, window, property, kind, 0, 4096)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().map(|v| v.collect()))
            .unwrap_or_default()
    }

    fn has_state(&self, window: Window, state: u32) -> bool {
        self.property32(window, self.atoms._NET_WM_STATE, AtomEnum::ATOM.into())
            .contains(&state)
    }

    fn viewable(&self, window: Window) -> bool {
        self.conn
            .get_window_attributes(window)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|a| a.map_state == MapState::VIEWABLE)
    }

    /// A window's place on the screen (root coordinates) and size.
    fn rect(&self, window: Window) -> Option<(i32, i32, u32, u32)> {
        let geometry = self.conn.get_geometry(window).ok()?.reply().ok()?;
        let at = self
            .conn
            .translate_coordinates(window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some((
            at.dst_x.into(),
            at.dst_y.into(),
            geometry.width.into(),
            geometry.height.into(),
        ))
    }
}

/// The X11 window of Cantus's own window, when it has one (on X11, or XWayland).
fn owner_window(owner: Option<RawWindowHandle>) -> Option<Window> {
    match owner? {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok().filter(|w| *w != 0),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

impl Drop for PluginWindow {
    fn drop(&mut self) {
        let _ = self.conn.destroy_window(self.window);
        let _ = self.conn.flush();
    }
}
