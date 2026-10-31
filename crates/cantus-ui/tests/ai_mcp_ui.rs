//! The MCP server's door and what the window shows of agents (round 34, ADR 0081): the HTTP
//! door opened and closed as Settings ▸ AI says, the confirmation card in the corner stack,
//! the top bar's chip, the activity, the one off switch, and the files' roots.

use cantus_core::Project;
use cantus_mcp::client::TestClient;
use cantus_mcp::http_client::Client;
use cantus_ui::views::transport::layout::Group;
use cantus_ui::CantusApp;
use serde_json::{json, Value};

fn app() -> (CantusApp, egui::Context, tempfile::TempDir) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut p = Project::new("ai");
    let t = p.make_track("Bass");
    p.tracks.push(t);
    let mut app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    let dir = tempfile::tempdir().unwrap();
    app.settings_path = dir.path().join("settings.json");
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::ai_mcp::wire(&mut app, &ctx);
    app.ai.door.dir = Some(dir.path().to_path_buf());
    (app, ctx, dir)
}

fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.frame(ui)).textures_delta.clear();
}

#[test]
fn the_door_opens_with_the_setting_writes_its_file_and_closes_with_it() {
    let (mut app, ctx, dir) = app();
    frame(&mut app, &ctx);
    assert_eq!(app.ai.door.port(), None, "closed by default");
    app.settings.ai.external_agents = true;
    app.settings.ai.port = 0;
    frame(&mut app, &ctx);
    let port = app.ai.door.port().expect("open");
    let door = cantus_mcp::home::read_door(dir.path()).expect("the bridge's file");
    assert_eq!(door.port, port);
    assert_eq!(door.pid, std::process::id());
    let token = cantus_mcp::home::token(dir.path()).unwrap();
    // A real request through the socket, answered by the window's frames.
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let t = std::thread::spawn(move || {
        let mut c = Client::new(addr);
        let auth = format!("Bearer {token}");
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
            "name": "list_jobs", "arguments": {},
            "_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28",
                      "io.modelcontextprotocol/clientCapabilities": {},
                      "io.modelcontextprotocol/clientInfo": {"name": "curl"}}}});
        c.send(
            "POST",
            &[
                ("Authorization", &auth),
                ("Content-Type", "application/json"),
            ],
            body.to_string().as_bytes(),
        )
        .unwrap()
    });
    while !t.is_finished() {
        frame(&mut app, &ctx);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let m = t.join().unwrap();
    assert_eq!(m.start.0, "200");
    let v: Value = serde_json::from_slice(&m.body).unwrap();
    assert_eq!(v["result"]["isError"], false, "{v}");
    // The agent is seen: the chip shows on the bar.
    assert!(cantus_ui::ai_mcp::chip::in_use(&app));
    assert!(cantus_ui::views::transport::shown_groups(&app).contains(&Group::Ai));
    // Off again: closed, its file gone, nothing listening.
    app.settings.ai.external_agents = false;
    frame(&mut app, &ctx);
    assert_eq!(app.ai.door.port(), None);
    assert!(cantus_mcp::home::read_door(dir.path()).is_none());
    assert!(
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_err()
    );
}

#[test]
fn a_taken_port_is_said_once_and_not_tried_every_frame() {
    let (mut app, ctx, _dir) = app();
    let holder = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    app.settings.ai.external_agents = true;
    app.settings.ai.port = holder.local_addr().unwrap().port();
    frame(&mut app, &ctx);
    assert_eq!(app.ai.door.port(), None);
    let said = app.ai.door.error.clone().expect("said");
    assert!(said.contains(&app.settings.ai.port.to_string()), "{said}");
    frame(&mut app, &ctx);
    assert_eq!(
        app.ai.door.error.as_deref(),
        Some(said.as_str()),
        "kept, not retried"
    );
    app.settings.ai.port = 0;
    frame(&mut app, &ctx);
    assert!(app.ai.door.port().is_some(), "another port opens");
    assert!(app.ai.door.error.is_none());
}

fn tempo_schema() -> serde_json::Value {
    json!({"type": "object", "properties": {"bpm": {"type": "number"}}})
}

