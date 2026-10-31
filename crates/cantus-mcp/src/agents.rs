//! The UI thread's side of the server (ADR 0081 §3–7): [`Agents::pump`] runs once a frame,
//! takes every request the door queued, decides by class, plans a tool's command, asks the
//! user when the class says so, executes the plan with `Origin::Mcp` — one undo step per
//! call, or one per change (`begin_change`/`end_change`) — starts and collects jobs, and
//! answers through each request's oneshot. It keeps what the activity view shows: every
//! call and its outcome (refusals included) and every step an agent made.

use crate::catalog::Catalog;
use crate::door::{Caller, Inbox, Reply, ToolAnswer, UiCall, UiOp, UiReply};
use crate::jobs::Jobs;
use crate::jsonrpc::{self, RpcError};
use crate::permissions::{grantable, Grants, Policy, RateLimits};
use cantus_ai::{EditPlan, Job, Said, ToolClass, ToolDef, ToolError, ToolHost};
use cantus_core::{Command, HistoryEntry, MergeKey};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What the pump needs of the application, beyond a tool's host.
pub trait AgentHost: ToolHost {
    /// The user's permissions as the settings hold them now.
    fn policy(&self) -> Policy;
    /// Which project is open: a number that changes when another project takes its place
    /// (New, Open, a recovery). A confirmation or a job's edit from another project's time
    /// is refused, never applied to this one (review 1 #1).
    fn epoch(&self) -> u64;
    /// Execute `command` with `Origin::Mcp`, as its own undo step or merged into the open
    /// change's (`merge`), with every hold the application keeps for the user's own edits.
    fn execute(&mut self, command: Command, merge: Option<MergeKey>) -> Result<(), String>;
    /// Undo the step on top, as the user's Undo does.
    fn undo_step(&mut self) -> Result<(), String>;
    /// A sentence in the interface's language.
    fn say(&self, said: &Said) -> String;
    /// A history step in words (the history panel's name for it).
    fn describe_step(&self, entry: &HistoryEntry) -> String {
        entry.label_key.clone()
    }
    /// The planned command in words, for the confirmation card (what it would touch).
    fn preview(&self, _command: &Command) -> Option<String> {
        None
    }
}

/// The pump's timings and limits.
#[derive(Clone, Debug)]
pub struct Config {
    /// How long a confirmation waits for the user.
    pub ask_timeout: Duration,
    /// How long the call that starts a job waits for it before answering with its handle.
    pub inline_job_wait: Duration,
    /// A change with no call for this long ends by itself.
    pub change_idle: Duration,
    pub max_asks_per_client: usize,
    /// Confirmations waiting from every agent together (review 1 #4).
    pub max_asks: usize,
    pub max_jobs_per_client: usize,
    pub max_jobs: usize,
    /// Changes one agent may have open at once (the stateless generation opens many).
    pub max_changes_per_client: usize,
    /// How long an ended job is kept for `wait_job`.
    pub keep_jobs: Duration,
    /// An agent silent this long is forgotten: its grants, buckets and changes go.
    pub forget_after: Duration,
    /// An agent heard from within this is shown as connected (the top bar's chip).
    pub connected_within: Duration,
    /// Agents remembered at once: a new one past this is refused until one is forgotten
    /// (review 3 #13).
    pub max_agents: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ask_timeout: Duration::from_secs(120),
            inline_job_wait: Duration::from_secs(3),
            change_idle: Duration::from_secs(120),
            max_asks_per_client: 8,
            max_asks: 24,
            max_jobs_per_client: 4,
            max_jobs: 16,
            max_changes_per_client: 8,
            keep_jobs: Duration::from_secs(600),
            forget_after: Duration::from_secs(3600),
            connected_within: Duration::from_secs(1800),
            max_agents: 256,
        }
    }
}

/// The user's answer to a confirmation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Allow,
    /// Allow, and this edit tool from this agent until it disconnects (an edit only: for a
    /// destructive or file tool it allows this call alone).
    AllowForSession,
    Deny,
}

/// A call waiting for the user, as the confirmation card shows it.
#[derive(Clone, Debug)]
pub struct Ask {
    pub id: u64,
    pub caller: Caller,
    pub tool: &'static str,
    pub class: ToolClass,
    /// What it would do, in the user's language.
    pub said: String,
    /// The planned command in words, when the host can say it.
    pub preview: Option<String>,
    pub asked: Instant,
    /// When it stops waiting (the card counts down to it).
    pub deadline: Instant,
    /// Whether *Allow for this session* means anything for it (edits only).
    pub grantable: bool,
}

