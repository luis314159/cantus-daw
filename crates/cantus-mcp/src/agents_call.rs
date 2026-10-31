//! A tool call through the pump: limits, permission by class, `expected_revision`, the
//! change it joins, the plan, the confirmation, the execution with `Origin::Mcp`
//! (ADR 0081 §3–6).

use crate::agents::{AgentHost, Agents, AiAction, Answer, Ask, Outcome, PendingAsk, Prepared};
use crate::catalog::{self, CHANGE, EXPECTED_REVISION};
use crate::door::{Caller, Reply, ToolAnswer, UiReply};
use crate::generic;
use crate::jobs::{JobStart, Waiter};
use crate::jsonrpc::RpcError;
use crate::permissions::{self, Refusal};
use cantus_ai::{Decision, EditPlan, Job, Said, ToolCx, ToolDef, ToolError, ToolRun};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// What was prepared, what the card says, and the plan in words.
pub(crate) type Asking = (Prepared, String, Option<String>);

/// Take `expected_revision` (and, for a project tool, `change`) out of the arguments:
/// tools never see them.
fn take_server_args(
    args: &mut Value,
    with_change: bool,
) -> Result<(Option<u64>, Option<String>), ToolError> {
    let Some(obj) = args.as_object_mut() else {
        return Ok((None, None));
    };
    let expected = match obj.remove(EXPECTED_REVISION) {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().ok_or_else(|| {
            ToolError::bad(
                EXPECTED_REVISION,
                "an integer revision, as a result gave it",
            )
        })?),
    };
    if !with_change {
        return Ok((expected, None));
    }
    let change = match obj.remove(CHANGE) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s),
        Some(_) => return Err(ToolError::bad(CHANGE, "a change handle from begin_change")),
    };
    Ok((expected, change))
}

/// The result of a mutating call says the revision it left.
pub(crate) fn with_revision(mut result: Value, revision: u64) -> Value {
    if let Some(obj) = result.as_object_mut() {
        obj.entry("revision").or_insert(json!(revision));
        return result;
    }
    if result.is_null() {
        json!({ "revision": revision })
    } else {
        json!({ "result": result, "revision": revision })
    }
}

pub(crate) fn refusal(r: Refusal, tool: &str) -> ToolError {
    ToolError::Refused(r.words(tool))
}

/// A tool that may not change the project changed it (review 2 #13): reported, never
/// passed off as a read.
fn moved_unasked(def: &ToolDef) -> ToolError {
    ToolError::Failed(format!(
        "{} changed the project although it is not an edit tool; its answer is withheld — this is a fault in the tool, the user can undo the change",
        def.name
    ))
}

impl Agents {
    /// The change a mutating call joins: the one it names (it must be the caller's, in
    /// this project), or — for a session, never statelessly — the session's open one.
    pub(crate) fn change_for(
        &self,
        caller: &Caller,
        named: Option<&str>,
        epoch: u64,
    ) -> Result<Option<String>, ToolError> {
        if let Some(h) = named {
            return match self.changes.get(h) {
                Some(c) if c.owner == caller.key && c.epoch == epoch => Ok(Some(h.to_string())),
                _ => Err(ToolError::NotFound(format!(
                    "change {h} (it ended, belongs to another agent or project, or ended by itself after 2 minutes without a call)"
                ))),
            };
        }
        if caller.stateless {
            return Ok(None);
        }
        Ok(self
            .changes
            .values()
            .find(|c| c.owner == caller.key && c.implicit && c.epoch == epoch)
            .map(|c| c.handle.clone()))
    }

