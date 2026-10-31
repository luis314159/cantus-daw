//! The first review's findings (round 34, mcp), each held by a test that failed before
//! its fix.

mod common;

use cantus_core::{Command, Origin};
use cantus_mcp::client::TestClient;
use cantus_mcp::{Answer, Limits, Outcome, Server, ServerInfo, Session};
use common::Rig;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

fn change_of(r: &cantus_mcp::client::ToolCall) -> String {
    r.structured.as_ref().unwrap()["change"]
        .as_str()
        .unwrap()
        .to_string()
}

/// #1: the card says "remove Bass"; the user inserts "Mine" first and allows: the new
/// plan says something else, so the user is asked again — and "Mine" is not removed.
#[test]
fn a_plan_that_would_now_do_something_else_is_asked_again() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut answers = 0;
    let mut seen = Vec::new();
    let r = c.call_tool("remove_first_track", json!({}), &mut || {
        agents.pump(host);
        let asks: Vec<(u64, String)> = agents.asks().map(|a| (a.id, a.said.clone())).collect();
        for (id, said) in asks {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if answers == 0 {
                assert_eq!(said, "ai.test.remove name=Bass");
                let t = host.bus.new_track("Mine");
                host.bus
                    .execute(
                        Command::InsertTrack {
                            index: 0,
                            track: Box::new(t),
                        },
                        Origin::User,
                    )
                    .unwrap();
                agents.answer(id, Answer::Allow);
            } else {
                assert_eq!(
                    said, "ai.test.remove name=Mine",
                    "the card says the new plan"
                );
                agents.answer(id, Answer::Deny);
            }
            answers += 1;
        }
    });
    assert!(r.is_error && r.text.contains("said no"), "{}", r.text);
    assert_eq!(answers, 2, "asked twice");
    let names: Vec<&str> = host
        .bus
        .project()
        .tracks
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names, ["Mine", "Bass"], "nothing was removed");
}

/// #1: a confirmation from before Open/New is refused, never run in the new project.
#[test]
fn a_confirmation_from_another_project_is_refused() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let r = c.call_tool("remove_first_track", json!({}), &mut || {
        agents.pump(host);
        let first = agents.asks().map(|a| a.id).next();
        if let Some(id) = first {
            host.epoch += 1; // another project opened while the card was up
            agents.answer(id, Answer::Allow);
        }
    });
    assert!(
        r.is_error && r.text.contains("another project"),
        "{}",
        r.text
    );
    assert_eq!(host.bus.project().tracks.len(), 1);
}

/// #1: a job's edit lands only in the project it was started in.
#[test]
fn a_jobs_edit_from_another_project_is_not_made() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "measure_tempo", json!({"bpm": 77, "ms": 200}), None);
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    rig.host.epoch += 1;
    let w = rig.call(&mut c, "wait_job", json!({"job": job}), None);
    assert!(
        w.is_error && w.text.contains("another project"),
        "{}",
        w.text
    );
    assert_eq!(rig.host.bus.project().tempo_bpm, 120.0);
}

/// #2: "Allow for this session" on a destructive tool allows that call only; on an edit
/// tool it covers that tool, not the class.
#[test]
fn a_session_grant_is_one_edit_tool_never_destruction() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    rig.host.policy.allow_edits = true;
    let _ = rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None);
    rig.host.policy.allow_edits = false;
    let r = rig.call(
        &mut c,
        "undo_last_ai_action",
        json!({}),
        Some(Answer::AllowForSession),
    );
    assert!(!r.is_error, "{}", r.text);
    rig.agents.config.ask_timeout = Duration::ZERO;
    let r = rig.call(&mut c, "remove_first_track", json!({}), None);
    assert!(
        r.is_error && r.text.contains("did not answer"),
        "destruction still asks: {}",
        r.text
    );
    rig.agents.config.ask_timeout = Duration::from_secs(120);
    assert!(
        !rig.call(
            &mut c,
            "set_tempo",
            json!({"bpm": 101}),
            Some(Answer::AllowForSession)
        )
        .is_error
    );
    let asked = |rig: &Rig| {
        rig.agents
            .activity()
            .filter(|a| a.outcome == Outcome::Asked)
            .count()
    };
    let n = asked(&rig);
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 102}), None)
            .is_error
    );
    assert_eq!(asked(&rig), n, "the granted tool runs unasked");
    rig.agents.config.ask_timeout = Duration::ZERO;
    assert!(
        rig.call(&mut c, "add_track", json!({}), None).is_error,
        "another edit tool asks"
    );
}

