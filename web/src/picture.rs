//! The program's picture — the one the hero shows full-width and the tour crops. One asset,
//! swapped in one place: replace `assets/program/cantus.webp` and `cantus-es.webp` (the
//! Spanish interface) with `cargo run --release -p cantus-ui --example shoot_site -- <dir>
//! night [es]` re-encoded as lossless WebP, and change `SIZE` here if the shot's size changes.

use crate::lang::Lang;

/// The picture's size in pixels (the tour's crops are in these units).
pub const SIZE: (u32, u32) = (1912, 1022);

/// The picture for this language's page.
pub fn program(l: Lang) -> &'static str {
    l.t("/program/cantus.webp", "/program/cantus-es.webp")
}

/// The same picture at 960 and 1280 pixels wide (lossy WebP, q82, made from the lossless one
/// with Pillow) and whole, for `srcset`: a phone fetches a third of the bytes, and the hero's
/// picture is the page's largest paint (round 39, web-struct: mobile LCP).
pub fn program_srcset(l: Lang) -> &'static str {
    l.t(
        "/program/cantus-960.webp 960w, /program/cantus-1280.webp 1280w, /program/cantus.webp 1912w",
        "/program/cantus-es-960.webp 960w, /program/cantus-es-1280.webp 1280w, /program/cantus-es.webp 1912w",
    )
}

/// The hero's `sizes`: the stage's width (a phone's crop is wider than the screen, but the
/// detail it shows reads at the screen's width).
pub const PROGRAM_SIZES: &str = "(max-width: 52rem) 100vw, 76rem";

/// What it shows, for the alt text.
pub fn program_alt(l: Lang) -> &'static str {
    l.t(
        "Cantus playing its demo song Neon Tide: the arrangement, the mixer with its cables and the inspector",
        "Cantus tocando su canción de demostración Neon Tide: el arreglo, el mezclador con sus cables y el inspector",
    )
}