/// A destructive tool, so the card has something to ask (the application's own table has
/// none of its own yet: the tools builder's join it).
fn remove_first(
    cx: &mut cantus_ai::ToolCx<'_>,
    _: &serde_json::Value,
) -> Result<cantus_ai::EditPlan, cantus_ai::ToolError> {
    let t = cx.project().tracks[0].clone();
    Ok(cantus_ai::EditPlan {
        command: cantus_core::Command::RemoveTrack { id: t.id },
        said: cantus_ai::Said::new("ai.said.undo_last").arg("what", t.name),
        result: json!({}),
    })
}

static REMOVE: cantus_ai::ToolDef = cantus_ai::ToolDef {
    name: "test_remove_first",
    title: "Remove the first track",
    description: "Test.",
    class: cantus_ai::ToolClass::Destructive,
    input_schema: tempo_schema,
    output_schema: None,
    run: cantus_ai::ToolRun::Edit(remove_first),
};

fn with_remove() -> Vec<&'static cantus_ai::ToolDef> {
    let mut v = cantus_ui::ai_tools::registry();
    v.push(&REMOVE);
    v
}

#[test]
fn a_waiting_call_is_a_card_in_the_corner_and_its_buttons_answer_it() {
    let (mut app, ctx, _dir) = app();
    let catalog = cantus_mcp::Catalog {
        tools: with_remove,
        ..cantus_ui::ai_mcp::catalog::app()
    };
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, catalog);
    let server = app.ai.server().unwrap().clone();
    let mut c = TestClient::legacy(server, "2025-11-25");
    let mut cards = 0;
    let mut texts = None;
    // A destructive call asks: the card shows; Deny answers it.
    let r = c.call_tool("test_remove_first", json!({}), &mut || {
        frame(&mut app, &ctx);
        if let Some(card) = cantus_ui::ai_mcp::card::card(&app) {
            cards = cantus_ui::views::audio_notice::card_rects(&app, &ctx).len();
            assert_eq!(card.close.1, cantus_mcp::Answer::Deny, "the ✕ denies");
            let deny = card
                .buttons
                .iter()
                .find(|b| b.1 .1 == cantus_mcp::Answer::Deny)
                .unwrap()
                .1;
            texts = Some(card);
            cantus_ui::ai_mcp::card::answer(&mut app, deny.0, deny.1);
        }
    });
    let card = texts.expect("a card was shown");
    assert_eq!(cards, 1);
    assert_eq!(card.who, "“test-client”");
    assert_eq!(card.via, "· inside Cantus");
    assert_eq!(card.title, "wants to:");
    let body = &card.body;
    assert!(body.contains("Waits"), "{body}");
    assert!(body.contains("Bass"), "what it touches: {body}");
    let buttons = &card.buttons;
    let labels: Vec<&str> = buttons.iter().map(|b| b.0.as_str()).collect();
    assert_eq!(
        labels,
        ["Allow", "Deny"],
        "no session grant for a destructive tool"
    );
    assert!(r.is_error && r.text.contains("said no"), "{}", r.text);
    assert!(
        cantus_ui::ai_mcp::card::card(&app).is_none(),
        "answered, gone"
    );
}

#[test]
fn the_one_switch_turns_the_whole_area_off() {
    let (mut app, ctx, dir) = app();
    app.settings.ai.external_agents = true;
    app.settings.ai.port = 0;
    frame(&mut app, &ctx);
    assert!(app.ai.door.port().is_some());
    let server = app.ai.server().unwrap().clone();
    let mut c = TestClient::legacy(server, "2025-11-25");
    let r = c.call_tool("list_jobs", json!({}), &mut || frame(&mut app, &ctx));
    assert!(!r.is_error);
    assert!(cantus_ui::ai_mcp::chip::in_use(&app));
    app.ai.activity_open = true;
    app.settings.ai.enabled = false;
    frame(&mut app, &ctx);
    assert_eq!(app.ai.door.port(), None, "the door closes");
    assert!(cantus_mcp::home::read_door(dir.path()).is_none());
    assert!(!cantus_ui::ai_mcp::chip::in_use(&app), "the chip hides");
    assert!(!cantus_ui::views::transport::shown_groups(&app).contains(&Group::Ai));
    let r = c.call_tool("list_jobs", json!({}), &mut || frame(&mut app, &ctx));
    assert!(r.is_error && r.text.contains("switched off"), "{}", r.text);
    assert!(cantus_ui::ai_mcp::card::card(&app).is_none());
}