/// #3: in 2026-07-28 a change is joined only by naming it; nothing hangs on the connection.
#[test]
fn a_stateless_change_is_joined_only_by_its_handle() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut a = rig.modern();
    let ch = change_of(&rig.call(&mut a, "begin_change", json!({"label": "verse"}), None));
    // Another subagent on the same credential, not naming it: a step of its own.
    assert!(
        !rig.call(&mut a, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    for bpm in [101, 102] {
        let r = rig.call(&mut a, "set_tempo", json!({"bpm": bpm, "change": ch}), None);
        assert!(!r.is_error, "{}", r.text);
    }
    assert!(
        !rig.call(&mut a, "add_track", json!({"change": ch}), None)
            .is_error
    );
    assert_eq!(
        rig.host.bus.history().len(),
        2,
        "one plain step, one change"
    );
    // Another credential cannot join it.
    let mut b = rig.modern();
    let r = rig.call(&mut b, "add_track", json!({"change": ch}), None);
    assert!(r.is_error && r.text.contains("not found"), "{}", r.text);
    let r = rig.call(&mut b, "end_change", json!({"change": ch}), None);
    assert!(r.is_error);
    let r = rig.call(&mut a, "end_change", json!({"change": ch}), None);
    assert_eq!(r.structured.unwrap()["calls"], 3);
}

/// #3, #4: a stateless agent is its credential: renaming its client changes nothing —
/// not its limits, not its caps, not what it owns.
#[test]
fn renaming_the_client_does_not_make_another_agent() {
    let mut rig = Rig::new();
    let session = Session::external("http");
    rig.host.policy.external_agents = true;
    let mut a = TestClient::modern_on(rig.server.clone(), session.clone());
    let mut b = TestClient::modern_on(rig.server.clone(), session);
    b.name = "renamed".into();
    assert!(!rig.call(&mut a, "get_tempo", json!({}), None).is_error);
    assert!(!rig.call(&mut b, "get_tempo", json!({}), None).is_error);
    let callers: Vec<_> = rig.agents.activity().map(|x| x.caller.clone()).collect();
    assert_eq!(callers[0].key, callers[1].key, "one agent");
    assert_ne!(
        callers[0].name, callers[1].name,
        "named as it says, for display only"
    );
    assert_eq!(rig.agents.connected(std::time::Instant::now()).len(), 1);
    // What one name opened, the other owns too (and no third credential does).
    let ch = change_of(&rig.call(&mut a, "begin_change", json!({"label": "x"}), None));
    let r = rig.call(&mut b, "end_change", json!({"change": ch}), None);
    assert!(!r.is_error, "{}", r.text);
}

/// #4: confirmations have a cap over every agent together.
#[test]
fn confirmations_have_a_global_cap() {
    let mut rig = Rig::new();
    rig.agents.config.max_asks = 0;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "set_tempo", json!({"bpm": 90}), None);
    assert!(
        r.is_error && r.text.contains("too many calls are waiting"),
        "{}",
        r.text
    );
}

/// #5: an edit under a reading class, or a reading job's edit, never changes the project.
#[test]
fn only_a_mutating_class_may_change_the_project() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "sneaky_edit", json!({"bpm": 60}), None);
    assert!(
        r.is_error && r.text.contains("may change the project"),
        "{}",
        r.text
    );
    let r = rig.call(&mut c, "sneaky_job", json!({"bpm": 61}), None);
    assert!(
        r.is_error && r.text.contains("may change the project"),
        "{}",
        r.text
    );
    assert_eq!(rig.host.bus.project().tempo_bpm, 120.0);
    assert!(rig.host.bus.history().is_empty());
}

