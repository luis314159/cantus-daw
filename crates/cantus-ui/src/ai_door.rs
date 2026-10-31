//! The HTTP door in the application (ADR 0081 §9): open while the AI area is on and the
//! user allows external agents, on `127.0.0.1` at the port Settings ▸ AI names, with the
//! install's token; `mcp-door.json` written beside the token while it is open (what the
//! stdio bridge reads), removed when it closes. A port another program holds is said once
//! (in Settings ▸ AI) and not tried again every frame.

use crate::app::CantusApp;
use crate::i18n::tf;
use cantus_mcp::home::{self, DoorFile};
use cantus_mcp::http::{HttpConfig, HttpDoor};
use std::path::{Path, PathBuf};

/// The door, open, and what it wrote.
struct Open {
    door: HttpDoor,
    dir: PathBuf,
    /// The port the settings asked for (0: any).
    asked: u16,
}

impl Drop for Open {
    fn drop(&mut self) {
        home::remove_door(&self.dir);
    }
}

/// The door's state.
#[derive(Default)]
pub struct DoorState {
    open: Option<Open>,
    /// Why it could not open (the port is taken…), for Settings ▸ AI.
    pub error: Option<String>,
    /// The port a failed open asked for: not tried again until the settings change.
    failed: Option<u16>,
    /// The settings folder (tests point it elsewhere); `None`: `app::config_dir()`.
    pub dir: Option<PathBuf>,
}

impl DoorState {
    /// The port it listens on, when open.
    pub fn port(&self) -> Option<u16> {
        self.open.as_ref().map(|o| o.door.port())
    }

    /// The settings folder the token and the door file live in.
    pub fn home(&self) -> PathBuf {
        self.dir.clone().unwrap_or_else(crate::app::config_dir)
    }

    /// End a session (its agent was disconnected).
    pub fn end_session(&mut self, id: &str) {
        if let Some(o) = &self.open {
            o.door.end_session(id);
        }
    }

    /// The sessions that ended since the last frame.
    pub fn take_ended(&mut self) -> Vec<String> {
        self.open
            .as_ref()
            .map(|o| o.door.take_ended())
            .unwrap_or_default()
    }

    /// Close it (it opens again at the next frame if the settings still want it).
    pub fn close(&mut self) {
        self.open = None;
        self.failed = None;
    }
}

/// The install's default token (made the first time): the bridge's and a hand-made setup's.
pub fn token(dir: &Path) -> Option<String> {
    home::token(dir).ok()
}

/// `client`'s own token (*Copy setup for…*), made the first time; the open door accepts
/// it at once. Each client is an agent of its own to Cantus (review 2 #11).
pub fn client_token(app: &mut CantusApp, client: &str) -> Result<String, String> {
    let dir = app.ai.door.home();
    let t = home::client_token(&dir, client).map_err(|e| e.to_string())?;
    if let (Some(o), Ok(all)) = (&app.ai.door.open, home::tokens(&dir)) {
        o.door.set_tokens(all);
    }
    Ok(t)
}

/// A new token: every client set up before must be set up again; the open door's
/// sessions end and it opens again with the new one.
pub fn regenerate(app: &mut CantusApp) -> Result<String, String> {
    let dir = app.ai.door.home();
    let t = home::regenerate_token(&dir).map_err(|e| e.to_string())?;
    if let Some(o) = &app.ai.door.open {
        o.door.end_all();
    }
    app.ai.door.close();
    Ok(t)
}

/// Open or close the door as the settings say. Called each frame (cheap when nothing
/// changes).
pub fn sync(app: &mut CantusApp) {
    let ai = &app.settings.ai;
    let want = ai.enabled && ai.external_agents && app.ai.is_wired();
    let port = ai.port;
    let state = &mut app.ai.door;
    if !want {
        state.open = None;
        state.error = None;
        state.failed = None;
        return;
    }
    if state.open.as_ref().is_some_and(|o| o.asked == port) {
        return;
    }
    if state.failed == Some(port) {
        return;
    }
    state.open = None;
    let dir = state.home();
    let Some(server) = app.ai.server().cloned() else {
        return;
    };
    let state = &mut app.ai.door;
    let opened = home::tokens(&dir).and_then(|tokens| {
        HttpDoor::open(
            server,
            HttpConfig {
                port,
                tokens,
                max_connections: 16,
                ..HttpConfig::default()
            },
        )
    });
    match opened {
        Ok(door) => {
            let file = DoorFile {
                port: door.port(),
                pid: std::process::id(),
                version: crate::app::APP_VERSION.into(),
            };
            let written = home::write_door(&dir, &file);
            state.error = written
                .err()
                .map(|e| tf("ai.door.no_file", &[("error", &e.to_string())]));
            state.failed = None;
            state.open = Some(Open {
                door,
                dir,
                asked: port,
            });
        }
        Err(e) => {
            state.error = Some(tf(
                "ai.door.failed",
                &[("port", &port.to_string()), ("error", &e.to_string())],
            ));
            state.failed = Some(port);
        }
    }
}
