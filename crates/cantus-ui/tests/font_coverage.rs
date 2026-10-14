//! Every character Cantus can put on screen has a glyph in the fonts it installs
//! (`design::install_fonts`): each text face's chain — IBM Plex (SIL OFL 1.1) and the
//! emoji-icon-font fallback (MIT) — covers every character of every locale string (both
//! languages, the instruments' and effects' areas, the verse files) and every non-ASCII
//! character or `\u{…}` escape written in the interface's source. Round 26: egui's other
//! default fonts (Ubuntu Light under the Ubuntu Font Licence, Hack under the Bitstream Vera
//! licence, Noto Emoji) were never in a chain, and they are no longer built in; this test
//! is what proves nothing depended on them.

//! Drawn text only: locale lines that are not `#` comments, the content manifest's strings,
//! and in the source of the crates whose text the interface shows (`cantus-ui`,
//! `cantus-plugins`, `cantus-core`, `cantus-content`, `cantus-host`, `cantus-export`) the
//! string literals and the char literals that are not
//! match patterns — a pattern (`Some('♯') | … =>`) is a parsing table, input never shown.

use cantus_ui::design::{self, role::Role};
use egui::FontFamily;
use std::collections::BTreeMap;
use std::path::Path;

fn files(dir: &Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

/// The character a `\u{…}` escape stands for, and the bytes it spans after the backslash.
fn escape_at(s: &[char], i: usize) -> Option<(char, usize)> {
    // s[i] == '\\'
    match s.get(i + 1)? {
        'u' if s.get(i + 2) == Some(&'{') => {
            let end = (i + 3..s.len()).find(|&j| s[j] == '}')?;
            let hex: String = s[i + 3..end].iter().collect();
            let c = u32::from_str_radix(&hex, 16)
                .ok()
                .and_then(char::from_u32)?;
            Some((c, end + 1 - i))
        }
        'n' | 't' | 'r' | '0' => Some(('\n', 2)),
        &c => Some((c, 2)),
    }
}

/// What a Rust source can draw: the characters of its string literals and of the char
/// literals that are not match patterns. Comments, attribute arguments (`reason = "…"`),
/// char literals in a pattern (`Some('♯') | … =>`, a parsing table: input, never output)
/// and range bounds (`'\u{300}'..='\u{36f}'`, a filter's comparison) are left out. `where` gets the first place each character was seen.
fn drawable_in_source(text: &str, file: &str, out: &mut BTreeMap<char, String>) {
    let mut in_str = false;
    let lines: Vec<&str> = text.lines().collect();
    for (n, line) in lines.iter().enumerate() {
        if !in_str {
            let t = line.trim_start();
            // A file's unit tests (`#[cfg(test)] mod …`, at its end by the house style)
            // hold test data and assertion messages, never drawn.
            if t.starts_with("#[cfg(test)]")
                && lines[n + 1..]
                    .iter()
                    .map(|l| l.trim_start())
                    .find(|l| !l.is_empty())
                    .is_some_and(|l| l.starts_with("mod ") || l.starts_with("pub mod "))
            {
                break;
            }
            if t.starts_with("//") || t.starts_with("#[") || t.starts_with("reason =") {
                continue;
            }
        }
        let s: Vec<char> = line.chars().collect();
        let mut i = 0;
        let put = |c: char, out: &mut BTreeMap<char, String>| {
            out.entry(c).or_insert_with(|| format!("{file}:{}", n + 1));
        };
        while i < s.len() {
            let c = s[i];
            if in_str {
                match c {
                    '\\' => {
                        let (e, len) = escape_at(&s, i).unwrap_or(('\\', 1));
                        put(e, out);
                        i += len;
                        continue;
                    }
                    '"' => in_str = false,
                    _ => put(c, out),
                }
                i += 1;
                continue;
            }
            match c {
                '/' if s.get(i + 1) == Some(&'/') => break,
                '"' => in_str = true,
                '\'' => {
                    // A char literal: 'x' or '\u{…}' / '\n'; anything else is a lifetime.
                    let lit = if s.get(i + 1) == Some(&'\\') {
                        escape_at(&s, i + 1)
                            .filter(|&(_, len)| s.get(i + 1 + len) == Some(&'\''))
                            .map(|(e, len)| (e, i + 1 + len + 1))
                    } else if s.get(i + 2) == Some(&'\'') {
                        s.get(i + 1).map(|&e| (e, i + 3))
                    } else {
                        None
                    };
                    if let Some((e, next)) = lit {
                        let rest: String = s[next..].iter().collect();
                        let rest = rest.trim_start().trim_start_matches(')').trim_start();
                        let pattern = rest.starts_with('|') || rest.starts_with("=>");
                        // A range's bound (`'a'..='z'`, `..'z'`): compared, never drawn.
                        let before: String = s[..i].iter().collect();
                        let bound = rest.starts_with("..")
                            || before.trim_end().ends_with("..")
                            || before.trim_end().ends_with("..=");
                        if !pattern && !bound {
                            put(e, out);
                        }
                        i = next;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

fn used_characters() -> BTreeMap<char, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut chars = BTreeMap::new();
    let mut texts = Vec::new();
    files(&root.join("locales"), "txt", &mut texts);
    assert!(
        texts.len() > 60,
        "the locale files were found: {}",
        texts.len()
    );
    for f in &texts {
        let name = f.strip_prefix(root).unwrap_or(f).display().to_string();
        for (n, line) in std::fs::read_to_string(f).unwrap().lines().enumerate() {
            // `#` lines are the files' comments; everything else is shown.
            if line.trim_start().starts_with('#') {
                continue;
            }
            for c in line.chars() {
                chars
                    .entry(c)
                    .or_insert_with(|| format!("{name}:{}", n + 1));
            }
        }
    }
    let mut sources = Vec::new();
    files(&root.join("src"), "rs", &mut sources);
    files(&root.join("../cantus-plugins/src"), "rs", &mut sources);
    // Crates whose strings the interface draws as they are: parameter names and units
    // (`cantus-core`'s descriptors: °, ×…), the content manager's messages, the plug-in
    // host's scan reports, the export's summaries (round 26, the second review).
    for other in [
        "cantus-core",
        "cantus-content",
        "cantus-host",
        "cantus-export",
    ] {
        files(&root.join("..").join(other).join("src"), "rs", &mut sources);
    }
    for f in &sources {
        let name = f.strip_prefix(root).unwrap_or(f).display().to_string();
        drawable_in_source(&std::fs::read_to_string(f).unwrap(), &name, &mut chars);
    }
    // The sound packs' names, descriptions and credits come from the manifest.
    let manifest = root.join("../cantus-content/manifest/content.json");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
            serde_json::Value::Object(o) => o.values().for_each(|x| strings(x, out)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    strings(&json, &mut texts);
    assert!(texts.len() > 50, "the manifest's strings were found");
    for t in texts {
        for c in t.chars() {
            chars
                .entry(c)
                .or_insert_with(|| "cantus-content/manifest/content.json".to_owned());
        }
    }
    chars.retain(|c, _| !c.is_control() && *c != '\u{feff}');
    chars
}

/// Whether `face` (a font file and its index) maps `c` to a glyph.
fn maps(data: &egui::FontData, c: char) -> bool {
    use skrifa::MetadataProvider;
    skrifa::FontRef::from_index(&data.font, data.index)
        .map(|f| f.charmap().map(c).is_some())
        .unwrap_or(false)
}

#[test]
fn every_character_the_interface_shows_has_a_glyph() {
    let chars = used_characters();
    assert!(
        chars.contains_key(&'ñ') && chars.contains_key(&'…'),
        "the scan found text"
    );
    let defs = design::font_definitions();
    let mut families = vec![
        FontFamily::Proportional,
        FontFamily::Monospace,
        design::family_strong(),
        design::family_mono_strong(),
    ];
    families.extend(Role::ALL.iter().map(|r| r.family()));
    let mut missing = Vec::new();
    for family in &families {
        let chain = &defs.families[family];
        assert!(!chain.is_empty(), "{family:?} has no font");
        for (&c, at) in &chars {
            // Spaces are laid out by advance, not drawn; egui handles them without a glyph.
            if c.is_whitespace() {
                continue;
            }
            if !chain.iter().any(|k| maps(&defs.font_data[k], c)) {
                missing.push(format!("{family:?}: {c:?} (U+{:04X}) at {at}", c as u32));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "{} characters without a glyph (a replacement box on screen):\n{}",
        missing.len(),
        missing.join("\n")
    );
}

/// The scanner reads drawn text, not parsing tables, comments or attributes.
#[test]
fn the_scan_takes_strings_and_shown_chars_but_not_patterns_comments_or_attributes() {
    let src = r#"
// a comment with ▾
#[expect(dead_code, reason = "▼ chips")]
    reason = "▼ chips"
fn f(c: Option<char>) -> &'static str {
    match c {
        Some('♯') | Some('#') => "sharp",
        Some('♭') => "flat \u{3c4}",
        _ => "x",
    }
}
fn g(s: &mut String) { s.push('•'); s.push_str("Ü\u{2026}"); } // trailing ●
fn h(s: &str) -> String { s.chars().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).collect() }
fn k(c: char) -> bool { matches!(c, 'α'..'ω' | '\u{2190}'..) || ('Ā'..='ž').contains(&c) }
#[cfg(test)]
mod tests {
    fn data() -> &'static str { "日本 ⇝" }
}
"#;
    let mut got = BTreeMap::new();
    drawable_in_source(src, "t.rs", &mut got);
    let chars: String = got.keys().filter(|c| !c.is_ascii()).collect();
    assert_eq!(chars, "Üτ•…");
    assert!(got.contains_key(&'s') && !got.contains_key(&'♯'));
}

/// Only fonts whose licences are OSI-approved are built in (SignPath's condition,
/// `docs/signing/README.md` §2): IBM Plex (OFL-1.1) and emoji-icon-font (MIT). egui's own
/// defaults — Ubuntu Light (Ubuntu Font Licence) and Hack (Bitstream Vera licence) are not
/// OSI-approved — are compiled out (`default_fonts` off for egui and eframe).
#[test]
fn only_plex_and_the_emoji_fallback_are_built_in() {
    assert_eq!(
        egui::FontDefinitions::builtin_font_names(),
        &[] as &[&str],
        "egui's default fonts are compiled in (a dependency turned `default_fonts` on)"
    );
    let defs = design::font_definitions();
    for key in defs.font_data.keys() {
        assert!(
            key == "emoji-icon-font" || !key.contains("Ubuntu") && !key.contains("Hack"),
            "{key} is built in"
        );
    }
    assert!(defs.font_data.contains_key("emoji-icon-font"));
    // Every chain is one Plex face, then the emoji fallback: the same chains as before the
    // defaults were compiled out, so the same glyph is chosen for every character.
    for (family, chain) in &defs.families {
        assert_eq!(chain.len(), 2, "{family:?}: {chain:?}");
        assert_eq!(chain[1], "emoji-icon-font", "{family:?}: {chain:?}");
    }
}
