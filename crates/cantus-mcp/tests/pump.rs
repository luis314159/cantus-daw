//! The pump end to end through the in-process client: permissions by class, the
//! confirmation, one undo step per call with `Origin::Mcp`, undo restoring the project
//! exactly, `expected_revision`, refusals audited, limits.

mod common;

use cantus_core::{Command, Origin};
use cantus_mcp::{Answer, Outcome};
use common::Rig;
use serde_json::json;
use std::time::Duration;

#[test]
fn an_allowed_edit_is_one_undo_step_and_undo_restores_the_project_exactly() {
    let mut rig = Rig::new();
    let before = rig.host.bus.project().clone();
    let steps = rig.host.bus.history().len();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "add_track", json!({}), Some(Answer::Allow));
    assert!(!r.is_error, "{}", r.text);
    let s = r.structured.unwrap();
    assert!(s["track"].as_u64().is_some(), "the new id comes back");
    assert_eq!(
        s["revision"],
        rig.host.bus.revision(),
        "and the revision it left"
    );
    assert_eq!(rig.host.bus.history().len(), steps + 1, "one step");
    let top = rig.host.bus.history().last().unwrap();
    assert_eq!(top.origin, Origin::Mcp);
    assert_eq!(
        rig.agents.step_words(top.step()).as_deref(),
        Some("ai.test.add_track")
    );
    rig.host.bus.undo();
    assert_eq!(
        common::ids_aside(rig.host.bus.project()),
        common::ids_aside(&before),
        "undo restores the project exactly"
    );
    let outcomes: Vec<Outcome> = rig.agents.activity().map(|a| a.outcome.clone()).collect();
    assert_eq!(outcomes, vec![Outcome::Asked, Outcome::Done]);
}

#[test]
fn the_user_saying_no_or_nothing_leaves_the_project_untouched() {
    let mut rig = Rig::new();
    let before = rig.host.bus.project().clone();
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "set_tempo", json!({"bpm": 90}), Some(Answer::Deny));
    assert!(r.is_error);
    assert!(r.text.contains("said no"), "{}", r.text);
    rig.agents.config.ask_timeout = Duration::ZERO;
    let r = rig.call(&mut c, "set_tempo", json!({"bpm": 90}), None);
    assert!(r.is_error);
    assert!(r.text.contains("did not answer"), "{}", r.text);
    assert_eq!(*rig.host.bus.project(), before);
    assert!(rig.host.bus.history().is_empty());
    let outcomes: Vec<Outcome> = rig.agents.activity().map(|a| a.outcome.clone()).collect();
    assert_eq!(
        outcomes,
        vec![
            Outcome::Asked,
            Outcome::DeniedByUser,
            Outcome::Asked,
            Outcome::NoAnswer
        ]
    );
}

#[test]
fn each_class_is_decided_in_cantus() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    // Reads and transport run without asking.
    assert!(!rig.call(&mut c, "get_tempo", json!({}), None).is_error);
    assert!(!rig.call(&mut c, "play", json!({}), None).is_error);
    // Files are refused until enabled — and the refusal is audited.
    let r = rig.call(&mut c, "export_file", json!({}), Some(Answer::Allow));
    assert!(r.is_error && r.text.contains("files is off"), "{}", r.text);
    assert!(matches!(
        rig.agents.activity().last().unwrap().outcome,
        Outcome::Refused(_)
    ));
    rig.host.policy.allow_files = true;
    let asks_before = rig
        .agents
        .activity()
        .filter(|a| a.outcome == Outcome::Asked)
        .count();
    assert!(
        !rig.call(&mut c, "export_file", json!({}), Some(Answer::Allow))
            .is_error
    );
    let asks_after = rig
        .agents
        .activity()
        .filter(|a| a.outcome == Outcome::Asked)
        .count();
    assert_eq!(asks_after, asks_before + 1, "enabled files still ask");
    // Allowing edits runs edits at once, never destruction.
    rig.host.policy.allow_edits = true;
    assert!(
        !rig.call(&mut c, "set_tempo", json!({"bpm": 100}), None)
            .is_error
    );
    assert_eq!(rig.host.bus.project().tempo_bpm, 100.0);
    rig.agents.config.ask_timeout = Duration::ZERO;
    let r = rig.call(&mut c, "remove_first_track", json!({}), None);
    assert!(r.is_error, "a destructive tool still asks");
    assert_eq!(rig.host.bus.project().tracks.len(), 1);
}

