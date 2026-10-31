//! Seams the integrated tree's review found between the builders (round 34), each held by
//! a test that fails without its fix.

mod common;

use common::Rig;
use serde_json::json;
use std::time::{Duration, Instant};

/// An agent's job started in one project is cancelled when another project opens — as
/// the application's own runs stop — not left to run on with its work thrown away.
#[test]
fn another_projects_jobs_are_cancelled() {
    let mut rig = Rig::new();
    rig.agents.config.inline_job_wait = Duration::ZERO;
    let mut c = rig.legacy("2025-11-25");
    // A long job: 200 steps of 20 ms.
    let r = rig.call(&mut c, "count", json!({"steps": 200, "ms": 20}), None);
    let job = r.structured.expect("a handle")["job"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(rig.agents.jobs().running(None), 1);
    // Another project.
    rig.host.epoch += 1;
    let started = Instant::now();
    while rig.agents.jobs().running(None) > 0 {
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "still running after another project opened"
        );
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
    let w = rig.call(&mut c, "wait_job", json!({"job": job}), None);
    assert!(
        w.is_error && w.text.contains("another project") && w.text.contains("stopped"),
        "{}",
        w.text
    );
}

/// A client that names itself only with joiners has no name to show: it is "agent", not
/// a pair of empty quotes.
#[test]
fn a_name_of_joiners_is_no_name() {
    let mut rig = Rig::new();
    let mut c = cantus_mcp::client::TestClient::raw(rig.server.clone());
    c.name = "\u{200D}\u{200C} \u{200D}".into();
    let _ = c.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": c.name.clone(), "version": "1"}}),
        &mut || {},
    );
    let r = rig.call(&mut c, "get_tempo", json!({}), None);
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(rig.agents.activity().last().unwrap().caller.name, "agent");
}
