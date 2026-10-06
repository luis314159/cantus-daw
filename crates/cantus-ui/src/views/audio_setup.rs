//! Round 20's audio setup on screen, once a frame after the workspace: the popover over
//! the chip, the first-run setup, the routing page, and the device cards. The chip itself
//! is drawn by the status bar (`workspace_status`), which leaves it room.

use crate::app::CantusApp;

pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    // Coming back to Cantus (after installing a driver, say): the drivers are listed
    // again, so a driver installed meanwhile appears.
    let focused = ctx.input(|i| i.focused);
    let looking =
        app.audio_setup.drivers || app.audio_setup.first_run.is_some() || app.audio_setup.popover;
    if focused && !app.audio_setup.focused && looking {
        app.audio_setup.request_scan();
    }
    app.audio_setup.focused = focused;
    super::audio_routing::show(app, ctx);
    super::audio_notice::show(app, ctx);
    super::audio_popover::show(app, ctx);
    super::audio_firstrun::show(app, ctx);
    super::audio_drivers::show(app, ctx);
}
