//! r39:lo-arrange — the arrangement's tool row, its tools, the clips' states, the piano
//! roll's head and the maximised arrangement, end to end: through `CantusApp::assemble` as
//! the app starts, the whole window drawn at 1920 × 1080, the pointer pressing where the
//! user would.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::{AutoPoint, AutoTarget, Automation, Command, Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::Action;
use cantus_ui::views::arrangement::tools::Tool;
use cantus_ui::CantusApp;
use egui::{pos2, Pos2, Rect};
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct W {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    texts: Vec<(String, Rect)>,
    mods: egui::Modifiers,
}

impl W {
    fn new() -> W {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, _proc) = EngineHandle::manual(48_000.0);
        let project = cantus_ui::demos::startup_project();
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        let mut w = W {
            app,
            ctx,
            t: 0.0,
            texts: Vec::new(),
            mods: egui::Modifiers::NONE,
        };
        for _ in 0..4 {
            w.frame(vec![]);
        }
        w
    }

    fn frame(&mut self, mut events: Vec<egui::Event>) {
        self.t += 1.0 / 60.0;
        events.insert(0, egui::Event::ModifiersChanged(self.mods));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1920.0, 1080.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        self.texts.clear();
        for c in &out.shapes {
            walk(&c.shape, &mut self.texts);
        }
    }

    fn word(&self, word: &str) -> Option<Rect> {
        self.texts
            .iter()
            .rev()
            .find(|(t, _)| t.trim() == word)
            .map(|(_, r)| *r)
    }

    fn has(&self, part: &str) -> bool {
        self.texts.iter().any(|(t, _)| t.contains(part))
    }

    fn button(&self, at: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: self.mods,
        }
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        let (d, u) = (self.button(at, true), self.button(at, false));
        self.frame(vec![d]);
        self.frame(vec![u]);
        self.frame(vec![]);
        // Past the double-click window, so the next click is a click of its own.
        self.t += 1.0;
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        let d = self.button(from, true);
        self.frame(vec![d]);
        for i in 1..=6 {
            let p = from + (to - from) * (i as f32 / 6.0);
            self.frame(vec![egui::Event::PointerMoved(p)]);
        }
        let u = self.button(to, false);
        self.frame(vec![u]);
        self.frame(vec![]);
        self.t += 1.0;
    }

    fn geo(&self) -> cantus_ui::views::arrangement::TimelineGeometry {
        self.app.arrangement.geometry().expect("drawn")
    }

    /// A track whose playing lane is on screen and holds a note clip in view: the track's
    /// index, the clip's index, and the clip's rect as drawn.
    fn note_clip(&mut self) -> (usize, usize, Rect) {
        // The song's clips are eight bars: at 20 points a beat two of them fit in view.
        if (self.app.arrangement.px_per_beat - 20.0).abs() > 0.01
            || self.app.arrangement.scroll_beats != 0.0
        {
            self.app.arrangement.px_per_beat = 20.0;
            self.app.arrangement.scroll_beats = 0.0;
            self.frame(vec![]);
            self.frame(vec![]);
        }
        let g = self.geo();
        for (i, t) in self.app.project().tracks.iter().enumerate() {
            let Some(row) = g.rows.get(i) else { continue };
            if row.height < 20.0 || row.top < g.body_top || row.top + row.height > g.lanes_bottom {
                continue;
            }
            for (k, c) in t.clips.iter().enumerate() {
                if c.audio.is_some() || c.automation.is_some() || c.lane > 0 {
                    continue;
                }
                let x0 = g.x(c.start.as_beats_f64() as f32);
                let x1 = g.x(c.end().as_beats_f64() as f32);
                if x0 >= g.left - 1.0 && x1 - x0 > 120.0 && x1 < g.left + g.width - 4.0 {
                    let r = Rect::from_min_max(pos2(x0, row.top), pos2(x1, row.top + row.height));
                    return (i, k, r);
                }
            }
        }
        let dump: Vec<String> = self
            .app
            .project()
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "{i} {:?} {:?}",
                    g.rows.get(i).map(|r| (r.top, r.height)),
                    t.clips
                        .iter()
                        .map(|c| (
                            c.start.as_beats_f64(),
                            c.length.as_beats_f64(),
                            c.notes.len(),
                            c.lane
                        ))
                        .collect::<Vec<_>>()
                )
            })
            .collect();
        panic!(
            "no note clip in view: left {} width {} px {} body {}..{}
{}",
            g.left,
            g.width,
            g.px_per_beat,
            g.body_top,
            g.lanes_bottom,
            dump.join(
                "
"
            )
        );
    }

    /// An empty stretch of a track's lane in view: the track and a point there.
    fn empty_spot(&mut self) -> (usize, Pos2) {
        let end = self.app.project().content_end().as_beats_f64() as f32;
        self.app.arrangement.scroll_beats = (end - 8.0).max(0.0);
        self.frame(vec![]);
        self.frame(vec![]);
        let g = self.geo();
        let x = g.x(end + 6.0);
        assert!(x < g.left + g.width - 40.0, "the song's end is in view");
        let row = g.rows[0];
        (0, pos2(x, row.top + row.height * 0.5))
    }

    fn pick(&mut self, tool: Tool) {
        self.app.focus_arrangement();
        self.app.run_action(tool.action());
        assert_eq!(self.app.arrangement.tool, tool);
    }
}

