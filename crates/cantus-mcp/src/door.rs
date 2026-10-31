//! The door between the server's threads and the UI thread (ADR 0081 §2): a request that
//! needs the project (a tool call, a resource, a prompt) is queued here with a oneshot for
//! its answer, the window is woken so an idle one still answers, and the server's thread
//! waits. The UI thread drains the queue in its frame ([`crate::Agents::pump`]); nothing of
//! this reaches the audio thread.

use crate::jsonrpc::RpcError;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Who is asking: what the permissions, the turns and the activity view go by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    /// Whose calls these are: the session's id (2025-06-18, 2025-11-25, where the session
    /// is explicit state), or the transport's credential (2026-07-28, which relates no call
    /// to another by connection — review 1 #3): every agent behind one credential is one
    /// agent to Cantus. Ownership (changes, `undo_last_ai_action`), grants, limits and the
    /// activity go by it.
    pub key: String,
    /// The client's own name (`clientInfo`), for display only — never for a decision.
    pub name: String,
    /// The protocol version of the request.
    pub version: &'static str,
    /// The stateless generation: nothing is implied by an earlier call (a change is joined
    /// only by passing its handle).
    pub stateless: bool,
    /// Came through a door to the outside (HTTP, the stdio bridge), which *Allow external
    /// agents* governs; the in-process client is not.
    pub external: bool,
    /// What let it in, for the user to see beside its own name (review 5 #5, review 6 #3):
    /// the client whose token it carried (`default` for the install's own), `None` inside
    /// Cantus — never from anything the client says.
    pub via: Option<String>,
    /// It says it is a stdio bridge (its `Mcp-Cantus-Instance` header): a detail shown as
    /// what it says, never a proof — any holder of the token can send it.
    pub bridge: bool,
}

/// What the UI thread is asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum UiOp {
    CallTool { name: String, args: Value },
    ListResources,
    ListResourceTemplates,
    ReadResource { uri: String },
    ListPrompts,
    GetPrompt { name: String, args: Value },
}

/// A tool's answer: a value (`isError: false`) or the reason it did not do it
/// (`isError: true`) — never a protocol error.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolAnswer {
    Done(Value),
    /// A job still running: its handle and state. Not the tool's own result, so it is not
    /// `structuredContent` when the tool declares an `outputSchema` (review 1 #7).
    Pending(Value),
    Error(String),
}

/// The UI thread's answer.
#[derive(Clone, Debug, PartialEq)]
pub enum UiReply {
    Tool(ToolAnswer),
    /// A result object (resources, prompts).
    Value(Value),
    /// A protocol error (an unknown resource or prompt).
    Rpc(RpcError),
}

/// The oneshot a request is answered through. Answering twice is impossible (it is
/// consumed); dropping it unanswered tells the waiting thread that Cantus is closing.
pub struct Reply(SyncSender<UiReply>);

impl Reply {
    pub fn send(self, reply: UiReply) {
        let _ = self.0.send(reply);
    }

    /// A tool's answer.
    pub fn tool(self, answer: ToolAnswer) {
        self.send(UiReply::Tool(answer));
    }

    /// A reply the pump waits on itself (a job's edit asked on the card).
    pub(crate) fn pair() -> (Reply, Receiver<UiReply>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (Reply(tx), rx)
    }
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reply")
    }
}

/// One request waiting for the UI thread.
#[derive(Debug)]
pub struct UiCall {
    pub caller: Caller,
    pub op: UiOp,
    pub reply: Reply,
    /// When the transport stops waiting: a call still queued then is dropped unrun, so a
    /// retry never doubles it (review 1 #8).
    pub deadline: Instant,
    /// Set when its client cancelled it (`notifications/cancelled`) or hung up: dropped
    /// unrun, its card withdrawn (review 3 #3, #12).
    pub cancel: Arc<AtomicBool>,
}

/// What wakes the window (`egui::Context::request_repaint` in the application).
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// The server's side of the door. Cheap to clone; one per transport thread.
#[derive(Clone)]
pub struct Door {
    tx: Sender<UiCall>,
    wake: Waker,
}

/// The UI thread's side: drained once a frame.
pub struct Inbox {
    rx: Receiver<UiCall>,
    wake: Waker,
}

/// Why the UI thread gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DoorError {
    /// The window is gone (Cantus is closing).
    Closed,
    /// No answer within the door's limit.
    TimedOut,
    /// The client hung up or cancelled the request while it waited.
    Gone,
}