    /// The decision for `def` from `caller` now.
    pub(crate) fn decision<H: AgentHost>(
        &self,
        host: &H,
        caller: &Caller,
        def: &ToolDef,
    ) -> Result<Decision, ToolError> {
        let grants = self.grants.get(&caller.key).cloned().unwrap_or_default();
        match permissions::decide(
            def.class,
            def.name,
            caller.external,
            &host.policy(),
            &grants,
        ) {
            Ok(Decision::Deny) => Err(ToolError::Refused(format!(
                "the user does not allow {}",
                def.name
            ))),
            Ok(d) => Ok(d),
            Err(r) => Err(refusal(r, def.name)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call<H: AgentHost>(
        &mut self,
        host: &mut H,
        caller: Caller,
        name: &str,
        mut args: Value,
        reply: Reply,
        now: Instant,
        (deadline, cancel): (Instant, Arc<AtomicBool>),
    ) {
        let Some(def) = self.catalog.tool(name) else {
            reply.send(UiReply::Rpc(RpcError::invalid_params(format!(
                "unknown tool: {name}"
            ))));
            return;
        };
        let mutating = catalog::mutates(def.class);
        let own = generic::is_generic(def.name);
        if let Err(wait) = self.rates.take(&caller.key, mutating, caller.external, now) {
            let why = format!(
                "too many calls: wait {} ms and try again",
                wait.as_millis().max(1)
            );
            return self.finish(&caller, def.name, reply, Err(ToolError::Refused(why)));
        }
        let decision = match self.decision(host, &caller, def) {
            Ok(d) => d,
            Err(e) => return self.finish(&caller, def.name, reply, Err(e)),
        };
        // Only mutating tools take the server's arguments (their schema says so), and the
        // server's own tools no change (review 2 #7).
        let taken = if mutating {
            take_server_args(&mut args, !own)
        } else {
            Ok((None, None))
        };
        let (expected, named) = match taken {
            Ok(both) => both,
            Err(e) => return self.finish(&caller, def.name, reply, Err(e)),
        };
        // The arguments fit the tool's schema: a misspelt or unknown key, a wrong type, a
        // value out of range is the agent's to correct, before anything is planned.
        // An optional argument sent as `null` is one left out (review 4 #5).
        let schema = (def.input_schema)();
        crate::schema::drop_nulls(&schema, &mut args);
        if let Err(m) = crate::schema::check(&schema, &args) {
            let e = ToolError::bad(m.path, m.reason);
            return self.finish(&caller, def.name, reply, Err(e));
        }
        if expected.is_some_and(|e| e != host.bus().revision()) {
            let stale = ToolError::Stale(host.bus().revision());
            return self.finish(&caller, def.name, reply, Err(stale));
        }
        let change = if own {
            None
        } else {
            match self.change_for(&caller, named.as_deref(), host.epoch()) {
                Ok(c) => c,
                Err(e) => return self.finish(&caller, def.name, reply, Err(e)),
            }
        };
        let ask = decision == Decision::AskUser;
        let deadline = deadline.min(now + self.config.ask_timeout);
        if own {
            if ask {
                // The card names the step it would undo, and only that step is undone.
                let Some(top) = host.bus().history().last() else {
                    let e = ToolError::Failed("there is nothing to undo".into());
                    return self.finish(&caller, def.name, reply, Err(e));
                };
                // Not its own step: refused at once, saying whose — never put on the card
                // (review 3 #14).
                if self
                    .step_owner(top.step())
                    .is_none_or(|o| o.key != caller.key)
                {
                    return self.generic(host, caller, def, args, reply, now);
                }
                let words = self
                    .step_words(top.step())
                    .unwrap_or_else(|| host.describe_step(top));
                let said = host.say(&Said::new("ai.said.undo_last").arg("what", words));
                let p = (Prepared::Undo(top.step()), said, None);
                let at = (now, deadline, cancel.clone());
                return self.ask(host, caller, def, args, expected, None, p, reply, at, false);
            }
            return self.generic(host, caller, def, args, reply, now);
        }
        let before = host.bus().revision();
        match def.run {
            ToolRun::Read(f) => {
                let r = f(&ToolCx { host }, &args);
                self.finish(&caller, def.name, reply, r);
            }
            ToolRun::Host(f) | ToolRun::HostSaid { run: f, .. } => {
                if ask {
                    // The card's sentence: the tool's own, worded from its arguments, or its
                    // title.
                    let said = match def.run {
                        ToolRun::HostSaid { said, .. } => {
                            let words = said(&ToolCx { host }, &args);
                            host.say(&words)
                        }
                        _ => def.title.to_string(),
                    };
                    let p = (Prepared::Host, said, None);
                    let at = (now, deadline, cancel.clone());
                    return self.ask(
                        host, caller, def, args, expected, change, p, reply, at, false,
                    );
                }
                let mut r = f(&mut ToolCx { host }, &args);
                if !mutating && host.bus().revision() != before {
                    r = Err(moved_unasked(def));
                }
                self.finish(&caller, def.name, reply, r);
            }
            ToolRun::Edit(f) => {
                match planned(host, before, def, |h| f(&mut ToolCx { host: h }, &args)) {
                    Err(e) => self.finish(&caller, def.name, reply, Err(e)),
                    Ok(plan) if ask => {
                        let said = host.say(&plan.said);
                        let preview = host.preview(&plan.command);
                        let rev = host.bus().revision();
                        let p = (Prepared::Edit(plan, rev), said, preview);
                        let at = (now, deadline, cancel.clone());
                        self.ask(
                            host, caller, def, args, expected, change, p, reply, at, false,
                        );
                    }
                    Ok(plan) => {
                        let r = self.execute_plan(host, &caller, def, plan, change.as_deref());
                        self.finish(&caller, def.name, reply, r);
                    }
                }
            }
            ToolRun::Job(f) => {
                if let Err(e) = self.job_room(&caller) {
                    return self.finish(&caller, def.name, reply, Err(e));
                }
                let job = planned(host, before, def, |h| f(&mut ToolCx { host: h }, &args));
                match job {
                    Err(e) => self.finish(&caller, def.name, reply, Err(e)),
                    Ok(job) if ask => {
                        let said = host.say(&job.said);
                        let rev = host.bus().revision();
                        let p = (Prepared::Job(job, rev), said, None);
                        let at = (now, deadline, cancel.clone());
                        self.ask(
                            host, caller, def, args, expected, change, p, reply, at, false,
                        );
                    }
                    Ok(job) => {
                        let started = (now, cancel.clone());
                        self.start_job(host, caller, def, job, change, false, reply, started)
                    }
                }
            }
        }
    }

    /// Put a call on the card. `at` is (when it is asked, when it stops waiting); a re-ask
    /// (`again`) replaces the call it continues, so it meets no cap (review 2 #14).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ask<H: AgentHost>(
        &mut self,
        host: &H,
        caller: Caller,
        def: &'static ToolDef,
        args: Value,
        expected: Option<u64>,
        change: Option<String>,
        (prepared, said, preview): Asking,
        reply: Reply,
        (now, deadline, cancel): (Instant, Instant, Arc<AtomicBool>),
        again: bool,
    ) {
        let mine = self
            .asks
            .iter()
            .filter(|p| p.ask.caller.key == caller.key)
            .count();
        let pooled = self
            .asks
            .iter()
            .filter(|p| p.ask.caller.external == caller.external)
            .count();
        let why = if again {
            None
        } else if mine >= self.config.max_asks_per_client {
            Some(format!(
                "{mine} of your calls are waiting for the user's answer; wait for them first"
            ))
        } else if pooled >= self.config.max_asks {
            Some("too many calls are waiting for the user's answer; try again later".into())
        } else {
            None
        };
        if let Some(why) = why {
            return self.finish(&caller, def.name, reply, Err(ToolError::Refused(why)));
        }
        self.note(&caller, def.name, Outcome::Asked);
        let id = self.next_ask_id();
        self.asks.push(PendingAsk {
            ask: Ask {
                id,
                caller,
                tool: def.name,
                class: def.class,
                said,
                preview,
                asked: now,
                deadline,
                grantable: permissions::grantable(def.class),
            },
            def,
            args,
            expected,
            change,
            prepared,
            reply,
            answer: None,
            epoch: host.epoch(),
            deadline,
            cancel,
        });
    }

    /// Carry out what the user answered, and refuse what waited too long.
    pub(crate) fn settle_asks<H: AgentHost>(&mut self, host: &mut H, now: Instant) {
        // A grant for the session answers the same agent's other calls of that tool.
        for p in &mut self.asks {
            let granted = self
                .grants
                .get(&p.ask.caller.key)
                .is_some_and(|g| g.tools.contains(p.ask.tool));
            if p.answer.is_none() && granted && permissions::grantable(p.ask.class) {
                p.answer = Some(Answer::Allow);
            }
        }
        for p in std::mem::take(&mut self.asks) {
            // Its client cancelled it or hung up: the card goes, nothing is done.
            if p.cancel.load(Ordering::SeqCst) {
                self.note(&p.ask.caller, p.def.name, Outcome::Dropped);
                continue;
            }
            match p.answer {
                None if now < p.deadline => self.asks.push(p),
                None => {
                    self.note(&p.ask.caller, p.def.name, Outcome::NoAnswer);
                    p.reply.tool(ToolAnswer::Error(
                        "the user did not answer in time; nothing was done".into(),
                    ));
                }
                Some(Answer::Deny) => {
                    self.note(&p.ask.caller, p.def.name, Outcome::DeniedByUser);
                    p.reply.tool(ToolAnswer::Error(
                        "the user said no; nothing was done".into(),
                    ));
                }
                Some(Answer::Allow | Answer::AllowForSession) => self.allowed(host, p, now),
            }
        }
    }
}

/// Whether two plans are the same to the user: what they say and what they would touch
/// (review 2 #5 — words alone miss a tool whose words do not name its target).
fn same_plan<H: AgentHost>(
    host: &H,
    plan: &EditPlan,
    said: &str,
    preview: &Option<String>,
) -> bool {
    host.say(&plan.said) == said && host.preview(&plan.command) == *preview
}

impl Agents {
    /// The user allowed a call: run it. The permission is decided again (a tool denied, the
    /// door closed meanwhile); if the project moved, the tool plans again and — unless the
    /// decision is now *allow* — the user is asked again when the new plan would do
    /// something else (review 1 #1, review 2 #5, #14); a call from another project's
    /// time is refused.
    fn allowed<H: AgentHost>(&mut self, host: &mut H, p: PendingAsk, now: Instant) {
        let PendingAsk {
            ask,
            def,
            args,
            expected,
            change,
            prepared,
            reply,
            epoch,
            deadline,
            cancel,
            ..
        } = p;
        let caller = ask.caller.clone();
        let revision = host.bus().revision();
        let now_decided = match self.decision(host, &caller, def) {
            Ok(d) => d,
            Err(e) => return self.finish(&caller, def.name, reply, Err(e)),
        };
        if epoch != host.epoch() {
            let gone = ToolError::Refused(
                "another project was opened while the user was asked; nothing was done".into(),
            );
            return self.finish(&caller, def.name, reply, Err(gone));
        }
        if expected.is_some_and(|e| e != revision) {
            return self.finish(&caller, def.name, reply, Err(ToolError::Stale(revision)));
        }
        if let Some(h) = change.as_ref().filter(|h| !self.changes.contains_key(*h)) {
            let e = ToolError::NotFound(format!("change {h} (it ended while the user was asked)"));
            return self.finish(&caller, def.name, reply, Err(e));
        }
        let at = (now, deadline, cancel.clone());
        match prepared {
            Prepared::Undo(step) => {
                // Only the step the card named (review 2 #6).
                if host.bus().history().last().map(|e| e.step()) != Some(step) {
                    let e = ToolError::Refused(
                        "the last step changed while the user was asked; nothing was undone".into(),
                    );
                    return self.finish(&caller, def.name, reply, Err(e));
                }
                self.generic(host, caller, def, args, reply, now)
            }
            Prepared::Host => {
                let r = match def.run {
                    ToolRun::Host(f) | ToolRun::HostSaid { run: f, .. } => {
                        f(&mut ToolCx { host }, &args)
                    }
                    _ => Err(ToolError::Failed("not a host tool".into())),
                };
                self.finish(&caller, def.name, reply, r);
            }
            Prepared::JobEdit(plan, at_rev) => {
                let r = if at_rev == revision {
                    self.execute_plan(host, &caller, def, plan, change.as_deref())
                } else {
                    Err(ToolError::Refused(
                        "the project changed while the user was asked; the job's edit was not made"
                            .into(),
                    ))
                };
                self.finish(&caller, def.name, reply, r);
            }
            Prepared::Edit(plan, at_rev) => {
                if at_rev == revision {
                    let r = self.execute_plan(host, &caller, def, plan, change.as_deref());
                    return self.finish(&caller, def.name, reply, r);
                }
                match replan(host, def, &args) {
                    Err(e) => self.finish(&caller, def.name, reply, Err(e)),
                    Ok(plan)
                        if now_decided == Decision::Allow
                            || same_plan(host, &plan, &ask.said, &ask.preview) =>
                    {
                        let r = self.execute_plan(host, &caller, def, plan, change.as_deref());
                        self.finish(&caller, def.name, reply, r);
                    }
                    Ok(plan) => {
                        // It would do something else now: the user sees it and answers again.
                        let said = host.say(&plan.said);
                        let preview = host.preview(&plan.command);
                        let p = (Prepared::Edit(plan, revision), said, preview);
                        self.ask(
                            host, caller, def, args, expected, change, p, reply, at, true,
                        );
                    }
                }
            }
            Prepared::Job(job, at_rev) => {
                if at_rev == revision {
                    let started = (now, cancel.clone());
                    return self.start_job(host, caller, def, job, change, true, reply, started);
                }
                let job = match def.run {
                    ToolRun::Job(f) => f(&mut ToolCx { host }, &args),
                    _ => Err(ToolError::Failed("not a job".into())),
                };
                match job {
                    Err(e) => self.finish(&caller, def.name, reply, Err(e)),
                    Ok(job)
                        if now_decided == Decision::Allow || host.say(&job.said) == ask.said =>
                    {
                        let started = (now, cancel.clone());
                        self.start_job(host, caller, def, job, change, true, reply, started)
                    }
                    Ok(job) => {
                        let said = host.say(&job.said);
                        let p = (Prepared::Job(job, revision), said, None);
                        self.ask(
                            host, caller, def, args, expected, change, p, reply, at, true,
                        );
                    }
                }
            }
        }
    }

    /// Execute a plan as the agent's step (merged into `change`'s, if it names one). Only
    /// a tool whose class changes the project may (review 1 #5).
    pub(crate) fn execute_plan<H: AgentHost>(
        &mut self,
        host: &mut H,
        caller: &Caller,
        def: &'static ToolDef,
        plan: EditPlan,
        change: Option<&str>,
    ) -> Result<Value, ToolError> {
        if !catalog::mutates(def.class) {
            return Err(ToolError::Failed(format!(
                "{} is not a tool that may change the project; nothing was done",
                def.name
            )));
        }
        let now = Instant::now();
        let joined = change.and_then(|h| self.changes.get_mut(h)).map(|c| {
            c.calls += 1;
            c.last = now;
            (c.key, (c.handle.clone(), c.label.clone()))
        });
        let said = host.say(&plan.said);
        let mark = host.bus().mark();
        let top_before = host.bus().current_step();
        host.execute(plan.command, joined.as_ref().map(|c| c.0))
            .map_err(ToolError::Failed)?;
        let revision = host.bus().revision();
        let step = host.bus().current_step();
        // Merged into the step on top (a change's later call) — known exactly: no step was
        // pushed (review 2 #3; a full history drops its oldest on a push, so its length
        // says nothing). That step has a new number, and the change's earlier calls follow.
        if joined.is_some() && host.bus().mark() == mark {
            for a in self.actions.iter_mut() {
                if a.step == top_before {
                    a.step = step;
                }
            }
        }
        self.actions.push_back(AiAction {
            step,
            epoch: host.epoch(),
            caller: caller.clone(),
            tool: def.name,
            said,
            change: joined.map(|c| c.1),
            revision,
            at: now,
        });
        while self.actions.len() > crate::agents::KEEP_ACTIONS {
            self.actions.pop_front();
        }
        Ok(with_revision(plan.result, revision))
    }

    fn job_room(&self, caller: &Caller) -> Result<(), ToolError> {
        let mine = self.jobs.running(Some(&caller.key));
        if mine >= self.config.max_jobs_per_client {
            return Err(ToolError::Refused(format!(
                "{mine} of your jobs are running; wait for one (wait_job) or cancel one first"
            )));
        }
        if self.jobs.running(None) >= self.config.max_jobs {
            return Err(ToolError::Refused(
                "Cantus is running as many jobs as it can; try again shortly".into(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn start_job<H: AgentHost>(
        &mut self,
        host: &mut H,
        caller: Caller,
        def: &'static ToolDef,
        job: Job,
        change: Option<String>,
        asked: bool,
        reply: Reply,
        (now, cancel): (Instant, Arc<AtomicBool>),
    ) {
        if let Err(e) = self.job_room(&caller) {
            return self.finish(&caller, def.name, reply, Err(e));
        }
        let Some(wake) = self.inbox.as_ref().map(|i| i.waker()) else {
            let e = ToolError::Failed("no window".into());
            return self.finish(&caller, def.name, reply, Err(e));
        };
        let said = host.say(&job.said);
        let start = JobStart {
            epoch: host.epoch(),
            revision: host.bus().revision(),
            change,
            asked,
        };
        match self
            .jobs
            .start(caller.clone(), def.name, said, start, job.work, wake)
        {
            Ok(handle) => {
                self.note(&caller, def.name, Outcome::JobStarted(handle.clone()));
                let waiter = Waiter {
                    reply,
                    deadline: now + self.config.inline_job_wait,
                    starting_call: true,
                    cancel: Some(cancel),
                };
                self.jobs.waiters.push((handle, waiter));
            }
            Err(e) => self.finish(&caller, def.name, reply, Err(e)),
        }
    }

    fn next_ask_id(&mut self) -> u64 {
        let id = self.next_ask;
        self.next_ask += 1;
        id
    }
}

fn replan<H: AgentHost>(
    host: &mut H,
    def: &'static ToolDef,
    args: &Value,
) -> Result<EditPlan, ToolError> {
    let before = host.bus().revision();
    match def.run {
        ToolRun::Edit(f) => planned(host, before, def, |h| f(&mut ToolCx { host: h }, args)),
        _ => Err(ToolError::Failed("not an edit".into())),
    }
}

/// Run a tool's planning (`Edit`, `Job`, a job's `Then`): it may allocate ids but never
/// change the project — if it did, its plan is refused and the fault reported (review 3
/// #10: a plan that executed would be an edit nobody allowed).
pub(crate) fn planned<H: AgentHost, T>(
    host: &mut H,
    before: u64,
    def: &ToolDef,
    plan: impl FnOnce(&mut H) -> Result<T, ToolError>,
) -> Result<T, ToolError> {
    let r = plan(host);
    if host.bus().revision() != before {
        return Err(moved_unasked(def));
    }
    r
}
