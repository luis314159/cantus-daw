//! The plug-in's own editor in a window of its own on Linux (ADR 0065): a top-level X11
//! window the plug-in's view is a child of, sized to the editor, its toolkit's descriptor
//! and timer served by the host's run loop, closed by its close box, gone when closed.
//!
//! Needs an X server (`DISPLAY`): WSLg's, a desktop's, or `xvfb-run` in CI. Without one the
//! tests say so and pass — CI runs them under Xvfb.
#![cfg(target_os = "linux")]

mod support;
use support::*;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask};
use x11rb::rust_connection::RustConnection;

fn display() -> Option<RustConnection> {
    std::env::var_os("DISPLAY")?;
    match x11rb::connect(None) {
        Ok((c, _)) => Some(c),
        Err(e) => {
            eprintln!("no X server ({e}): the X11 window tests are skipped");
            None
        }
    }
}

fn atom(c: &RustConnection, name: &[u8]) -> u32 {
    c.intern_atom(false, name).unwrap().reply().unwrap().atom
}

fn cardinal(c: &RustConnection, window: u32, name: &[u8]) -> u32 {
    let a = atom(c, name);
    c.get_property(false, window, a, AtomEnum::CARDINAL, 0, 1)
        .unwrap()
        .reply()
        .ok()
        .and_then(|r| r.value32().and_then(|mut v| v.next()))
        .unwrap_or(0)
}

fn name_of(c: &RustConnection, window: u32) -> String {
    c.get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 256)
        .unwrap()
        .reply()
        .map(|r| String::from_utf8_lossy(&r.value).into_owned())
        .unwrap_or_default()
}

/// Frames of the application for `ms` milliseconds (each serves the plug-ins' run loops).
fn frames(rig: &mut Rig, ms: u64) {
    let started = std::time::Instant::now();
    while started.elapsed() < std::time::Duration::from_millis(ms) {
        rig.block(256);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn the_editor_opens_in_an_x11_window_runs_its_loop_and_closes() {
    let Some(x) = display() else { return };
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    assert!(
        rig.host.hosted(id).unwrap().has_editor(),
        "it offers one on X11"
    );
    rig.host
        .open_window(id, "Cantus Test Gain — test", None)
        .expect("the window opens");
    let window = rig
        .host
        .hosted(id)
        .unwrap()
        .window_handle()
        .expect("a window") as u32;
    // The plug-in's view is a child of our window, and the window is the editor's size.
    let tree = x.query_tree(window).unwrap().reply().unwrap();
    let view = *tree
        .children
        .iter()
        .find(|c| name_of(&x, **c) == "Cantus Test Gain")
        .expect("the plug-in's view is a child of the window");
    let g = x.get_geometry(window).unwrap().reply().unwrap();
    assert_eq!(
        (g.width, g.height),
        (320, 200),
        "the window is the editor's size"
    );
    assert_eq!(name_of(&x, window), "Cantus Test Gain — test");
    // Its timer and its connection's descriptor are served by the host's frames: the view
    // counts timer calls, and draws when its expose event is read through the descriptor.
    frames(&mut rig, 300);
    let ticks = cardinal(&x, view, b"_CANTUS_TEST_TICKS");
    assert!(ticks >= 5, "its 20 ms timer ran: {ticks}");
    assert!(
        cardinal(&x, view, b"_CANTUS_TEST_EXPOSED") >= 1,
        "it drew on its expose, read through the run loop"
    );
    // A second open only brings it forward.
    rig.host.open_window(id, "again", None).expect("still open");
    assert!(rig.host.close_window(id));
    assert!(!rig.host.hosted(id).unwrap().window_open());
    x.get_input_focus().unwrap().reply().unwrap();
    assert!(
        x.get_geometry(window).unwrap().reply().is_err(),
        "the window is gone"
    );
    // The timer went with the editor.
    frames(&mut rig, 60);
    rig.settle();
    assert!(rig.host.hosted(id).is_some(), "the plug-in carries on");
}

#[test]
fn its_close_box_closes_it() {
    let Some(x) = display() else { return };
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    rig.host
        .open_window(id, "Cantus Test Gain — close", None)
        .expect("the window opens");
    let window = rig.host.hosted(id).unwrap().window_handle().unwrap() as u32;
    // What a window manager sends when its close box is pressed.
    let protocols = atom(&x, b"WM_PROTOCOLS");
    let delete = atom(&x, b"WM_DELETE_WINDOW");
    let event = ClientMessageEvent::new(32, window, protocols, [delete, 0, 0, 0, 0]);
    x.send_event(false, window, EventMask::NO_EVENT, event)
        .unwrap();
    x.flush().unwrap();
    frames(&mut rig, 100);
    assert!(
        !rig.host.hosted(id).unwrap().window_open(),
        "closed by its close box"
    );
}
