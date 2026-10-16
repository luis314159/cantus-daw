//! What Cantus and its plug-in sandbox say to each other on the main thread (ADR 0063).
//!
//! The audio never travels here — it goes through each instance's shared [`super::slot`]
//! in lock-step. This channel carries the main-thread calls (`load`, state, parameters,
//! editors, restarts), each with an id and a timeout on Cantus's side, and the helper's
//! own news: the plug-ins' requests, their changed values, a heartbeat.
//!
//! A frame is `[json length: u32 LE][blob length: u32 LE][json][blob]` on the helper's
//! standard input and output (the helper moves its own standard handles away first, so a
//! plug-in that prints cannot break a frame). The blob carries a plug-in state as it is,
//! without JSON's cost.

use crate::backend::{Notice, ParamEvent, ParamInfo, PluginInfo};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Bumped whenever a message or the slot layout changes: a helper from another build is
/// refused at the handshake.
pub const PROTOCOL: u32 = 1;

/// The largest frame accepted (a plug-in state of 256 MB is already absurd).
pub const MAX_FRAME: usize = 256 << 20;

/// A call from Cantus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Call {
    Hello {
        protocol: u32,
    },
    /// Load, instantiate and activate; serve its audio half from the shared `slot`.
    Load {
        instance: u32,
        info: PluginInfo,
        sample_rate: f64,
        max_block: usize,
        slot: String,
    },
    Unload {
        instance: u32,
    },
    /// Cantus dropped the audio half: drop the plug-in's (a restart follows, or an unload).
    DropAudio {
        instance: u32,
    },
    Restart {
        instance: u32,
    },
    SaveState {
        instance: u32,
    },
    /// The blob is the state.
    LoadState {
        instance: u32,
    },
    Flush {
        instance: u32,
        params: Vec<(u32, u32, f64)>,
    },
    ParamText {
        instance: u32,
        id: u32,
        value: f64,
    },
    Params {
        instance: u32,
    },
    OpenEditor {
        instance: u32,
        title: String,
    },
    CloseEditor {
        instance: u32,
    },
    FocusEditor {
        instance: u32,
    },
    ResizeEditor {
        instance: u32,
        width: u32,
        height: u32,
    },
    Quit,
}

/// A request: a call with its id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub call: Call,
}

/// A plug-in's own news, as the backend contract's [`Notice`] says it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum WireNotice {
    LatencyChanged(usize),
    ParamsChanged,
    StateDirty,
    EditorResized(u32, u32),
    EditorClosed,
    RestartNeeded,
}

impl WireNotice {
    pub fn of(n: &Notice) -> Option<WireNotice> {
        Some(match n {
            Notice::LatencyChanged(l) => WireNotice::LatencyChanged(*l),
            Notice::ParamsChanged => WireNotice::ParamsChanged,
            Notice::StateDirty => WireNotice::StateDirty,
            Notice::EditorResized(s) => WireNotice::EditorResized(s.width, s.height),
            Notice::EditorClosed => WireNotice::EditorClosed,
            Notice::RestartNeeded => WireNotice::RestartNeeded,
            Notice::Crashed { .. } => return None,
        })
    }

    pub fn notice(self) -> Notice {
        match self {
            WireNotice::LatencyChanged(l) => Notice::LatencyChanged(l),
            WireNotice::ParamsChanged => Notice::ParamsChanged,
            WireNotice::StateDirty => Notice::StateDirty,
            WireNotice::EditorResized(width, height) => {
                Notice::EditorResized(crate::backend::EditorSize { width, height })
            }
            WireNotice::EditorClosed => Notice::EditorClosed,
            WireNotice::RestartNeeded => Notice::RestartNeeded,
        }
    }
}

/// A plug-in event reported outside processing (a flush it asked for).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum WireEvent {
    Begin(u32),
    Value(u32, f64),
    End(u32),
}

impl WireEvent {
    pub fn of(e: &ParamEvent) -> WireEvent {
        match *e {
            ParamEvent::Begin(id) => WireEvent::Begin(id),
            ParamEvent::Value(id, v) => WireEvent::Value(id, v),
            ParamEvent::End(id) => WireEvent::End(id),
        }
    }

