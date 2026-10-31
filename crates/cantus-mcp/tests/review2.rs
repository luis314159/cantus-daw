//! The second review's findings (round 34, mcp, the fix round): each held by a test that
//! fails without its fix. (#1 is in review1.rs's output-schema test, #8 in catalog.rs's
//! unit tests, #9 in cantus-ai's, #14's buckets in permissions.rs's.)

mod common;

use cantus_core::{Command, Origin};
use cantus_mcp::client::TestClient;
use cantus_mcp::{Answer, Limits, Outcome, Server, ServerInfo, Session, UndoAll};
use common::Rig;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

fn change_of(r: &cantus_mcp::client::ToolCall) -> String {
    r.structured.as_ref().unwrap()["change"]
        .as_str()
        .unwrap()
        .to_string()
}

fn asked(rig: &Rig) -> usize {
    rig.agents
        .activity()
        .filter(|a| a.outcome == Outcome::Asked)
        .count()
}

/// #2: a job's edit is decided again as it ends: a tool denied meanwhile, or external
/// agents switched off, stops it.
#[test]
fn a_jobs_edit_is_decided_again_when_it_ends() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.host.policy.external_agents = true;
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut ext = TestClient::legacy_on(
        rig.server.clone(),
        "2025-11-25",
        Session::external("http:x"),
    );
    let r = rig.call(
        &mut ext,
        "measure_tempo",
        json!({"bpm": 70, "ms": 150}),
        None,
    );
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    rig.host.policy.denied = vec!["measure_tempo".into()];
    let w = rig.call(&mut ext, "wait_job", json!({"job": job}), None);
    rig.host.policy.denied.clear();
    let _ = w;
    assert_eq!(
        rig.host.bus.project().tempo_bpm,
        120.0,
        "a denied tool's edit is not made"
    );
    let r = rig.call(
        &mut ext,
        "measure_tempo",
        json!({"bpm": 71, "ms": 150}),
        None,
    );
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    rig.host.policy.external_agents = false;
    // The agent cannot even wait now; the job ends on the pump's next turns.
    let _ = rig.call(&mut ext, "wait_job", json!({"job": job}), None);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while rig
        .agents
        .jobs()
        .get(&job)
        .is_some_and(|j| j.ended.is_none())
    {
        assert!(std::time::Instant::now() < deadline);
        rig.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        rig.host.bus.project().tempo_bpm,
        120.0,
        "the door closed: no edit"
    );
}

/// #2: a job the user had to allow, whose project moved while it ran, asks again for its
/// edit — and nothing is made when the user says no.
#[test]
fn a_jobs_edit_on_a_moved_project_is_asked_again() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    agents.config.inline_job_wait = Duration::ZERO;
    let mut first = true;
    let r = c.call_tool("measure_tempo", json!({"bpm": 72, "ms": 100}), &mut || {
        agents.pump(host);
        let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
        for id in ids {
            if first {
                agents.answer(id, Answer::Allow);
                first = false;
            }
        }
    });
    let job = r.structured.unwrap()["job"].as_str().unwrap().to_string();
    host.bus
        .execute(
            Command::SetProjectName {
                name: "moved".into(),
            },
            Origin::User,
        )
        .unwrap();
    let w = c.call_tool("wait_job", json!({"job": job}), &mut || {
        agents.pump(host);
        let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
        for id in ids {
            agents.answer(id, Answer::Deny);
        }
    });
    assert!(w.is_error && w.text.contains("said no"), "{}", w.text);
    assert_eq!(host.bus.project().tempo_bpm, 120.0);
}

/// #3: at a full history a fresh push drops the oldest step; another agent's action on top
/// keeps its step.
#[test]
fn a_fresh_push_at_a_full_history_renumbers_nobody() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    for i in 0..10_000 {
        let name = format!("n{i}");
        rig.host
            .bus
            .execute(Command::SetProjectName { name }, Origin::User)
            .unwrap();
    }
    let mut a = rig.legacy("2025-11-25");
    let mut b = rig.legacy("2025-11-25");
    assert!(
        !rig.call(&mut a, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    let a_step = rig.host.bus.current_step();
    let ch = change_of(&rig.call(&mut b, "begin_change", json!({"label": "x"}), None));
    assert!(
        !rig.call(&mut b, "add_track", json!({"change": ch}), None)
            .is_error
    );
    let a_key = a.session().id.clone().unwrap();
    assert_eq!(
        rig.agents.step_owner(a_step).map(|c| c.key.clone()),
        Some(a_key),
        "a's step is still a's"
    );
}

/// #4: an agent's steps keep their owner and words through an imported project undone.
#[test]
fn an_agents_steps_survive_an_imported_project_undone() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    let step = rig.host.bus.current_step();
    let other = Box::new(cantus_core::Project::new("imported"));
    rig.host
        .bus
        .execute(Command::ReplaceProject { project: other }, Origin::User)
        .unwrap();
    rig.host.epoch += 1;
    rig.host.bus.undo();
    rig.host.epoch += 1;
    assert_eq!(
        rig.agents.step_words(step).as_deref(),
        Some("ai.test.tempo bpm=100")
    );
    let r = rig.call(&mut c, "list_ai_actions", json!({}), None);
    assert_eq!(r.structured.unwrap()["actions"][0]["in_project"], true);
    let r = rig.call(
        &mut c,
        "undo_last_ai_action",
        json!({}),
        Some(Answer::Allow),
    );
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(rig.host.bus.project().tempo_bpm, 120.0);
}

