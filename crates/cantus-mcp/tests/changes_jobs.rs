//! The server's own tools: a change groups a turn into one undo step; an agent undoes only
//! its own steps; jobs run beside the window and are waited on, listed, cancelled.

mod common;

use cantus_core::{Command, Origin};
use cantus_mcp::Answer;
use common::Rig;
use serde_json::{json, Value};
use std::time::Duration;

fn change_of(r: &cantus_mcp::client::ToolCall) -> String {
    r.structured.as_ref().unwrap()["change"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_change_is_one_undo_step_labelled_and_undone_whole() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let before = rig.host.bus.project().clone();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(
        &mut c,
        "begin_change",
        json!({"label": "write the verse"}),
        None,
    );
    assert!(!r.is_error, "{}", r.text);
    let change = change_of(&r);
    let again = rig.call(&mut c, "begin_change", json!({"label": "another"}), None);
    assert!(
        again.is_error && again.text.contains(&change),
        "one change at a time"
    );
    for args in [json!({"bpm": 100}), json!({"bpm": 104})] {
        assert!(!rig.call(&mut c, "set_tempo", args, None).is_error);
    }
    assert!(!rig.call(&mut c, "add_track", json!({}), None).is_error);
    let r = rig.call(&mut c, "end_change", json!({"change": change}), None);
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(r.structured.as_ref().unwrap()["calls"], 3);
    assert_eq!(rig.host.bus.history().len(), 1, "three calls, one step");
    let top = rig.host.bus.history().last().unwrap();
    assert_eq!(top.origin, Origin::Mcp);
    assert_eq!(
        rig.agents.step_words(top.step()).as_deref(),
        Some("write the verse")
    );
    // After the change, a call is a step of its own again.
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 110}), None)
            .is_error
    );
    assert_eq!(rig.host.bus.history().len(), 2);
    rig.host.bus.undo();
    rig.host.bus.undo();
    assert_eq!(
        common::ids_aside(rig.host.bus.project()),
        common::ids_aside(&before),
        "the change undoes whole and exactly"
    );
    let r = rig.call(&mut c, "end_change", json!({"change": change}), None);
    assert!(r.is_error, "an ended change is gone");
}

#[test]
fn a_change_never_swallows_the_users_step_and_ends_when_idle() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    let change = change_of(&rig.call(&mut c, "begin_change", json!({"label": "x"}), None));
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    rig.host
        .bus
        .execute(
            Command::SetProjectName {
                name: "mine".into(),
            },
            Origin::User,
        )
        .unwrap();
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 101}), None)
            .is_error
    );
    let origins: Vec<Origin> = rig.host.bus.history().iter().map(|e| e.origin).collect();
    assert_eq!(origins, [Origin::Mcp, Origin::User, Origin::Mcp]);
    rig.host.bus.undo();
    assert_eq!(
        rig.host.bus.project().name,
        "mine",
        "the user's step stays its own"
    );
    rig.host.bus.redo();
    // A change with no call for its idle time ends by itself.
    rig.agents.config.change_idle = Duration::ZERO;
    rig.pump();
    assert!(
        rig.call(&mut c, "end_change", json!({"change": change}), None)
            .is_error
    );
}

#[test]
fn an_agent_undoes_its_own_last_step_only() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut a = rig.legacy("2025-11-25");
    let mut b = rig.legacy("2025-11-25");
    assert!(
        !rig.call(&mut a, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    // Another agent's step on top: refused, saying whose.
    assert!(!rig.call(&mut b, "add_track", json!({}), None).is_error);
    let r = rig.call(
        &mut a,
        "undo_last_ai_action",
        json!({}),
        Some(Answer::Allow),
    );
    assert!(r.is_error && r.text.contains("another agent"), "{}", r.text);
    assert_eq!(rig.host.bus.project().tracks.len(), 2);
    let r = rig.call(
        &mut b,
        "undo_last_ai_action",
        json!({}),
        Some(Answer::Allow),
    );
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        r.structured.as_ref().unwrap()["undone"],
        "ai.test.add_track"
    );
    assert_eq!(rig.host.bus.project().tracks.len(), 1);
    // The user's step on top: refused, naming it.
    rig.host
        .bus
        .execute(
            Command::SetProjectName {
                name: "mine".into(),
            },
            Origin::User,
        )
        .unwrap();
    let r = rig.call(
        &mut a,
        "undo_last_ai_action",
        json!({}),
        Some(Answer::Allow),
    );
    assert!(r.is_error && r.text.contains("user's"), "{}", r.text);
    assert!(r.text.contains("cmd.set_project_name"), "{}", r.text);
    assert_eq!(rig.host.bus.project().name, "mine");
    // Undo is destructive: it asks.
    rig.host.bus.undo();
    rig.agents.config.ask_timeout = Duration::ZERO;
    let r = rig.call(&mut a, "undo_last_ai_action", json!({}), None);
    assert!(
        r.is_error && r.text.contains("did not answer"),
        "{}",
        r.text
    );
    assert_eq!(rig.host.bus.project().tempo_bpm, 100.0);
}

