//! Cantus's icon, rendered from the logo's SVG masters at build time (ADR 0024 §12). Shared
//! by two build scripts with `#[path]`: the app's (`build.rs`, which also bakes the
//! splash) and the launcher's (`crates/cantus-launch/build.rs`) — in a release the
//! launcher is the `cantus.exe` that the Start menu, the file association and Explorer
//! show, so it carries the same icon.
//!
//! The small master is the interface's own drawing (`kit/logo_small.rs`), included here.

#[path = "../cantus-ui/src/kit/logo_small.rs"]
mod logo_small;

use resvg::{tiny_skia, usvg};
use std::path::Path;

pub const LOGO: &str = include_str!("../cantus-ui/assets/logos/02.svg");

/// The files a build script that includes this one must be re-run for (relative to the
/// including crate, which sits beside `cantus-app`).
pub const SOURCES: [&str; 3] = [
    "../cantus-app/icon.rs",
    "../cantus-ui/assets/logos/02.svg",
    "../cantus-ui/src/kit/logo_small.rs",
];

pub fn render(svg: &str, px: u32) -> tiny_skia::Pixmap {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("the logo parses");
    let size = tree.size();
    let mut pix = tiny_skia::Pixmap::new(px, px).expect("a pixmap");
    let scale = px as f32 / size.width().max(size.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pix.as_mut(),
    );
    pix
}

/// The master for `px`: the small one on the pixel grid below 48, the full logo above.
pub fn master(px: u32) -> tiny_skia::Pixmap {
    if px < 48 {
        render(&logo_small::small_master(px), px)
    } else {
        render(LOGO, px)
    }
}

/// An `.ico` whose entries are PNG images (Windows Vista and later read them at any size).
pub fn ico(sizes: &[u32]) -> Vec<u8> {
    let pngs: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&px| master(px).encode_png().expect("png"))
        .collect();
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (px, png) in sizes.iter().zip(&pngs) {
        let side = if *px >= 256 { 0 } else { *px as u8 };
        out.push(side);
        out.push(side);
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in &pngs {
        out.extend_from_slice(png);
    }
    out
}

/// The sizes the `.exe`'s icon holds: 16–32 px from the small master, 48–256 from the logo.
pub const EXE_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

pub fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// Write `cantus.ico` into `out` and, building for Windows, put it in the executable as
/// its icon, with `description` as the version resource's FileDescription (what Task
/// Manager lists).
pub fn embed_exe_icon(out: &Path, description: &str) {
    let ico_path = out.join("cantus.ico");
    write(&ico_path, &ico(&EXE_SIZES));
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&ico_path.to_string_lossy());
        res.set("FileDescription", description);
        res.set("ProductName", "Cantus");
        if let Err(e) = res.compile() {
            // A machine without the Windows SDK's resource compiler still builds Cantus;
            // the executable just keeps the default icon.
            println!("cargo:warning=no .exe icon: {e}");
        }
    }
}
