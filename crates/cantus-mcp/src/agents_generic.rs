//! The server's own tools as the pump serves them (`generic.rs` describes them), and the
//! jobs' and changes' bookkeeping each frame.

use crate::agents::{AgentHost, Agents, Change, Prepared, UndoAll};
use crate::door::{Caller, Reply, ToolAnswer, UiReply};
use crate::generic::*;
use crate::jobs::{JobState, Waiter};
use cantus_ai::{Decision, JobDone, ToolCx, ToolDef, ToolError};
use cantus_core::{MergeKey, Origin};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn text_arg(args: &Value, name: &str) -> Result<String, ToolError> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ToolError::bad(name, "a string"))
}

fn gone(handle: &str) -> ToolError {
    ToolError::NotFound(format!(
        "job {handle} (ended jobs are kept 10 minutes; list_jobs lists yours)"
    ))
}

impl Agents {
    pub(crate) fn generic<H: AgentHost>(
        &mut self,
        host: &mut H,
        caller: Caller,
        def: &'static ToolDef,
        args: Value,
        reply: Reply,
        now: Instant,
    ) {
        let r = match def.name {
            WAIT_JOB => return self.wait_job(caller, &args, reply, now),
            CANCEL_JOB => self.cancel_job(&args, now),
            LIST_JOBS => Ok(self.list_jobs(&caller, now)),
            BEGIN_CHANGE => self.begin_change(host, &caller, &args, now),
            END_CHANGE => self.end_change(host, &caller, &args),
            UNDO_LAST_AI_ACTION => self.undo_last(host, &caller),
            LIST_AI_ACTIONS => self.list_actions(host, &caller, &args, now),
            other => Err(ToolError::Failed(format!("{other} is not the server's"))),
        };
        self.finish(&caller, def.name, reply, r);
    }

    fn wait_job(&mut self, caller: Caller, args: &Value, reply: Reply, now: Instant) {
        let handle = match text_arg(args, "job") {
            Ok(h) => h,
            Err(e) => return self.finish(&caller, WAIT_JOB, reply, Err(e)),
        };
        let timeout = match args.get("timeout_s") {
            None | Some(Value::Null) => 30.0,
            Some(v) => match v.as_f64().filter(|s| (0.0..=60.0).contains(s)) {
                Some(s) => s,
                None => {
                    let e = ToolError::bad("timeout_s", "seconds, from 0 to 60");
                    return self.finish(&caller, WAIT_JOB, reply, Err(e));
                }
            },
        };
        let Some(job) = self.jobs.get(&handle) else {
            return self.finish(&caller, WAIT_JOB, reply, Err(gone(&handle)));
        };
        if job.state != JobState::Running {
            let r = waited(job.describe(now), &job.state);
            return self.finish(&caller, WAIT_JOB, reply, r);
        }
        let waiter = Waiter {
            reply,
            deadline: now + Duration::from_secs_f64(timeout),
            starting_call: false,
            cancel: None,
        };
        self.note(&caller, WAIT_JOB, crate::agents::Outcome::Done);
        self.jobs.waiters.push((handle, waiter));
    }

    fn cancel_job(&mut self, args: &Value, now: Instant) -> Result<Value, ToolError> {
        let handle = text_arg(args, "job")?;
        let job = self.jobs.get_mut(&handle).ok_or_else(|| gone(&handle))?;
        if job.state == JobState::Running {
            job.request_cancel();
        }
        // Its edit waiting on the card: the card is withdrawn and the job ends cancelled
        // (review 3 #5).
        if let Some((_, _, id)) = job.awaiting.take() {
            let owner = job.owner.clone();
            let tool = job.tool;
            self.asks.retain(|p| p.ask.id != id);
            self.end_job(&handle, &owner, tool, JobState::Cancelled, now);
        }
        let job = self.jobs.get(&handle).ok_or_else(|| gone(&handle))?;
        Ok(job.describe(now))
    }

    fn list_jobs(&self, caller: &Caller, now: Instant) -> Value {
        let jobs: Vec<Value> = self
            .jobs
            .iter()
            .filter(|j| j.owner.key == caller.key)
            .map(|j| j.describe(now))
            .collect();
        json!({ "jobs": jobs })
    }

