//! MusicXML in: a partwise file's notes, as a part's notes on the timeline — with its
//! tempo, metres and key — so a score written elsewhere comes in as a clip.
//!
//! Read: pitches (with each part's transposition back to sounding), durations through
//! `<divisions>`, chords, voices and staves through `<backup>`/`<forward>`, ties joined
//! into one note, velocity from the note's `dynamics` attribute or the last dynamic
//! marking, tempo from `<sound tempo>`, metres and the key. Grace and cue notes are left
//! out (they take no time of their own).

use cantus_core::time::{TimeSignature, PPQ};

/// A note read from MusicXML: start and length in Cantus ticks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InNote {
    pub start: i64,
    pub len: i64,
    pub pitch: u8,
    pub velocity: u8,
}

/// A part read from MusicXML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InPart {
    pub name: String,
    pub notes: Vec<InNote>,
}

/// A whole file read.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub title: String,
    pub composer: String,
    pub parts: Vec<InPart>,
    /// `(tick, quarter notes per minute)`.
    pub tempo: Vec<(i64, f64)>,
    pub metres: Vec<(i64, TimeSignature)>,
    /// Fifths (the concert key's) and mode, from the first part's first key.
    pub key: Option<(i8, crate::model::KeyMode)>,
}

/// Why a file could not be read.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImportError {
    #[error("not XML: {0}")]
    Xml(String),
    #[error("a timewise MusicXML file (only partwise files are read)")]
    Timewise,
    #[error("not a MusicXML score")]
    NotMusicXml,
    #[error("a compressed .mxl file: unpack it first")]
    Compressed,
}

/// The velocity a dynamic marking reads as (MuseScore's values).
fn dynamic_velocity(name: &str) -> Option<u8> {
    Some(match name {
        "pppp" => 10,
        "ppp" => 16,
        "pp" => 33,
        "p" => 49,
        "mp" => 64,
        "mf" => 80,
        "f" => 96,
        "ff" => 112,
        "fff" | "ffff" => 127,
        "sf" | "sfz" | "fz" => 112,
        _ => return None,
    })
}

fn child<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(name))
}

fn text<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    child(n, name).and_then(|c| c.text()).map(str::trim)
}

fn num<T: std::str::FromStr>(n: roxmltree::Node, name: &str) -> Option<T> {
    text(n, name).and_then(|t| t.parse().ok())
}

