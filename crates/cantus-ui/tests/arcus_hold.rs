//! r42:arcus-upper — what holds Arcus's instruments in their pictures (the owner,
//! 2026-11-24): the preference and its migration through the file the app reads at start,
//! the figure's left hand reaching the stopped note (to the pixel at 100 %), its positions
//! and its glide, and the stop's marker holding 3:1 in every theme.

use cantus_ui::scene::bowed::{rig_for, Bowed};
use cantus_ui::scene::bowed_draw;
use cantus_ui::scene::bowed_hold::{elbow, Hold, Holder, StopShow};
use cantus_ui::scene::Room;
use cantus_ui::settings::Settings;
use cantus_ui::views::inst::arcus::hand;

#[test]
fn the_fingertip_switch_migrates_into_how_the_stop_is_shown() {
    let read = |j: &str| serde_json::from_str::<Settings>(j).unwrap().migrated();
    assert_eq!(
        read(r#"{"arcus_finger": false}"#).arcus_stop,
        StopShow::None
    );
    assert_eq!(
        read(r#"{"arcus_finger": true}"#).arcus_stop,
        StopShow::Finger
    );
    assert_eq!(read("{}").arcus_stop, StopShow::Finger);
    assert_eq!(
        read(r#"{"arcus_stop": "marker"}"#).arcus_stop,
        StopShow::Marker
    );
    assert_eq!(read("{}").arcus_holder, Holder::default());
    // Through the file the app reads at start.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{ "arcus_finger": false, "arcus_holder": { "hold": "player", "skin": 2 } }"#,
    )
    .unwrap();
    let s = Settings::load(&path);
    assert_eq!(s.arcus_stop, StopShow::None);
    assert_eq!(s.arcus_holder.hold, Hold::Player);
    assert_eq!(s.arcus_holder.skin, 2);
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        !json.contains("arcus_finger") && json.contains(r#""arcus_stop":"none""#),
        "{json}"
    );
}

#[test]
fn the_hand_keeps_its_position_and_shifts_beyond_it() {
    // First position on the violin's A string (first finger a tone up): up to the fourth
    // finger's E, the hand stays.
    for s in [2.0, 4.0, 5.0, 7.0] {
        assert_eq!(hand::target(2.0, s), 2.0, "{s} semitones");
    }
    // An octave up: it shifts so the note falls under a finger.
    let up = hand::target(2.0, 12.0);
    assert!((7.0..=11.0).contains(&up), "{up}");
    assert!(12.0 - up <= 5.0);
    // An open string never moves it; a low note brings it back down.
    assert_eq!(hand::target(7.0, 0.0), 7.0);
    assert!(hand::target(7.0, 3.0) <= 3.0);
    // The shift glides: about a tenth of a second at 60 Hz, never past the target.
    let mut at = 2.0;
    let mut frames = 0;
    while at != 7.0 && frames < 60 {
        let n = hand::glide(at, 7.0, 1.0 / 60.0);
        assert!(n > at && n <= 7.0);
        at = n;
        frames += 1;
    }
    assert!((4..=20).contains(&frames), "{frames} frames");
}

#[test]
fn the_figures_hand_lands_on_the_stopped_point() {
    let size = egui::vec2(590.0, 430.0);
    for b in [Bowed::Violin, Bowed::Viola, Bowed::Cello, Bowed::Bass] {
        for hold in [Hold::Player, Hold::Mannequin] {
            let h = Holder::of(hold);
            let rig = rig_for(b, h);
            let mut cam = bowed_draw::camera(b, h, Room::Studio);
            let fr = bowed_draw::frame_for(size);
            let [l, t, r, bt] = fr.inset;
            let fb = (l, t, size.x - l - r, size.y - t - bt);
            cam.fit(&bowed_draw::points(b, h), fb, fr.margin, fr.anchor);
            for (string, st) in [(1usize, 0.75f32), (2, 0.5), (3, 0.89)] {
                let l = hand::Left {
                    rig: &rig,
                    string,
                    semis: hand::semis(st),
                    at: hand::target(1.0, hand::semis(st)),
                    rock: 0.0,
                    side: rig.pose.ex,
                };
                let tip = hand::playing_tip(&l).expect("a stopped note");
                let [_, bridge, nut] = rig.strings[string];
                let stop = bridge + (nut - bridge) * st + rig.normal[string] * rig.gauge[string];
                let d = cam.p(tip) - cam.p(stop);
                assert!(
                    d.length() < 1.0,
                    "{b:?} {hold:?}: string {string} at {st}: {d:?} px off"
                );
            }
            // An open string lifts the finger.
            let open = hand::Left {
                rig: &rig,
                string: 0,
                semis: hand::semis(1.0),
                at: 3.0,
                rock: 0.0,
                side: rig.pose.ex,
            };
            assert!(hand::playing_tip(&open).is_none());
        }
    }
}

#[test]
fn the_arms_elbow_keeps_the_arms_lengths() {
    use cantus_ui::scene::geo::v;
    let s = v(0.0, 100.0, 0.0);
    let w = v(30.0, 80.0, 20.0);
    let e = elbow(s, w, 28.0, 26.0, v(0.0, -1.0, 0.0));
    assert!(((e - s).len() - 28.0).abs() < 0.05);
    assert!(((w - e).len() - 26.0).abs() < 0.05);
    assert!(e.y < s.y.min(w.y) + 5.0, "the elbow falls toward its pole");
}

/// The stop's marker is the theme's focus colour on a halo of its own (as the scene's
/// selection): 3:1 at least in every theme, whatever wood or ebony is under it.
#[test]
fn the_stop_marker_holds_three_to_one_in_every_theme() {
    use cantus_ui::kit::appearance::PRESETS;
    use cantus_ui::kit::color::contrast;
    for (id, make) in PRESETS {
        let t = make().kit_tokens();
        let halo = cantus_ui::scene::halo_for(t.focus);
        let c = contrast(t.focus, halo);
        assert!(c >= 3.0, "{id}: the marker {c:.2}:1 on its halo");
    }
}