    fn begin_change<H: AgentHost>(
        &mut self,
        host: &H,
        caller: &Caller,
        args: &Value,
        now: Instant,
    ) -> Result<Value, ToolError> {
        let label = text_arg(args, "label")?;
        let label: String = label.chars().filter(|c| !c.is_control()).collect();
        let label = label.trim().to_string();
        if label.is_empty() || label.chars().count() > 80 {
            return Err(ToolError::bad("label", "1 to 80 characters"));
        }
        // A session's change is joined without naming it, so it has one at a time; a
        // stateless agent names its changes on every call and may hold several.
        let mine: Vec<&Change> = self
            .changes
            .values()
            .filter(|c| c.owner == caller.key)
            .collect();
        if let Some(open) = mine
            .iter()
            .find(|c| c.implicit)
            .filter(|_| !caller.stateless)
        {
            return Err(ToolError::Refused(format!(
                "a change is open already ({}); end it with end_change first",
                open.handle
            )));
        }
        if mine.len() >= self.config.max_changes_per_client {
            return Err(ToolError::Refused(format!(
                "{} of your changes are open; end one first",
                mine.len()
            )));
        }
        let handle = crate::ids::fresh("change");
        let key = MergeKey::of(&("cantus-mcp-change", &handle));
        self.changes.insert(
            handle.clone(),
            Change {
                handle: handle.clone(),
                owner: caller.key.clone(),
                label,
                key,
                calls: 0,
                last: now,
                implicit: !caller.stateless,
                epoch: host.epoch(),
            },
        );
        let how = if caller.stateless {
            "pass change with this handle to every edit that belongs to it"
        } else {
            "your edits join it until end_change (or pass change with this handle)"
        };
        Ok(json!({ "change": handle, "how": how }))
    }

    fn end_change<H: AgentHost>(
        &mut self,
        host: &mut H,
        caller: &Caller,
        args: &Value,
    ) -> Result<Value, ToolError> {
        let handle = text_arg(args, "change")?;
        let open = self.changes.get(&handle).filter(|c| c.owner == caller.key);
        let Some(open) = open.cloned() else {
            return Err(ToolError::NotFound(format!(
                "change {handle} (it ended, is another agent's, or ended by itself after 2 minutes without a call)"
            )));
        };
        self.changes.remove(&handle);
        host.bus_mut().end_gesture_if(open.key);
        let revision = host.bus().revision();
        Ok(json!({ "change": handle, "calls": open.calls, "revision": revision }))
    }

    fn undo_last<H: AgentHost>(
        &mut self,
        host: &mut H,
        caller: &Caller,
    ) -> Result<Value, ToolError> {
        let Some(top) = host.bus().history().last() else {
            return Err(ToolError::Failed("there is nothing to undo".into()));
        };
        let words = host.describe_step(top);
        if top.origin != Origin::Mcp {
            return Err(ToolError::Refused(format!(
                "the last step is the user's (\"{words}\"); nothing was undone"
            )));
        }
        let step = top.step();
        match self.step_owner(step) {
            Some(a) if a.key == caller.key => {}
            Some(a) => {
                return Err(ToolError::Refused(format!(
                    "the last step is another agent's ({}: \"{words}\"); nothing was undone",
                    a.name
                )))
            }
            None => {
                return Err(ToolError::Refused(format!(
                    "the last step (\"{words}\") is not yours; nothing was undone"
                )))
            }
        }
        self.end_changes_on(host, caller, step);
        let said = self.step_words(step).unwrap_or(words);
        host.undo_step().map_err(ToolError::Failed)?;
        Ok(json!({ "undone": said, "revision": host.bus().revision() }))
    }

    fn list_actions<H: AgentHost>(
        &self,
        host: &H,
        caller: &Caller,
        args: &Value,
        now: Instant,
    ) -> Result<Value, ToolError> {
        let limit = match args.get("limit") {
            None | Some(Value::Null) => 20,
            Some(v) => v
                .as_u64()
                .filter(|n| (1..=200).contains(n))
                .ok_or_else(|| ToolError::bad("limit", "an integer from 1 to 200"))?
                as usize,
        };
        let all = args
            .get("all_agents")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let in_project: std::collections::BTreeSet<u64> =
            host.bus().history().iter().map(|e| e.step()).collect();
        let mut out = Vec::new();
        for a in self.actions.iter().rev() {
            if !all && a.caller.key != caller.key {
                continue;
            }
            if out.len() == limit {
                break;
            }
            let mut v = json!({
                "tool": a.tool,
                "said": a.said,
                "in_project": in_project.contains(&a.step),
                "seconds_ago": now.saturating_duration_since(a.at).as_secs(),
                "revision": a.revision,
            });
            if let Some((handle, label)) = &a.change {
                v["change"] = json!({ "change": handle, "label": label });
            }
            if all {
                v["agent"] = json!(a.caller.name);
            }
            out.push(v);
        }
        Ok(json!({ "actions": out }))
    }

