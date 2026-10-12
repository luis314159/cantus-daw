//! Cantus's icon, rendered at build time from the mark (ADR 0046): the website's C of waves
//! and playhead with no tile behind it, in its designed amber → copper. Shared
//! by two build scripts with `#[path]`: the app's (`build.rs`, which also bakes the
//! splash) and the launcher's (`crates/cantus-launch/build.rs`) — in a release the
//! launcher is the `cantus.exe` that the Start menu, the file association and Explorer
//! show, so it carries the same icon.
//!
//! The mark is the interface's own drawing (`kit/mark.rs`, which recolours it live for the
//! running window), included here; the splash's logo is still #2, as designed.

#[path = "../cantus-ui/src/kit/mark.rs"]
#[allow(dead_code)]
mod mark;

use resvg::{tiny_skia, usvg};
use std::path::Path;

pub const LOGO: &str = include_str!("../cantus-ui/assets/logos/02.svg");

/// The files a build script that includes this one must be re-run for (relative to the
/// including crate, which sits beside `cantus-app`).
pub const SOURCES: [&str; 3] = [
    "../cantus-app/icon.rs",
    "../cantus-ui/assets/logos/02.svg",
    "../cantus-ui/src/kit/mark.rs",
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

/// The icon at `px`: the mark drawn for that size (redrawn for the pixel grid below 40).
pub fn master(px: u32) -> tiny_skia::Pixmap {
    render(&mark::mark_svg(px, &mark::MarkColours::ORIGINAL), px)
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

/// The sizes the `.exe`'s (and the installer's) icon holds: every size Windows asks for at
/// 100–200 % scale — 16, 20, 24 and 32 for small icons and the taskbar, 40, 48 and 64 for
/// the large ones, 256 for Explorer's tiles.
pub const EXE_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

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