#[test]
fn the_arrangement_s_tools_end_to_end() {
    let _ = data();
    let mut w = W::new();
    the_tool_row_is_the_sheet_s(&mut w);
    the_tool_keys_reach_the_view_being_worked_in(&mut w);
    draw_adds_and_stretches_a_clip(&mut w);
    slice_cuts_a_clip_on_the_grid(&mut w);
    mute_and_erase_act_on_what_they_touch(&mut w);
    zoom_clicks_and_fits(&mut w);
    the_notch_repeats_a_clip(&mut w);
    the_grid_menu_and_its_keys(&mut w);
    automation_values_show_at_the_close_zoom(&mut w);
    the_piano_roll_s_head(&mut w);
    the_maximised_arrangement_folds_the_docks_to_a_row(&mut w);
}

fn the_tool_row_is_the_sheet_s(w: &mut W) {
    w.frame(vec![]);
    for want in ["Arrangement", "Grid", "1 bar", "Follow"] {
        assert!(
            w.word(want).is_some(),
            "the tool row never painted {want:?}"
        );
    }
    assert_eq!(
        w.app.arrangement.tool,
        Tool::Select,
        "Select is in hand at start"
    );
    assert!(w.app.arrangement.follow, "Follow is on at start");
    // Follow's key turns it off and on (a click on its word).
    let f = w.word("Follow").unwrap();
    w.click(f.center());
    assert!(!w.app.arrangement.follow);
    w.click(f.center());
    assert!(w.app.arrangement.follow);
}

fn the_tool_keys_reach_the_view_being_worked_in(w: &mut W) {
    use cantus_ui::views::piano_roll::Tool as RollTool;
    let roll = w.app.piano_roll.tool;
    w.app.focus_arrangement();
    w.app.run_action(Action::ToolDraw);
    assert_eq!(w.app.arrangement.tool, Tool::Draw);
    assert_eq!(
        w.app.piano_roll.tool, roll,
        "the piano roll's tool is its own"
    );
    w.app.run_action(Action::ToolZoom);
    assert_eq!(w.app.arrangement.tool, Tool::Zoom);
    w.app.run_action(Action::ToolSelect);
    assert_eq!(w.app.arrangement.tool, Tool::Select);
    // The piano roll worked in: its tool, as before.
    w.app.focus_piano_roll();
    w.app.run_action(Action::ToolSlice);
    assert_eq!(w.app.piano_roll.tool, RollTool::Slice);
    assert_eq!(w.app.arrangement.tool, Tool::Select);
    w.app.run_action(Action::ToolDraw);
    w.app.focus_arrangement();
}

fn clips_on(w: &W, i: usize) -> usize {
    w.app.project().tracks[i].clips.len()
}

fn draw_adds_and_stretches_a_clip(w: &mut W) {
    w.pick(Tool::Draw);
    let (i, at) = w.empty_spot();
    let before = clips_on(w, i);
    w.click(at);
    assert_eq!(clips_on(w, i), before + 1, "a click drew a clip");
    let c = w.app.project().tracks[i].clips.last().unwrap().clone();
    let bar = w.app.project().time_signature.ticks_per_bar();
    assert_eq!(c.start.0 % bar.0, 0, "on the bar it was clicked in");
    assert_eq!(c.length, bar, "a bar long");
    w.app.undo();
    assert_eq!(clips_on(w, i), before, "one undo step");
    // A drag stretches it: four bars.
    let g = w.geo();
    let to = pos2(at.x + 4.0 * 4.0 * g.px_per_beat, at.y);
    w.drag(at, to);
    assert_eq!(clips_on(w, i), before + 1, "a drag drew one clip");
    let c = w.app.project().tracks[i].clips.last().unwrap().clone();
    assert!(c.length.0 >= bar.0 * 3, "stretched: {:?}", c.length);
    w.app.undo();
    assert_eq!(clips_on(w, i), before);
    w.app.arrangement.scroll_beats = 0.0;
    w.frame(vec![]);
    w.pick(Tool::Select);
}

