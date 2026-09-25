//! The plug-in's own editor in a window of its own (Windows): a top-level window the
//! plug-in's view is a child of, sized to the editor, gone when it is closed.
#![cfg(windows)]

mod support;
use support::*;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetClientRect, IsWindow};

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
