//! What the render is, measured, and the page that presents it (in Spanish, the owner's).

use super::build::Built;
use super::measure::{self, Edge, Level};
use super::score::{Who, SECTIONS};
use cantus_core::note_art::Articulation;
use cantus_core::Project;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// A boundary standing this far over the music around it is a click (dB).
const CLICK_DB: f64 = 15.0;
/// Where an instrument plays its stem is at least this loud (LUFS), and within this many
/// LU of the mix …
const AUDIBLE_LUFS: f64 = -45.0;
const AUDIBLE_LU: f64 = 20.0;
/// … and where it does not, at most this (after two seconds for its ring to end).
const SILENT_DBFS: f64 = -60.0;

pub struct SectionRow {
    pub from: f64,
    pub to: f64,
    /// Per instrument: whether it plays, its techniques, its stem's loudness where it plays
    /// (LUFS) or its level where it does not (dBFS RMS, after its ring).
    pub who: Vec<(Who, bool, Vec<&'static str>, f64)>,
    pub mix_db: f64,
}

pub struct Report {
    pub level: Level,
    pub rows: Vec<SectionRow>,
    pub last_note: f64,
    pub tail: Vec<f64>,
    /// Per instrument: the worst bowed boundary, the worst pizzicato onset, and how many
    /// boundaries were measured.
    pub edges: Vec<(Who, Option<Edge>, Option<Edge>, usize)>,
    /// The same at every instrument's boundaries in the mix (what is heard): the worst
    /// bowed one and the worst pizzicato onset.
    pub mix_edges: (Option<Edge>, Option<Edge>),
    pub worst_spot: (f64, f64),
    pub stems: Vec<String>,
}

fn label(a: Option<Articulation>) -> &'static str {
    match a {
        None | Some(Articulation::Sustain) => "arco",
        Some(Articulation::Legato) => "ligado",
        Some(Articulation::Staccato) => "staccato",
        Some(Articulation::Spiccato) => "spiccato",
        Some(Articulation::Pizzicato) => "pizzicato",
        Some(Articulation::Tremolo) => "trémolo",
        Some(Articulation::Chop) => "chop", // r42 fiddle
    }
}

fn stem_of(stems: &[PathBuf], who: Who) -> Option<&PathBuf> {
    stems.iter().find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(&format!(" {}.wav", who.name())))
    })
}