/// #5: a plan whose words do not name its target is asked again when its target moved.
#[test]
fn a_vague_plan_on_another_target_is_asked_again() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut seen = Vec::new();
    let r = c.call_tool("remove_first_vague", json!({}), &mut || {
        agents.pump(host);
        let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
        for id in ids {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if seen.len() == 1 {
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
                agents.answer(id, Answer::Deny);
            }
        }
    });
    assert!(r.is_error && r.text.contains("said no"), "{}", r.text);
    assert_eq!(seen.len(), 2, "asked again");
    assert_eq!(host.bus.project().tracks.len(), 2, "nothing removed");
}

/// #6: an allowed undo undoes the step the card named, or nothing.
#[test]
fn an_allowed_undo_undoes_only_the_step_it_named() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut said = None;
    let r = c.call_tool("undo_last_ai_action", json!({}), &mut || {
        agents.pump(host);
        let asks: Vec<(u64, String)> = agents.asks().map(|a| (a.id, a.said.clone())).collect();
        if let Some((id, s)) = asks.into_iter().next() {
            said = Some(s);
            host.bus
                .execute(
                    Command::SetProjectName {
                        name: "mine".into(),
                    },
                    Origin::User,
                )
                .unwrap();
            agents.answer(id, Answer::Allow);
        }
    });
    assert!(
        said.unwrap().contains("ai.test.tempo bpm=100"),
        "the card names the step"
    );
    assert!(
        r.is_error && r.text.contains("last step changed"),
        "{}",
        r.text
    );
    assert_eq!(host.bus.project().name, "mine");
    assert_eq!(host.bus.project().tempo_bpm, 100.0);
}

/// #7: a change stays open while its call waits on the card; the server's own tools take
/// no change.
#[test]
fn a_change_waits_with_its_call_on_the_card() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let ch = change_of(&rig.call(&mut c, "begin_change", json!({"label": "x"}), None));
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut turns = 0;
    let r = c.call_tool("add_track", json!({"change": ch}), &mut || {
        agents.pump(host);
        // Idle at once, from the moment the call waits on the card (before it, the change
        // would end on a pump the call had not reached yet: a race, not the case held).
        if agents.asks().count() == 1 {
            agents.config.change_idle = Duration::ZERO;
            turns += 1;
        }
        if turns > 20 {
            let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
            for id in ids {
                agents.answer(id, Answer::Allow);
            }
        }
    });
    assert!(!r.is_error, "{}", r.text);
    let list = c.request("tools/list", json!({}), &mut || {}).unwrap();
    let undo = list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "undo_last_ai_action")
        .unwrap()
        .clone();
    assert!(undo["inputSchema"]["properties"].get("change").is_none());
    assert!(undo["inputSchema"]["properties"]
        .get("expected_revision")
        .is_some());
}

/// #10: a job's edit joins the change it was started in.
#[test]
fn a_jobs_edit_joins_its_change() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.modern();
    let ch = change_of(&rig.call(&mut c, "begin_change", json!({"label": "x"}), None));
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 90, "change": ch}), None)
            .is_error
    );
    let r = rig.call(
        &mut c,
        "measure_tempo",
        json!({"bpm": 91, "change": ch}),
        None,
    );
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(rig.host.bus.history().len(), 1, "one step");
}

/// #11: the in-process pool is never starved by external agents.
#[test]
fn external_agents_never_starve_the_in_process_ones() {
    let mut rig = Rig::new();
    rig.host.policy.external_agents = true;
    rig.agents.config.max_asks = 1;
    let mut ext = TestClient::legacy_on(
        rig.server.clone(),
        "2025-11-25",
        Session::external("http:x"),
    );
    let mut inside = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    // An external call waits on the card while an in-process one asks: both are asked.
    let t = std::thread::scope(|scope| {
        let h = scope.spawn(move || ext.call_tool("set_tempo", json!({"bpm": 90}), &mut || {}));
        let r = inside.call_tool("set_tempo", json!({"bpm": 91}), &mut || {
            agents.pump(host);
            let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
            if ids.len() == 2 {
                for id in ids {
                    agents.answer(id, Answer::Deny);
                }
            }
        });
        while !h.is_finished() {
            agents.pump(host);
            std::thread::sleep(Duration::from_millis(1));
        }
        (r, h.join().unwrap())
    });
    assert!(
        t.0.text.contains("said no"),
        "the in-process call was asked: {}",
        t.0.text
    );
    assert!(
        t.1.text.contains("said no"),
        "and the external one: {}",
        t.1.text
    );
}

