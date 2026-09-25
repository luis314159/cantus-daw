//! The VST3 view: the same editor as CLAP's (`crate::editor`), in an `IPlugView`.
//!
//! The host gives a `HWND` in `attached`; the editor opens as its child. Edits go to the
//! host through the component handler — `beginEdit`, `performEdit` (normalised through
//! the knob's taper), `endEdit` — on the UI thread, as the standard wants, and into the
//! effect at once.

use super::timer::Notifier;
use crate::editor::{self, Edit, EditSink, Editor};
use crate::instance::Shared;
use ::vst3::Class;
use ::vst3::Steinberg::*;
use std::cell::RefCell;
use std::ffi::{c_void, CStr};
use std::sync::Arc;

pub struct View {
    shared: Arc<Shared>,
    notifier: Arc<Notifier>,
    editor: RefCell<Option<Editor>>,
}

// SAFETY: a view is created, used and destroyed on the host's UI thread.
unsafe impl Send for View {}
unsafe impl Sync for View {}

impl Class for View {
    type Interfaces = (IPlugView,);
}

impl View {
    pub fn new(shared: Arc<Shared>, notifier: Arc<Notifier>) -> View {
        View {
            shared,
            notifier,
            editor: RefCell::new(None),
        }
    }

    fn size(&self) -> (i32, i32) {
        if let Some(e) = self.editor.borrow().as_ref() {
            let (w, h) = e.physical_size();
            return (w as i32, h as i32);
        }
        let s = scale();
        (
            (editor::SIZE.0 * s).round() as i32,
            (editor::SIZE.1 * s).round() as i32,
        )
    }
}

fn scale() -> f64 {
    // SAFETY: a plain query.
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() };
    if dpi == 0 {
        1.0
    } else {
        dpi as f64 / 96.0
    }
}

/// The editor's edits to the VST3 host, through its component handler.
struct Sink {
    shared: Arc<Shared>,
    notifier: Arc<Notifier>,
}

// SAFETY: the sink lives in the editor, on the host's UI thread, where the handler is
// called.
unsafe impl Send for Sink {}

impl EditSink for Sink {
    fn edit(&mut self, edit: Edit) {
        if let Edit::Value(i, v) = edit {
            // Into the effect at once; the host echoes it through its parameter stream.
            self.shared.set_value(i as usize, v as f64);
        }
        self.notifier.edit(edit);
    }
}

impl IPlugViewTrait for View {
    unsafe fn isPlatformTypeSupported(&self, kind: FIDString) -> tresult {
        if !kind.is_null() && CStr::from_ptr(kind) == c"HWND" {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn attached(&self, parent: *mut c_void, kind: FIDString) -> tresult {
        if self.isPlatformTypeSupported(kind) != kResultTrue {
            return kResultFalse;
        }
        let Some(parent) = editor::win32_parent(parent) else {
            return kInvalidArgument;
        };
        let sink = Sink {
            shared: self.shared.clone(),
            notifier: self.notifier.clone(),
        };
        match Editor::open(self.shared.clone(), Box::new(sink), parent) {
            Ok(e) => {
                *self.editor.borrow_mut() = Some(e);
                kResultOk
            }
            Err(_) => kResultFalse,
        }
    }

    unsafe fn removed(&self) -> tresult {
        *self.editor.borrow_mut() = None;
        kResultOk
    }

    unsafe fn onWheel(&self, _distance: f32) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyDown(&self, _key: char16, _code: int16, _modifiers: int16) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyUp(&self, _key: char16, _code: int16, _modifiers: int16) -> tresult {
        kResultFalse
    }

    unsafe fn getSize(&self, size: *mut ViewRect) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        let (w, h) = self.size();
        *size = ViewRect {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        kResultOk
    }

    unsafe fn onSize(&self, _size: *mut ViewRect) -> tresult {
        kResultOk
    }

    unsafe fn onFocus(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setFrame(&self, _frame: *mut IPlugFrame) -> tresult {
        kResultOk
    }

    unsafe fn canResize(&self) -> tresult {
        kResultFalse
    }

    unsafe fn checkSizeConstraint(&self, rect: *mut ViewRect) -> tresult {
        if rect.is_null() {
            return kInvalidArgument;
        }
        let (w, h) = self.size();
        *rect = ViewRect {
            left: (*rect).left,
            top: (*rect).top,
            right: (*rect).left + w,
            bottom: (*rect).top + h,
        };
        kResultTrue
    }
}