#[test]
fn allow_for_this_session_covers_one_client_and_one_class() {
    let mut rig = Rig::new();
    let mut a = rig.legacy("2025-11-25");
    let mut b = rig.legacy("2025-06-18");
    assert!(
        !rig.call(
            &mut a,
            "set_tempo",
            json!({"bpm": 91}),
            Some(Answer::AllowForSession)
        )
        .is_error
    );
    let asked = |rig: &Rig| {
        rig.agents
            .activity()
            .filter(|x| x.outcome == Outcome::Asked)
            .count()
    };
    let n = asked(&rig);
    assert!(
        !rig.call(&mut a, "set_tempo", json!({"bpm": 92}), None)
            .is_error
    );
    assert_eq!(asked(&rig), n, "the same client is not asked again");
    rig.agents.config.ask_timeout = Duration::ZERO;
    assert!(
        rig.call(&mut b, "set_tempo", json!({"bpm": 93}), None)
            .is_error,
        "another client is"
    );
    assert!(
        rig.call(&mut a, "remove_first_track", json!({}), None)
            .is_error,
        "and another class is"
    );
    // Revoking the client takes its grant away.
    let key = a.session().id.clone().unwrap();
    rig.agents.revoke(&mut rig.host, &key);
    assert!(
        rig.call(&mut a, "set_tempo", json!({"bpm": 94}), None)
            .is_error
    );
    assert_eq!(rig.host.bus.project().tempo_bpm, 92.0);
}

#[test]
fn the_off_switch_and_a_denied_tool_refuse() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    rig.host.policy.denied = vec!["get_tempo".into()];
    let r = rig.call(&mut c, "get_tempo", json!({}), None);
    assert!(
        r.is_error && r.text.contains("does not allow"),
        "{}",
        r.text
    );
    rig.host.policy.enabled = false;
    for tool in ["play", "list_jobs", "set_tempo"] {
        let r = rig.call(&mut c, tool, json!({"bpm": 100}), Some(Answer::Allow));
        assert!(
            r.is_error && r.text.contains("switched off"),
            "{tool}: {}",
            r.text
        );
    }
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let e = c
        .request("resources/list", json!({}), &mut || agents.pump(host))
        .unwrap_err();
    assert!(e["message"].as_str().unwrap().contains("switched off"));
}

#[test]
fn expected_revision_refuses_a_stale_plan_and_names_the_revision() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    let rev = rig.host.bus.revision();
    rig.host
        .bus
        .execute(Command::SetTempo { bpm: 99.0 }, Origin::User)
        .unwrap();
    let r = rig.call(
        &mut c,
        "set_tempo",
        json!({"bpm": 100, "expected_revision": rev}),
        None,
    );
    assert!(r.is_error);
    assert!(
        r.text.contains(&rig.host.bus.revision().to_string()),
        "{}",
        r.text
    );
    assert_eq!(rig.host.bus.project().tempo_bpm, 99.0);
    let now = rig.host.bus.revision();
    let r = rig.call(
        &mut c,
        "set_tempo",
        json!({"bpm": 100, "expected_revision": now}),
        None,
    );
    assert!(!r.is_error, "{}", r.text);
    let r = rig.call(
        &mut c,
        "set_tempo",
        json!({"bpm": 100, "expected_revision": "x"}),
        None,
    );
    assert!(r.is_error && r.text.contains("expected_revision"));
}

