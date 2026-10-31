//! The job protocol between Cantus and `cantus-ml` (ADR 0085): one JSON object per line,
//! requests on the helper's standard input, events on its standard output. Nothing else
//! ever writes to those pipes: the helper points its own standard handles elsewhere before
//! it loads a runtime (`platform::take_pipes`).
//!
//! ```text
//! Cantus                                   cantus-ml --serve 1
//!                                     ◄──  {"event":"hello","protocol":1,"version":…,"jobs":["separate"]}
//! {"op":"start","id":1,"job":"separate","args":{…}}  ──►
//!                                     ◄──  {"event":"started","id":1}
//!                                     ◄──  {"event":"progress","id":1,"done":0.25,"stage":"separate"}
//! {"op":"cancel","id":1}              ──►
//!                                     ◄──  {"event":"cancelled","id":1}
//! {"op":"quit"}                       ──►  (the helper ends; so does a closed input)
//! ```
//!
//! A job's arguments and its result are the job's own JSON (`spec.rs` for `separate`), so a
//! new job adds no message here. The helper runs one job at a time, in the order started;
//! Cantus's client sends the next only when the last has ended (`client.rs`).

use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

/// Bumped whenever a message changes shape. The helper says its number first; a client of
/// another number refuses it (another build's helper beside this Cantus).
pub const PROTOCOL: u32 = 1;

/// The longest line either side accepts: a result names files, it never carries audio.
pub const MAX_LINE: usize = 1 << 20;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Start job `id` of kind `job` (a name in the helper's job table) with its arguments.
    Start {
        id: u64,
        job: String,
        args: serde_json::Value,
    },
    /// Stop job `id` as soon as it can: between two steps, or inside the runtime's own
    /// run where the runtime allows it. Answered with `cancelled` (or with the job's end, if
    /// it ended first). Unknown ids are ignored.
    Cancel { id: u64 },
    /// End the helper.
    Quit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// The first line, always: the protocol, the build and the jobs this helper knows.
    Hello {
        protocol: u32,
        version: String,
        jobs: Vec<String>,
    },
    Started {
        id: u64,
    },
    /// `done` runs from 0 to 1 across the whole job; `stage` names the step (`load`,
    /// `separate`, `write`), for the interface's words.
    Progress {
        id: u64,
        done: f32,
        stage: String,
    },
    Done {
        id: u64,
        result: serde_json::Value,
    },
    Failed {
        id: u64,
        error: JobError,
    },
    Cancelled {
        id: u64,
    },
    /// A line for Cantus's log (a runtime's warning, the device a job fell back to).
    Log {
        level: String,
        message: String,
    },
}

impl Event {
    /// The job the event is about, if any.
    pub fn id(&self) -> Option<u64> {
        match self {
            Event::Started { id }
            | Event::Progress { id, .. }
            | Event::Done { id, .. }
            | Event::Failed { id, .. }
            | Event::Cancelled { id } => Some(*id),
            Event::Hello { .. } | Event::Log { .. } => None,
        }
    }

    /// Whether the event ends its job.
    pub fn ends_job(&self) -> bool {
        matches!(
            self,
            Event::Done { .. } | Event::Failed { .. } | Event::Cancelled { .. }
        )
    }
}

/// Why a job failed, as the helper knows it. What the client adds — the helper crashed,
/// hung, could not start — is `client::Failure`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum JobError {
    #[error("this helper has no job called {0}")]
    UnknownJob(String),
    #[error("the job's arguments are wrong: {0}")]
    BadArgs(String),
    #[error("the input cannot be used: {0}")]
    Input(String),
    #[error("the inference runtime cannot be loaded: {0}")]
    Runtime(String),
    #[error("the model cannot be run: {0}")]
    Model(String),
    #[error("the result cannot be written: {0}")]
    Output(String),
}

/// Read one line (without its end) into `buf`; `Ok(false)` at the end of the input. A line
/// longer than [`MAX_LINE`] is an error: the other side is not speaking this protocol.
pub fn read_line(r: &mut impl BufRead, buf: &mut String) -> std::io::Result<bool> {
    buf.clear();
    let mut bytes = Vec::new();
    loop {
        let chunk = r.fill_buf()?;
        if chunk.is_empty() {
            if bytes.is_empty() {
                return Ok(false);
            }
            break;
        }
        if let Some(i) = chunk.iter().position(|&b| b == b'\n') {
            bytes.extend_from_slice(&chunk[..i]);
            r.consume(i + 1);
            break;
        }
        let n = chunk.len();
        bytes.extend_from_slice(chunk);
        r.consume(n);
        if bytes.len() > MAX_LINE {
            return Err(std::io::Error::other("a protocol line is too long"));
        }
    }
    if bytes.len() > MAX_LINE {
        return Err(std::io::Error::other("a protocol line is too long"));
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    *buf = String::from_utf8(bytes).map_err(|_| std::io::Error::other("not UTF-8"))?;
    Ok(true)
}

/// Write `msg` as one line and flush it.
pub fn write_line(w: &mut impl Write, msg: &impl Serialize) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(msg).map_err(std::io::Error::other)?;
    line.push(b'\n');
    w.write_all(&line)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_read_back_as_written() {
        let reqs = [
            Request::Start {
                id: 7,
                job: "separate".into(),
                args: serde_json::json!({"input": "a.wav"}),
            },
            Request::Cancel { id: 7 },
            Request::Quit,
        ];
        for r in &reqs {
            let s = serde_json::to_string(r).unwrap();
            assert_eq!(&serde_json::from_str::<Request>(&s).unwrap(), r);
        }
        let e = Event::Failed {
            id: 3,
            error: JobError::Input("44 100 Hz only".into()),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert_eq!(
            s,
            r#"{"event":"failed","id":3,"error":{"kind":"input","message":"44 100 Hz only"}}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&s).unwrap(), e);
        assert!(e.ends_job());
        assert_eq!(e.id(), Some(3));
    }

    #[test]
    fn lines_are_framed_and_bounded() {
        let mut input = std::io::Cursor::new(b"one\r\ntwo\nthree".to_vec());
        let mut buf = String::new();
        let mut got = vec![];
        while read_line(&mut input, &mut buf).unwrap() {
            got.push(buf.clone());
        }
        assert_eq!(got, ["one", "two", "three"]);
        let long = vec![b'x'; MAX_LINE + 2];
        let mut input = std::io::Cursor::new(long);
        assert!(read_line(&mut input, &mut buf).is_err());
    }
}
