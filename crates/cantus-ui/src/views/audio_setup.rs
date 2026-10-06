//! Round 20's audio setup on screen, once a frame after the workspace: the popover over
//! the chip, the first-run setup, the routing page, and the device cards. The chip itself
//! is drawn by the status bar (`workspace_status`), which leaves it room.

use crate::app::CantusApp;

pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    super::audio_routing::show(app, ctx);
    super::audio_notice::show(app, ctx);
    super::audio_popover::show(app, ctx);
    super::audio_firstrun::show(app, ctx);
}