/// #12: a confirmation never outlives its transport's wait.
#[test]
fn a_confirmation_ends_with_its_transports_wait() {
    let (door, inbox) = cantus_mcp::door::door(Arc::new(|| {}));
    let limits = Limits {
        ui_wait: Duration::from_millis(150),
        ..Limits::default()
    };
    let server = Server::new(ServerInfo::cantus("t"), common::catalog(), door, limits);
    let mut agents = cantus_mcp::Agents::new(inbox, common::catalog());
    let mut host = common::Host::new();
    let mut c = TestClient::legacy(server, "2025-11-25");
    let r = c.call_tool("set_tempo", json!({"bpm": 66}), &mut || {
        agents.pump(&mut host)
    });
    assert!(r.is_error);
    std::thread::sleep(Duration::from_millis(50));
    agents.pump(&mut host);
    assert_eq!(agents.asks().count(), 0, "gone with its transport");
    assert!(agents.activity().any(|a| a.outcome == Outcome::NoAnswer));
}

/// #13: a non-edit tool that edits is reported, its answer withheld.
#[test]
fn a_transport_tool_that_edits_is_reported() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "sneaky_host", json!({}), None);
    assert!(
        r.is_error && r.text.contains("not an edit tool"),
        "{}",
        r.text
    );
}

/// #14: under a session grant a moved plan runs without asking again; a re-ask meets no
/// cap.
#[test]
fn a_granted_plan_runs_and_a_re_ask_meets_no_cap() {
    let mut rig = Rig::new();
    rig.agents.config.max_asks_per_client = 1;
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut done = false;
    let r = c.call_tool("add_track", json!({}), &mut || {
        agents.pump(host);
        let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
        if let (Some(id), false) = (ids.first(), done) {
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
            agents.answer(*id, Answer::AllowForSession);
            done = true;
        }
    });
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(
        asked(&rig),
        1,
        "granted: not asked again though the plan moved"
    );
    // A re-ask at the cap.
    let mut c2 = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut n = 0;
    let r = c2.call_tool("remove_first_track", json!({}), &mut || {
        agents.pump(host);
        let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
        for id in ids {
            n += 1;
            if n == 1 {
                let t = host.bus.new_track("Other");
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
                agents.answer(id, Answer::Deny);
            }
        }
    });
    assert!(
        r.text.contains("said no"),
        "re-asked, not refused by the cap: {}",
        r.text
    );
}

/// Untested before: *Undo all of this agent's changes* stops at the user's step.
#[test]
fn undo_all_stops_at_the_users_step_and_says_so() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    let key = c.session().id.clone();
    rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None);
    rig.host
        .bus
        .execute(
            Command::SetProjectName {
                name: "mine".into(),
            },
            Origin::User,
        )
        .unwrap();
    rig.call(&mut c, "set_tempo", json!({"bpm": 101}), None);
    rig.call(&mut c, "add_track", json!({}), None);
    let key = key.unwrap_or_else(|| c.session().id.clone().unwrap());
    let done = rig.agents.undo_all(&mut rig.host, &key);
    assert_eq!(
        done,
        UndoAll {
            undone: 2,
            stopped_at: Some("cmd.set_project_name".into())
        }
    );
    assert_eq!(rig.host.bus.project().name, "mine");
    assert_eq!(rig.host.bus.project().tempo_bpm, 100.0);
    rig.host.bus.undo();
    let done = rig.agents.undo_all(&mut rig.host, &key);
    assert_eq!(
        done,
        UndoAll {
            undone: 1,
            stopped_at: None
        },
        "nothing of its own left below"
    );
}

/// Untested before: idle agents are forgotten; an agent's open changes are capped.
#[test]
fn idle_agents_are_forgotten_and_changes_are_capped() {
    let mut rig = Rig::new();
    let mut m = rig.modern();
    for i in 0..8 {
        let r = rig.call(
            &mut m,
            "begin_change",
            json!({"label": format!("c{i}")}),
            None,
        );
        assert!(!r.is_error, "{}", r.text);
    }
    let r = rig.call(&mut m, "begin_change", json!({"label": "c9"}), None);
    assert!(
        r.is_error && r.text.contains("changes are open"),
        "{}",
        r.text
    );
    assert_eq!(rig.agents.connected(std::time::Instant::now()).len(), 1);
    rig.agents.config.forget_after = Duration::ZERO;
    rig.pump();
    assert!(rig.agents.connected(std::time::Instant::now()).is_empty());
    let r = rig.call(&mut m, "begin_change", json!({"label": "again"}), None);
    assert!(!r.is_error, "its changes ended with it: {}", r.text);
}
