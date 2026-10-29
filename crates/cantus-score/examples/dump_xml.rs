//! Writes every golden passage as MusicXML (to open in MuseScore and compare):
//!
//! ```text
//! cargo run -p cantus-score --example dump_xml -- <dir>
//! ```
use cantus_score::build::{build, Song};
use cantus_score::musicxml::{write, XmlOptions};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).expect("the folder");
    for p in cantus_score::samples::all() {
        let song = Song {
            title: p.title.into(),
            composer: "Cantus".into(),
            tempo: &p.tempo,
            key: p.key,
        };
        let score = build(&song, &p.parts, None);
        let xml = write(
            &score,
            &XmlOptions {
                software: "Cantus".into(),
                date: None,
            },
        );
        let path = std::path::Path::new(&dir).join(format!("{}.musicxml", p.name));
        std::fs::write(&path, xml).expect("written");
        println!("{}", path.display());
    }
}