impl Report {
    pub fn measure(p: &Project, built: &Built, wav: &Path, stems_dir: &Path) -> Report {
        let tm = p.tempo_map();
        let secs = |t: cantus_core::Ticks| tm.seconds_at(t.0 as f64);
        let mix = measure::read_wav(wav);
        let rate = mix.rate;
        let mix_mono = measure::mono(&mix);
        let mut stems: Vec<PathBuf> = std::fs::read_dir(stems_dir)
            .expect("the stems")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "wav"))
            .collect();
        stems.sort();
        let stem_audio: Vec<(Who, Vec<f32>, cantus_export::Audio)> = Who::ALL
            .iter()
            .map(|w| {
                let path = stem_of(&stems, *w).expect("each instrument's stem");
                let a = measure::read_wav(path);
                (*w, measure::mono(&a), a)
            })
            .collect();
        // Every note's start and end, in seconds, by instrument.
        let notes = |who: Who| -> Vec<(f64, f64, Option<Articulation>)> {
            let (track, _) = built.tracks[&who];
            let t = p.track(track).expect("the track");
            t.clips
                .iter()
                .flat_map(|c| {
                    c.notes.iter().map(move |n| {
                        (
                            secs(c.start + n.start),
                            secs(c.start + n.start + n.length),
                            n.art.map(|a| a.articulation),
                        )
                    })
                })
                .collect()
        };
        let mut rows = Vec::new();
        for s in &SECTIONS {
            let (a, b) = (
                super::build::bar_tick(s.bar),
                super::build::bar_tick(s.bar + s.bars),
            );
            let (from, to) = (secs(a), secs(b));
            let mut who = Vec::new();
            for (w, x, a) in &stem_audio {
                let here: Vec<_> = notes(*w)
                    .into_iter()
                    .filter(|(on, _, _)| *on >= from - 1e-6 && *on < to - 1e-6)
                    .collect();
                let plays = !here.is_empty();
                let mut tech: BTreeSet<&'static str> = BTreeSet::new();
                for (_, _, art) in &here {
                    tech.insert(label(*art));
                }
                let level = if plays {
                    measure::lufs(a, from, to)
                } else {
                    measure::rms_db(x, rate, from + 2.0, to)
                };
                who.push((*w, plays, tech.into_iter().collect(), level));
            }
            rows.push(SectionRow {
                from,
                to,
                who,
                mix_db: measure::lufs(&mix, from, to),
            });
        }
        let mut edges = Vec::new();
        let mut all = Vec::new();
        let mut last_note: f64 = 0.0;
        for (w, x, _) in &stem_audio {
            let ns = notes(*w);
            let mut at = Vec::new();
            for (on, off, art) in &ns {
                last_note = last_note.max(*off);
                let pizz = *art == Some(Articulation::Pizzicato);
                at.push((*on, if pizz { "pizz" } else { "on" }));
                at.push((*off, "off"));
            }
            all.extend_from_slice(&at);
            let measured = measure::edges(x, rate, &at);
            let worst = |pizz: bool| {
                measured
                    .iter()
                    .filter(|e| (e.what == "pizz") == pizz)
                    .max_by(|a, b| a.over_db.total_cmp(&b.over_db))
                    .cloned()
            };
            edges.push((*w, worst(false), worst(true), measured.len()));
        }
        let in_mix = measure::edges(&mix_mono, rate, &all);
        let worst = |pizz: bool| {
            in_mix
                .iter()
                .filter(|e| (e.what == "pizz") == pizz)
                .max_by(|a, b| a.over_db.total_cmp(&b.over_db))
                .cloned()
        };
        Report {
            mix_edges: (worst(false), worst(true)),
            level: measure::level(&mix),
            rows,
            last_note,
            tail: measure::tail(&mix_mono, rate, last_note),
            edges,
            worst_spot: measure::worst_spot(&mix_mono, rate),
            stems: stems
                .iter()
                .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
                .collect(),
        }
    }

    /// Whether the tail rings down to the end: no quarter second louder than the one
    /// before by more than 1 dB, and the last under −60 dBFS.
    pub fn tail_ok(&self) -> bool {
        self.tail.windows(2).all(|w| w[1] <= w[0] + 1.0)
            && self.tail.last().is_some_and(|l| *l < -60.0)
    }

    pub fn passes(&self) -> bool {
        let l = &self.level;
        let loud = l
            .lufs
            .is_some_and(|v| (v - f64::from(super::LUFS)).abs() <= 0.5)
            && l.dbtp <= super::CEILING + 0.05
            && l.full_scale == 0;
        let heard = self.rows.iter().all(|r| {
            r.who.iter().all(|(_, plays, _, db)| {
                if *plays {
                    *db >= AUDIBLE_LUFS && *db >= r.mix_db - AUDIBLE_LU
                } else {
                    *db <= SILENT_DBFS
                }
            })
        });
        let clean = [&self.mix_edges.0, &self.mix_edges.1]
            .iter()
            .all(|e| e.as_ref().is_none_or(|e| e.over_db < CLICK_DB));
        loud && heard && clean && self.tail_ok()
    }

    pub fn print(&self) {
        let l = &self.level;
        println!(
            "the mix: {:.1} s, {:.2} LUFS, {:.2} dBTP, sample peak {:.2} dBFS, {} samples at full scale",
            l.seconds,
            l.lufs.unwrap_or(f64::NAN),
            l.dbtp,
            l.peak_db,
            l.full_scale
        );
        for (s, r) in SECTIONS.iter().zip(&self.rows) {
            println!(
                "{} ({:.1}–{:.1} s), mix {:.1} LUFS",
                s.name, r.from, r.to, r.mix_db
            );
            for (w, plays, tech, db) in &r.who {
                if *plays {
                    println!("    {:<12} {:>6.1} LUFS  {}", w.name(), db, tech.join(", "));
                } else {
                    println!("    {:<12} {:>6.1} dBFS  (silent)", w.name(), db);
                }
            }
        }
        for (w, bowed, pizz, n) in &self.edges {
            let f = |e: &Option<Edge>| {
                e.as_ref().map_or("—".to_string(), |e| {
                    format!("{:+.1} dB at {:.2} s", e.over_db, e.at)
                })
            };
            println!(
                "edges {:<12} {n} measured; worst bowed {}; worst pizzicato {}",
                w.name(),
                f(bowed),
                f(pizz)
            );
        }
        let f = |e: &Option<Edge>| {
            e.as_ref().map_or("—".to_string(), |e| {
                format!("{:+.1} dB at {:.2} s", e.over_db, e.at)
            })
        };
        println!(
            "edges in the mix: worst bowed {}; worst pizzicato {}",
            f(&self.mix_edges.0),
            f(&self.mix_edges.1)
        );
        println!(
            "the mix's sharpest 1 ms anywhere: {:+.1} dB over its surroundings at {:.2} s",
            self.worst_spot.0, self.worst_spot.1
        );
        let t: Vec<String> = self.tail.iter().map(|d| format!("{d:.0}")).collect();
        println!(
            "the tail from the last note's end ({:.2} s), each 0.25 s: {} dBFS — {}",
            self.last_note,
            t.join(" "),
            if self.tail_ok() { "rings out" } else { "CUT" }
        );
    }
}

