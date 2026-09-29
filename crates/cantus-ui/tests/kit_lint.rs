//! Lints over the source that keep the look (ADR 0024 §3, §14).

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Files that still name a font by size, written before the type roles existed.
///
/// TODO(ADR 0024 Phase B): move each onto `design::role::Role` and delete its line here;
/// the list may only shrink.
const ALLOWED: [&str; 0] = [];

#[test]
fn fonts_are_named_by_role_only_in_design_rs() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut offenders = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&src)
            .unwrap_or(f)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "design.rs" || ALLOWED.contains(&rel.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            if line.contains("FontId::new") && !line.trim_start().starts_with("//") {
                offenders.push(format!("src/{rel}:{}", n + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "text must ask for a role (design::role::Role), not a size: {offenders:#?}"
    );
}

#[test]
fn the_allow_list_only_names_files_that_still_need_it() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    for rel in ALLOWED {
        let text = std::fs::read_to_string(src.join(rel)).unwrap_or_default();
        assert!(
            text.contains("FontId::new"),
            "{rel} no longer names a font by size: remove it from the allow-list"
        );
    }
}

#[test]
fn the_kit_paints_no_stock_egui_widget() {
    // The kit is the look: a painter that fell back to an egui button would bring the
    // stock look back through the side door.
    let kit = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("kit");
    let mut files = Vec::new();
    rust_files(&kit, &mut files);
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        for bad in [
            "egui::Button",
            "ui.button(",
            "selectable_label",
            "egui::Slider",
            "ui.label(",
        ] {
            assert!(!text.contains(bad), "{} uses {bad}", f.display());
        }
    }
}

#[test]
fn painters_paint_only_with_tokens() {
    // Every colour the kit paints resolves from a named token (so a theme can change it
    // and the eyedropper can name it). Only the files that *are* the colour machinery
    // may spell a colour out.
    const MACHINERY: [&str; 6] = [
        "tokens.rs",
        "color.rs",
        "brush.rs",
        "cache.rs",
        "logo.rs",
        "appearance.rs",
    ];
    const LITERALS: [&str; 12] = [
        "Color32::from_rgb",
        "Color32::from_rgba",
        "Color32::from_gray",
        "Color32::from_white_alpha",
        "Color32::from_black_alpha",
        "Color32::WHITE",
        "Color32::BLACK",
        "Color32::GRAY",
        "Color32::RED",
        "Color32::YELLOW",
        "Color32::GREEN",
        "hex(0x",
    ];
    let kit = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("kit");
    let mut files = Vec::new();
    rust_files(&kit, &mut files);
    let mut found = Vec::new();
    for f in files {
        let name = f
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if MACHINERY.contains(&name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for lit in LITERALS {
                if line.contains(lit) {
                    found.push(format!("{name}:{}: {lit}", n + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "a painter spells out a colour instead of naming a token: {found:#?}"
    );
}