/// Read a MusicXML document (the text of a `.musicxml` / `.xml` file).
pub fn read(xml: &str) -> Result<Imported, ImportError> {
    if xml.starts_with("PK") {
        return Err(ImportError::Compressed);
    }
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(xml, opts)
        .map_err(|e| ImportError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.has_tag_name("score-timewise") {
        return Err(ImportError::Timewise);
    }
    if !root.has_tag_name("score-partwise") {
        return Err(ImportError::NotMusicXml);
    }
    let title = child(root, "work")
        .and_then(|w| text(w, "work-title"))
        .or_else(|| text(root, "movement-title"))
        .unwrap_or("")
        .to_string();
    let composer = child(root, "identification")
        .and_then(|i| {
            i.children()
                .find(|c| c.has_tag_name("creator") && c.attribute("type") == Some("composer"))
        })
        .and_then(|c| c.text())
        .unwrap_or("")
        .trim()
        .to_string();
    let names: Vec<(String, String)> = child(root, "part-list")
        .map(|pl| {
            pl.children()
                .filter(|c| c.has_tag_name("score-part"))
                .map(|sp| {
                    (
                        sp.attribute("id").unwrap_or("").to_string(),
                        text(sp, "part-name").unwrap_or("").to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let mut out = Imported {
        title,
        composer,
        parts: Vec::new(),
        tempo: Vec::new(),
        metres: Vec::new(),
        key: None,
    };
    for (pi, part) in root
        .children()
        .filter(|c| c.has_tag_name("part"))
        .enumerate()
    {
        let id = part.attribute("id").unwrap_or("");
        let name = names
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| format!("Part {}", pi + 1));
        let notes = read_part(part, pi == 0, &mut out);
        out.parts.push(InPart { name, notes });
    }
    out.tempo.sort_by_key(|t| t.0);
    out.tempo.dedup_by_key(|t| t.0);
    out.metres.sort_by_key(|m| m.0);
    out.metres.dedup_by_key(|m| m.0);
    Ok(out)
}

fn read_part(part: roxmltree::Node, first: bool, out: &mut Imported) -> Vec<InNote> {
    let mut notes: Vec<InNote> = Vec::new();
    let mut divisions: i64 = 1;
    let mut chromatic: i32 = 0;
    let mut measure_start: i64 = 0;
    let mut velocity: u8 = 80;
    // Open ties by pitch: the note they continue.
    let mut open: Vec<(u8, usize)> = Vec::new();
    let mut last_metre: Option<TimeSignature> = None;
    for m in part.children().filter(|c| c.has_tag_name("measure")) {
        let mut cursor: i64 = 0;
        let mut longest: i64 = 0;
        let mut last_start: i64 = 0;
        let ticks = |div: i64, d: i64| d * PPQ / div.max(1);
        for el in m.children().filter(|c| c.is_element()) {
            match el.tag_name().name() {
                "attributes" => {
                    if let Some(d) = num::<i64>(el, "divisions") {
                        divisions = d.max(1);
                    }
                    if let Some(k) = child(el, "key") {
                        if first && out.key.is_none() {
                            let f = num::<i8>(k, "fifths").unwrap_or(0);
                            let mode = crate::model::KeyMode::from_musicxml(
                                text(k, "mode").unwrap_or("major"),
                            );
                            out.key = Some((f, mode));
                        }
                    }
                    if let Some(t) = child(el, "time") {
                        if let (Some(b), Some(bt)) =
                            (num::<u8>(t, "beats"), num::<u8>(t, "beat-type"))
                        {
                            let sig = TimeSignature::new(b, bt);
                            if first && last_metre != Some(sig) {
                                out.metres.push((measure_start + cursor, sig));
                            }
                            last_metre = Some(sig);
                        }
                    }
                    if let Some(t) = child(el, "transpose") {
                        chromatic = num::<i32>(t, "chromatic").unwrap_or(0)
                            + 12 * num::<i32>(t, "octave-change").unwrap_or(0);
                    }
                }
                "direction" => {
                    if let Some(s) = child(el, "sound") {
                        if let Some(t) = s.attribute("tempo").and_then(|t| t.parse::<f64>().ok()) {
                            if first && t > 0.0 {
                                out.tempo.push((measure_start + cursor, t));
                            }
                        }
                    }
                    for dt in el.children().filter(|c| c.has_tag_name("direction-type")) {
                        if let Some(dy) = child(dt, "dynamics") {
                            if let Some(v) = dy
                                .children()
                                .filter(|c| c.is_element())
                                .find_map(|c| dynamic_velocity(c.tag_name().name()))
                            {
                                velocity = v;
                            }
                        }
                    }
                }
                "sound" => {
                    if let Some(t) = el.attribute("tempo").and_then(|t| t.parse::<f64>().ok()) {
                        if first && t > 0.0 {
                            out.tempo.push((measure_start + cursor, t));
                        }
                    }
                }
                "backup" => {
                    cursor -= ticks(divisions, num::<i64>(el, "duration").unwrap_or(0));
                    cursor = cursor.max(0);
                }
                "forward" => {
                    cursor += ticks(divisions, num::<i64>(el, "duration").unwrap_or(0));
                    longest = longest.max(cursor);
                }
                "note" => {
                    if child(el, "grace").is_some() || child(el, "cue").is_some() {
                        continue;
                    }
                    let dur = ticks(divisions, num::<i64>(el, "duration").unwrap_or(0));
                    let is_chord = child(el, "chord").is_some();
                    let start = if is_chord { last_start } else { cursor };
                    if !is_chord {
                        last_start = cursor;
                        cursor += dur;
                        longest = longest.max(cursor);
                    }
                    let Some(p) = child(el, "pitch") else {
                        continue;
                    };
                    let step = text(p, "step").unwrap_or("C");
                    let alter: f64 = text(p, "alter").and_then(|a| a.parse().ok()).unwrap_or(0.0);
                    let octave: i32 = num(p, "octave").unwrap_or(4);
                    let pc = match step {
                        "C" => 0,
                        "D" => 2,
                        "E" => 4,
                        "F" => 5,
                        "G" => 7,
                        "A" => 9,
                        "B" => 11,
                        _ => 0,
                    };
                    let midi = (octave + 1) * 12 + pc + alter.round() as i32 + chromatic;
                    let pitch = midi.clamp(0, 127) as u8;
                    let vel = el
                        .attribute("dynamics")
                        .and_then(|d| d.parse::<f64>().ok())
                        .map(|d| (d * 0.9).round().clamp(1.0, 127.0) as u8)
                        .unwrap_or(velocity);
                    let ties: Vec<&str> = el
                        .children()
                        .filter(|c| c.has_tag_name("tie"))
                        .filter_map(|c| c.attribute("type"))
                        .collect();
                    let abs = measure_start + start;
                    let stop = ties.contains(&"stop");
                    let start_tie = ties.contains(&"start");
                    let joined = if stop {
                        open.iter()
                            .position(|(op, idx)| {
                                *op == pitch && notes[*idx].start + notes[*idx].len == abs
                            })
                            .map(|k| open.remove(k).1)
                    } else {
                        None
                    };
                    let idx = match joined {
                        Some(i) => {
                            notes[i].len += dur;
                            i
                        }
                        None => {
                            notes.push(InNote {
                                start: abs,
                                len: dur,
                                pitch,
                                velocity: vel,
                            });
                            notes.len() - 1
                        }
                    };
                    if start_tie {
                        open.push((pitch, idx));
                    }
                }
                _ => {}
            }
        }
        // A measure is as long as what it holds (an implicit pickup bar is shorter).
        measure_start += longest;
    }
    // A transposing part's key is written transposed: the song's key is its concert key.
    if first && chromatic != 0 {
        if let Some((f, mode)) = out.key {
            let concert = (f as i32 + 7 * chromatic).rem_euclid(12);
            let concert = if concert > 6 { concert - 12 } else { concert };
            out.key = Some((concert as i8, mode));
        }
    }
    notes.retain(|n| n.len > 0);
    notes.sort_by_key(|n| (n.start, n.pitch));
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_what_it_cannot_read() {
        assert_eq!(read("PK\u{3}\u{4}"), Err(ImportError::Compressed));
        assert_eq!(
            read("<score-timewise version=\"4.0\"/>").unwrap_err(),
            ImportError::Timewise
        );
        assert!(matches!(read("<html/>"), Err(ImportError::NotMusicXml)));
        assert!(matches!(read("not xml <"), Err(ImportError::Xml(_))));
    }
}