/// What was prepared before asking, kept for when the user answers.
pub(crate) enum Prepared {
    Edit(EditPlan, u64),
    Job(Job, u64),
    Host,
    /// `undo_last_ai_action`, for the step that was on top when the card was shown.
    Undo(u64),
    /// A job's edit (`JobDone::Then`), planned at that revision: never planned again.
    JobEdit(EditPlan, u64),
}

pub(crate) struct PendingAsk {
    pub ask: Ask,
    pub def: &'static ToolDef,
    pub args: Value,
    pub expected: Option<u64>,
    /// The change the call joins, if any (its handle).
    pub change: Option<String>,
    pub prepared: Prepared,
    pub reply: Reply,
    pub answer: Option<Answer>,
    /// The project it was planned in.
    pub epoch: u64,
    /// When it stops waiting: the confirmation's own timeout from when it was first asked,
    /// or the transport's deadline if that comes first — a re-ask never restarts it
    /// (review 2 #12).
    pub deadline: Instant,
    /// Set when its client cancelled it or hung up: the card is withdrawn (review 3 #12).
    pub cancel: Arc<AtomicBool>,
}

/// One step an agent made.
#[derive(Clone, Debug)]
pub struct AiAction {
    /// The history step it made or merged into ([`cantus_core::HistoryEntry::step`]); the
    /// earlier calls of a change follow the step as it is renewed (review 1 #6).
    pub step: u64,
    /// The project it was made in ([`AgentHost::epoch`]).
    pub epoch: u64,
    pub caller: Caller,
    pub tool: &'static str,
    pub said: String,
    /// The change it belongs to, and the change's label.
    pub change: Option<(String, String)>,
    pub revision: u64,
    pub at: Instant,
}

/// What became of a call.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Done,
    /// Refused before it ran (permissions, limits, the AI area off).
    Refused(String),
    Failed(String),
    /// Waiting for the user.
    Asked,
    DeniedByUser,
    NoAnswer,
    JobStarted(String),
    /// Dropped unrun: its transport had stopped waiting (review 1 #8, review 2 #12).
    Dropped,
}

/// One line of the AI activity.
#[derive(Clone, Debug)]
pub struct Activity {
    pub at: Instant,
    pub caller: Caller,
    pub tool: String,
    pub outcome: Outcome,
}

/// A change an agent opened.
#[derive(Clone, Debug)]
pub(crate) struct Change {
    pub handle: String,
    pub owner: String,
    pub label: String,
    pub key: MergeKey,
    pub calls: usize,
    pub last: Instant,
    /// Opened by a session (2025-06-18, 2025-11-25): its calls join it without naming it.
    pub implicit: bool,
    pub epoch: u64,
}

/// What *Undo all of this agent's changes* did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoAll {
    pub undone: usize,
    /// The step it stopped at, in words, when that is not the agent's (the user's, or
    /// another agent's) — never undone silently.
    pub stopped_at: Option<String>,
}

pub(crate) const KEEP_ACTIVITY: usize = 5000;
pub(crate) const KEEP_ACTIONS: usize = 10_000;

/// The UI thread's state of the MCP server.
pub struct Agents {
    pub(crate) inbox: Option<Inbox>,
    pub(crate) catalog: Catalog,
    pub config: Config,
    pub(crate) grants: HashMap<String, Grants>,
    pub(crate) rates: RateLimits,
    pub(crate) asks: Vec<PendingAsk>,
    pub(crate) next_ask: u64,
    pub(crate) jobs: Jobs,
    /// Open changes, by handle.
    pub(crate) changes: HashMap<String, Change>,
    pub(crate) actions: VecDeque<AiAction>,
    pub(crate) activity: VecDeque<Activity>,
    /// Every agent heard from, and when last.
    pub(crate) seen: HashMap<String, (Caller, Instant)>,
}

impl Agents {
    /// The pump for `inbox`, dispatching over `catalog`.
    pub fn new(inbox: Inbox, catalog: Catalog) -> Self {
        Agents {
            inbox: Some(inbox),
            catalog,
            config: Config::default(),
            grants: HashMap::new(),
            rates: RateLimits::default(),
            asks: Vec::new(),
            next_ask: 1,
            jobs: Jobs::default(),
            changes: HashMap::new(),
            actions: VecDeque::new(),
            activity: VecDeque::new(),
            seen: HashMap::new(),
        }
    }

