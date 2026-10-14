//! The plug-in scanner's Windows version resource (ADR 0048 §9): product name "Cantus", the
//! workspace's version (winresource takes it from the package), "Cantus plug-in scanner"
//! as the description Task Manager and Explorer show. Code signing requires every signed
//! file to carry the product name and version; `xtask package` checks it. Off Windows this
//! does nothing.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Cantus");
    res.set("FileDescription", "Cantus plug-in scanner");
    res.set("CompanyName", "Cantus");
    res.set("LegalCopyright", "GPL-3.0-or-later");
    res.set("InternalName", "cantus-scan");
    res.set("OriginalFilename", "cantus-scan.exe");
    if let Err(e) = res.compile() {
        // A machine without the Windows SDK's resource compiler still builds; a release
        // made there is refused by `xtask package`'s check.
        println!("cargo:warning=no version resource for cantus-scan: {e}");
    }
}
