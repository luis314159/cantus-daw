//! Bakes Cantus's art into the executable (ADR 0024 §12):
//!
//! - the splash's logo — #2, "C of waves with a playhead cross", as designed — at 1× and
//!   2× of its 184-point side, as premultiplied RGBA, so the splash's first frame shows it
//!   without rendering anything;
//! - the window's and the taskbar's icon: #2's small master at 32 px, drawn on the pixel
//!   grid (a 32-px render of the full logo blurs);
//! - the `.exe`'s icon: a Windows resource (`winresource`, MIT) holding 16, 24 and 32 px
//!   from the small masters and 48, 64, 128 and 256 px from the full logo.
//!
//! The small master is the interface's own drawing (`kit/logo_small.rs`), included here.

#[path = "../cantus-ui/src/kit/logo_small.rs"]
mod logo_small;

use resvg::{tiny_skia, usvg};
use std::path::{Path, PathBuf};

const LOGO: &str = include_str!("../cantus-ui/assets/logos/02.svg");
/// The splash's logo side in points (`cantus_ui::boot::LOGO`).
const SPLASH: u32 = 184;

fn render(svg: &str, px: u32) -> tiny_skia::Pixmap {
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
fn master(px: u32) -> tiny_skia::Pixmap {
    if px < 48 {
        render(&logo_small::small_master(px), px)
    } else {
        render(LOGO, px)
    }
}

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

/// An `.ico` whose entries are PNG images (Windows Vista and later read them at any size).
fn ico(sizes: &[u32]) -> Vec<u8> {
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

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../cantus-ui/assets/logos/02.svg");
    println!("cargo:rerun-if-changed=../cantus-ui/src/kit/logo_small.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    let one = render(LOGO, SPLASH);
    let two = render(LOGO, SPLASH * 2);
    write(&out.join("splash_1x.rgba"), one.data());
    write(&out.join("splash_2x.rgba"), two.data());
    let icon = master(32);
    write(&out.join("icon_32.rgba"), &straight(&icon));
    let baked = format!(
        r#"/// The art baked in at build time (`build.rs`).
pub const ART: cantus_ui::boot::Art = cantus_ui::boot::Art {{
    logo_1x: cantus_ui::boot::Baked {{ side: {s1}, rgba: include_bytes!(concat!(env!("OUT_DIR"), "/splash_1x.rgba")) }},
    logo_2x: cantus_ui::boot::Baked {{ side: {s2}, rgba: include_bytes!(concat!(env!("OUT_DIR"), "/splash_2x.rgba")) }},
    icon: cantus_ui::boot::Baked {{ side: 32, rgba: include_bytes!(concat!(env!("OUT_DIR"), "/icon_32.rgba")) }},
}};
"#,
        s1 = SPLASH,
        s2 = SPLASH * 2
    );
    write(&out.join("baked.rs"), baked.as_bytes());

    let ico_path = out.join("cantus.ico");
    write(&ico_path, &ico(&[16, 24, 32, 48, 64, 128, 256]));
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&ico_path.to_string_lossy());
        if let Err(e) = res.compile() {
            // A machine without the Windows SDK's resource compiler still builds Cantus;
            // the executable just keeps the default icon.
            println!("cargo:warning=no .exe icon: {e}");
        }
    }
}
