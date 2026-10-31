//! The MCP server in the application (round 34, ADR 0081): the server and its UI-thread
//! pump made at start-up ([`wire`], from `CantusApp::assemble` — the boot's path), the pump
//! turned each frame ([`frame`]) with the application as the tools' host, the HTTP door
//! opened while *Allow external agents* is on ([`door`]), the user's AI settings, and what
//! the window shows of agents: the confirmation card ([`card`]), the top bar's chip
//! ([`chip`]), the AI activity ([`activity`]) and Settings ▸ AI ([`settings_page`], with
//! *Copy setup for…*, [`setup`]).
//!
//! A request reaches the project like this: a transport thread (the HTTP door, which the
//! stdio bridge reaches too; the in-process client in tests) calls
//! `cantus_mcp::Server::handle`; a tool call goes through the door to this window, which is
//! woken; the frame's pump decides by class, plans the tool's command against `app.bus`,
//! asks the user on the card when the class says so, and executes it with `Origin::Mcp` —
//! one undo step — through [`host`]'s `execute`, which keeps the same holds as
//! `CantusApp::exec`.

#[path = "views/ai_activity.rs"]
pub mod activity;
#[path = "views/ai_card.rs"]
pub mod card;
#[path = "ai_catalog.rs"]
pub mod catalog;
#[path = "views/ai_chip.rs"]
pub mod chip;
#[path = "ai_door.rs"]
pub mod door;
#[path = "ai_host.rs"]
mod host;
#[path = "views/ai_settings.rs"]
pub mod settings_page;
#[path = "ai_setup.rs"]
pub mod setup;

use crate::app::CantusApp;
use crate::i18n::tf;
use cantus_core::{HistoryEntry, Origin};
use cantus_mcp::{Agents, Catalog, Policy, Server, ServerInfo, UndoAll};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// The user's AI choices (*Settings ▸ AI*), kept in the settings file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    /// The whole AI area: off refuses every agent's call and hides the area (the chip,
    /// the card, the activity, Settings ▸ AI but its switch).
    pub enabled: bool,
    /// External agents may connect (the HTTP door and the stdio bridge). Off by default.
    pub external_agents: bool,
    /// The door's port on `127.0.0.1` (`cantus_mcp::home::DEFAULT_PORT`; 0: any free one).
    pub port: u16,
    /// Agents' edits run without asking.
    pub allow_edits: bool,
    /// Agents may ask to read or write files (always asked, inside the project's folder
    /// and the folders below).
    pub allow_files: bool,
    /// Folders the user allowed file tools to use, besides the project's own.
    pub folders: Vec<std::path::PathBuf>,
    /// Tools the user refused by name.
    pub denied_tools: Vec<String>,
}

impl Default for AiSettings {
    fn default() -> Self {
        AiSettings {
            enabled: true,
            external_agents: false,
            port: cantus_mcp::home::DEFAULT_PORT,
            allow_edits: false,
            allow_files: false,
            folders: Vec::new(),
            denied_tools: Vec::new(),
        }
    }
}

impl AiSettings {
    /// What the pump decides by.
    pub fn policy(&self) -> Policy {
        Policy {
            enabled: self.enabled,
            external_agents: self.external_agents,
            allow_edits: self.allow_edits,
            allow_files: self.allow_files,
            folders: self.folders.clone(),
            denied: self.denied_tools.clone(),
        }
    }
}

/// The server, its pump and its door, and the views' own state.
#[derive(Default)]
pub struct AiState {
    wired: Option<Wired>,
    /// The HTTP door, open while the user allows external agents.
    pub door: door::DoorState,
    /// The AI activity's panel is shown.
    pub activity_open: bool,
    /// The activity's own state (its scroll).
    pub activity: activity::ActivityView,
    /// Settings ▸ AI's fields and its last word ("Copied", an error).
    pub page: settings_page::PageState,
    /// The call whose card is in front, and since when (review 5 #4: a press counts only
    /// on a card shown for a moment).
    pub card_front: Option<(u64, Instant)>,
}

struct Wired {
    server: Server,
    agents: Agents,
}

impl AiState {
    /// Whether start-up made the server (it does, in `assemble`).
    pub fn is_wired(&self) -> bool {
        self.wired.is_some()
    }

