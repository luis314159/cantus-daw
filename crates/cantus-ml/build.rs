//! The machine-learning helper's Windows version resource (ADR 0048 §9, ADR 0085): product
//! name "Cantus", the workspace's version, "Cantus machine-learning helper" as the
//! description Task Manager and Explorer show beside the helper process. Code signing
//! requires every signed file to carry the product name and version; `xtask package` checks
//! it. Off Windows this does nothing.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "Cantus");
    res.set("FileDescription", "Cantus machine-learning helper");
    res.set("CompanyName", "Cantus");
    res.set("LegalCopyright", "GPL-3.0-or-later");
    res.set("InternalName", "cantus-ml");
    res.set("OriginalFilename", "cantus-ml.exe");
    if let Err(e) = res.compile() {
        // A machine without the Windows SDK's resource compiler still builds; a release
        // made there is refused by `xtask package`'s check.
        println!("cargo:warning=no version resource for cantus-ml: {e}");
    }
}
