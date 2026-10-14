//! The system font fallback (round 26, `font_fallback.rs`): names in scripts the bundled
//! fonts lack get the operating system's glyphs; unarmed, nothing changes.

use cantus_ui::design;
use cantus_ui::font_fallback::{self, Script};

fn maps(data: &egui::FontData, c: char) -> bool {
    use skrifa::MetadataProvider;
    skrifa::FontRef::from_index(&data.font, data.index)
        .map(|f| f.charmap().map(c).is_some())
        .unwrap_or(false)
}

fn chain_has(defs: &egui::FontDefinitions, family: &egui::FontFamily, c: char) -> bool {
    defs.families[family]
        .iter()
        .any(|k| maps(&defs.font_data[k], c))
}

#[test]
fn characters_fall_in_the_right_script() {
    assert_eq!(font_fallback::script_of('あ'), Script::Japanese);
    assert_eq!(font_fallback::script_of('日'), Script::Japanese);
    assert_eq!(font_fallback::script_of('한'), Script::Hangul);
    assert_eq!(font_fallback::script_of('ह'), Script::Indic);
    assert_eq!(font_fallback::script_of('ไ'), Script::Thai);
    assert_eq!(font_fallback::script_of('ع'), Script::Letters);
    assert_eq!(font_fallback::script_of('ש'), Script::Letters);
    assert_eq!(font_fallback::script_of('😀'), Script::Emoji);
    assert!(font_fallback::bundled_has('a') && font_fallback::bundled_has('ñ'));
    assert!(!font_fallback::bundled_has('あ') && !font_fallback::bundled_has('한'));
}

/// Unarmed (tests, shots, benchmarks): exactly the bundled fonts, whatever the text.
#[test]
fn unarmed_the_fonts_are_the_bundled_ones_only() {
    assert!(!font_fallback::armed());
    let ctx = egui::Context::default();
    font_fallback::note_text(&ctx, "日本語のトラック");
    font_fallback::poll(&ctx);
    let defs = font_fallback::definitions();
    let bundled = design::font_definitions();
    assert_eq!(
        defs.font_data.keys().collect::<Vec<_>>(),
        bundled.font_data.keys().collect::<Vec<_>>()
    );
    assert_eq!(defs.families, bundled.families);
}

/// With the system's faces after the bundled chain, Japanese, Korean, Hindi, Thai, Arabic
/// and Hebrew get real glyphs: each sample is checked against every face this system has
/// for its script (as the loader tries them in turn), and skipped with a message only where
/// none maps it — a bare Linux runner, or a macOS whose first Latin-script face lacks Arabic.
#[test]
fn the_systems_faces_cover_other_scripts_when_present() {
    let samples = [
        (Script::Japanese, "日本語のトラック"),
        (Script::Hangul, "드럼 클립"),
        (Script::Indic, "हिन्दी"),
        (Script::Thai, "กลอง"),
        (Script::Letters, "عربي"),
        (Script::Letters, "עברית"),
        (Script::Letters, "Ελληνικά Кириллица"),
    ];
    let mut faces: Vec<(font_fallback::Face, &'static [u8])> = Vec::new();
    let mut checked = 0;
    for (script, text) in samples {
        let mut maps: Vec<(font_fallback::Face, std::collections::HashSet<u32>)> = Vec::new();
        for f in font_fallback::faces_for(script) {
            let bytes = std::fs::read(&f.path).unwrap();
            let cmap = font_fallback::cmap_of(&bytes, f.index);
            if !faces.iter().any(|(g, _)| g.key == f.key) {
                faces.push((f.clone(), Box::leak(bytes.into_boxed_slice())));
            }
            maps.push((f, cmap));
        }
        let letters: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        if !letters
            .iter()
            .all(|c| maps.iter().any(|(_, m)| m.contains(&(*c as u32))))
        {
            eprintln!("no system face here maps all of {text:?} ({script:?}): not checked");
            continue;
        }
        checked += 1;
    }
    if checked == 0 {
        eprintln!("no system fonts for these scripts here: skipped");
        return;
    }
    let mut defs = design::font_definitions();
    font_fallback::with_faces(&mut defs, &faces);
    // Each face once, after the bundled ones.
    let chain = defs.families[&egui::FontFamily::Proportional].clone();
    let mut keys = chain.clone();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), chain.len(), "a face twice in the chain");
    assert!(!chain[0].starts_with("system:") && !chain[1].starts_with("system:"));
    // Given the same faces again, nothing is added.
    font_fallback::with_faces(&mut defs, &faces);
    assert_eq!(defs.families[&egui::FontFamily::Proportional], chain);
    for (_, text) in samples {
        for c in text.chars().filter(|c| !c.is_whitespace()) {
            if font_fallback::faces_for(font_fallback::script_of(c)).is_empty() {
                continue;
            }
            let covered_somewhere = faces
                .iter()
                .any(|(f, b)| font_fallback::cmap_of(b, f.index).contains(&(c as u32)));
            if covered_somewhere {
                assert!(
                    chain_has(&defs, &egui::FontFamily::Proportional, c),
                    "{c:?} in {text:?} has no glyph"
                );
            }
        }
    }
}