    pub fn event(self) -> ParamEvent {
        match self {
            WireEvent::Begin(id) => ParamEvent::Begin(id),
            WireEvent::Value(id, v) => ParamEvent::Value(id, v),
            WireEvent::End(id) => ParamEvent::End(id),
        }
    }
}

/// What a freshly loaded (or restarted) instance is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Described {
    pub info: PluginInfo,
    pub params: Vec<ParamInfo>,
    pub latency: usize,
    pub has_editor: bool,
}

/// An answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    Done,
    Hello { protocol: u32, pid: u32 },
    Loaded(Described),
    /// `None`: the old audio half is still on its way back; call again.
    Restarted(Option<Described>),
    /// The blob is the state.
    State,
    Flushed { processed: bool, out: Vec<WireEvent> },
    Text(Option<String>),
    Params(Vec<ParamInfo>),
    Editor { width: u32, height: u32 },
}

/// A failure, as `HostError` says it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Failure {
    Load(String),
    NotFound(String),
    Instantiate(String),
    Activate(String),
    /// What is unsupported, among the names `HostError::Unsupported` uses.
    Unsupported(String),
    State(String),
    Editor(String),
    /// No instance with that number (it was unloaded).
    NoInstance,
}

/// What the plug-in holds after a call: its requests and values, so Cantus's mirror is
/// current the moment the call returns.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct After {
    pub notices: Vec<WireNotice>,
    pub values: Vec<(u32, f64)>,
    pub latency: usize,
}

/// A message from the helper.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    Reply {
        id: u64,
        result: Result<Answer, Failure>,
        /// The instance's news after the call, when the call named one.
        after: Option<(u32, After)>,
    },
    /// An instance's requests, values or flushed events, between calls.
    News {
        instance: u32,
        notices: Vec<WireNotice>,
        values: Vec<(u32, f64)>,
        flushed: Vec<WireEvent>,
    },
    /// The helper's main thread is alive (every 200 ms).
    Alive,
}

/// Writes one frame.
pub fn write_frame<T: Serialize>(w: &mut impl Write, msg: &T, blob: &[u8]) -> io::Result<()> {
    let json = serde_json::to_vec(msg).map_err(io::Error::other)?;
    let mut head = [0u8; 8];
    head[..4].copy_from_slice(&(json.len() as u32).to_le_bytes());
    head[4..].copy_from_slice(&(blob.len() as u32).to_le_bytes());
    w.write_all(&head)?;
    w.write_all(&json)?;
    w.write_all(blob)?;
    w.flush()
}

/// Reads one frame; `Ok(None)` at the end of the stream.
pub fn read_frame<T: for<'de> Deserialize<'de>>(
    r: &mut impl Read,
) -> io::Result<Option<(T, Vec<u8>)>> {
    let mut head = [0u8; 8];
    match r.read_exact(&mut head) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let json_len = u32::from_le_bytes(head[..4].try_into().unwrap_or([0; 4])) as usize;
    let blob_len = u32::from_le_bytes(head[4..].try_into().unwrap_or([0; 4])) as usize;
    if json_len > MAX_FRAME || blob_len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut json = vec![0u8; json_len];
    r.read_exact(&mut json)?;
    let mut blob = vec![0u8; blob_len];
    r.read_exact(&mut blob)?;
    let msg = serde_json::from_slice(&json).map_err(io::Error::other)?;
    Ok(Some((msg, blob)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_round_trips_with_its_blob() {
        let msg = Request {
            id: 7,
            call: Call::LoadState { instance: 3 },
        };
        let blob = vec![0u8, 1, 2, 255, 254];
        let mut buf = Vec::new();
        write_frame(&mut buf, &msg, &blob).unwrap();
        write_frame(&mut buf, &Message::Alive, &[]).unwrap();
        let mut r = &buf[..];
        let (back, b): (Request, Vec<u8>) = read_frame(&mut r).unwrap().unwrap();
        assert_eq!(back, msg);
        assert_eq!(b, blob);
        let (alive, b): (Message, Vec<u8>) = read_frame(&mut r).unwrap().unwrap();
        assert_eq!(alive, Message::Alive);
        assert!(b.is_empty());
        assert!(read_frame::<Message>(&mut r).unwrap().is_none());
    }
}