fn mmss(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// `index.html` beside the files.
pub fn write(out: &Path, r: &Report) -> std::io::Result<()> {
    let mut h = String::new();
    h.push_str(include_str!("page_head.html"));
    let _ = writeln!(
        h,
        "<p class=\"meta\">{} · 48 kHz, 24 bits · {:.1} LUFS · pico {:.1} dBTP</p>",
        mmss(r.level.seconds),
        r.level.lufs.unwrap_or(f64::NAN),
        r.level.dbtp
    );
    h.push_str(
        "<audio id=\"song\" controls preload=\"metadata\" src=\"arcus-cuarteto.wav\"></audio>\n",
    );
    h.push_str("<h2>La forma</h2>\n<ol class=\"form\">\n");
    for (s, row) in SECTIONS.iter().zip(&r.rows) {
        let _ = write!(
            h,
            "<li><button data-t=\"{:.2}\">{}–{}</button><div><h3>{}</h3><p>{}</p><p class=\"who\">",
            row.from,
            mmss(row.from),
            mmss(row.to),
            s.name,
            s.what
        );
        let playing: Vec<String> = row
            .who
            .iter()
            .filter(|(_, plays, _, _)| *plays)
            .map(|(w, _, tech, _)| format!("<b>{}</b> {}", w.name(), tech.join(", ")))
            .collect();
        h.push_str(&playing.join(" · "));
        h.push_str("</p></div></li>\n");
    }
    h.push_str("</ol>\n<h2>Las pistas por separado</h2>\n<ul class=\"stems\">\n");
    for s in &r.stems {
        let name = s.trim_end_matches(".wav");
        let src = s.replace(' ', "%20");
        let _ = writeln!(
            h,
            "<li><span>{name}</span><audio controls preload=\"none\" src=\"stems/{src}\"></audio></li>"
        );
    }
    h.push_str("</ul>\n");
    h.push_str(include_str!("page_tail.html"));
    std::fs::write(out.join("index.html"), h)
}