#[test]
fn a_plan_the_project_moved_under_while_asking_is_planned_again() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut moved = false;
    let r = c.call_tool("add_track", json!({}), &mut || {
        agents.pump(host);
        let ids: Vec<u64> = agents.asks().map(|a| a.id).collect();
        if let Some(id) = ids.first() {
            if !moved {
                // The user adds a track while the card is up.
                let t = host.bus.new_track("Mine");
                let index = host.bus.project().tracks.len();
                host.bus
                    .execute(
                        Command::InsertTrack {
                            index,
                            track: Box::new(t),
                        },
                        Origin::User,
                    )
                    .unwrap();
                moved = true;
            }
            agents.answer(*id, Answer::Allow);
        }
    });
    assert!(!r.is_error, "{}", r.text);
    let names: Vec<&str> = host
        .bus
        .project()
        .tracks
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Bass", "Mine", "Lead"],
        "planned against the project as it is now"
    );
}

#[test]
fn rate_and_waiting_limits_refuse_with_a_reason() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    rig.agents.config.max_asks_per_client = 0;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "remove_first_track", json!({}), None);
    assert!(
        r.is_error && r.text.contains("are waiting for the user"),
        "{}",
        r.text
    );
    // A burst of edits meets the limit (40 at once, 10 a second after): called until it
    // does, so a slow machine that refills the bucket between calls still gets there.
    let mut refused = 0;
    for i in 0..400 {
        let r = rig.call(&mut c, "set_tempo", json!({"bpm": 60 + i % 100}), None);
        if r.is_error {
            assert!(r.text.contains("too many calls"), "{}", r.text);
            refused += 1;
            break;
        }
    }
    assert!(refused > 0, "a burst of edits meets the limit");
}

#[test]
fn a_pending_confirmation_asks_for_a_frame_by_its_deadline() {
    let mut rig = Rig::new();
    let mut c = rig.legacy("2025-11-25");
    assert_eq!(
        rig.agents.next_wake(std::time::Instant::now()),
        None,
        "idle: no frames"
    );
    let agents = &mut rig.agents;
    let host = &mut rig.host;
    let mut saw = None;
    let r = c.call_tool("set_tempo", json!({"bpm": 100}), &mut || {
        agents.pump(host);
        let first = agents.asks().map(|a| a.id).next();
        if let Some(id) = first {
            saw = agents.next_wake(std::time::Instant::now());
            agents.answer(id, Answer::Deny);
        }
    });
    assert!(r.is_error);
    let wake = saw.expect("a wake while the card is up");
    assert!(wake <= Duration::from_secs(120) && wake > Duration::from_secs(100));
}

/// The integrator, after tools: arguments are checked against the tool's schema before it
/// plans — a misspelt key is refused, naming the keys the tool takes; a wrong type too.
#[test]
fn a_misspelt_or_mistyped_argument_is_refused_before_planning() {
    let mut rig = Rig::new();
    rig.host.policy.allow_edits = true;
    let mut c = rig.legacy("2025-11-25");
    let r = rig.call(&mut c, "set_tempo", json!({"bmp": 100}), None);
    assert!(r.is_error, "{}", r.text);
    assert!(
        r.text.contains("bmp") && r.text.contains("it takes: bpm"),
        "{}",
        r.text
    );
    let r = rig.call(&mut c, "set_tempo", json!({"bpm": "fast"}), None);
    assert!(
        r.is_error && r.text.contains("must be number"),
        "{}",
        r.text
    );
    // The server's own arguments stay allowed on every mutating tool.
    let rev = rig.host.bus.revision();
    let r = rig.call(
        &mut c,
        "set_tempo",
        json!({"bpm": 101, "expected_revision": rev}),
        None,
    );
    assert!(!r.is_error, "{}", r.text);
    assert!(rig.host.bus.history().len() == 1, "only the good call ran");
}