    /// Serve everything waiting. Call once a frame on the UI thread.
    pub fn pump<H: AgentHost>(&mut self, host: &mut H) {
        while let Some(call) = self.inbox.as_ref().and_then(Inbox::next) {
            // A call its transport gave up on is dropped unrun (review 1 #8), judged at
            // its own turn, not at the pump's start (review 2 #12).
            let now = Instant::now();
            if call.deadline <= now || call.cancel.load(Ordering::SeqCst) {
                if let UiOp::CallTool { name, .. } = &call.op {
                    self.note(&call.caller, name, Outcome::Dropped);
                }
                continue;
            }
            self.serve(host, call, now);
        }
        let now = Instant::now();
        self.cancel_other_projects_jobs(host.epoch());
        self.settle_asks(host, now);
        self.collect_jobs(host, now);
        self.drop_abandoned_starts(now);
        self.expire_waiters(now);
        self.end_idle_changes(host, now);
        self.jobs.prune(now, self.config.keep_jobs);
        self.forget_idle(host, now);
    }

    /// When the pump wants the next frame even if nothing else moves: a confirmation's or
    /// a wait's deadline, a change ending by itself, a job's progress.
    pub fn next_wake(&self, now: Instant) -> Option<Duration> {
        let until = |t: Instant| t.saturating_duration_since(now);
        let asks = self.asks.iter().map(|a| until(a.deadline));
        let waits = self.jobs.waiters.iter().map(|(_, w)| until(w.deadline));
        let changes = self
            .changes
            .values()
            .map(|c| until(c.last + self.config.change_idle));
        let running = (self.jobs.running(None) > 0).then_some(Duration::from_millis(100));
        asks.chain(waits).chain(changes).chain(running).min()
    }

    // ---- what the views read -----------------------------------------------------------

    /// The calls waiting for the user, oldest first.
    pub fn asks(&self) -> impl Iterator<Item = &Ask> {
        self.asks.iter().map(|p| &p.ask)
    }

    /// The user answers a confirmation; it is carried out at the next pump. `false` when
    /// it is no longer waiting.
    pub fn answer(&mut self, id: u64, answer: Answer) -> bool {
        let Some(p) = self.asks.iter_mut().find(|p| p.ask.id == id) else {
            return false;
        };
        p.answer = Some(answer);
        if answer == Answer::AllowForSession && grantable(p.ask.class) {
            let key = p.ask.caller.key.clone();
            let tool = p.ask.tool.to_string();
            self.grants.entry(key).or_default().tools.insert(tool);
        }
        true
    }

    /// Every call and what became of it, oldest first.
    pub fn activity(&self) -> impl DoubleEndedIterator<Item = &Activity> + ExactSizeIterator {
        self.activity.iter()
    }

    /// Every step agents made, oldest first.
    pub fn actions(&self) -> impl DoubleEndedIterator<Item = &AiAction> + ExactSizeIterator {
        self.actions.iter()
    }

    pub fn jobs(&self) -> &Jobs {
        &self.jobs
    }

    /// The agent's words for a history step it made: the change's label, or what its last
    /// call said. `None` for a step no agent made. Steps are numbered once for the bus's
    /// whole life, so a step is found by its number alone — an imported project undone
    /// brings the agent's steps back with their words (review 2 #4).
    pub fn step_words(&self, step: u64) -> Option<String> {
        let a = self.actions.iter().rev().find(|a| a.step == step)?;
        Some(match &a.change {
            Some((_, label)) => label.clone(),
            None => a.said.clone(),
        })
    }

    /// The agent that made a step, if one did.
    pub fn step_owner(&self, step: u64) -> Option<&Caller> {
        self.actions
            .iter()
            .rev()
            .find(|a| a.step == step)
            .map(|a| &a.caller)
    }

    /// The agents heard from recently (the top bar's chip), most recent first.
    pub fn connected(&self, now: Instant) -> Vec<Caller> {
        let within = self.config.connected_within;
        let mut v: Vec<(&Caller, Instant)> = self
            .seen
            .values()
            .filter(|(_, at)| now.saturating_duration_since(*at) < within)
            .map(|(c, at)| (c, *at))
            .collect();
        v.sort_by_key(|c| std::cmp::Reverse(c.1));
        v.into_iter().map(|(c, _)| c.clone()).collect()
    }

