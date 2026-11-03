//! ARA 2 hosting's session (ADR 0097): the documents of the plug-ins a project's clips are
//! bound to, a bound plug-in instance per clip, the renders those clips play, the plug-in's
//! own editor for a clip, and the plug-in's edits as steps of Cantus's undo.
//!
//! **How a bound clip sounds.** Its plug-in instance is bound to the plug-in's document as
//! the playback renderer, editor renderer and editor view of the clip's region, **in
//! Cantus's own process** (ARA's objects cannot cross to the sandbox, ADR 0063; the
//! interface says so), and renders the region on the render thread ([`crate::ara_worker`])
//! — never the interface's, never the audio thread — into the clip's stretch of the source,
//! converted to the engine's rate there. The engine plays it in place of the source
//! (`cantus_engine::SampleCache::set_clip_renders`), so clip gain, fades and the mix apply
//! as to any clip, the bounce and the stems hear it, and offline equals real time. While a
//! new render is on its way the clip plays its last one; a newer edit cancels it; a clip
//! dragged is rendered once it rests ([`DEBOUNCE`]); an edit in the plug-in renders only the
//! part it says changed.
//!
//! **The plug-in's edits are Cantus's undo steps.** When the plug-in says a clip's content
//! changed and its state of the clip is not the one the project holds, the session hands
//! the window a `SetClipAraState` with the new archive ([`AraSession::take_edits`]), keyed so
//! the edits of one gesture (within [`GESTURE`]) merge into one step. Undo puts the
//! project's state back, and the session puts it back into the plug-in.
//!
//! **A plug-in that crashes or hangs.** Before every call into a plug-in that can take it
//! down (opening, binding, restoring, rendering) the session writes a marker naming it, and
//! clears it after; a Cantus that starts with the marker there opens the project with that
//! plug-in's clips playing their sources, says why, and offers *Retry*. A render that runs
//! past [`AraSession::set_hang_limit`] is given up: the plug-in is stopped for the session
//! and the window never waits for it.

#[path = "ara_session.rs"]
mod session;

pub use crate::ara_worker::{ClipRender, RENDER_BLOCK};
pub use cantus_ara::callbacks::PlaybackRequest;
pub use session::{AraSession, SourceState, MARKER_FILE};

use cantus_core::ara::AraPlugin;

use crate::backend::{HostError, PluginInfo};

/// How a bound clip stands.
#[derive(Clone, Debug, PartialEq)]
pub enum AraStatus {
    /// Its plug-in renders it.
    Live,
    /// Its source is still being read: it plays nothing yet, as any clip whose file loads.
    Loading,
    /// It plays its source: the plug-in is not installed (not in the database, and not at
    /// the path the project names).
    NotInstalled,
    /// It plays its source: the plug-in would not open, bind or restore (what it said).
    Failed(String),
    /// It plays its source: the plug-in crashed Cantus or stopped answering (the reason);
    /// *Retry* runs it again.
    Stopped(String),
    /// It plays its source: ARA plug-ins are off for this session.
    Off,
    /// It plays its source… which is not there either.
    NoSource,
    /// On a take lane: never heard, never rendered.
    OnTakeLane,
}

impl AraStatus {
    /// Whether the user can ask for the plug-in to run again.
    pub fn retryable(&self) -> bool {
        matches!(self, AraStatus::Stopped(_) | AraStatus::Failed(_))
    }
}

/// Edits of one clip by the plug-in closer together than this are one undo step.
pub const GESTURE: std::time::Duration = std::time::Duration::from_millis(700);

/// A region that changed (a clip dragged, trimmed) is rendered again once it has rested
/// this long.
pub const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(150);

/// A project's bound plug-in as the project stores it.
pub fn plugin_ref(info: &PluginInfo, factory_id: &str) -> AraPlugin {
    AraPlugin {
        id: info.id.clone(),
        name: info.name.clone(),
        vendor: info.vendor.clone(),
        path: info.path.to_string_lossy().into_owned(),
        factory_id: factory_id.to_string(),
    }
}

/// The factory id of an ARA plug-in (opens its module).
pub fn factory_id(info: &PluginInfo) -> Result<String, String> {
    crate::vst3::ara::open(&info.path, Some(&info.name))
        .map(|m| m.factory.factory_id())
        .map_err(|e: HostError| e.to_string())
}

/// The plug-ins a project's clips are bound to, by class id (upper case).
pub fn bound_plugins(
    project: &cantus_core::Project,
) -> std::collections::BTreeMap<String, AraPlugin> {
    let mut out = std::collections::BTreeMap::new();
    for t in &project.tracks {
        for c in &t.clips {
            if let Some(a) = c.audio.as_ref().and_then(|a| a.ara.as_ref()) {
                out.entry(a.plugin.id.to_ascii_uppercase())
                    .or_insert_with(|| a.plugin.clone());
            }
        }
    }
    out
}