/// A door and its inbox.
pub fn door(wake: Waker) -> (Door, Inbox) {
    let (tx, rx) = mpsc::channel();
    (
        Door {
            tx,
            wake: wake.clone(),
        },
        Inbox { rx, wake },
    )
}

impl Door {
    /// Queue `op` for the UI thread, wake it and wait at most `limit` for its answer.
    pub fn ask(&self, caller: Caller, op: UiOp, limit: Duration) -> Result<UiReply, DoorError> {
        self.ask_while(
            caller,
            op,
            limit,
            &|| true,
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// [`Self::ask`] while `alive` says the client is still there (looked at every quarter
    /// second); `cancel` is the call's flag, set by its client's `notifications/cancelled`.
    /// A gone client's call is dropped unrun — its card withdrawn — and frees its thread.
    pub fn ask_while(
        &self,
        caller: Caller,
        op: UiOp,
        limit: Duration,
        alive: &dyn Fn() -> bool,
        cancel: Arc<AtomicBool>,
    ) -> Result<UiReply, DoorError> {
        let (tx, rx) = mpsc::sync_channel(1);
        let deadline = Instant::now() + limit;
        let call = UiCall {
            caller,
            op,
            reply: Reply(tx),
            deadline,
            cancel: cancel.clone(),
        };
        self.tx.send(call).map_err(|_| DoorError::Closed)?;
        (self.wake)();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(DoorError::TimedOut);
            }
            match rx.recv_timeout(left.min(Duration::from_millis(250))) {
                Ok(reply) => return Ok(reply),
                // Its card withdrawn because it was cancelled: that, not a closing window.
                Err(RecvTimeoutError::Disconnected) if cancel.load(Ordering::SeqCst) => {
                    return Err(DoorError::Gone)
                }
                Err(RecvTimeoutError::Disconnected) => return Err(DoorError::Closed),
                Err(RecvTimeoutError::Timeout) => {
                    if cancel.load(Ordering::SeqCst) || !alive() {
                        cancel.store(true, Ordering::SeqCst);
                        (self.wake)();
                        return Err(DoorError::Gone);
                    }
                }
            }
        }
    }
}

impl Inbox {
    /// The next request waiting, if any.
    pub fn next(&self) -> Option<UiCall> {
        self.rx.try_recv().ok()
    }

    /// The window's waker, for the jobs' workers.
    pub fn waker(&self) -> Waker {
        self.wake.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn caller() -> Caller {
        Caller {
            key: "k".into(),
            name: "test".into(),
            version: "2025-11-25",
            stateless: false,
            external: false,
            via: None,
            bridge: false,
        }
    }

    #[test]
    fn a_request_wakes_the_window_and_waits_for_its_answer() {
        let woken = Arc::new(AtomicUsize::new(0));
        let w = woken.clone();
        let (door, inbox) = door(Arc::new(move || {
            w.fetch_add(1, Ordering::SeqCst);
        }));
        let t = std::thread::spawn(move || {
            door.ask(caller(), UiOp::ListPrompts, Duration::from_secs(10))
        });
        let call = loop {
            if let Some(c) = inbox.next() {
                break c;
            }
            std::thread::yield_now();
        };
        assert_eq!(call.op, UiOp::ListPrompts);
        call.reply
            .send(UiReply::Value(serde_json::json!({"prompts": []})));
        let got = t.join().unwrap().unwrap();
        assert_eq!(got, UiReply::Value(serde_json::json!({"prompts": []})));
        assert_eq!(woken.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_dropped_answer_or_a_closed_window_is_said() {
        let (door, inbox) = door(Arc::new(|| {}));
        let d = door.clone();
        let t =
            std::thread::spawn(move || d.ask(caller(), UiOp::ListPrompts, Duration::from_secs(10)));
        let call = loop {
            if let Some(c) = inbox.next() {
                break c;
            }
            std::thread::yield_now();
        };
        drop(call);
        assert_eq!(t.join().unwrap(), Err(DoorError::Closed));
        drop(inbox);
        assert_eq!(
            door.ask(caller(), UiOp::ListPrompts, Duration::from_millis(10)),
            Err(DoorError::Closed)
        );
    }

    #[test]
    fn no_answer_in_time_is_a_timeout() {
        let (door, _inbox) = door(Arc::new(|| {}));
        assert_eq!(
            door.ask(caller(), UiOp::ListPrompts, Duration::from_millis(20)),
            Err(DoorError::TimedOut)
        );
    }
}
