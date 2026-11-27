//! Hands the site the DAW's version (`CANTUS_VERSION`, read in `src/top.rs`) from the
//! repository's `Cargo.toml`, so a release's version bump is the site's too — and the latest
//! release's facts (`assets/release.json`: version, date, each file's size and SHA-256), read
//! into `release.rs` for the download page (round 39: fetched at build time, so the visitor's
//! browser never asks api.github.com).

#[path = "site_version.rs"]
mod site_version;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let toml_path = std::path::Path::new(&dir).join("../Cargo.toml");
    let toml = std::fs::read_to_string(&toml_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", toml_path.display()));
    let version = site_version::workspace_version(&toml)
        .unwrap_or_else(|| panic!("no [workspace.package] version in {}", toml_path.display()));
    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=site_version.rs");
    println!("cargo:rustc-env=CANTUS_VERSION={version}");
    release_facts(&dir);
    logo_uri(&dir);
}

/// `assets/logo-48.svg` → `$OUT_DIR/logo.rs`: `LOGO_48`, the bar's mark as a `data:` address.
/// The mark is in the first paint; drawn from the page itself it needs no request of its own,
/// which on a slow phone would share the line with the hero's picture (the site budget's LCP).
fn logo_uri(dir: &str) {
    let path = std::path::Path::new(dir).join("assets/logo-48.svg");
    println!("cargo:rerun-if-changed=assets/logo-48.svg");
    let svg =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        !svg.contains('\''),
        "{}: the data: address quotes the attributes with ', so the drawing must use \"",
        path.display()
    );
    let mut uri = String::from("data:image/svg+xml,");
    for c in svg.trim().chars() {
        match c {
            '"' => uri.push('\''),
            '%' => uri.push_str("%25"),
            '#' => uri.push_str("%23"),
            '<' => uri.push_str("%3C"),
            '>' => uri.push_str("%3E"),
            '&' => uri.push_str("%26"),
            '\r' | '\n' | '\t' => uri.push(' '),
            c => uri.push(c),
        }
    }
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let dest = std::path::Path::new(&out_dir).join("logo.rs");
    std::fs::write(&dest, format!("pub const LOGO_48: &str = {uri:?};\n"))
        .unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
}

/// `assets/release.json` → `$OUT_DIR/release.rs`: `REL_VERSION`, `REL_DATE`, `REL_NOTES`
/// and `REL_FILES` (name, size in bytes, SHA-256, link) — and, natively only, `REL_PLUGINS`:
/// the plug-in zips (round 42).
fn release_facts(dir: &str) {
    let path = std::path::Path::new(dir).join("assets/release.json");
    println!("cargo:rerun-if-changed=assets/release.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let s = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("").to_string();
    let mut out = String::new();
    out += &format!("pub const REL_VERSION: &str = {:?};\n", s(&v, "version"));
    out += &format!("pub const REL_DATE: &str = {:?};\n", s(&v, "date"));
    out += &format!("pub const REL_NOTES: &str = {:?};\n", s(&v, "notes"));
    // Each effect's plug-in zips (round 42, `plugins.rs`): over a hundred files, kept out of
    // `REL_FILES` and out of the WebAssembly (its budget), which reads them from
    // `/plugins.json`; natively, for the pre-renderer, `REL_PLUGINS`.
    let all = v["files"].as_array().cloned().unwrap_or_default();
    let plugin = |f: &serde_json::Value| s(f, "name").starts_with("Cantus-plugin");
    let list = |want: bool| {
        let mut o = String::new();
        for f in all.iter().filter(|f| plugin(f) == want) {
            o += &format!(
                "    RelFile {{ name: {:?}, size: {}, sha256: {:?}, url: {:?} }},\n",
                s(f, "name"),
                f["size"].as_u64().unwrap_or(0),
                s(f, "sha256"),
                s(f, "url")
            );
        }
        o
    };
    out += "pub const REL_FILES: &[RelFile] = &[\n";
    out += &list(false);
    out += "];\n";
    out += "#[cfg(not(target_arch = \"wasm32\"))]\npub const REL_PLUGINS: &[RelFile] = &[\n";
    out += &list(true);
    out += "];\n";
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let dest = std::path::Path::new(&out_dir).join("release.rs");
    std::fs::write(&dest, out).unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
}
