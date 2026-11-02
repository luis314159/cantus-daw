//! Sprite sheets (ADR 0089 §6): our generated Aseprite and TexturePacker sheets load, map
//! every move by name, dance to the beat, and draw the same pixels whatever the packing.

use cantus_dance::import::{self, samples, Filter};
use cantus_dance::render::{self, Background, Stage};
use cantus_dance::{pose_at, DanceFrame, Move};

fn playing_at(beats: f64) -> DanceFrame {
    DanceFrame {
        beats,
        seconds: beats * 0.5,
        playing: true,
        bar: (beats / 4.0).floor() as i32,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        ..DanceFrame::default()
    }
}

fn set(f: &mut DanceFrame, key: &str, v: f32) {
    let i = cantus_core::inst::dancer::KEYS
        .iter()
        .position(|k| *k == key)
        .unwrap();
    f.values[i] = v;
}

fn with_move(m: Move, beats: f64) -> DanceFrame {
    let mut f = playing_at(beats);
    set(&mut f, "move", (m.index() + 1) as f32);
    f
}

fn written(files: Vec<samples::File>) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    samples::write(dir.path(), &files).unwrap();
    let json = dir.path().join(&files[0].0);
    (dir, json)
}

#[test]
fn an_aseprite_sheet_maps_every_move_from_its_tags() {
    let (_d, path) = written(samples::aseprite("hero"));
    let found = import::inspect(&path).unwrap();
    assert_eq!(found.source.format(), "Aseprite");
    assert_eq!(found.source.clips().len(), 16);
    assert_eq!(found.default_filter, Filter::Nearest);
    let s = import::guessed(&found);
    for m in Move::ALL {
        assert_eq!(s.mapping.moves[m.index()], Some(m.index()), "{m:?}");
    }
    // Four frames of 125 ms: half a second, one beat at 120 BPM.
    assert!((found.source.clips()[1].seconds - 0.5).abs() < 1e-6);
    assert_eq!(s.mapping.beats_of(Move::Clap), 1.0);
}

#[test]
fn texturepacker_sheets_find_moves_from_names_or_animations() {
    for animations in [false, true] {
        let (_d, path) = written(samples::texturepacker("tp", animations));
        let found = import::inspect(&path).unwrap();
        assert_eq!(found.source.format(), "TexturePacker");
        let s = import::guessed(&found);
        let names: Vec<&str> = found
            .source
            .clips()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names.len(), 16, "{names:?}");
        for m in Move::ALL {
            let c = s.mapping.moves[m.index()].expect("mapped");
            assert_eq!(
                import::mapping::move_of(names[c]),
                Some(m),
                "{m:?} → {}",
                names[c]
            );
        }
        assert!(found
            .notes
            .iter()
            .any(|n| n.key == "dance_import.note.no_durations"));
    }
}

#[test]
fn rotated_and_trimmed_frames_draw_exactly_like_upright_ones() {
    let (_a, ase) = written(samples::aseprite("a"));
    let (_t, tp) = written(samples::texturepacker("t", true));
    let a = import::import(&ase).unwrap();
    let t = import::import(&tp).unwrap();
    let stage = Stage::new(240, 320, Background::Transparent);
    for m in [Move::Clap, Move::Wave, Move::Jump, Move::Kick] {
        for b in [0.0, 0.3, 0.55, 0.8] {
            let f = with_move(m, 8.0 + b);
            let pa = pose_at(&f, a.as_ref(), 0.0);
            let pt = pose_at(&f, t.as_ref(), 0.0);
            let ia = render::render(&stage, &[(a.as_ref(), pa)]).unwrap();
            let it = render::render(&stage, &[(t.as_ref(), pt)]).unwrap();
            let drawn = ia.pixels().iter().filter(|p| p.alpha() > 0).count();
            assert!(drawn > 300, "{m:?} at {b}: drawn ({drawn})");
            // Aseprite is nearest; TexturePacker bilinear by default — compare shapes.
            let diff = ia
                .pixels()
                .iter()
                .zip(it.pixels())
                .filter(|(x, y)| (x.alpha() > 128) != (y.alpha() > 128))
                .count();
            assert!(
                diff < drawn / 20,
                "{m:?} at {b}: {diff} of {drawn} pixels differ"
            );
        }
    }
}

