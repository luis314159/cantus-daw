//! Each plug-in library's Windows version resource (ADR 0048 §9): product name "Cantus",
//! the workspace's version (winresource takes it from the package), and the plug-in's
//! name as the description Explorer shows. `xtask bundle` builds one library per plug-in
//! (`--no-default-features --features <name>`), so the one feature switched on names it.
//! Code signing requires every signed file to carry the product name and version;
//! `xtask package` checks it. Off Windows this does nothing.

/// The cargo feature (as `CARGO_FEATURE_*` spells it) of each plug-in, and its full name
/// (`desc.rs`; a test keeps the two lists equal).
const PLUGINS: [(&str, &str); 10] = [
    ("EQ", "Cantus Aequalis"),
    ("COMPRESSOR", "Cantus Comprimo"),
    ("LIMITER", "Cantus Terminus"),
    ("SPACE", "Cantus Spatium"),
    ("MULTIBAND", "Cantus Multiplex"),
    ("DEESSER", "Cantus Sibilus"),
    ("GATE", "Cantus Porta"),
    ("STEREO", "Cantus Latitudo"),
    ("MAXIMIZER", "Cantus Culmen"),
    ("SATURATOR", "Cantus Fervor"),
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let on: Vec<&str> = PLUGINS
        .iter()
        .filter(|(feature, _)| std::env::var_os(format!("CARGO_FEATURE_{feature}")).is_some())
        .map(|(_, name)| *name)
        .collect();
    let (description, internal) = match on.as_slice() {
        [one] => (format!("{one} (CLAP and VST3 plug-in)"), (*one).to_owned()),
        _ => (
            "Cantus plug-ins (CLAP and VST3)".to_owned(),
            "cantus-plugins".to_owned(),
        ),
    };
    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Cantus");
    res.set("FileDescription", &description);
    res.set("CompanyName", "Cantus");
    res.set("LegalCopyright", "GPL-3.0-or-later");
    res.set("InternalName", &internal);
    res.set("OriginalFilename", &internal);
    if let Err(e) = res.compile() {
        // A machine without the Windows SDK's resource compiler still builds; a release
        // made there is refused by `xtask package`'s check.
        println!("cargo:warning=no version resource for the plug-ins: {e}");
    }
}
