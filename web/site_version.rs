//! The DAW's version, as the site shows it: `[workspace.package] version` of the repository's
//! `Cargo.toml`. Shared by `build.rs` (which hands it to the site as `CANTUS_VERSION`) and by
//! xtask's test that holds the two equal (`package_ci_tests.rs`), so the page cannot drift
//! behind a release again (it said beta.11 through beta.12 and beta.13).

/// `version = "…"` under `[workspace.package]`, or `None` when the section or key is missing.
pub fn workspace_version(cargo_toml: &str) -> Option<String> {
    let mut in_section = false;
    for line in cargo_toml.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_section = line == "[workspace.package]";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "version" {
            let v = value.trim().trim_matches('"');
            return (!v.is_empty()).then(|| v.to_string());
        }
    }
    None
}
