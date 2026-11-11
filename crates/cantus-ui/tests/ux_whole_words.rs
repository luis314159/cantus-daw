//! Nothing truncated (round 39, ADR 0109): every view a user reaches, at 100 % and 150 %
//! of a 1920 × 1080 screen, in both themes and both languages (Spanish is longer), with
//! the kit's text probe on — and any kit text elided with "…" or not painted at all for
//! want of room fails, as does any text egui cut short. The app `CantusApp::assemble` puts
//! together on the demo it opens with; the pointer off the window.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

#[path = "ux_sweep/mod.rs"]
mod sweep;

use cantus_ui::kit::text::probe;
use sweep::{Look, Rig, View};

/// A text that was not painted whole: where, in which look, what it says, what showed.
#[derive(Debug, Clone)]
struct Cut {
    look: Look,
    view: String,
    full: String,
    shown: String,
    how: &'static str,
    width: f32,
    at: egui::Pos2,
}

/// The cuts of `view` in the rig's look.
fn cuts_of(r: &mut Rig, look: Look, view: &View) -> Vec<Cut> {
    r.reset();
    sweep::show(r, view);
    r.idle(2);
    probe::start();
    let texts = r.frame(None);
    // The project's own names (tracks, strips, clips) are the user's words: a name too long
    // for its place is cut in its middle by design (ADR 0024 §3, FL's way) and is not an
    // interface word cut (counted apart, below).
    let p = r.app.project();
    let mut names: Vec<String> = p.tracks.iter().map(|t| t.name.clone()).collect();
    names.extend(p.strips.iter().map(|s| s.name.clone()));
    names.extend(
        p.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(|c| c.name.clone())),
    );
    names.retain(|n| n.chars().count() >= 3);
    let ours = |full: &str| !names.iter().any(|n| full.contains(n.as_str()));
    let mut out: Vec<Cut> = probe::take()
        .into_iter()
        .filter(|c| ours(&c.full))
        .map(|c| Cut {
            look,
            view: format!("{view:?}"),
            full: c.full,
            shown: c.shown,
            how: c.how,
            width: c.rect.width(),
            at: c.rect.min,
        })
        .collect();
    out.extend(
        texts
            .into_iter()
            .filter(|p| p.elided && ours(&p.text))
            .map(|p| Cut {
                look,
                view: format!("{view:?}"),
                full: p.text.clone(),
                shown: p.text,
                how: "egui",
                width: p.rect.width(),
                at: p.rect.min,
            }),
    );
    out
}

#[test]
fn no_word_is_cut_in_any_view_scale_theme_or_language() {
    let mut cuts = Vec::new();
    let looks = match std::env::var("CANTUS_UX_LOOKS").as_deref() {
        Ok("plain") => vec![Look::PLAIN],
        _ => Look::all(),
    };
    for look in looks {
        let mut r = Rig::new(look);
        for view in sweep::views() {
            let c = cuts_of(&mut r, look, &view);
            if !c.is_empty() {
                eprintln!("{look:?} {view:?}: {} cut", c.len());
            }
            cuts.extend(c);
        }
    }
    // One line a distinct text, with every place it was cut.
    let mut distinct: Vec<(String, Vec<String>)> = Vec::new();
    for c in &cuts {
        let place = format!(
            "{:?}/{}/{}% {} [{} → {:?}, {:.0} pt at ({:.0}, {:.0})]",
            c.look.locale,
            if c.look.dark { "night" } else { "day" },
            (c.look.scale * 100.0).round(),
            c.view,
            c.how,
            c.shown,
            c.width,
            c.at.x,
            c.at.y
        );
        match distinct.iter_mut().find(|(t, _)| *t == c.full) {
            Some((_, places)) => places.push(place),
            None => distinct.push((c.full.clone(), vec![place])),
        }
    }
    for (text, places) in &distinct {
        eprintln!("CUT {text:?} ({} places): {}", places.len(), places[0]);
        for p in places.iter().skip(1).take(3) {
            eprintln!("      {p}");
        }
    }
    assert!(
        cuts.is_empty(),
        "{} cuts of {} distinct texts (listed above)",
        cuts.len(),
        distinct.len()
    );
}

/// Every view photographed in one look into `CANTUS_UX_SHOTS`, to look at the cuts where
/// they are: `CANTUS_UX_LOOK=es-day-150` (language, theme, scale) and `-- --ignored shoot`.
#[test]
#[ignore]
fn shoot_every_view_in_a_look() {
    let dir = std::path::PathBuf::from(
        std::env::var("CANTUS_UX_SHOTS").unwrap_or_else(|_| "ux-shots".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let spec = std::env::var("CANTUS_UX_LOOK").unwrap_or_else(|_| "en-night-100".into());
    let parts: Vec<&str> = spec.split('-').collect();
    let look = Look {
        locale: if parts.first() == Some(&"es") {
            cantus_ui::i18n::Locale::Es
        } else {
            cantus_ui::i18n::Locale::En
        },
        dark: parts.get(1) != Some(&"day"),
        scale: parts
            .get(2)
            .and_then(|s| s.parse::<f32>().ok())
            .map_or(1.0, |p| p / 100.0),
    };
    let only = std::env::var("CANTUS_UX_VIEWS").unwrap_or_default();
    let mut r = Rig::new(look);
    for (i, view) in sweep::views().iter().enumerate() {
        let name = format!("{spec}-{i:02}-{view:?}")
            .replace(['(', ')', '"', ' '], "")
            .to_lowercase();
        if !only.is_empty() && !only.split(',').any(|v| name.contains(v)) {
            continue;
        }
        r.reset();
        sweep::show(&mut r, view);
        sweep::shoot(&mut r, &dir.join(format!("{name}.png")), None);
    }
}