    /// The tools an agent may run unasked this session.
    pub fn grants_of(&self, key: &str) -> Vec<String> {
        self.grants
            .get(key)
            .map(|g| g.tools.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Disconnect an agent: its grants go, its changes end, its jobs are cancelled, its
    /// confirmations refused, and it is no longer shown as connected.
    pub fn revoke<H: AgentHost>(&mut self, host: &mut H, key: &str) {
        self.grants.remove(key);
        self.rates.forget(key);
        self.seen.remove(key);
        let ended: Vec<String> = self
            .changes
            .values()
            .filter(|c| c.owner == key)
            .map(|c| c.handle.clone())
            .collect();
        for h in ended {
            if let Some(c) = self.changes.remove(&h) {
                host.bus_mut().end_gesture_if(c.key);
            }
        }
        for e in self.jobs.entries.iter().filter(|e| e.owner.key == key) {
            e.request_cancel();
        }
        for p in std::mem::take(&mut self.asks) {
            if p.ask.caller.key == key {
                self.note(&p.ask.caller, p.def.name, Outcome::DeniedByUser);
                p.reply
                    .tool(ToolAnswer::Error("the user disconnected this agent".into()));
            } else {
                self.asks.push(p);
            }
        }
    }

    /// Another project opened: the jobs started in an earlier one are cancelled, as the
    /// application's own runs stop (the integrated review: they ran on, their work thrown
    /// away only at their end).
    fn cancel_other_projects_jobs(&mut self, epoch: u64) {
        for e in self
            .jobs
            .entries
            .iter()
            .filter(|e| e.epoch != epoch && e.state == crate::jobs::JobState::Running)
        {
            e.request_cancel();
        }
    }

    /// Agents silent past `forget_after` are forgotten (the maps never grow without end).
    fn forget_idle<H: AgentHost>(&mut self, host: &mut H, now: Instant) {
        let after = self.config.forget_after;
        let gone: Vec<String> = self
            .seen
            .iter()
            .filter(|(_, (_, at))| now.saturating_duration_since(*at) >= after)
            .map(|(k, _)| k.clone())
            .collect();
        for k in gone {
            self.revoke(host, &k);
        }
        self.rates.prune(now);
    }

    // ---- serving -------------------------------------------------------------------------

    pub(crate) fn note(&mut self, caller: &Caller, tool: &str, outcome: Outcome) {
        self.activity.push_back(Activity {
            at: Instant::now(),
            caller: caller.clone(),
            tool: tool.to_string(),
            outcome,
        });
        while self.activity.len() > KEEP_ACTIVITY {
            self.activity.pop_front();
        }
    }

    /// Answer a call and write down what became of it.
    pub(crate) fn finish(
        &mut self,
        caller: &Caller,
        tool: &str,
        reply: Reply,
        result: Result<Value, ToolError>,
    ) {
        match result {
            Ok(v) => {
                self.note(caller, tool, Outcome::Done);
                reply.tool(ToolAnswer::Done(v));
            }
            Err(e) => {
                let outcome = match &e {
                    ToolError::Refused(r) => Outcome::Refused(r.clone()),
                    other => Outcome::Failed(other.to_string()),
                };
                self.note(caller, tool, outcome);
                reply.tool(ToolAnswer::Error(e.to_string()));
            }
        }
    }

    fn serve<H: AgentHost>(&mut self, host: &mut H, call: UiCall, now: Instant) {
        let UiCall {
            caller,
            op,
            reply,
            deadline,
            cancel,
        } = call;
        if !self.seen.contains_key(&caller.key) && self.seen.len() >= self.config.max_agents {
            let why = "too many agents are connected to Cantus; try again later".to_string();
            reply.tool(ToolAnswer::Error(why));
            return;
        }
        self.seen.insert(caller.key.clone(), (caller.clone(), now));
        match op {
            UiOp::CallTool { name, args } => {
                self.call(host, caller, &name, args, reply, now, (deadline, cancel))
            }
            other => {
                let policy = host.policy();
                let refused = if !policy.enabled {
                    Some(crate::permissions::Refusal::AiOff)
                } else if caller.external && !policy.external_agents {
                    Some(crate::permissions::Refusal::ExternalOff)
                } else {
                    None
                };
                let answer = match refused {
                    Some(r) => UiReply::Rpc(RpcError::new(jsonrpc::INTERNAL_ERROR, r.words(""))),
                    None => crate::agents_catalog::serve(&self.catalog, host, other),
                };
                reply.send(answer);
            }
        }
    }
}