#[test]
fn the_activity_lists_an_agent_and_disconnecting_forgets_it() {
    let (mut app, ctx, _dir) = app();
    app.settings.ai.allow_edits = true;
    let server = app.ai.server().unwrap().clone();
    let mut c = TestClient::legacy(server, "2025-11-25");
    let r = c.call_tool("list_jobs", json!({}), &mut || frame(&mut app, &ctx));
    assert!(!r.is_error);
    app.ai.activity_open = true;
    frame(&mut app, &ctx);
    let agents = cantus_ui::ai_mcp::connected(&app);
    assert_eq!(agents.len(), 1);
    assert_eq!(cantus_ui::ai_mcp::chip::words(&app), "test-client");
    cantus_ui::ai_mcp::disconnect(&mut app, &agents[0].key);
    assert!(cantus_ui::ai_mcp::connected(&app).is_empty());
}

#[test]
fn file_tools_reach_the_project_folder_and_the_users_folders_only() {
    use cantus_ai::ToolHost;
    let (mut app, _ctx, dir) = app();
    let song = dir.path().join("song");
    let extra = dir.path().join("samples");
    std::fs::create_dir_all(&song).unwrap();
    std::fs::create_dir_all(&extra).unwrap();
    app.path = Some(song.join("song.cantus"));
    app.settings.ai.folders = vec![extra.clone()];
    assert_eq!(app.file_roots(), vec![song.clone(), extra.clone()]);
    assert!(cantus_ai::may_touch(&app, &song.join("mix.wav")));
    assert!(cantus_ai::may_touch(&app, &extra.join("kick.wav")));
    assert!(!cantus_ai::may_touch(
        &app,
        &dir.path().join("elsewhere.wav")
    ));
}

#[test]
fn the_settings_folder_is_the_one_the_bridge_reads() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    assert_eq!(cantus_mcp::home::dir(), cantus_ui::app::config_dir());
    assert_eq!(
        cantus_mcp::home::PORTABLE_MARKER,
        cantus_update::install::PORTABLE_MARKER
    );
    assert_eq!(
        cantus_mcp::home::PORTABLE_DATA,
        cantus_update::install::PORTABLE_DATA
    );
}