fn slice_cuts_a_clip_on_the_grid(w: &mut W) {
    w.pick(Tool::Slice);
    let (i, _, r) = w.note_clip();
    let before = clips_on(w, i);
    w.click(pos2(r.center().x, r.center().y));
    assert_eq!(clips_on(w, i), before + 1, "the clip is in two");
    w.app.undo();
    assert_eq!(clips_on(w, i), before, "one undo step");
    w.pick(Tool::Select);
}

fn mute_and_erase_act_on_what_they_touch(w: &mut W) {
    w.pick(Tool::Mute);
    let (i, k, r) = w.note_clip();
    w.click(r.center());
    assert!(w.app.project().tracks[i].clips[k].muted, "muted");
    w.click(r.center());
    assert!(!w.app.project().tracks[i].clips[k].muted, "and back");
    w.pick(Tool::Erase);
    let before = clips_on(w, i);
    w.drag(r.center(), pos2(r.right() - 4.0, r.center().y));
    assert_eq!(clips_on(w, i), before - 1, "the eraser took the clip");
    w.app.undo();
    assert_eq!(clips_on(w, i), before, "one undo step for the sweep");
    w.pick(Tool::Select);
}

fn zoom_clicks_and_fits(w: &mut W) {
    w.pick(Tool::Zoom);
    let (_, _, r) = w.note_clip();
    let px = w.app.arrangement.px_per_beat;
    w.click(r.center());
    assert!(
        (w.app.arrangement.px_per_beat - px * 2.0).abs() < 0.01,
        "zoomed in"
    );
    w.mods = egui::Modifiers::ALT;
    w.click(r.center());
    w.mods = egui::Modifiers::NONE;
    assert!((w.app.arrangement.px_per_beat - px).abs() < 0.01, "and out");
    // A drag across eight beats fits them.
    let g = w.geo();
    let y = g.rows[0].top + 4.0;
    let a = pos2(g.x(4.0), y);
    let b = pos2(g.x(12.0), y);
    w.drag(a, b);
    let v = &w.app.arrangement;
    assert!((v.scroll_beats - 4.0).abs() < 0.1, "{}", v.scroll_beats);
    assert!(
        (v.px_per_beat * 8.0 - g.width).abs() < 2.0,
        "{}",
        v.px_per_beat
    );
    w.app.arrangement.px_per_beat = 26.0;
    w.app.arrangement.scroll_beats = 0.0;
    w.pick(Tool::Select);
    w.frame(vec![]);
}

fn the_notch_repeats_a_clip(w: &mut W) {
    w.frame(vec![]);
    let (i, k, r) = w.note_clip();
    let c = w.app.project().tracks[i].clips[k].clone();
    let g = w.geo();
    let from = pos2(r.right() - 3.0, r.top() + 3.0);
    let to = pos2(from.x + 4.0 * 4.0 * g.px_per_beat, from.y);
    w.drag(from, to);
    let now = w.app.project().tracks[i].clips[k].clone();
    assert!(
        now.length > c.length,
        "drawn out: {:?} → {:?}",
        c.length,
        now.length
    );
    assert_eq!(
        now.content_length,
        c.content_length.or(Some(c.length)),
        "repeating at the length it had"
    );
    w.app.undo();
    let back = &w.app.project().tracks[i].clips[k];
    assert_eq!(
        (back.length, back.content_length),
        (c.length, c.content_length)
    );
}

fn the_grid_menu_and_its_keys(w: &mut W) {
    use cantus_ui::views::arrangement::tools::is_triplet;
    w.frame(vec![]);
    // The grid's well opens the menu in musical order with Triplets and Follow the zoom.
    let well = w.word("1 bar").expect("the grid's value");
    w.click(well.center());
    for want in [
        "Bar",
        "1/2",
        "Beat",
        "1/8",
        "1/16",
        "1/32",
        "Triplets",
        "Follow the zoom",
    ] {
        assert!(w.word(want).is_some(), "the grid menu has no {want:?}");
    }
    let (b, t) = (w.word("Beat").unwrap(), w.word("Triplets").unwrap());
    assert!(b.top() < t.top(), "the values, then Triplets");
    w.click(w.word("Beat").unwrap().center());
    assert_eq!(w.app.arrangement.grid, Ticks(cantus_core::PPQ));
    // Shift+3, Triplets' key.
    w.app.focus_arrangement();
    w.app.run_action(Action::GridTriplets);
    assert!(is_triplet(w.app.arrangement.grid));
    w.app.run_action(Action::GridTriplets);
    assert!(!is_triplet(w.app.arrangement.grid));
    // Follow the zoom: the grid follows the zoom.
    w.app.arrangement.grid_auto = true;
    w.app.arrangement.px_per_beat = 200.0;
    w.frame(vec![]);
    let fine = w.app.arrangement.grid;
    w.app.arrangement.px_per_beat = 8.0;
    w.frame(vec![]);
    assert!(w.app.arrangement.grid > fine, "coarser as it zooms out");
    assert!(w.has("Auto"), "the well says it follows");
    w.app.arrangement.grid_auto = false;
    w.app.arrangement.grid = Ticks::beats(4);
    w.app.arrangement.px_per_beat = 26.0;
    w.frame(vec![]);
}

