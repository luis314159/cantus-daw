//! Bakes Cantus's art into the executable (ADR 0024 §12):
//!
//! - the splash's logo — #2, "C of waves with a playhead cross", as designed — at 1× and
//!   2× of its 184-point side, as premultiplied RGBA, so the splash's first frame shows it
//!   without rendering anything;
//! - the window's icon off Windows: the mark (ADR 0046) at 64 px (on Windows the running
//!   application draws its own, in the colour the user chose: `cantus_ui::window_icon`);
//! - the `.exe`'s icon: a Windows resource (`winresource`, MIT) holding the mark at every
//!   size from 16 to 256 px, the small ones redrawn for the pixel grid.
//!
//! The icon code is `icon.rs`, shared with the launcher's build script.

#[path = "icon.rs"]
mod icon;

use icon::{master, render, write, LOGO};
use resvg::tiny_skia;
use std::path::PathBuf;

/// The splash's logo side in points (`cantus_ui::boot::LOGO`).
const SPLASH: u32 = 184;

/// Straight (not premultiplied) RGBA, as window icons want it.
fn straight(pix: &tiny_skia::Pixmap) -> Vec<u8> {
    pix.pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for source in icon::SOURCES {
        println!("cargo:rerun-if-changed={source}");
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    let one = render(LOGO, SPLASH);
    let two = render(LOGO, SPLASH * 2);
    write(&out.join("splash_1x.rgba"), one.data());
    write(&out.join("splash_2x.rgba"), two.data());
    let icon = master(64);
    write(&out.join("icon_64.rgba"), &straight(&icon));
    let baked = format!(
        r#"/// The art baked in at build time (`build.rs`).
pub const ART: cantus_ui::boot::Art = cantus_ui::boot::Art {{
    logo_1x: cantus_ui::boot::Baked {{ side: {s1}, rgba: include_bytes!(concat!(env!("OUT_DIR"), "/splash_1x.rgba")) }},
    logo_2x: cantus_ui::boot::Baked {{ side: {s2}, rgba: include_bytes!(concat!(env!("OUT_DIR"), "/splash_2x.rgba")) }},
    icon: cantus_ui::boot::Baked {{ side: 64, rgba: include_bytes!(concat!(env!("OUT_DIR"), "/icon_64.rgba")) }},
}};
"#,
        s1 = SPLASH,
        s2 = SPLASH * 2
    );
    write(&out.join("baked.rs"), baked.as_bytes());

    icon::embed_exe_icon(&out, "Cantus");
    // macOS: the bundle's icon, which `packaging/macos/build-dmg.sh` takes from here
    // (ADR 0066).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        write(&out.join("Cantus.icns"), &icon::icns());
    }
}
