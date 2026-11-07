//! The preset counts the README's unit grid and the website's unit cards state are the
//! counts the units have (round 37's fixes, item 12: Exemplar was said to have 26 when
//! Sectio's six slicing styles made it 32). Both are kept by hand — `web/src/units_data.rs`
//! says so — so this test is what keeps them true: change a unit's presets, and it names
//! every line to change.

use cantus_ui::preset_catalog::{entries, units, Unit};

/// A unit's card name (`web/src/units_data.rs`'s `file`, the README's `#unit-…` anchor).
fn card(u: Unit) -> String {
    use cantus_ai::tools_project::args;
    match u {
        Unit::Instrument(k) => args::native_id(k).to_string(),
        Unit::Effect(k) => args::effect_names(k).0.to_string(),
        Unit::Kit => "pulsus".to_string(),
    }
}

fn actual() -> Vec<(String, usize)> {
    units()
        .into_iter()
        .filter(|u| !matches!(u, Unit::Effect(k) if !k.offered()))
        .map(|u| (card(u), entries(u, false).len()))
        .collect()
}

fn root(path: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `(card, count)` for every "N factory presets — [hear them](…#unit-card)" in the README.
fn readme() -> Vec<(String, usize)> {
    let text = root("README.md");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(at) = line.find(" factory presets — [hear them](https://cantusdaw.com/#unit-")
        else {
            continue;
        };
        let n: usize = line[..at]
            .rsplit(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|d| d.parse().ok())
            .expect("a count before the words");
        let rest = &line[at..];
        let anchor = rest.split("#unit-").nth(1).expect("an anchor");
        let name: String = anchor.chars().take_while(|c| *c != ')').collect();
        out.push((name, n));
    }
    out
}

/// `(card, count)` for every unit card on the website that states a count.
fn site() -> Vec<(String, usize)> {
    let text = root("web/src/units_data.rs");
    let mut out = Vec::new();
    let mut file: Option<String> = None;
    for line in text.lines() {
        let l = line.trim();
        if let Some(f) = l.strip_prefix("file: \"") {
            file = Some(f.trim_end_matches("\",").to_string());
        } else if let Some(n) = l.strip_prefix("presets: Some(") {
            let n: usize = n.trim_end_matches("),").parse().expect("a count");
            out.push((file.clone().expect("a file before its count"), n));
        }
    }
    out
}

#[test]
fn the_readme_and_the_site_state_every_units_preset_count_as_it_is() {
    let actual = actual();
    let count = |name: &str| actual.iter().find(|(n, _)| n == name).map(|(_, c)| *c);
    let mut wrong = Vec::new();
    let readme = readme();
    let site = site();
    assert!(readme.len() >= 30, "the README's grid read: {readme:?}");
    assert!(site.len() >= 30, "the site's cards read: {site:?}");
    for (place, stated) in [("README.md", &readme), ("web/src/units_data.rs", &site)] {
        for (name, n) in stated {
            match count(name) {
                Some(c) if c == *n => {}
                Some(c) => wrong.push(format!("{place}: {name} says {n}, has {c}")),
                None => wrong.push(format!("{place}: {name} is no unit with presets")),
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "stated counts that are not the units':\n{}",
        wrong.join("\n")
    );
}
