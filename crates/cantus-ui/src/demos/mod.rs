//! The demo songs (round 19; round 28's showcase, *With All Your Heart*, first): seven
//! pieces in seven styles that show what Cantus does —
//! every native instrument, the effects on strips with musical settings, sends to shared
//! returns, buses, a mastering chain on the master — and the project the program opens
//! on, cut from the strongest of them.
//!
//! Each song is built in code from a chord chart, melodies written as text and drum
//! grids ([`theory`], [`parts`]), played with a player's feel ([`song::Feel`]), into an
//! ordinary [`Project`]: nothing here that a musician could not have made by hand, and
//! everything survives save and open. The File menu's *Demo songs* opens them
//! (`views::demos`).

pub mod parts;
pub mod song;
pub mod theory;

mod ambient;
mod ballad;
mod funk;
mod heart;
mod house;
mod lofi;
mod trap;

use cantus_core::{Project, Ticks};

/// A demo: its id, its name and style (locale keys) and how to build it.
#[derive(Copy, Clone, Debug)]
pub struct Demo {
    pub id: &'static str,
    pub name_key: &'static str,
    pub style_key: &'static str,
    pub build: fn() -> Project,
}

/// Every demo, in the order the picker lists them.
pub const ALL: [Demo; 7] = [
    Demo {
        id: "heart",
        name_key: "demos.song.heart",
        style_key: "demos.style.heart",
        build: heart::build,
    },
    Demo {
        id: "house",
        name_key: "demos.song.house",
        style_key: "demos.style.house",
        build: house::build,
    },
    Demo {
        id: "ballad",
        name_key: "demos.song.ballad",
        style_key: "demos.style.ballad",
        build: ballad::build,
    },
    Demo {
        id: "funk",
        name_key: "demos.song.funk",
        style_key: "demos.style.funk",
        build: funk::build,
    },
    Demo {
        id: "lofi",
        name_key: "demos.song.lofi",
        style_key: "demos.style.lofi",
        build: lofi::build,
    },
    Demo {
        id: "ambient",
        name_key: "demos.song.ambient",
        style_key: "demos.style.ambient",
        build: ambient::build,
    },
    Demo {
        id: "trap",
        name_key: "demos.song.trap",
        style_key: "demos.style.trap",
        build: trap::build,
    },
];

/// A demo by its id.
pub fn find(id: &str) -> Option<&'static Demo> {
    ALL.iter().find(|d| d.id == id)
}

/// What the program opens on: the house track's second drop, sixteen bars looping, with
/// its whole desk: named, coloured strips, the drum bus, the
/// sidechained chords, the reverb and delay returns and the master chain.
pub fn startup_project() -> Project {
    house::startup()
}

/// How long a project plays, in seconds: to the end of its last clip.
pub fn seconds(p: &Project) -> f64 {
    p.tempo_map().seconds_between(Ticks::ZERO, p.content_end())
}
