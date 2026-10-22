//! The test editor on macOS (ADR 0066): a label of a fixed 320 × 200 points, added to the
//! `NSView` the host gives (CLAP `cocoa`). Shared by the gain and the synth (`#[path]`);
//! each names itself with its `TITLE`. AppKit runs on the main thread, where the gui
//! extension's calls come.

use super::{clap_window, HEIGHT, TITLE, WIDTH};
use crate::plugin::me;
use clap_sys::plugin::clap_plugin;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// The label `me.editor` holds (a retained pointer).
unsafe fn label(editor: isize) -> Option<&'static NSTextField> {
    (editor as *const NSTextField).as_ref()
}

pub unsafe extern "C" fn set_parent(p: *const clap_plugin, window: *const clap_window) -> bool {
    let me = me(p);
    if !me.editor_created || me.editor != 0 || window.is_null() {
        return false;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(parent) = ((*window).specific.cocoa as *const NSView).as_ref() else {
        return false;
    };
    let text = NSTextField::labelWithString(&NSString::from_str(TITLE), mtm);
    text.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(f64::from(WIDTH), f64::from(HEIGHT)),
    ));
    parent.addSubview(&text);
    me.editor = Retained::into_raw(text) as isize;
    true
}

pub unsafe extern "C" fn show(p: *const clap_plugin) -> bool {
    match label(me(p).editor) {
        Some(l) => {
            l.setHidden(false);
            true
        }
        None => false,
    }
}

pub unsafe extern "C" fn hide(p: *const clap_plugin) -> bool {
    match label(me(p).editor) {
        Some(l) => {
            l.setHidden(true);
            true
        }
        None => false,
    }
}

pub unsafe extern "C" fn destroy(p: *const clap_plugin) {
    let me = me(p);
    if me.editor != 0 {
        if let Some(l) = Retained::from_raw(me.editor as *mut NSTextField) {
            l.removeFromSuperview();
        }
    }
    me.editor = 0;
    me.editor_created = false;
}