/// Where a fallback face's glyph for `c` sits against Plex's baseline, in points, at
/// `size`: the face after Plex Sans as [`font_fallback::with_faces`] chains it. `None` for a
/// glyph without ink (nothing to place).
fn baseline_gap(
    face: &font_fallback::Face,
    bytes: &'static [u8],
    c: char,
    size: f32,
) -> Option<f32> {
    use egui::FontFamily::{Monospace, Proportional};
    let plex = design::font_definitions();
    let plex_key = plex.families[&Proportional][0].clone();
    let mut mixed = egui::FontDefinitions::empty();
    mixed
        .font_data
        .insert(plex_key.clone(), plex.font_data[&plex_key].clone());
    mixed.families.insert(Proportional, vec![plex_key.clone()]);
    mixed.families.insert(Monospace, vec![plex_key]);
    font_fallback::with_faces(&mut mixed, &[(face.clone(), bytes)]);

    // The face alone and untweaked: where its glyph's top sits from its own baseline.
    let mut alone = egui::FontDefinitions::empty();
    let mut data = egui::FontData::from_static(bytes);
    data.index = face.index;
    alone
        .font_data
        .insert(face.key.clone(), std::sync::Arc::new(data));
    alone.families.insert(Proportional, vec![face.key.clone()]);
    alone.families.insert(Monospace, vec![face.key.clone()]);

    let glyphs = |defs: egui::FontDefinitions, text: String| {
        let ctx = egui::Context::default();
        ctx.set_fonts(defs);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        let g = ctx.fonts_mut(|f| {
            f.layout_no_wrap(text, egui::FontId::proportional(size), egui::Color32::WHITE)
        });
        g.rows[0].glyphs.clone()
    };
    let own = glyphs(alone, c.to_string());
    if own[0].uv_rect.size.y <= 0.0 {
        return None;
    }
    let both = glyphs(mixed, format!("H{c}"));
    assert_eq!(both[1].chr, c);
    // The glyph's own top from its baseline is the same in both; the rest is the shift.
    let shift = both[1].uv_rect.offset.y - own[0].uv_rect.offset.y;
    Some(both[1].pos.y + shift - both[0].pos.y)
}

/// A system face's glyphs sit on Plex's baseline, whatever the face's ascent, descent and
/// line gap (a Japanese name rode several pixels high in round 26's final review).
#[test]
fn fallback_faces_sit_on_plexs_baseline() {
    // A stand-in with metrics of its own: the icon font, read as if it were a system face.
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/emoji-icon-font.ttf");
    let bytes: &'static [u8] = Box::leak(std::fs::read(&path).unwrap().into_boxed_slice());
    let face = font_fallback::Face {
        path,
        index: 0,
        key: "system:emoji-icon-font.ttf#0".into(),
    };
    let plex = design::font_definitions();
    let sans = &plex.font_data[&plex.families[&egui::FontFamily::Proportional][0]];
    let icons = font_fallback::cmap_of(bytes, 0);
    let plex_has = font_fallback::cmap_of(&sans.font, sans.index);
    let mut only: Vec<char> = icons
        .iter()
        .copied()
        .filter(|u| !plex_has.contains(u))
        .filter_map(char::from_u32)
        .collect();
    only.sort();
    let c = only
        .into_iter()
        .find(|&c| baseline_gap(&face, bytes, c, 24.0).is_some())
        .expect("an icon with ink");
    let mut off = Vec::new();
    for size in [11.0, 13.0, 24.0, 40.0] {
        let gap = baseline_gap(&face, bytes, c, size).unwrap();
        eprintln!("{}: {size} pt, {gap} pt", face.key);
        if gap.abs() > 1.0 {
            off.push(format!("{} at {size} pt: {gap} pt", face.key));
        }
    }
    // The system's own Japanese face, where there is one.
    let mut checked = false;
    for f in font_fallback::faces_for(Script::Japanese) {
        let Ok(b) = std::fs::read(&f.path) else {
            continue;
        };
        if !font_fallback::cmap_of(&b, f.index).contains(&('ド' as u32)) {
            continue;
        }
        let b: &'static [u8] = Box::leak(b.into_boxed_slice());
        for size in [11.0, 13.0, 24.0] {
            let gap = baseline_gap(&f, b, 'ド', size).unwrap();
            eprintln!("{}: {size} pt, {gap} pt", f.key);
            if gap.abs() > 1.0 {
                off.push(format!("{} at {size} pt: {gap} pt", f.key));
            }
        }
        checked = true;
        break;
    }
    if !checked {
        eprintln!("no system Japanese face here: the stand-in only");
    }
    assert!(off.is_empty(), "off Plex's baseline: {off:#?}");
}
