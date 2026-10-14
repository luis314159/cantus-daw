//! The plug-in's own editor in a window of its own (Windows): a top-level window the
//! plug-in's view is a child of, sized to the editor, gone when it is closed.
#![cfg(windows)]

mod support;
use support::*;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, FindWindowExW, GetAncestor, GetClientRect, GetWindow,
    GetWindowRect, IsIconic, IsWindow, IsWindowVisible, IsZoomed, SetWindowPos, ShowWindow,
    WindowFromPoint, GA_ROOT, GW_HWNDPREV, HWND_TOP, SWP_NOMOVE, SWP_NOSIZE, SW_MAXIMIZE,
    SW_MINIMIZE, SW_RESTORE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[test]
fn the_editor_opens_embedded_in_a_window_of_its_own_and_closes() {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    assert!(rig.host.hosted(id).unwrap().has_editor(), "it offers one");
    rig.host
        .open_window(id, "Cantus Test Gain — test", None)
        .expect("the window opens");
    let hosted = rig.host.hosted(id).unwrap();
    assert!(hosted.window_open());
    let hwnd = hosted.window_handle().expect("a native window") as HWND;
    let class = wide("STATIC");
    // SAFETY: plain Win32 queries of windows this test made.
    unsafe {
        let child = FindWindowExW(hwnd, std::ptr::null_mut(), class.as_ptr(), std::ptr::null());
        assert!(
            !child.is_null(),
            "the plug-in's view is a child of the window"
        );
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(hwnd, &mut rect);
        assert_eq!(
            (rect.right - rect.left, rect.bottom - rect.top),
            (320, 200),
            "the window is the editor's size"
        );
    }
    // A second open only brings it forward.
    rig.host.open_window(id, "again", None).expect("still open");
    assert!(rig.host.close_window(id));
    assert!(!rig.host.hosted(id).unwrap().window_open());
    // SAFETY: as above.
    assert_eq!(unsafe { IsWindow(hwnd) }, 0, "the window is gone");
    // The plug-in carries on without it.
    rig.settle();
    assert!(rig.host.hosted(id).is_some());
}

/// Whether `a` is above `b` in the windows' z-order (`GW_HWNDPREV` walks up from `b`).
fn above(a: HWND, b: HWND) -> bool {
    // SAFETY: plain Win32 queries of windows this test made.
    unsafe {
        let mut w = GetWindow(b, GW_HWNDPREV);
        while !w.is_null() {
            if w == a {
                return true;
            }
            w = GetWindow(w, GW_HWNDPREV);
        }
    }
    false
}

/// Round 27 (ADR 0051): *Open editor* on an editor already open brings it back — restored
/// when it was minimised, still maximised when it was maximised (`SW_RESTORE` alone would
/// un-maximise it), and above the other windows.
#[test]
fn a_second_open_restores_raises_and_keeps_a_maximised_editor_maximised() {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, host_with_plugin(shared_folder()));
    let id = rig.insert_plugin(strip);
    rig.host
        .open_window(id, "Cantus Test Gain — test", None)
        .expect("the window opens");
    let hwnd = rig.host.hosted(id).unwrap().window_handle().unwrap() as HWND;
    // SAFETY (every block below): plain Win32 calls on windows this test made.
    unsafe {
        ShowWindow(hwnd, SW_MINIMIZE);
        assert_ne!(IsIconic(hwnd), 0, "minimised");
    }
    assert!(
        !rig.host.hosted(id).unwrap().window_frontmost(),
        "minimised: not in front"
    );
    rig.host.open_window(id, "again", None).expect("still open");
    unsafe {
        assert_eq!(IsIconic(hwnd), 0, "restored");
        assert_ne!(IsWindowVisible(hwnd), 0, "shown");
        ShowWindow(hwnd, SW_MAXIMIZE);
        assert_ne!(IsZoomed(hwnd), 0, "maximised");
    }
    rig.host.open_window(id, "again", None).expect("still open");
    unsafe {
        assert_ne!(IsZoomed(hwnd), 0, "still maximised");
        ShowWindow(hwnd, SW_RESTORE);
    }
    // Another window above it: the open puts the editor back on top.
    // A button: a static control is transparent to the hit test.
    let class = wide("BUTTON");
    let other = unsafe {
        let w = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            400,
            300,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        assert!(!w.is_null());
        SetWindowPos(w, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        w
    };
    assert!(above(other, hwnd), "the other window on top");
    rig.host.open_window(id, "again", None).expect("still open");
    assert!(above(hwnd, other), "the editor back on top");
    // In front only when nothing else is over its middle (another program's window may
    // be, on a desktop in use: then that half is not asserted).
    let editor = rig.host.hosted(id).unwrap();
    if top_at_middle(hwnd) == hwnd {
        assert!(editor.window_frontmost(), "on top: in front");
    }
    // Our other window moved over it, on top: not in front.
    unsafe {
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(hwnd, &mut r);
        SetWindowPos(
            other,
            HWND_TOP,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            0,
        );
    }
    assert!(!editor.window_frontmost(), "covered: not in front");
    unsafe {
        DestroyWindow(other);
    }
    assert!(rig.host.close_window(id));
}

/// The top-level window at `w`'s middle, whatever program it belongs to.
fn top_at_middle(w: HWND) -> HWND {
    // SAFETY: plain Win32 queries.
    unsafe {
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(w, &mut r);
        let p = POINT {
            x: (r.left + r.right) / 2,
            y: (r.top + r.bottom) / 2,
        };
        GetAncestor(WindowFromPoint(p), GA_ROOT)
    }
}