fn automation_values_show_at_the_close_zoom(w: &mut W) {
    let t: TrackId = w.app.project().tracks[0].id;
    let mut clip = w.app.bus.new_clip("gain", Ticks::ZERO, Ticks::beats(8));
    clip.automation = Some(Automation::new(
        AutoTarget::TrackGain(t),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.2),
            AutoPoint::new(Ticks::beats(4), 0.8),
            AutoPoint::new(Ticks::beats(8), 0.5),
        ],
    ));
    assert!(w.app.exec(Command::InsertClip {
        track: t,
        clip: Box::new(clip)
    }));
    w.app.arrangement.scroll_beats = 0.0;
    w.app.arrangement.scroll_y = 0.0;
    let in_lanes = |w: &W| {
        let g = w.geo();
        w.texts
            .iter()
            .filter(|(s, r)| {
                s.contains("dB")
                    && r.left() > g.left + 2.0
                    && r.right() < g.left + g.width
                    && r.top() > g.body_top
                    && r.bottom() < g.lanes_bottom
            })
            .count()
    };
    w.app.arrangement.px_per_beat = 26.0;
    w.frame(vec![]);
    w.frame(vec![]);
    assert_eq!(in_lanes(w), 0, "no values at the default zoom");
    w.app.arrangement.px_per_beat = 90.0;
    w.frame(vec![]);
    w.frame(vec![]);
    assert!(in_lanes(w) >= 2, "each point's value at the close zoom");
    w.app.undo();
    w.app.arrangement.px_per_beat = 26.0;
    w.frame(vec![]);
}

fn the_piano_roll_s_head(w: &mut W) {
    let (i, k, _) = w.note_clip();
    let (t, c) = {
        let tr = &w.app.project().tracks[i];
        (tr.id, tr.clips[k].id)
    };
    let name = w.app.project().tracks[i].name.clone();
    w.app.open_clip_editor(t, c);
    w.frame(vec![]);
    w.frame(vec![]);
    assert!(w.word(&name).is_some(), "the clip menu names the track");
    assert!(w.word("›").is_some(), "track › clip");
    assert!(w.has(" notes"), "the count at rest");
    // Two notes selected: the readout says so, with their range.
    let ids: Vec<_> = w.app.project().tracks[i].clips[k]
        .notes
        .iter()
        .take(2)
        .map(|n| n.id)
        .collect();
    w.app.selected_notes = ids;
    w.frame(vec![]);
    assert!(w.has("2 notes selected"), "the selection readout");
    assert!(w.has("velocity"), "its range");
    // The clip menu lists the track's clips and the other tracks.
    let at = w.word(&name).unwrap().center();
    w.click(at);
    assert!(w.has("Other tracks"), "the other tracks in the clip menu");
    w.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    w.app.selected_notes.clear();
    w.frame(vec![]);
}

fn the_maximised_arrangement_folds_the_docks_to_a_row(w: &mut W) {
    use cantus_ui::views::workspace::Pane;
    w.app.workspace.toggle_maximise(Pane::Arrangement);
    w.frame(vec![]);
    w.frame(vec![]);
    assert_eq!(w.app.workspace.maximised(), Some(Pane::Arrangement));
    let show = w.word("Show the docks").expect("the docks' row");
    assert!(show.bottom() > 1000.0, "at the foot: {show:?}");
    assert!(w.word("Mixer").is_some(), "the mixer's tab in the row");
    w.click(show.center());
    assert_eq!(w.app.workspace.maximised(), None, "the docks are back");
    // A tab in the row brings its area back in front.
    w.app.workspace.toggle_maximise(Pane::Arrangement);
    w.frame(vec![]);
    w.frame(vec![]);
    let mixer = w
        .texts
        .iter()
        .rev()
        .find(|(t, r)| t.trim() == "Mixer" && r.top() > 1000.0)
        .map(|(_, r)| *r)
        .expect("the mixer's tab in the row");
    w.click(mixer.center());
    assert_eq!(w.app.workspace.maximised(), None);
    assert!(w.app.workspace.is_visible(Pane::Mixer));
}
