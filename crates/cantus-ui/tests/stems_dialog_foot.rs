//! Separate stems keeps its actions in view (round 39, `shots`): the kit's foot band is the
//! dialog's foot, outside the rows' scroll. Drawn inside the rows, it sat in the middle of
//! the dialog with the model's card under it, and once the card grew, *Accept the licence
//! and download* was below the fold.

mod support;

use cantus_ui::stems_ui::Target;

/// Every text shape a frame painted with its rect and the rect it is clipped to.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect, egui::Rect)> {
    fn walk(
        shape: &egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(String, egui::Rect, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Text(t) => {
                out.push((t.galley.text().to_string(), t.visual_bounding_rect(), clip))
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

#[test]
fn the_dialogs_actions_stay_in_view_under_its_rows() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let project = cantus_ui::welcome_project();
    let track = &project.tracks[0];
    let target = Target::Clips(vec![(track.id, track.clips[0].id)]);
    let mut app = support::app(project);
    app.settings_path = data.path().join("settings.json");
    app.stems.dialog = Some(target);
    let ctx = support::ctx();
    let size = egui::vec2(1280.0, 720.0);
    let mut shapes = Vec::new();
    for i in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.1),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| {
            cantus_ui::stems_ui::view::dialog(&mut app, root.ctx())
        });
        out.textures_delta.clear();
        shapes = out.shapes;
    }
    let dialog = ctx
        .memory(|m| m.area_rect(egui::Id::new("cantus-stems-dialog")))
        .expect("the dialog is drawn");
    let painted = texts(&shapes);
    let action = painted
        .iter()
        .find(|(t, _, _)| {
            // A fresh data folder: the model is not accepted yet.
            t.contains("Accept the licence")
        })
        .unwrap_or_else(|| {
            panic!(
                "an action is painted: {:?}",
                painted.iter().map(|p| &p.0).collect::<Vec<_>>()
            )
        });
    let (_, rect, clip) = action;
    assert!(
        dialog.contains_rect(*rect),
        "the action {rect:?} is inside the dialog {dialog:?}"
    );
    assert!(
        clip.contains_rect(*rect),
        "the action {rect:?} is not clipped away ({clip:?})"
    );
    // The foot is the dialog's last thing: nothing of the rows is painted under it.
    let lowest_row = painted
        .iter()
        .filter(|(_, r, c)| {
            dialog.contains_rect(*r) && c.intersects(*r) && r.bottom() < dialog.bottom()
        })
        .filter(|(t, _, _)| {
            // The foot's own words: the note on accepting, the key's hint, the button.
            !(t.contains("Accept")
                || t.contains("accepting")
                || t.eq_ignore_ascii_case("esc")
                || t == "cancels")
        })
        .map(|(t, r, c)| (r.intersect(*c).bottom(), t))
        .fold(
            (f32::MIN, None),
            |a, (b, t)| if b > a.0 { (b, Some(t)) } else { a },
        );
    assert!(
        lowest_row.0 <= rect.top(),
        "a row is painted under the foot's action ({:?} at {} > {})",
        lowest_row.1,
        lowest_row.0,
        rect.top()
    );
}