/// Review 4 #10: every sentence key a tool names (`Said::new("…")` in cantus-ai's tools,
/// read from their source) has its text in English and in Spanish — a missing one would
/// put a bare key on the user's card.
#[test]
fn every_sentence_a_tool_says_has_its_words() {
    use cantus_ui::i18n::{has, Locale};
    // Every crate whose code says a sentence: the tools' (cantus-ai), the application's
    // tools and cards (cantus-ui), the server's own (cantus-mcp) — read whole, folders
    // and all (review 5 #12: only cantus-ai/src was read).
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    let mut folders: Vec<std::path::PathBuf> = ["cantus-ai/src", "cantus-ui/src", "cantus-mcp/src"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    while let Some(dir) = folders.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let mut missing = Vec::new();
    let mut seen = 0;
    for path in files {
        // A file of tests (`tests.rs`, `*_tests.rs`) says made-up sentences.
        let name = path.file_name().unwrap().to_string_lossy();
        if name == "tests.rs" || name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // The code itself: what comes before the file's tests.
        let code = text.split("#[cfg(test)]").next().unwrap_or("");
        for piece in code.split("Said::new(\"").skip(1) {
            let Some(key) = piece.split('"').next() else {
                continue;
            };
            seen += 1;
            for locale in [Locale::En, Locale::Es] {
                if !has(locale, key) {
                    missing.push(format!("{key} ({locale:?}, {})", path.display()));
                }
            }
        }
    }
    assert!(
        seen > 50,
        "only {seen} sentences found: is the source where it was?"
    );
    assert!(missing.is_empty(), "{seen} keys seen; missing: {missing:?}");
}

/// Review 5 #4: a stray second click never answers the next call — its card is another
/// card (keyed by the call), and a press counts only once that card has been in front for
/// `ARMED`.
#[test]
fn a_second_click_never_answers_the_next_call() {
    use cantus_ui::ai_mcp::card::{self, ARMED};
    use std::time::{Duration, Instant};
    let (mut app, ctx, _dir) = app();
    let catalog = cantus_mcp::Catalog {
        tools: with_remove,
        ..cantus_ui::ai_mcp::catalog::app()
    };
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, catalog);
    let server = app.ai.server().unwrap().clone();
    let calls: Vec<_> = (0..2)
        .map(|_| {
            let s = server.clone();
            std::thread::spawn(move || {
                let mut c = TestClient::legacy(s, "2025-11-25");
                c.call_tool("test_remove_first", json!({}), &mut || {
                    std::thread::sleep(Duration::from_millis(5))
                })
            })
        })
        .collect();
    let end = Instant::now() + Duration::from_secs(20);
    while app.ai.agents().unwrap().asks().count() < 2 {
        assert!(Instant::now() < end, "both calls wait");
        frame(&mut app, &ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    frame(&mut app, &ctx);
    let a = card::card(&app).unwrap();
    let (front, since) = app
        .ai
        .card_front
        .expect("the frame notes the card in front");
    assert_eq!(front, a.id);
    // Pressed the instant it appeared: not taken.
    card::pressed(&mut app, a.id, cantus_mcp::Answer::Deny, since);
    assert_eq!(app.ai.agents().unwrap().asks().count(), 2);
    // Pressed once it has been in front a moment: taken.
    card::pressed(&mut app, a.id, cantus_mcp::Answer::Deny, since + ARMED);
    assert_eq!(app.ai.agents().unwrap().asks().count(), 1);
    frame(&mut app, &ctx);
    let b = card::card(&app).unwrap();
    assert_ne!(b.id, a.id, "the next call is another card");
    let (front, b_since) = app.ai.card_front.unwrap();
    assert_eq!(front, b.id);
    // The second click of a double click, meant for the first card: lands on the second
    // just after it appeared — not taken; nor a press for the call already answered.
    card::pressed(
        &mut app,
        b.id,
        cantus_mcp::Answer::Allow,
        b_since + ARMED / 4,
    );
    card::pressed(
        &mut app,
        a.id,
        cantus_mcp::Answer::Allow,
        b_since + ARMED * 2,
    );
    assert_eq!(app.ai.agents().unwrap().asks().count(), 1, "still waiting");
    card::pressed(&mut app, b.id, cantus_mcp::Answer::Deny, b_since + ARMED);
    assert_eq!(app.ai.agents().unwrap().asks().count(), 0);
    for c in calls {
        let r = c.join().unwrap();
        assert!(r.is_error && r.text.contains("said no"), "{}", r.text);
    }
    assert_eq!(app.project().tracks.len(), 1, "nothing was removed");
}

/// Review 5 #5: a client cannot word the card by naming itself — its name is quoted, its
/// invisible and reordering characters taken out, never filled again, and what let it in
/// is shown beside it.
#[test]
fn a_clients_name_cannot_forge_the_card() {
    let (mut app, ctx, _dir) = app();
    let catalog = cantus_mcp::Catalog {
        tools: with_remove,
        ..cantus_ui::ai_mcp::catalog::app()
    };
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, catalog);
    let server = app.ai.server().unwrap().clone();
    let mut c = TestClient::raw_on(
        server,
        cantus_mcp::Session::external("http:cursor#bridge-1"),
    );
    c.name = "Cantus\u{202E}” · inside Cantus {via} \u{200B}wants".into();
    let _ = c.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": c.name.clone(), "version": "1"}}),
        &mut || {},
    );
    app.settings.ai.external_agents = true;
    let mut seen = None;
    let _ = c.call_tool("test_remove_first", json!({}), &mut || {
        frame(&mut app, &ctx);
        if let Some(card) = cantus_ui::ai_mcp::card::card(&app) {
            let id = card.id;
            seen = Some(card);
            cantus_ui::ai_mcp::card::answer(&mut app, id, cantus_mcp::Answer::Deny);
        }
    });
    let card = seen.expect("a card");
    assert_eq!(
        card.who, "“Cantus\" · inside Cantus ｛via｝ wants”",
        "quoted, its quote made plain, its braces not a placeholder, nothing invisible"
    );
    assert!(!card.who.contains('\u{202E}') && !card.who.contains('\u{200B}'));
    // Review 6 #3: always the token it carried; the bridge only as what it says.
    assert_eq!(
        card.via,
        "· with the “cursor” token, through a bridge (as it says)"
    );
    assert_eq!(card.title, "wants to:", "the name is not in the title");
}

