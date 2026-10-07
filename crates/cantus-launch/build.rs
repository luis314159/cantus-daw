//! The launcher's icon: Cantus's own, from the app's icon code (`../cantus-app/icon.rs`),
//! so the `cantus.exe` that the Start menu, the `.cantus` association and Explorer show
//! looks like Cantus.

#[path = "../cantus-app/icon.rs"]
#[allow(dead_code)]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for source in icon::SOURCES {
        println!("cargo:rerun-if-changed={source}");
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    icon::embed_exe_icon(&out, "Cantus");
}