/// #6: every call of a change is in the project until the change is undone.
#[test]
fn a_changes_calls_are_all_in_the_project_until_undone() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    let ch = change_of(&rig.call(&mut c, "begin_change", json!({"label": "x"}), None));
    for bpm in [100, 101, 102] {
        rig.call(&mut c, "set_tempo", json!({"bpm": bpm}), None);
    }
    rig.call(&mut c, "end_change", json!({"change": ch}), None);
    let list = |rig: &mut Rig, c: &mut TestClient| -> Vec<Value> {
        let r = rig.call(c, "list_ai_actions", json!({}), None);
        r.structured.unwrap()["actions"].as_array().unwrap().clone()
    };
    let a = list(&mut rig, &mut c);
    assert_eq!(a.len(), 3);
    assert!(a.iter().all(|x| x["in_project"] == true), "{a:?}");
    rig.host.bus.undo();
    let a = list(&mut rig, &mut c);
    assert!(a.iter().all(|x| x["in_project"] == false), "{a:?}");
}

/// #7: a mutating tool's output schema says `revision`; a job's handle is not offered as
/// the tool's structured result when it declares one.
#[test]
fn structured_content_matches_the_declared_output_schema() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    let list = c.request("tools/list", json!({}), &mut || {}).unwrap();
    let typed = list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "set_tempo_typed")
        .unwrap()
        .clone();
    assert_eq!(
        typed["outputSchema"]["properties"]["revision"]["type"],
        "integer"
    );
    let r = rig.call(&mut c, "set_tempo_typed", json!({"bpm": 99}), None);
    let s = r.structured.unwrap();
    assert!(s["bpm"].is_number() && s["revision"].is_u64());
    let r = rig.call(&mut c, "count_typed", json!({"steps": 2000, "ms": 5}), None);
    assert!(!r.is_error);
    // Review 2 #1: the handle is structured, and the schema names it (anyOf).
    let job = r
        .structured
        .clone()
        .expect("structured, as the schema promises");
    assert_eq!(job["state"], "running");
    let typed_job = list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "count_typed")
        .unwrap()
        .clone();
    assert_eq!(
        typed_job["outputSchema"]["type"], "object",
        "the root is an object (review 3 #1)"
    );
    let any = typed_job["outputSchema"]["anyOf"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(any.len(), 2);
    assert_eq!(any[0]["required"], json!(["counted"]));
    assert_eq!(any[1]["required"], json!(["job", "state"]));
    rig.call(&mut c, "cancel_job", json!({"job": job["job"]}), None);
}

/// #8: a call its transport gave up on is never run later.
#[test]
fn a_call_the_transport_gave_up_on_never_runs() {
    let (door, inbox) = cantus_mcp::door::door(Arc::new(|| {}));
    let limits = Limits {
        ui_wait: Duration::from_millis(30),
        ..Limits::default()
    };
    let server = Server::new(ServerInfo::cantus("t"), common::catalog(), door, limits);
    let mut agents = cantus_mcp::Agents::new(inbox, common::catalog());
    let mut host = common::Host::new();
    host.policy.allow_edits = true;
    let mut c = TestClient::legacy(server, "2025-11-25");
    // Nobody turns the pump while the transport waits.
    let r = c.call_tool("set_tempo", json!({"bpm": 66}), &mut || {});
    assert!(
        r.is_error && r.text.contains("may have been done"),
        "{}",
        r.text
    );
    agents.pump(&mut host);
    assert_eq!(host.bus.project().tempo_bpm, 120.0, "dropped, not run late");
    assert_eq!(
        agents.activity().last().map(|a| a.outcome.clone()),
        Some(Outcome::Dropped),
        "and the activity says so (review 2 #12)"
    );
}

/// #10: an external agent is refused while *Allow external agents* is off.
#[test]
fn external_agents_are_refused_while_the_switch_is_off() {
    let mut rig = Rig::new();
    let mut ext =
        TestClient::legacy_on(rig.server.clone(), "2025-11-25", Session::external("http"));
    let r = rig.call(&mut ext, "get_tempo", json!({}), None);
    assert!(
        r.is_error && r.text.contains("external agents"),
        "{}",
        r.text
    );
    rig.host.policy.external_agents = true;
    assert!(!rig.call(&mut ext, "get_tempo", json!({}), None).is_error);
    let mut inside = rig.legacy("2025-11-25");
    rig.host.policy.external_agents = false;
    assert!(
        !rig.call(&mut inside, "get_tempo", json!({}), None).is_error,
        "in-process is not external"
    );
}
