//! Review 1: a hostile file in a project can never take the app down — a panic while
//! reading becomes an error in words (the dancer then draws the default character), and a
//! panic while drawing leaves that character undrawn instead of the frame.

use cantus_dance::character::{Bounds, CharacterInfo, Origin};
use cantus_dance::import::{self, Clip, Filter, Imported, Mapping, Settings, Source};
use cantus_dance::render::{self, Background, Canvas, Stage};
use cantus_dance::{pose_at, DanceFrame, Licence};

#[test]
fn a_panic_while_reading_is_an_error_in_words() {
    let r: Result<(), import::ImportError> = import::guarded(|| panic!("a parser's bug"));
    assert!(matches!(r, Err(import::ImportError::Invalid(_))), "{r:?}");
    let ok: Result<u8, import::ImportError> = import::guarded(|| Ok(7));
    assert_eq!(ok.unwrap(), 7);
}

struct Bomb(Vec<Clip>);

impl Source for Bomb {
    fn format(&self) -> &'static str {
        "Bomb"
    }
    fn clips(&self) -> &[Clip] {
        &self.0
    }
    fn bounds(&self) -> Bounds {
        Bounds::STANDING
    }
    fn draw(&self, _: usize, _: f32, _: Filter, _: &mut Canvas) {
        panic!("a drawing bug");
    }
    fn frame_rate(&self) -> f32 {
        24.0
    }
}

#[test]
fn a_panic_while_drawing_leaves_the_character_out_not_the_frame() {
    let clips = vec![Clip {
        name: "idle".into(),
        seconds: 1.0,
        frames: 24,
    }];
    let info = CharacterInfo {
        id: "file:bomb".into(),
        name: "bomb".into(),
        author: String::new(),
        origin: Origin::Imported { format: "Bomb" },
        licence: Licence::Unknown,
    };
    let settings = Settings {
        mapping: Mapping::guess(&clips),
        filter: Filter::Bilinear,
        own_speed: false,
    };
    let ch = Imported::new(info, Box::new(Bomb(clips)), settings);
    let stage = Stage::new(64, 64, Background::Transparent);
    let f = DanceFrame {
        playing: true,
        beats: 1.0,
        ..DanceFrame::default()
    };
    for _ in 0..2 {
        let img = render::render(&stage, &[(&ch, pose_at(&f, &ch, 0.0))]).unwrap();
        assert!(img.pixels().iter().all(|p| p.alpha() == 0));
    }
    assert!(ch.is_broken());
}