#[test]
fn list_ai_actions_says_what_is_still_in_the_project() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    assert!(!rig.call(&mut c, "add_track", json!({}), None).is_error);
    rig.host.bus.undo();
    let r = rig.call(&mut c, "list_ai_actions", json!({}), None);
    let actions = r.structured.unwrap()["actions"].as_array().unwrap().clone();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0]["tool"], "add_track");
    assert_eq!(actions[0]["in_project"], false);
    assert_eq!(actions[1]["in_project"], true);
    let r = rig.call(&mut c, "list_ai_actions", json!({"limit": 0}), None);
    assert!(r.is_error);
}

#[test]
fn a_quick_job_answers_its_result_in_the_call() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "count", json!({"steps": 2}), None);
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(r.structured.unwrap()["counted"], 2);
}

#[test]
fn a_long_job_answers_a_handle_to_wait_on() {
    let mut rig = Rig::new();
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "count", json!({"steps": 20, "ms": 5}), None);
    assert!(!r.is_error, "{}", r.text);
    let s = r.structured.unwrap();
    let job = s["job"].as_str().unwrap().to_string();
    assert!(job.starts_with("job-"));
    assert_eq!(s["state"], "running");
    assert!(s["next"].as_str().unwrap().contains("wait_job"));
    let l = rig.call(&mut c, "list_jobs", json!({}), None);
    assert_eq!(l.structured.unwrap()["jobs"][0]["job"], job);
    let w = rig.call(
        &mut c,
        "wait_job",
        json!({"job": job, "timeout_s": 30}),
        None,
    );
    assert!(!w.is_error, "{}", w.text);
    let s = w.structured.unwrap();
    assert_eq!(s["state"], "done");
    assert_eq!(s["progress"], 1.0);
    assert_eq!(s["result"]["counted"], 20);
    let w = rig.call(&mut c, "wait_job", json!({"job": "job-nope"}), None);
    assert!(w.is_error && w.text.contains("not found"), "{}", w.text);
    let w = rig.call(
        &mut c,
        "wait_job",
        json!({"job": job, "timeout_s": 99}),
        None,
    );
    assert!(w.is_error, "timeout_s is bounded");
}

#[test]
fn waiting_past_the_timeout_says_the_job_still_runs_and_cancel_stops_it() {
    let mut rig = Rig::new();
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "count", json!({"steps": 2000, "ms": 5}), None);
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    let w = rig.call(
        &mut c,
        "wait_job",
        json!({"job": job, "timeout_s": 0.05}),
        None,
    );
    assert!(!w.is_error);
    let s = w.structured.unwrap();
    assert_eq!(s["state"], "running");
    assert!(s["progress"].as_f64().unwrap() < 1.0);
    let x = rig.call(&mut c, "cancel_job", json!({"job": job}), None);
    assert_eq!(x.structured.unwrap()["state"], "cancelling");
    let w = rig.call(&mut c, "wait_job", json!({"job": job}), None);
    assert!(w.is_error && w.text.contains("cancelled"), "{}", w.text);
}

#[test]
fn a_job_that_panics_fails_alone() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "count", json!({"panic": true}), None);
    assert!(
        r.is_error && r.text.contains("internal error"),
        "{}",
        r.text
    );
    assert!(
        !rig.call(&mut c, "get_tempo", json!({}), None).is_error,
        "Cantus goes on"
    );
}

#[test]
fn a_jobs_edit_is_planned_on_the_ui_thread_as_one_step_after_the_users_leave() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(
        &mut c,
        "measure_tempo",
        json!({"bpm": 87}),
        Some(Answer::Allow),
    );
    assert!(!r.is_error, "{}", r.text);
    let s: Value = r.structured.unwrap();
    assert_eq!(s["measured"], 87.0);
    assert_eq!(s["bpm"], 87.0);
    assert_eq!(s["revision"], rig.host.bus.revision());
    assert_eq!(rig.host.bus.project().tempo_bpm, 87.0);
    assert_eq!(rig.host.bus.history().len(), 1);
    assert_eq!(rig.host.bus.history()[0].origin, Origin::Mcp);
    // Refused by the user: the job never runs.
    let r = rig.call(
        &mut c,
        "measure_tempo",
        json!({"bpm": 60}),
        Some(Answer::Deny),
    );
    assert!(r.is_error);
    assert_eq!(rig.host.bus.project().tempo_bpm, 87.0);
    assert_eq!(rig.agents.jobs().iter().count(), 1);
}

#[test]
fn jobs_are_limited_per_client() {
    let mut rig = Rig::new();
    rig.agents.config.inline_job_wait = Duration::ZERO;
    rig.agents.config.max_jobs_per_client = 1;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "count", json!({"steps": 2000, "ms": 5}), None);
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    let r = rig.call(&mut c, "count", json!({"steps": 1}), None);
    assert!(r.is_error && r.text.contains("running"), "{}", r.text);
    rig.call(&mut c, "cancel_job", json!({"job": job}), None);
    let _ = rig.call(&mut c, "wait_job", json!({"job": job}), None);
    assert!(
        !rig.call(&mut c, "count", json!({"steps": 1}), None)
            .is_error
    );
}