    /// Take the jobs that ended: decide their edits again and plan them here, answer
    /// whoever waits; and the jobs whose edit was waiting on the card, answered.
    pub(crate) fn collect_jobs<H: AgentHost>(&mut self, host: &mut H, now: Instant) {
        for (handle, out) in self.jobs.drain_finished() {
            let Some(job) = self.jobs.get(&handle) else {
                continue;
            };
            let (owner, tool, cancelled) = (job.owner.clone(), job.tool, job.cancel_requested());
            let (epoch, start) = (job.epoch, job.start.clone());
            let def = self.catalog.tool(tool);
            let state = match out {
                // Stopped because another project opened: said so, as its edit would be.
                _ if cancelled && epoch != host.epoch() => JobState::Failed(
                    ToolError::Refused(
                        "another project was opened while the job ran; it was stopped and its edit not made"
                            .into(),
                    )
                    .to_string(),
                ),
                _ if cancelled => JobState::Cancelled,
                Err(ToolError::Cancelled) => JobState::Cancelled,
                Err(e) => JobState::Failed(e.to_string()),
                Ok(JobDone::Answer(v)) => JobState::Done(v),
                Ok(JobDone::Then(v, then)) => match def {
                    None => JobState::Failed(format!("{tool} is gone")),
                    Some(def) => {
                        match self.job_edit(host, &owner, def, &handle, epoch, &start, v, then) {
                            Some(state) => state,
                            // Its edit waits on the card: the job ends when the user answers.
                            None => continue,
                        }
                    }
                },
            };
            self.end_job(&handle, &owner, tool, state, now);
        }
        // Jobs whose edit was on the card: the user's answer came.
        let answered: Vec<(String, UiReply, Value)> = self
            .jobs
            .entries
            .iter_mut()
            .filter_map(|j| {
                let got = j.awaiting.as_ref()?.0.try_recv();
                let reply = match got {
                    Ok(r) => r,
                    Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        UiReply::Tool(ToolAnswer::Error("the edit was not made".into()))
                    }
                };
                let (_, v, _) = j.awaiting.take().expect("checked");
                Some((j.handle.clone(), reply, v))
            })
            .collect();
        for (handle, reply, v) in answered {
            let Some(job) = self.jobs.get(&handle) else {
                continue;
            };
            let (owner, tool) = (job.owner.clone(), job.tool);
            let state = match reply {
                UiReply::Tool(ToolAnswer::Done(r) | ToolAnswer::Pending(r)) => {
                    JobState::Done(merge(v, r))
                }
                UiReply::Tool(ToolAnswer::Error(e)) => JobState::Failed(e),
                UiReply::Value(r) => JobState::Done(merge(v, r)),
                UiReply::Rpc(e) => JobState::Failed(e.message),
            };
            self.end_job(&handle, &owner, tool, state, now);
        }
    }

    /// A job's edit (`JobDone::Then`), decided again as it ends (review 2 #2): refused if
    /// the permission now says so or another project is open; planned on the project as it
    /// is; asked on the card again if the user had to allow the job and the project moved
    /// since it started (`None`: it waits there). Joins the change it started in, if still
    /// open (review 2 #10).
    #[allow(clippy::too_many_arguments)]
    fn job_edit<H: AgentHost>(
        &mut self,
        host: &mut H,
        owner: &Caller,
        def: &'static ToolDef,
        handle: &str,
        epoch: u64,
        start: &crate::jobs::JobStart,
        v: Value,
        then: cantus_ai::ThenPlan,
    ) -> Option<JobState> {
        let decided = match self.decision(host, owner, def) {
            Ok(d) => d,
            Err(e) => return Some(JobState::Failed(e.to_string())),
        };
        if epoch != host.epoch() {
            return Some(JobState::Failed(
                ToolError::Refused(
                    "another project was opened while the job ran; its edit was not made".into(),
                )
                .to_string(),
            ));
        }
        let change = start
            .change
            .clone()
            .filter(|h| self.changes.get(h).is_some_and(|c| c.owner == owner.key));
        let before = host.bus().revision();
        let then =
            crate::agents_call::planned(host, before, def, |h| then(&mut ToolCx { host: h }));
        let plan = match then {
            Ok(plan) => plan,
            Err(e) => return Some(JobState::Failed(e.to_string())),
        };
        let moved = host.bus().revision() != start.revision;
        // Asked again when the user must be asked now and either was not asked at the
        // start (edits were allowed then, not now) or the project moved since (review 3 #4).
        if decided == Decision::AskUser && (!start.asked || moved) {
            let (reply, rx) = Reply::pair();
            let said = host.say(&plan.said);
            let preview = host.preview(&plan.command);
            let rev = host.bus().revision();
            let now = Instant::now();
            let deadline = now + self.config.ask_timeout;
            let p = (Prepared::JobEdit(plan, rev), said, preview);
            self.ask(
                host,
                owner.clone(),
                def,
                Value::Null,
                None,
                change,
                p,
                reply,
                (now, deadline, Arc::new(AtomicBool::new(false))),
                true,
            );
            let id = self.next_ask - 1;
            if let Some(job) = self.jobs.get_mut(handle) {
                job.awaiting = Some((rx, v, id));
            }
            return None;
        }
        // `execute_plan` refuses a class that may not change the project (review 1 #5).
        Some(
            match self.execute_plan(host, owner, def, plan, change.as_deref()) {
                Ok(r) => JobState::Done(merge(v, r)),
                Err(e) => JobState::Failed(e.to_string()),
            },
        )
    }

    fn end_job(&mut self, handle: &str, owner: &Caller, tool: &str, state: JobState, now: Instant) {
        if let Some(job) = self.jobs.get_mut(handle) {
            job.state = state;
            job.ended = Some(now);
            job.progress_done();
        }
        let outcome = match self.jobs.get(handle).map(|j| &j.state) {
            Some(JobState::Done(_)) => crate::agents::Outcome::Done,
            Some(JobState::Failed(e)) => crate::agents::Outcome::Failed(e.clone()),
            _ => crate::agents::Outcome::Failed("cancelled".into()),
        };
        self.note(owner, tool, outcome);
        self.answer_waiters(handle, now, true);
    }

    /// A job whose starting call was cancelled, or whose client hung up, before the call
    /// was answered: nobody holds its handle, so it is cancelled too — its card withdrawn,
    /// its activity line saying so (review 4 #7).
    pub(crate) fn drop_abandoned_starts(&mut self, now: Instant) {
        let abandoned: Vec<String> = self
            .jobs
            .waiters
            .iter()
            .filter(|(_, w)| {
                w.starting_call
                    && w.cancel
                        .as_ref()
                        .is_some_and(|c| c.load(std::sync::atomic::Ordering::SeqCst))
            })
            .map(|(h, _)| h.clone())
            .collect();
        for handle in abandoned {
            self.jobs
                .waiters
                .retain(|(h, w)| !(h == &handle && w.starting_call));
            let Some(job) = self.jobs.get_mut(&handle) else {
                continue;
            };
            if job.state != JobState::Running {
                continue;
            }
            job.request_cancel();
            let (owner, tool) = (job.owner.clone(), job.tool);
            if let Some((_, _, id)) = job.awaiting.take() {
                self.asks.retain(|p| p.ask.id != id);
            }
            self.end_job(&handle, &owner, tool, JobState::Cancelled, now);
        }
    }

    /// Waiters whose deadline passed hear that the job still runs.
    pub(crate) fn expire_waiters(&mut self, now: Instant) {
        let handles: Vec<String> = self
            .jobs
            .waiters
            .iter()
            .filter(|(_, w)| w.deadline <= now)
            .map(|(h, _)| h.clone())
            .collect();
        for h in handles {
            self.answer_waiters(&h, now, false);
        }
    }

    /// Answer the waiters of `handle`: all of them when it ended, else those due.
    fn answer_waiters(&mut self, handle: &str, now: Instant, ended: bool) {
        let mut keep = Vec::new();
        for (h, w) in std::mem::take(&mut self.jobs.waiters) {
            if h != handle || (!ended && w.deadline > now) {
                keep.push((h, w));
                continue;
            }
            let Some(job) = self.jobs.get(&h) else {
                w.reply.tool(ToolAnswer::Error(gone(&h).to_string()));
                continue;
            };
            let described = job.describe(now);
            let answer = match (&job.state, w.starting_call) {
                // Ended within the call that started it: its plain result.
                (JobState::Done(v), true) => ToolAnswer::Done(v.clone()),
                (JobState::Running, true) => {
                    let mut d = described;
                    d["next"] = json!("call wait_job with this job's handle");
                    ToolAnswer::Pending(d)
                }
                (JobState::Running, false) => ToolAnswer::Done(described),
                (state, _) => match waited(described, state) {
                    Ok(v) => ToolAnswer::Done(v),
                    Err(e) => ToolAnswer::Error(e.to_string()),
                },
            };
            w.reply.tool(answer);
        }
        self.jobs.waiters = keep;
    }

    /// The changes of `caller` whose step is `step` end (that step is being undone).
    fn end_changes_on<H: AgentHost>(&mut self, host: &mut H, caller: &Caller, step: u64) {
        let on: Vec<String> = self
            .changes
            .values()
            .filter(|c| c.owner == caller.key)
            .filter(|c| {
                self.actions.iter().any(|a| {
                    a.step == step && a.change.as_ref().is_some_and(|(h, _)| *h == c.handle)
                })
            })
            .map(|c| c.handle.clone())
            .collect();
        for h in on {
            if let Some(c) = self.changes.remove(&h) {
                host.bus_mut().end_gesture_if(c.key);
            }
        }
    }

    /// *Undo all of this agent's changes*: its steps are undone from the top, one by one,
    /// until the top is not its own — the user's or another agent's step stops it, and the
    /// answer names it when the agent has steps left below (never undo the user's work
    /// silently).
    pub fn undo_all<H: AgentHost>(&mut self, host: &mut H, key: &str) -> UndoAll {
        let mut undone = 0;
        loop {
            let Some(top) = host.bus().history().last() else {
                return UndoAll {
                    undone,
                    stopped_at: None,
                };
            };
            let owner = self.step_owner(top.step()).cloned();
            let mine = top.origin == cantus_core::Origin::Mcp
                && owner.as_ref().is_some_and(|c| c.key == key);
            if !mine {
                let steps: std::collections::BTreeSet<u64> =
                    host.bus().history().iter().map(|e| e.step()).collect();
                let more = self
                    .actions
                    .iter()
                    .any(|a| a.caller.key == key && steps.contains(&a.step));
                let stopped_at = more.then(|| host.describe_step(top));
                return UndoAll { undone, stopped_at };
            }
            let caller = owner.expect("checked");
            self.end_changes_on(host, &caller, top.step());
            if let Err(e) = host.undo_step() {
                return UndoAll {
                    undone,
                    stopped_at: Some(e),
                };
            }
            undone += 1;
        }
    }

    /// A change with no call for a while ends by itself.
    pub(crate) fn end_idle_changes<H: AgentHost>(&mut self, host: &mut H, now: Instant) {
        let idle = self.config.change_idle;
        // A change whose call waits on the card stays open (review 2 #7).
        // So does one a running job will join (review 3 #6).
        let waiting: Vec<&str> = self
            .asks
            .iter()
            .filter_map(|p| p.change.as_deref())
            .chain(
                self.jobs
                    .iter()
                    .filter(|j| j.state == JobState::Running)
                    .filter_map(|j| j.start.change.as_deref()),
            )
            .collect();
        let ended: Vec<String> = self
            .changes
            .iter()
            .filter(|(h, c)| {
                now.saturating_duration_since(c.last) >= idle && !waiting.contains(&h.as_str())
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in ended {
            if let Some(c) = self.changes.remove(&k) {
                host.bus_mut().end_gesture_if(c.key);
            }
        }
    }
}

/// What `wait_job` answers for an ended job.
fn waited(described: Value, state: &JobState) -> Result<Value, ToolError> {
    match state {
        JobState::Failed(e) => Err(ToolError::Failed(format!(
            "the job {} failed: {e}",
            described["job"].as_str().unwrap_or("")
        ))),
        JobState::Cancelled => Err(ToolError::Cancelled),
        _ => Ok(described),
    }
}

/// A job's answer and its edit's.
fn merge(answer: Value, edit: Value) -> Value {
    match (answer, edit) {
        (Value::Object(mut a), Value::Object(e)) => {
            for (k, v) in e {
                a.entry(k).or_insert(v);
            }
            Value::Object(a)
        }
        (a, e) => json!({ "result": a, "edit": e }),
    }
}