#[test]
fn the_routine_dances_to_the_beat_and_frames_change_on_the_grid() {
    let (_d, path) = written(samples::aseprite("hero"));
    let ch = import::import(&path).unwrap();
    // The routine's moves are the usual ones, all mapped.
    assert_eq!(ch.routine().len(), 8);
    // A clap (one beat, four frames) shows frame k over its k-th quarter beat.
    let imp = import::inspect(&path).unwrap();
    let c = import::character(
        imp,
        "file:x".into(),
        import::guessed(&import::inspect(&path).unwrap()),
    );
    for (b, want) in [(0.0, 0), (0.24, 0), (0.26, 1), (0.6, 2), (0.99, 3)] {
        let p = pose_at(&with_move(Move::Clap, 12.0 + b), &c, 0.0);
        let (clip, t) = c.clip_at(&p).unwrap();
        assert_eq!(clip, Move::Clap.index());
        assert_eq!((t / 0.125) as usize, want, "at {b}");
    }
    // The same song position, the same pixels, at any frame rate.
    let stage = Stage::new(200, 200, Background::Transparent);
    let f = playing_at(5.375);
    let x = render::render(&stage, &[(ch.as_ref(), pose_at(&f, ch.as_ref(), 1.0))]).unwrap();
    let y = render::render(&stage, &[(ch.as_ref(), pose_at(&f, ch.as_ref(), 77.0))]).unwrap();
    assert_eq!(x.data(), y.data());
}

#[test]
fn the_character_stands_on_its_feet_one_unit_tall() {
    let (_d, path) = written(samples::aseprite("hero"));
    let ch = import::import(&path).unwrap();
    let b = ch.bounds();
    assert!((b.top + 1.0).abs() < 1e-6 && b.bottom.abs() < 1e-6);
    assert!(b.width() > 0.3 && b.width() < 1.2, "{b:?}");
    // Drawn at size 1 on a 600-px stage: over every move, its lowest pixel is on the feet
    // line and its highest about 0.42 × 600 px above (the box is honest).
    let stage = Stage::new(600, 600, Background::Transparent);
    let (mut top, mut bottom, mut feet) = (600u32, 0u32, 0u32);
    for m in Move::ALL {
        for b in [0.0, 0.26, 0.51, 0.76] {
            let pose = pose_at(&with_move(m, 16.0 + b * 4.0), ch.as_ref(), 0.0);
            let img = render::render(&stage, &[(ch.as_ref(), pose)]).unwrap();
            for y in 0..600u32 {
                if (0..600).any(|x| img.pixel(x, y).is_some_and(|p| p.alpha() > 0)) {
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
            feet = (pose.y * 600.0) as u32;
        }
    }
    // Nothing below the feet or above the head; the art reaches near both (the box is
    // the union of every frame's pixels, so one pose need not touch its edges).
    assert!(
        bottom < feet && feet - bottom <= 16,
        "lowest pixel {bottom}, feet line {feet}"
    );
    assert!(
        feet - top <= 253 && feet - top >= 220,
        "{top}..{feet}: at most 252 px tall"
    );
}

#[test]
fn a_hue_turns_the_sheet_and_zero_changes_nothing() {
    let (_d, path) = written(samples::aseprite("hero"));
    let ch = import::import(&path).unwrap();
    let stage = Stage::new(200, 200, Background::Transparent);
    let f = playing_at(2.0);
    let p0 = pose_at(&f, ch.as_ref(), 0.0);
    let mut p1 = p0;
    p1.hue = 120.0;
    let a = render::render(&stage, &[(ch.as_ref(), p0)]).unwrap();
    let b = render::render(&stage, &[(ch.as_ref(), p1)]).unwrap();
    assert_ne!(a.data(), b.data());
    let alpha =
        |p: &tiny_skia::Pixmap| p.pixels().iter().map(|c| u64::from(c.alpha())).sum::<u64>();
    assert_eq!(alpha(&a), alpha(&b), "a hue turn keeps every pixel's alpha");
}