/// Review 6 #3, #5, #7: drawn, a very long name is shortened in its middle while what let
/// it in is drawn whole; the buttons are dimmed (disabled to a screen reader) until the
/// card is armed, then answer; the card is a live region a screen reader announces.
#[test]
fn the_card_draws_who_whole_and_arms_for_eyes_and_screen_readers() {
    use egui::accesskit::{Live, Role};
    let (mut app, ctx, _dir) = app();
    ctx.enable_accesskit();
    let catalog = cantus_mcp::Catalog {
        tools: with_remove,
        ..cantus_ui::ai_mcp::catalog::app()
    };
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, catalog);
    let server = app.ai.server().unwrap().clone();
    let mut c = TestClient::raw_on(server, cantus_mcp::Session::external("http:default"));
    c.name = format!("Agent {}", "x".repeat(200));
    let _ = c.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": c.name.clone(), "version": "1"}}),
        &mut || {},
    );
    app.settings.ai.external_agents = true;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let mut first: Option<egui::FullOutput> = None;
    let mut armed: Option<egui::FullOutput> = None;
    let _ = c.call_tool("test_remove_first", json!({}), &mut || {
        let run = |app: &mut CantusApp| {
            let mut o = ctx.run_ui(input.clone(), |ui| app.frame(ui));
            o.textures_delta.clear();
            o
        };
        run(&mut app);
        let Some(card) = cantus_ui::ai_mcp::card::card(&app) else {
            return;
        };
        if first.is_none() {
            // Drawn again: the card is in front now, not yet armed.
            first = Some(run(&mut app));
            return;
        }
        if armed.is_none() {
            // Armed: the frame notes the time; pretend it came ARMED ago.
            let (id, since) = app.ai.card_front.unwrap();
            app.ai.card_front = Some((id, since - cantus_ui::ai_mcp::card::ARMED));
            armed = Some(run(&mut app));
            cantus_ui::ai_mcp::card::answer(&mut app, card.id, cantus_mcp::Answer::Deny);
        }
    });
    let (first, armed) = (first.expect("drawn"), armed.expect("armed"));
    // The texts drawn: the name elided, the "via" and title whole.
    fn texts(shape: &egui::Shape, out: &mut Vec<(String, bool)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.galley.elided)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }
    let mut drawn = Vec::new();
    for c in &first.shapes {
        texts(&c.shape, &mut drawn);
    }
    let via = drawn
        .iter()
        .find(|(t, _)| t.contains("with the “default” token"))
        .expect("what let it in is drawn");
    assert!(!via.1 && !via.0.contains('…'), "drawn whole: {via:?}");
    assert!(via.0.contains("wants to:"));
    assert!(
        drawn
            .iter()
            .any(|(t, _)| t.starts_with('“') && t.contains('…') && t.ends_with("x”")),
        "the long name is shortened: {drawn:?}"
    );
    // Screen reader: a live region announcing the card; the buttons disabled, then not.
    let nodes = |o: &egui::FullOutput| {
        o.platform_output
            .accesskit_update
            .clone()
            .expect("accesskit is on")
            .nodes
    };
    let live = nodes(&first)
        .into_iter()
        .find(|(_, n)| n.live() == Some(Live::Polite))
        .expect("a live region");
    assert_eq!(live.1.role(), Role::Alert);
    assert!(live.1.label().unwrap_or_default().contains("wants to:"));
    let allow = |o: &egui::FullOutput| {
        nodes(o)
            .into_iter()
            .find(|(_, n)| n.label() == Some("Allow"))
            .map(|(_, n)| n.is_disabled())
            .expect("the Allow button is labelled")
    };
    assert!(allow(&first), "dimmed until armed");
    assert!(!allow(&armed), "armed");
    assert!(
        first
            .viewport_output
            .values()
            .any(|v| v.repaint_delay < std::time::Duration::from_secs(1)),
        "drawn again when it arms"
    );
}