    /// The server every transport serves through.
    pub fn server(&self) -> Option<&Server> {
        self.wired.as_ref().map(|w| &w.server)
    }

    /// The pump's state: confirmations, activity, jobs, agents' steps.
    pub fn agents(&self) -> Option<&Agents> {
        self.wired.as_ref().map(|w| &w.agents)
    }

    pub fn agents_mut(&mut self) -> Option<&mut Agents> {
        self.wired.as_mut().map(|w| &mut w.agents)
    }
}

/// Make the server and its pump, waking `ctx` whenever a request waits. Called from
/// `CantusApp::assemble`.
pub fn wire(app: &mut CantusApp, ctx: &egui::Context) {
    wire_with(app, ctx, catalog::app());
}

/// [`wire`] over another catalogue (tests add tools of their own).
pub fn wire_with(app: &mut CantusApp, ctx: &egui::Context, catalog: Catalog) {
    let ctx = ctx.clone();
    let (server, agents) = cantus_mcp::start(
        ServerInfo::cantus(crate::app::APP_VERSION),
        catalog,
        Arc::new(move || ctx.request_repaint()),
    );
    app.ai.wired = Some(Wired { server, agents });
}

/// Run `f` with the pump taken out of `app.ai` (it needs `app` as its host). `None` when
/// the server is not wired.
fn with_agents<R>(
    app: &mut CantusApp,
    f: impl FnOnce(&mut Agents, &mut CantusApp) -> R,
) -> Option<R> {
    let mut wired = app.ai.wired.take()?;
    let r = f(&mut wired.agents, app);
    app.ai.wired = Some(wired);
    Some(r)
}

/// Serve what waits, once. While it runs the pump is out of `app.ai` (a tool that
/// downcasts the host sees `app.ai` empty).
pub fn pump(app: &mut CantusApp) {
    let _ = with_agents(app, |agents, app| agents.pump(app));
}

/// The frame's part: the door opened or closed as the settings say, the sessions it
/// ended forgotten, the calls served, a frame asked for by the pump's next deadline, and
/// the activity panel when it is open.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    door::sync(app);
    for id in app.ai.door.take_ended() {
        let _ = with_agents(app, |agents, app| agents.revoke(app, &id));
    }
    pump(app);
    card::note_front(app, Instant::now());
    if let Some(after) = app.ai.agents().and_then(|a| a.next_wake(Instant::now())) {
        crate::views::workspace_ticker::wake_after(ctx, after);
    }
    // The card counts its wait down by the second.
    if app.ai.agents().is_some_and(|a| a.asks().next().is_some()) {
        crate::views::workspace_ticker::wake_after(ctx, std::time::Duration::from_secs(1));
    }
    if app.settings.ai.enabled && app.ai.activity_open {
        activity::show(app, ctx);
    }
}

/// The history's name for a step: an agent's reads "AI: <what it did>" — its change's
/// label, or what its call said.
pub fn history_name(app: &CantusApp, entry: &HistoryEntry, described: String) -> String {
    if entry.origin != Origin::Mcp {
        return described;
    }
    let what = app
        .ai
        .agents()
        .and_then(|a| a.step_words(entry.step()))
        .unwrap_or(described);
    tf("ai.history.step", &[("what", &what)])
}

/// Disconnect an agent: the pump forgets it (grants, changes, jobs, confirmations) and
/// the door ends its session, so its next request must connect again — and is asked again.
pub fn disconnect(app: &mut CantusApp, key: &str) {
    app.ai.door.end_session(key);
    let _ = with_agents(app, |agents, app| agents.revoke(app, key));
}

/// *Undo all of this agent's changes*, and what to say of it.
pub fn undo_all(app: &mut CantusApp, key: &str) -> Option<UndoAll> {
    with_agents(app, |agents, app| agents.undo_all(app, key))
}

/// The agents to show as connected now (none while the AI area is off).
pub fn connected(app: &CantusApp) -> Vec<cantus_mcp::Caller> {
    if !app.settings.ai.enabled {
        return Vec::new();
    }
    app.ai
        .agents()
        .map(|a| a.connected(Instant::now()))
        .unwrap_or_default()
}
