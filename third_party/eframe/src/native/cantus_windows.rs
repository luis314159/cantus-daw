//! CANTUS PATCH (ADR 0151 §10): each window's winit handle, by its viewport, for the
//! application — so it can read where a window of its own is, place it in physical pixels
//! and own a plug-in's editor by the window that opened it. Kept up to date by the glow
//! integration after every pass's viewport output; only the event loop's thread sees it.

use std::cell::RefCell;
use std::sync::Arc;

use egui::ViewportId;
use winit::window::Window;

thread_local! {
    static WINDOWS: RefCell<Vec<(ViewportId, Arc<Window>)>> = const { RefCell::new(Vec::new()) };
}

/// The winit window of viewport `id`, while it exists (`None` off the event loop's thread,
/// before the window is made and after it is gone).
pub fn window_of(id: ViewportId) -> Option<Arc<Window>> {
    WINDOWS.with(|w| {
        w.borrow()
            .iter()
            .find(|(v, _)| *v == id)
            .map(|(_, win)| Arc::clone(win))
    })
}

/// Every viewport that has a window now (the integration calls it after each pass).
pub(crate) fn sync<'a>(now: impl Iterator<Item = (ViewportId, Option<&'a Arc<Window>>)>) {
    WINDOWS.with(|w| {
        let mut w = w.borrow_mut();
        w.clear();
        w.extend(now.filter_map(|(id, win)| win.map(|win| (id, Arc::clone(win)))));
    });
}
