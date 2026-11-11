//! The icon family's drawings (round 39, `icons`, second pass — approved by the owner on
//! 2026-11-09): every glyph drawn for each key height it sits in (16, 20, 24, 30, 36 device
//! pixels), as the design source has them (`docs/round39/design/icons/glyphs-n-*.js`, with
//! the first pass's drawings it reuses expanded). Generated; edit the design, not this file.
//! Inks are named, not given: `var(--main)`, `var(--second)`, `var(--rec)`, `var(--cut)` and
//! the classes `f1 f2 fr s1 s2 cut scut` take the key's state ([`super::icons`]).

/// (name, key height in device pixels, SVG body on a canvas that is the key).
pub const DRAWINGS: &[(&str, u16, &str)] = &[
    ("play", 16, r#"<path class="f1" d="M6 5 L11 8 L6 11 Z"/>"#),
    ("play", 20, r#"<path class="f1" d="M8 6 L15 10 L8 14 Z"/>"#),
    ("play", 24, r#"<path class="f1" d="M9 7 L18 12 L9 17 Z"/>"#),
    (
        "play",
        30,
        r#"<path class="f1" d="M12 9 L22 15 L12 21 Z"/>"#,
    ),
    (
        "play",
        36,
        r#"<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M14.5 11.8 L25.2 18 L14.5 24.2 Z"/>"#,
    ),
    (
        "pause",
        16,
        r#"<rect class="f1" x="5" y="5" width="2" height="6"/><rect class="f1" x="9" y="5" width="2" height="6"/>"#,
    ),
    (
        "pause",
        20,
        r#"<rect class="f1" x="6" y="6" width="3" height="8"/><rect class="f1" x="11" y="6" width="3" height="8"/>"#,
    ),
    (
        "pause",
        24,
        r#"<rect class="f1" x="8" y="7" width="3" height="10"/><rect class="f1" x="13" y="7" width="3" height="10"/>"#,
    ),
    (
        "pause",
        30,
        r#"<rect class="f1" x="10" y="9" width="3" height="12" rx="0.5"/><rect class="f1" x="17" y="9" width="3" height="12" rx="0.5"/>"#,
    ),
    (
        "pause",
        36,
        r#"<rect class="f1" x="12" y="11" width="4" height="14" rx="1"/><rect class="f1" x="20" y="11" width="4" height="14" rx="1"/>"#,
    ),
    (
        "stop",
        16,
        r#"<rect class="f1" x="5" y="5" width="5" height="5"/>"#,
    ),
    (
        "stop",
        20,
        r#"<rect class="f1" x="7" y="7" width="6" height="6"/>"#,
    ),
    (
        "stop",
        24,
        r#"<rect class="f1" x="8" y="8" width="8" height="8" rx="0.5"/>"#,
    ),
    (
        "stop",
        30,
        r#"<rect class="f1" x="10" y="10" width="10" height="10" rx="1"/>"#,
    ),
    (
        "stop",
        36,
        r#"<rect class="f1" x="12" y="12" width="12" height="12" rx="1.5"/>"#,
    ),
    (
        "record",
        16,
        r#"<circle class="fr" cx="8" cy="8" r="3.5"/>"#,
    ),
    (
        "record",
        20,
        r#"<circle class="fr" cx="10" cy="10" r="4.5"/>"#,
    ),
    (
        "record",
        24,
        r#"<circle class="fr" cx="12" cy="12" r="5.5"/>"#,
    ),
    (
        "record",
        30,
        r#"<circle class="fr" cx="15" cy="15" r="6.5"/>"#,
    ),
    (
        "record",
        36,
        r#"<circle class="fr" cx="18" cy="18" r="8"/>"#,
    ),
    (
        "toStart",
        16,
        r#"<rect class="f1" x="4" y="5" width="1" height="6"/><path class="f1" d="M11 5 L6 8 L11 11 Z"/>"#,
    ),
    (
        "toStart",
        20,
        r#"<rect class="f1" x="5" y="6" width="2" height="8"/><path class="f1" d="M15 6 L8 10 L15 14 Z"/>"#,
    ),
    (
        "toStart",
        24,
        r#"<rect class="f1" x="6" y="7" width="2" height="10"/><path class="f1" d="M18 7 L10 12 L18 17 Z"/>"#,
    ),
    (
        "toStart",
        30,
        r#"<rect class="f1" x="8" y="9" width="2" height="12"/><path class="f1" d="M22 9 L12 15 L22 21 Z"/>"#,
    ),
    (
        "toStart",
        36,
        r#"<rect class="f1" x="9" y="11" width="3" height="14" rx="0.5"/><path class="f1" d="M27 11 L14 18 L27 25 Z"/>"#,
    ),
    (
        "skipFwd",
        16,
        r#"<path class="f1" d="M4 5 L8 8 L4 11 Z M8 5 L12 8 L8 11 Z"/>"#,
    ),
    (
        "skipFwd",
        20,
        r#"<path class="f1" d="M5 6 L10 10 L5 14 Z M10 6 L15 10 L10 14 Z"/>"#,
    ),
    (
        "skipFwd",
        24,
        r#"<path class="f1" d="M6 7 L12 12 L6 17 Z M12 7 L18 12 L12 17 Z"/>"#,
    ),
    (
        "skipFwd",
        30,
        r#"<path class="f1" d="M8 9 L15 15 L8 21 Z M15 9 L22 15 L15 21 Z"/>"#,
    ),
    (
        "skipFwd",
        36,
        r#"<path class="f1" d="M10 11 L18 18 L10 25 Z M18 11 L26 18 L18 25 Z"/>"#,
    ),
    (
        "skipBack",
        16,
        r#"<path class="f1" d="M12 5 L8 8 L12 11 Z M8 5 L4 8 L8 11 Z"/>"#,
    ),
    (
        "skipBack",
        20,
        r#"<path class="f1" d="M15 6 L10 10 L15 14 Z M10 6 L5 10 L10 14 Z"/>"#,
    ),
    (
        "skipBack",
        24,
        r#"<path class="f1" d="M18 7 L12 12 L18 17 Z M12 7 L6 12 L12 17 Z"/>"#,
    ),
    (
        "skipBack",
        30,
        r#"<path class="f1" d="M22 9 L15 15 L22 21 Z M15 9 L8 15 L15 21 Z"/>"#,
    ),
    (
        "skipBack",
        36,
        r#"<path class="f1" d="M26 11 L18 18 L26 25 Z M18 11 L10 18 L18 25 Z"/>"#,
    ),
    (
        "loop",
        16,
        r#"<path class="s1" stroke-width="1" d="M3.5 9 V8 A2.5 2.5 0 0 1 6 5.5 H10"/><path class="s1" stroke-width="1" d="M12.5 7 V8 A2.5 2.5 0 0 1 10 10.5 H6"/><g shape-rendering="crispEdges" transform="translate(10,4)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(4,9)"><rect x="1" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="1" y="2" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "loop",
        20,
        r#"<path class="s1" stroke-width="1" d="M5.5 12 V10.5 A3 3 0 0 1 8.5 7.5 H12"/><path class="s1" stroke-width="1" d="M14.5 8 V9.5 A3 3 0 0 1 11.5 12.5 H8"/><g shape-rendering="crispEdges" transform="translate(12,5)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(5,10)"><rect x="2" y="0" width="1" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="1" y="3" width="2" height="1" fill="var(--main)"/><rect x="2" y="4" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "loop",
        24,
        r#"<path class="s1" stroke-width="1" d="M6.5 14 V12 A3.5 3.5 0 0 1 10 8.5 H15"/><path class="s1" stroke-width="1" d="M17.5 10 V12 A3.5 3.5 0 0 1 14 15.5 H9"/><g shape-rendering="crispEdges" transform="translate(15,6)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(6,13)"><rect x="2" y="0" width="1" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="1" y="3" width="2" height="1" fill="var(--main)"/><rect x="2" y="4" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "loop",
        30,
        r#"<path class="s1" stroke-width="2" stroke-linecap="round" d="M8 16 V14 A4 4 0 0 1 12 10 H21"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M18.5 7.5 L21.5 10 L18.5 12.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M22 14 V16 A4 4 0 0 1 18 20 H9"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M11.5 17.5 L8.5 20 L11.5 22.5"/>"#,
    ),
    (
        "loop",
        36,
        r#"<path class="s1" stroke-width="2" stroke-linecap="round" d="M9 19 V17 A5 5 0 0 1 14 12 H26"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M23 9 L26.5 12 L23 15"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M27 17 V19 A5 5 0 0 1 22 24 H10"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M13 21 L9.5 24 L13 27"/>"#,
    ),
    (
        "metronome",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="8" y="0" width="1" height="1" fill="var(--main)"/><rect x="8" y="1" width="1" height="1" fill="var(--main)"/><rect x="3" y="2" width="3" height="1" fill="var(--main)"/><rect x="7" y="2" width="1" height="1" fill="var(--main)"/><rect x="3" y="3" width="3" height="1" fill="var(--main)"/><rect x="7" y="3" width="1" height="1" fill="var(--main)"/><rect x="2" y="4" width="4" height="1" fill="var(--main)"/><rect x="7" y="4" width="1" height="1" fill="var(--main)"/><rect x="2" y="5" width="4" height="1" fill="var(--main)"/><rect x="7" y="5" width="1" height="1" fill="var(--main)"/><rect x="1" y="6" width="4" height="1" fill="var(--main)"/><rect x="6" y="6" width="3" height="1" fill="var(--main)"/><rect x="1" y="7" width="4" height="1" fill="var(--main)"/><rect x="6" y="7" width="3" height="1" fill="var(--main)"/><rect x="1" y="8" width="3" height="1" fill="var(--main)"/><rect x="5" y="8" width="4" height="1" fill="var(--main)"/><rect x="1" y="9" width="3" height="1" fill="var(--main)"/><rect x="5" y="9" width="4" height="1" fill="var(--main)"/><rect x="0" y="10" width="10" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "metronome",
        20,
        r#"<g shape-rendering="crispEdges" transform="translate(4,4)"><rect x="10" y="0" width="1" height="1" fill="var(--main)"/><rect x="10" y="1" width="1" height="1" fill="var(--main)"/><rect x="4" y="2" width="4" height="1" fill="var(--main)"/><rect x="9" y="2" width="1" height="1" fill="var(--main)"/><rect x="3" y="3" width="5" height="1" fill="var(--main)"/><rect x="9" y="3" width="1" height="1" fill="var(--main)"/><rect x="3" y="4" width="5" height="1" fill="var(--main)"/><rect x="3" y="5" width="5" height="1" fill="var(--main)"/><rect x="2" y="6" width="5" height="1" fill="var(--main)"/><rect x="8" y="6" width="2" height="1" fill="var(--main)"/><rect x="2" y="7" width="5" height="1" fill="var(--main)"/><rect x="8" y="7" width="2" height="1" fill="var(--main)"/><rect x="2" y="8" width="4" height="1" fill="var(--main)"/><rect x="7" y="8" width="3" height="1" fill="var(--main)"/><rect x="1" y="9" width="5" height="1" fill="var(--main)"/><rect x="7" y="9" width="4" height="1" fill="var(--main)"/><rect x="1" y="10" width="4" height="1" fill="var(--main)"/><rect x="6" y="10" width="5" height="1" fill="var(--main)"/><rect x="1" y="11" width="4" height="1" fill="var(--main)"/><rect x="6" y="11" width="5" height="1" fill="var(--main)"/><rect x="0" y="12" width="12" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "metronome",
        24,
        r#"<path class="f1" d="M10 5 H14 L17.5 19 H6.5 Z"/><path class="scut" stroke-width="1.5" d="M12 17 L14.9 8.6"/><path class="s1" stroke-width="1.5" d="M14.9 8.6 L16.6 3.5"/>"#,
    ),
    (
        "metronome",
        30,
        r#"<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M13 7.5 H17 L21.5 23.5 H8.5 Z"/><path class="scut" stroke-width="2" d="M15 22 L18.7 11.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M18.7 11.5 L21 5"/>"#,
    ),
    (
        "metronome",
        36,
        r#"<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M15.5 8.5 H20.5 L25.5 27.5 H10.5 Z"/><path class="scut" stroke-width="2.2" d="M18 26 L22.4 13.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M22.4 13.5 L25 6"/>"#,
    ),
    (
        "countIn",
        16,
        r#"<rect class="f2" x="3" y="7" width="2" height="2"/><rect class="f2" x="6" y="7" width="2" height="2"/><path class="f1" d="M9 5 L13 8 L9 11 Z"/>"#,
    ),
    (
        "countIn",
        20,
        r#"<rect class="f2" x="4" y="9" width="2" height="2"/><rect class="f2" x="7" y="9" width="2" height="2"/><rect class="f2" x="10" y="9" width="2" height="2"/><path class="f1" d="M13 7 L17 10 L13 13 Z"/>"#,
    ),
    (
        "countIn",
        24,
        r#"<rect class="f2" x="5" y="11" width="2" height="2"/><rect class="f2" x="8" y="11" width="2" height="2"/><rect class="f2" x="11" y="11" width="2" height="2"/><path class="f1" d="M14 8 L19 12 L14 16 Z"/>"#,
    ),
    (
        "countIn",
        30,
        r#"<circle class="f2" cx="8" cy="15" r="1.4"/><circle class="f2" cx="11.5" cy="15" r="1.4"/><circle class="f2" cx="15" cy="15" r="1.4"/><path class="f1" d="M18 10 L24 15 L18 20 Z"/>"#,
    ),
    (
        "countIn",
        36,
        r#"<circle class="f2" cx="9" cy="18" r="1.6"/><circle class="f2" cx="13.5" cy="18" r="1.6"/><circle class="f2" cx="18" cy="18" r="1.6"/><path class="f1" d="M22 12 L29 18 L22 24 Z"/>"#,
    ),
    (
        "follow",
        16,
        r#"<rect class="f1" x="5" y="6" width="1" height="7"/><rect class="f1" x="8" y="8" width="3" height="1"/><g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="0" y="0" width="5" height="1" fill="var(--main)"/><rect x="1" y="1" width="3" height="1" fill="var(--main)"/><rect x="2" y="2" width="1" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(11,7)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "follow",
        20,
        r#"<rect class="f1" x="6" y="7" width="1" height="9"/><rect class="f1" x="9" y="10" width="4" height="1"/><g shape-rendering="crispEdges" transform="translate(4,4)"><rect x="0" y="0" width="5" height="1" fill="var(--main)"/><rect x="1" y="1" width="3" height="1" fill="var(--main)"/><rect x="2" y="2" width="1" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(13,8)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "follow",
        24,
        r#"<rect class="f1" x="7" y="7" width="1" height="12"/><rect class="f1" x="11" y="12" width="5" height="1"/><g shape-rendering="crispEdges" transform="translate(5,5)"><rect x="0" y="0" width="5" height="1" fill="var(--main)"/><rect x="1" y="1" width="3" height="1" fill="var(--main)"/><rect x="2" y="2" width="1" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(16,10)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "follow",
        30,
        r#"<path class="f1" d="M6.5 7 H13.5 L10 11.5 Z"/><path class="s1" stroke-width="2" d="M10 10 V24"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M14 15 H22"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M19.5 12.5 L22.5 15 L19.5 17.5"/>"#,
    ),
    (
        "follow",
        36,
        r#"<path class="f1" d="M8 8 H16 L12 13 Z"/><path class="s1" stroke-width="2" d="M12 12 V28"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M17 18 H26"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M23 15 L26.5 18 L23 21"/>"#,
    ),
    (
        "snap",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(4,3)"><rect x="0" y="0" width="2" height="1" fill="var(--main)"/><rect x="6" y="0" width="2" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="6" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="6" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="2" height="1" fill="var(--main)"/><rect x="6" y="4" width="2" height="1" fill="var(--main)"/><rect x="0" y="5" width="2" height="1" fill="var(--main)"/><rect x="6" y="5" width="2" height="1" fill="var(--main)"/><rect x="0" y="6" width="2" height="1" fill="var(--main)"/><rect x="6" y="6" width="2" height="1" fill="var(--main)"/><rect x="0" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="1" y="7" width="2" height="1" fill="var(--main)"/><rect x="5" y="7" width="2" height="1" fill="var(--main)"/><rect x="7" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="1" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="2" y="8" width="4" height="1" fill="var(--main)"/><rect x="6" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g>"#,
    ),
    (
        "snap",
        20,
        r#"<g shape-rendering="crispEdges" transform="translate(6,4)"><rect x="0" y="0" width="2" height="1" fill="var(--main)"/><rect x="6" y="0" width="2" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="6" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="6" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="2" height="1" fill="var(--main)"/><rect x="6" y="4" width="2" height="1" fill="var(--main)"/><rect x="0" y="5" width="2" height="1" fill="var(--main)"/><rect x="6" y="5" width="2" height="1" fill="var(--main)"/><rect x="0" y="6" width="2" height="1" fill="var(--main)"/><rect x="6" y="6" width="2" height="1" fill="var(--main)"/><rect x="0" y="7" width="2" height="1" fill="var(--main)"/><rect x="6" y="7" width="2" height="1" fill="var(--main)"/><rect x="0" y="8" width="2" height="1" fill="var(--main)"/><rect x="6" y="8" width="2" height="1" fill="var(--main)"/><rect x="0" y="9" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="1" y="9" width="2" height="1" fill="var(--main)"/><rect x="5" y="9" width="2" height="1" fill="var(--main)"/><rect x="7" y="9" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="1" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="2" y="10" width="4" height="1" fill="var(--main)"/><rect x="6" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g>"#,
    ),
    (
        "snap",
        24,
        r#"<rect class="f1" x="7" y="6" width="2" height="3"/><rect class="f1" x="15" y="6" width="2" height="3"/><path class="s1" stroke-width="2" d="M8 10 V13 A4 4 0 0 0 16 13 V10"/>"#,
    ),
    (
        "snap",
        30,
        r#"<rect class="f1" x="8" y="7" width="3" height="3"/><rect class="f1" x="19" y="7" width="3" height="3"/><path class="s1" stroke-width="3" d="M9.5 11 V15 A5.5 5.5 0 0 0 20.5 15 V11"/>"#,
    ),
    (
        "snap",
        36,
        r#"<rect class="f1" x="10" y="8" width="3" height="3" rx="0.5"/><rect class="f1" x="23" y="8" width="3" height="3" rx="0.5"/><path class="s1" stroke-width="3" d="M11.5 13 V18 A6.5 6.5 0 0 0 24.5 18 V13"/>"#,
    ),
    (
        "undo",
        16,
        r#"<path class="s1" stroke-width="1" d="M6 6.5 H9.5 A3 3 0 0 1 9.5 12.5 H6"/><g shape-rendering="crispEdges" transform="translate(3,4)"><rect x="2" y="0" width="1" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="1" y="3" width="2" height="1" fill="var(--main)"/><rect x="2" y="4" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "undo",
        20,
        r#"<g transform="translate(3,2)"><g shape-rendering="crispEdges"><rect x="4" y="4" width="1" height="1" fill="var(--main)"/><rect x="3" y="5" width="2" height="1" fill="var(--main)"/><rect x="2" y="6" width="8" height="1" fill="var(--main)"/><rect x="10" y="6" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="3" y="7" width="2" height="1" fill="var(--main)"/><rect x="11" y="7" width="1" height="1" fill="var(--main)"/><rect x="4" y="8" width="1" height="1" fill="var(--main)"/><rect x="11" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="12" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="12" y="9" width="1" height="1" fill="var(--main)"/><rect x="11" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="12" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="11" y="11" width="1" height="1" fill="var(--main)"/><rect x="5" y="12" width="6" height="1" fill="var(--main)"/><rect x="11" y="12" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g></g>"#,
    ),
    (
        "undo",
        24,
        r#"<g transform="translate(3,2)"><g shape-rendering="crispEdges"><rect x="6" y="4" width="1" height="1" fill="var(--main)"/><rect x="5" y="5" width="2" height="1" fill="var(--main)"/><rect x="4" y="6" width="3" height="1" fill="var(--main)"/><rect x="3" y="7" width="10" height="1" fill="var(--main)"/><rect x="13" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="4" y="8" width="3" height="1" fill="var(--main)"/><rect x="13" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="14" y="8" width="1" height="1" fill="var(--main)"/><rect x="5" y="9" width="2" height="1" fill="var(--main)"/><rect x="14" y="9" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="15" y="9" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="6" y="10" width="1" height="1" fill="var(--main)"/><rect x="14" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="15" y="10" width="1" height="1" fill="var(--main)"/><rect x="15" y="11" width="1" height="1" fill="var(--main)"/><rect x="14" y="12" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="15" y="12" width="1" height="1" fill="var(--main)"/><rect x="14" y="13" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="15" y="13" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="13" y="14" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="14" y="14" width="1" height="1" fill="var(--main)"/><rect x="7" y="15" width="6" height="1" fill="var(--main)"/><rect x="13" y="15" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g></g>"#,
    ),
    (
        "undo",
        30,
        r#"<path class="s1" stroke-width="2" stroke-linecap="round" d="M8.5 11 H17 A5 5 0 0 1 17 21 H10"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M11.5 7.5 L8 11 L11.5 14.5"/>"#,
    ),
    (
        "undo",
        36,
        r#"<path class="s1" stroke-width="2" stroke-linecap="round" d="M9.5 13 H20.5 A6 6 0 0 1 20.5 25 H11.5"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M13 9 L9 13 L13 17"/>"#,
    ),
    (
        "redo",
        16,
        r#"<path class="s1" stroke-width="1" d="M10 6.5 H6.5 A3 3 0 0 0 6.5 12.5 H10"/><g shape-rendering="crispEdges" transform="translate(10,4)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="2" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "redo",
        20,
        r#"<g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="11" y="4" width="1" height="1" fill="var(--main)"/><rect x="11" y="5" width="2" height="1" fill="var(--main)"/><rect x="5" y="6" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="6" y="6" width="8" height="1" fill="var(--main)"/><rect x="4" y="7" width="1" height="1" fill="var(--main)"/><rect x="11" y="7" width="2" height="1" fill="var(--main)"/><rect x="3" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="4" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="11" y="8" width="1" height="1" fill="var(--main)"/><rect x="3" y="9" width="1" height="1" fill="var(--main)"/><rect x="3" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="4" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="4" y="11" width="1" height="1" fill="var(--main)"/><rect x="4" y="12" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="5" y="12" width="6" height="1" fill="var(--main)"/></g></g>"#,
    ),
    (
        "redo",
        24,
        r#"<g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="13" y="4" width="1" height="1" fill="var(--main)"/><rect x="13" y="5" width="2" height="1" fill="var(--main)"/><rect x="13" y="6" width="3" height="1" fill="var(--main)"/><rect x="6" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="7" y="7" width="10" height="1" fill="var(--main)"/><rect x="5" y="8" width="1" height="1" fill="var(--main)"/><rect x="6" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="13" y="8" width="3" height="1" fill="var(--main)"/><rect x="4" y="9" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="5" y="9" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="13" y="9" width="2" height="1" fill="var(--main)"/><rect x="4" y="10" width="1" height="1" fill="var(--main)"/><rect x="5" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="13" y="10" width="1" height="1" fill="var(--main)"/><rect x="4" y="11" width="1" height="1" fill="var(--main)"/><rect x="4" y="12" width="1" height="1" fill="var(--main)"/><rect x="5" y="12" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="4" y="13" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="5" y="13" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="5" y="14" width="1" height="1" fill="var(--main)"/><rect x="6" y="14" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="6" y="15" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="7" y="15" width="6" height="1" fill="var(--main)"/></g></g>"#,
    ),
    (
        "redo",
        30,
        r#"<path class="s1" stroke-width="2" stroke-linecap="round" d="M21.5 11 H13 A5 5 0 0 0 13 21 H20"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M18.5 7.5 L22 11 L18.5 14.5"/>"#,
    ),
    (
        "redo",
        36,
        r#"<path class="s1" stroke-width="2" stroke-linecap="round" d="M26.5 13 H15.5 A6 6 0 0 0 15.5 25 H24.5"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M23 9 L27 13 L23 17"/>"#,
    ),
    (
        "draw",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(4,4)"><rect x="6" y="0" width="1" height="1" fill="var(--main)"/><rect x="5" y="1" width="1" height="1" fill="var(--main)"/><rect x="7" y="1" width="1" height="1" fill="var(--main)"/><rect x="4" y="2" width="1" height="1" fill="var(--main)"/><rect x="8" y="2" width="1" height="1" fill="var(--main)"/><rect x="3" y="3" width="1" height="1" fill="var(--main)"/><rect x="7" y="3" width="1" height="1" fill="var(--main)"/><rect x="2" y="4" width="1" height="1" fill="var(--main)"/><rect x="6" y="4" width="1" height="1" fill="var(--main)"/><rect x="2" y="5" width="1" height="1" fill="var(--second)"/><rect x="3" y="5" width="1" height="1" fill="var(--main)"/><rect x="5" y="5" width="1" height="1" fill="var(--main)"/><rect x="2" y="6" width="2" height="1" fill="var(--second)"/><rect x="4" y="6" width="1" height="1" fill="var(--main)"/><rect x="0" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="1" y="7" width="1" height="1" fill="var(--main)"/><rect x="1" y="8" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/></g>"#,
    ),
    (
        "draw",
        20,
        r#"<g transform="translate(2,3)"><g shape-rendering="crispEdges"><rect x="11" y="2" width="1" height="1" fill="var(--main)"/><rect x="10" y="3" width="1" height="1" fill="var(--main)"/><rect x="12" y="3" width="1" height="1" fill="var(--main)"/><rect x="9" y="4" width="1" height="1" fill="var(--main)"/><rect x="13" y="4" width="1" height="1" fill="var(--main)"/><rect x="8" y="5" width="1" height="1" fill="var(--main)"/><rect x="12" y="5" width="1" height="1" fill="var(--main)"/><rect x="7" y="6" width="1" height="1" fill="var(--main)"/><rect x="11" y="6" width="1" height="1" fill="var(--main)"/><rect x="6" y="7" width="1" height="1" fill="var(--main)"/><rect x="10" y="7" width="1" height="1" fill="var(--main)"/><rect x="5" y="8" width="1" height="1" fill="var(--main)"/><rect x="9" y="8" width="1" height="1" fill="var(--main)"/><rect x="5" y="9" width="1" height="1" fill="var(--second)"/><rect x="6" y="9" width="1" height="1" fill="var(--main)"/><rect x="8" y="9" width="1" height="1" fill="var(--main)"/><rect x="4" y="10" width="3" height="1" fill="var(--second)"/><rect x="7" y="10" width="1" height="1" fill="var(--main)"/><rect x="3" y="11" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="4" y="11" width="2" height="1" fill="var(--second)"/><rect x="3" y="12" width="1" height="1" fill="var(--main)"/><rect x="4" y="12" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g></g>"#,
    ),
    (
        "draw",
        24,
        r#"<g transform="translate(2,3)"><g transform="translate(-1,1)"><g shape-rendering="crispEdges"><rect x="14" y="2" width="1" height="1" fill="var(--main)"/><rect x="13" y="3" width="1" height="1" fill="var(--main)"/><rect x="15" y="3" width="1" height="1" fill="var(--main)"/><rect x="12" y="4" width="1" height="1" fill="var(--main)"/><rect x="16" y="4" width="1" height="1" fill="var(--main)"/><rect x="11" y="5" width="1" height="1" fill="var(--main)"/><rect x="17" y="5" width="1" height="1" fill="var(--main)"/><rect x="10" y="6" width="1" height="1" fill="var(--main)"/><rect x="16" y="6" width="1" height="1" fill="var(--main)"/><rect x="9" y="7" width="1" height="1" fill="var(--main)"/><rect x="15" y="7" width="1" height="1" fill="var(--main)"/><rect x="8" y="8" width="1" height="1" fill="var(--main)"/><rect x="14" y="8" width="1" height="1" fill="var(--main)"/><rect x="7" y="9" width="1" height="1" fill="var(--main)"/><rect x="13" y="9" width="1" height="1" fill="var(--main)"/><rect x="7" y="10" width="1" height="1" fill="var(--second)"/><rect x="8" y="10" width="1" height="1" fill="var(--main)"/><rect x="12" y="10" width="1" height="1" fill="var(--main)"/><rect x="7" y="11" width="2" height="1" fill="var(--second)"/><rect x="9" y="11" width="1" height="1" fill="var(--main)"/><rect x="11" y="11" width="1" height="1" fill="var(--main)"/><rect x="6" y="12" width="4" height="1" fill="var(--second)"/><rect x="10" y="12" width="1" height="1" fill="var(--main)"/><rect x="5" y="13" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/><rect x="6" y="13" width="1" height="1" fill="var(--main)"/><rect x="7" y="13" width="1" height="1" fill="var(--second)"/><rect x="5" y="14" width="1" height="1" fill="var(--main)"/><rect x="6" y="14" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g></g></g>"#,
    ),
    (
        "draw",
        30,
        r#"<g transform="translate(3,3)"><g transform="rotate(-45 12 12)"><rect class="s1" stroke-width="1.5" x="9" y="9" width="12" height="6" rx="0.5"/><path class="f2" d="M7.25 9 V15 L2.5 12 Z"/><path class="f1" d="M2.5 12 L4.2 11 V13 Z"/></g></g>"#,
    ),
    (
        "draw",
        36,
        r#"<g transform="translate(2,2)"><g transform="rotate(-45 15.5 15.5)"><rect class="s1" stroke-width="2" x="11.5" y="12" width="15" height="7" rx="0.75"/><path class="f2" d="M9.5 12 V19 L3.5 15.5 Z"/><path class="f1" d="M3.5 15.5 L5.7 14.2 V16.8 Z"/></g></g>"#,
    ),
    (
        "select",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(5,4)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="4" height="1" fill="var(--main)"/><rect x="0" y="4" width="5" height="1" fill="var(--main)"/><rect x="0" y="5" width="6" height="1" fill="var(--main)"/><rect x="0" y="6" width="2" height="1" fill="var(--main)"/><rect x="0" y="7" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "select",
        20,
        r#"<g shape-rendering="crispEdges" transform="translate(7,5)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="4" height="1" fill="var(--main)"/><rect x="0" y="4" width="5" height="1" fill="var(--main)"/><rect x="0" y="5" width="6" height="1" fill="var(--main)"/><rect x="0" y="6" width="7" height="1" fill="var(--main)"/><rect x="0" y="7" width="3" height="1" fill="var(--main)"/><rect x="0" y="8" width="2" height="1" fill="var(--main)"/><rect x="0" y="9" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "select",
        24,
        r#"<g shape-rendering="crispEdges" transform="translate(8,6)"><rect x="0" y="0" width="1" height="1" fill="var(--main)"/><rect x="0" y="1" width="2" height="1" fill="var(--main)"/><rect x="0" y="2" width="3" height="1" fill="var(--main)"/><rect x="0" y="3" width="4" height="1" fill="var(--main)"/><rect x="0" y="4" width="5" height="1" fill="var(--main)"/><rect x="0" y="5" width="6" height="1" fill="var(--main)"/><rect x="0" y="6" width="7" height="1" fill="var(--main)"/><rect x="0" y="7" width="8" height="1" fill="var(--main)"/><rect x="0" y="8" width="3" height="1" fill="var(--main)"/><rect x="0" y="9" width="2" height="1" fill="var(--main)"/><rect x="0" y="10" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "select",
        30,
        r#"<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M10.5 8.5 V21.5 L13.7 18.3 H19.5 Z"/>"#,
    ),
    (
        "select",
        36,
        r#"<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M12.5 9.5 V25.5 L16.4 21.6 H23.5 Z"/>"#,
    ),
    (
        "slice",
        16,
        r#"<g class="f1" shape-rendering="crispEdges"><rect x="6" y="6" width="1" height="1"/><rect x="7" y="7" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="9" y="9" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="12" y="12" width="1" height="1"/><rect x="6" y="9" width="1" height="1"/><rect x="7" y="8" width="1" height="1"/><rect x="8" y="7" width="1" height="1"/><rect x="9" y="6" width="1" height="1"/><rect x="10" y="5" width="1" height="1"/><rect x="11" y="4" width="1" height="1"/><rect x="12" y="3" width="1" height="1"/></g><g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="0" y="0" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="1" y="0" width="1" height="1" fill="var(--main)"/><rect x="2" y="0" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="0" y="1" width="1" height="1" fill="var(--main)"/><rect x="2" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="1" y="2" width="1" height="1" fill="var(--main)"/><rect x="2" y="2" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/></g><g shape-rendering="crispEdges" transform="translate(3,10)"><rect x="0" y="0" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="1" y="0" width="1" height="1" fill="var(--main)"/><rect x="2" y="0" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="0" y="1" width="1" height="1" fill="var(--main)"/><rect x="2" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="1" y="2" width="1" height="1" fill="var(--main)"/><rect x="2" y="2" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/></g>"#,
    ),
    (
        "slice",
        20,
        r#"<g class="f1" shape-rendering="crispEdges"><rect x="7" y="7" width="1" height="1"/><rect x="8" y="8" width="1" height="1"/><rect x="9" y="9" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="12" y="12" width="1" height="1"/><rect x="13" y="13" width="1" height="1"/><rect x="14" y="14" width="1" height="1"/><rect x="15" y="15" width="1" height="1"/><rect x="7" y="12" width="1" height="1"/><rect x="8" y="11" width="1" height="1"/><rect x="9" y="10" width="1" height="1"/><rect x="10" y="9" width="1" height="1"/><rect x="11" y="8" width="1" height="1"/><rect x="12" y="7" width="1" height="1"/><rect x="13" y="6" width="1" height="1"/><rect x="14" y="5" width="1" height="1"/><rect x="15" y="4" width="1" height="1"/></g><g shape-rendering="crispEdges" transform="translate(4,4)"><rect x="1" y="0" width="2" height="1" fill="var(--main)"/><rect x="0" y="1" width="1" height="1" fill="var(--main)"/><rect x="3" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="3" y="2" width="1" height="1" fill="var(--main)"/><rect x="1" y="3" width="2" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(4,12)"><rect x="1" y="0" width="2" height="1" fill="var(--main)"/><rect x="0" y="1" width="1" height="1" fill="var(--main)"/><rect x="3" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="3" y="2" width="1" height="1" fill="var(--main)"/><rect x="1" y="3" width="2" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "slice",
        24,
        r#"<g class="f1" shape-rendering="crispEdges"><rect x="9" y="9" width="1" height="1"/><rect x="10" y="10" width="1" height="1"/><rect x="11" y="11" width="1" height="1"/><rect x="12" y="12" width="1" height="1"/><rect x="13" y="13" width="1" height="1"/><rect x="14" y="14" width="1" height="1"/><rect x="15" y="15" width="1" height="1"/><rect x="16" y="16" width="1" height="1"/><rect x="17" y="17" width="1" height="1"/><rect x="18" y="18" width="1" height="1"/><rect x="9" y="14" width="1" height="1"/><rect x="10" y="13" width="1" height="1"/><rect x="11" y="12" width="1" height="1"/><rect x="12" y="11" width="1" height="1"/><rect x="13" y="10" width="1" height="1"/><rect x="14" y="9" width="1" height="1"/><rect x="15" y="8" width="1" height="1"/><rect x="16" y="7" width="1" height="1"/><rect x="17" y="6" width="1" height="1"/><rect x="18" y="5" width="1" height="1"/></g><g shape-rendering="crispEdges" transform="translate(5,5)"><rect x="1" y="0" width="3" height="1" fill="var(--main)"/><rect x="0" y="1" width="1" height="1" fill="var(--main)"/><rect x="4" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="4" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="4" y="3" width="1" height="1" fill="var(--main)"/><rect x="1" y="4" width="3" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(5,14)"><rect x="1" y="0" width="3" height="1" fill="var(--main)"/><rect x="0" y="1" width="1" height="1" fill="var(--main)"/><rect x="4" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="4" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="4" y="3" width="1" height="1" fill="var(--main)"/><rect x="1" y="4" width="3" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "slice",
        30,
        r#"<circle class="s1" stroke-width="2" cx="10" cy="10" r="3"/><circle class="s1" stroke-width="2" cx="10" cy="20" r="3"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M12.4 12.2 L24 21.5 M12.4 17.8 L24 8.5"/>"#,
    ),
    (
        "slice",
        36,
        r#"<circle class="s1" stroke-width="2" cx="11.5" cy="12" r="3.5"/><circle class="s1" stroke-width="2" cx="11.5" cy="24" r="3.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M14.3 14.2 L28.5 25.5 M14.3 21.8 L28.5 10.5"/>"#,
    ),
    (
        "erase",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(4,4)"><rect x="5" y="0" width="1" height="1" fill="var(--second)"/><rect x="4" y="1" width="1" height="1" fill="var(--second)"/><rect x="6" y="1" width="1" height="1" fill="var(--second)"/><rect x="3" y="2" width="1" height="1" fill="var(--second)"/><rect x="7" y="2" width="1" height="1" fill="var(--second)"/><rect x="2" y="3" width="2" height="1" fill="var(--main)"/><rect x="8" y="3" width="1" height="1" fill="var(--second)"/><rect x="1" y="4" width="4" height="1" fill="var(--main)"/><rect x="7" y="4" width="1" height="1" fill="var(--second)"/><rect x="0" y="5" width="6" height="1" fill="var(--main)"/><rect x="6" y="5" width="1" height="1" fill="var(--second)"/><rect x="1" y="6" width="5" height="1" fill="var(--main)"/><rect x="2" y="7" width="3" height="1" fill="var(--main)"/><rect x="3" y="8" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "erase",
        20,
        r#"<g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="9" y="2" width="1" height="1" fill="var(--second)"/><rect x="8" y="3" width="1" height="1" fill="var(--second)"/><rect x="10" y="3" width="1" height="1" fill="var(--second)"/><rect x="7" y="4" width="1" height="1" fill="var(--second)"/><rect x="11" y="4" width="1" height="1" fill="var(--second)"/><rect x="6" y="5" width="1" height="1" fill="var(--main)"/><rect x="12" y="5" width="1" height="1" fill="var(--second)"/><rect x="5" y="6" width="3" height="1" fill="var(--main)"/><rect x="13" y="6" width="1" height="1" fill="var(--second)"/><rect x="4" y="7" width="5" height="1" fill="var(--main)"/><rect x="12" y="7" width="1" height="1" fill="var(--second)"/><rect x="3" y="8" width="7" height="1" fill="var(--main)"/><rect x="11" y="8" width="1" height="1" fill="var(--second)"/><rect x="4" y="9" width="7" height="1" fill="var(--main)"/><rect x="5" y="10" width="5" height="1" fill="var(--main)"/><rect x="6" y="11" width="3" height="1" fill="var(--main)"/><rect x="7" y="12" width="1" height="1" fill="var(--main)"/><rect x="9" y="13" width="5" height="1" fill="var(--second)"/></g></g>"#,
    ),
    (
        "erase",
        24,
        r#"<g transform="translate(2,2)"><g transform="translate(-1,0)"><g shape-rendering="crispEdges"><rect x="11" y="3" width="1" height="1" fill="var(--second)"/><rect x="10" y="4" width="1" height="1" fill="var(--second)"/><rect x="12" y="4" width="1" height="1" fill="var(--second)"/><rect x="9" y="5" width="1" height="1" fill="var(--second)"/><rect x="13" y="5" width="1" height="1" fill="var(--second)"/><rect x="8" y="6" width="1" height="1" fill="var(--second)"/><rect x="14" y="6" width="1" height="1" fill="var(--second)"/><rect x="7" y="7" width="2" height="1" fill="var(--main)"/><rect x="15" y="7" width="1" height="1" fill="var(--second)"/><rect x="6" y="8" width="4" height="1" fill="var(--main)"/><rect x="16" y="8" width="1" height="1" fill="var(--second)"/><rect x="5" y="9" width="6" height="1" fill="var(--main)"/><rect x="15" y="9" width="1" height="1" fill="var(--second)"/><rect x="4" y="10" width="8" height="1" fill="var(--main)"/><rect x="14" y="10" width="1" height="1" fill="var(--second)"/><rect x="5" y="11" width="8" height="1" fill="var(--main)"/><rect x="13" y="11" width="1" height="1" fill="var(--second)"/><rect x="6" y="12" width="7" height="1" fill="var(--main)"/><rect x="7" y="13" width="5" height="1" fill="var(--main)"/><rect x="8" y="14" width="3" height="1" fill="var(--main)"/><rect x="9" y="15" width="1" height="1" fill="var(--main)"/><rect x="11" y="16" width="6" height="1" fill="var(--second)"/></g></g></g>"#,
    ),
    (
        "erase",
        30,
        r#"<g transform="translate(3,3)"><g transform="rotate(-45 11.5 11.5)"><rect class="f1" x="4" y="8.5" width="7.5" height="6" rx="1"/><rect class="s2" stroke-width="1.5" x="12.25" y="9.25" width="6.5" height="4.5" rx="0.5"/></g><rect class="f2" x="12" y="19" width="8" height="2"/></g>"#,
    ),
    (
        "erase",
        36,
        r#"<g transform="translate(2,2)"><g transform="rotate(-45 14.5 14.5)"><rect class="f1" x="5" y="11" width="9.5" height="7" rx="1"/><rect class="s2" stroke-width="2" x="15.5" y="12" width="8" height="5" rx="0.5"/></g><rect class="f2" x="15" y="24" width="10" height="2"/></g>"#,
    ),
    (
        "muteTool",
        16,
        r#"<rect class="f1" x="3" y="7" width="2" height="3"/><rect class="f1" x="6" y="6" width="1" height="5"/><rect class="f1" x="7" y="5" width="1" height="7"/><g shape-rendering="crispEdges" transform="translate(9,6)"><rect x="0" y="0" width="1" height="1" fill="var(--second)"/><rect x="4" y="0" width="1" height="1" fill="var(--second)"/><rect x="1" y="1" width="1" height="1" fill="var(--second)"/><rect x="3" y="1" width="1" height="1" fill="var(--second)"/><rect x="2" y="2" width="1" height="1" fill="var(--second)"/><rect x="1" y="3" width="1" height="1" fill="var(--second)"/><rect x="3" y="3" width="1" height="1" fill="var(--second)"/><rect x="0" y="4" width="1" height="1" fill="var(--second)"/><rect x="4" y="4" width="1" height="1" fill="var(--second)"/></g>"#,
    ),
    (
        "muteTool",
        20,
        r#"<g transform="translate(2,3)"><g shape-rendering="crispEdges"><rect x="7" y="3" width="1" height="1" fill="var(--main)"/><rect x="6" y="4" width="2" height="1" fill="var(--main)"/><rect x="5" y="5" width="3" height="1" fill="var(--main)"/><rect x="9" y="5" width="1" height="1" fill="var(--second)"/><rect x="13" y="5" width="1" height="1" fill="var(--second)"/><rect x="2" y="6" width="2" height="1" fill="var(--main)"/><rect x="5" y="6" width="3" height="1" fill="var(--main)"/><rect x="10" y="6" width="1" height="1" fill="var(--second)"/><rect x="12" y="6" width="1" height="1" fill="var(--second)"/><rect x="2" y="7" width="2" height="1" fill="var(--main)"/><rect x="5" y="7" width="3" height="1" fill="var(--main)"/><rect x="11" y="7" width="1" height="1" fill="var(--second)"/><rect x="2" y="8" width="2" height="1" fill="var(--main)"/><rect x="5" y="8" width="3" height="1" fill="var(--main)"/><rect x="10" y="8" width="1" height="1" fill="var(--second)"/><rect x="12" y="8" width="1" height="1" fill="var(--second)"/><rect x="5" y="9" width="3" height="1" fill="var(--main)"/><rect x="9" y="9" width="1" height="1" fill="var(--second)"/><rect x="13" y="9" width="1" height="1" fill="var(--second)"/><rect x="6" y="10" width="2" height="1" fill="var(--main)"/><rect x="7" y="11" width="1" height="1" fill="var(--main)"/></g></g>"#,
    ),
    (
        "muteTool",
        24,
        r#"<g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="9" y="4" width="1" height="1" fill="var(--main)"/><rect x="8" y="5" width="2" height="1" fill="var(--main)"/><rect x="7" y="6" width="3" height="1" fill="var(--main)"/><rect x="3" y="7" width="3" height="1" fill="var(--main)"/><rect x="7" y="7" width="3" height="1" fill="var(--main)"/><rect x="12" y="7" width="1" height="1" fill="var(--second)"/><rect x="17" y="7" width="1" height="1" fill="var(--second)"/><rect x="3" y="8" width="3" height="1" fill="var(--main)"/><rect x="7" y="8" width="3" height="1" fill="var(--main)"/><rect x="13" y="8" width="1" height="1" fill="var(--second)"/><rect x="16" y="8" width="1" height="1" fill="var(--second)"/><rect x="3" y="9" width="3" height="1" fill="var(--main)"/><rect x="7" y="9" width="3" height="1" fill="var(--main)"/><rect x="14" y="9" width="2" height="1" fill="var(--second)"/><rect x="3" y="10" width="3" height="1" fill="var(--main)"/><rect x="7" y="10" width="3" height="1" fill="var(--main)"/><rect x="14" y="10" width="2" height="1" fill="var(--second)"/><rect x="3" y="11" width="3" height="1" fill="var(--main)"/><rect x="7" y="11" width="3" height="1" fill="var(--main)"/><rect x="13" y="11" width="1" height="1" fill="var(--second)"/><rect x="16" y="11" width="1" height="1" fill="var(--second)"/><rect x="3" y="12" width="3" height="1" fill="var(--main)"/><rect x="7" y="12" width="3" height="1" fill="var(--main)"/><rect x="12" y="12" width="1" height="1" fill="var(--second)"/><rect x="17" y="12" width="1" height="1" fill="var(--second)"/><rect x="7" y="13" width="3" height="1" fill="var(--main)"/><rect x="8" y="14" width="2" height="1" fill="var(--main)"/><rect x="9" y="15" width="1" height="1" fill="var(--main)"/></g></g>"#,
    ),
    (
        "muteTool",
        30,
        r#"<g transform="translate(3,3)"><rect class="f1" x="3" y="9" width="4" height="6"/><path class="f1" d="M8 9 L12 5 V19 L8 15 Z"/><path class="s2" stroke-width="2" d="M15 9 L21 15 M21 9 L15 15"/></g>"#,
    ),
    (
        "muteTool",
        36,
        r#"<g transform="translate(2,2)"><rect class="f1" x="4" y="12" width="5" height="7"/><path class="f1" d="M10 12 L15 6 V25 L10 19 Z"/><path class="s2" stroke-width="2" d="M18 12 L25 19 M25 12 L18 19"/></g>"#,
    ),
    (
        "zoom",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="2" y="0" width="3" height="1" fill="var(--main)"/><rect x="1" y="1" width="1" height="1" fill="var(--main)"/><rect x="5" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="1" height="1" fill="var(--main)"/><rect x="6" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="1" height="1" fill="var(--main)"/><rect x="6" y="3" width="1" height="1" fill="var(--main)"/><rect x="0" y="4" width="1" height="1" fill="var(--main)"/><rect x="6" y="4" width="1" height="1" fill="var(--main)"/><rect x="1" y="5" width="1" height="1" fill="var(--main)"/><rect x="5" y="5" width="1" height="1" fill="var(--main)"/><rect x="2" y="6" width="3" height="1" fill="var(--main)"/></g><g shape-rendering="crispEdges" transform="translate(9,9)"><rect x="0" y="0" width="2" height="1" fill="var(--main)"/><rect x="1" y="1" width="2" height="1" fill="var(--main)"/><rect x="2" y="2" width="2" height="1" fill="var(--main)"/><rect x="3" y="3" width="1" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "zoom",
        20,
        r#"<g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="5" y="2" width="3" height="1" fill="var(--main)"/><rect x="3" y="3" width="2" height="1" fill="var(--main)"/><rect x="8" y="3" width="2" height="1" fill="var(--main)"/><rect x="3" y="4" width="1" height="1" fill="var(--main)"/><rect x="5" y="4" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="9" y="4" width="1" height="1" fill="var(--main)"/><rect x="2" y="5" width="1" height="1" fill="var(--main)"/><rect x="4" y="5" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="10" y="5" width="1" height="1" fill="var(--main)"/><rect x="2" y="6" width="1" height="1" fill="var(--main)"/><rect x="10" y="6" width="1" height="1" fill="var(--main)"/><rect x="2" y="7" width="1" height="1" fill="var(--main)"/><rect x="10" y="7" width="1" height="1" fill="var(--main)"/><rect x="3" y="8" width="1" height="1" fill="var(--main)"/><rect x="9" y="8" width="1" height="1" fill="var(--main)"/><rect x="3" y="9" width="2" height="1" fill="var(--main)"/><rect x="8" y="9" width="2" height="1" fill="var(--main)"/><rect x="5" y="10" width="3" height="1" fill="var(--main)"/><rect x="9" y="10" width="2" height="1" fill="var(--main)"/><rect x="10" y="11" width="2" height="1" fill="var(--main)"/><rect x="11" y="12" width="2" height="1" fill="var(--main)"/><rect x="12" y="13" width="2" height="1" fill="var(--main)"/></g></g>"#,
    ),
    (
        "zoom",
        24,
        r#"<g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="7" y="3" width="3" height="1" fill="var(--main)"/><rect x="5" y="4" width="2" height="1" fill="var(--main)"/><rect x="10" y="4" width="2" height="1" fill="var(--main)"/><rect x="4" y="5" width="1" height="1" fill="var(--main)"/><rect x="12" y="5" width="1" height="1" fill="var(--main)"/><rect x="4" y="6" width="1" height="1" fill="var(--main)"/><rect x="6" y="6" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="12" y="6" width="1" height="1" fill="var(--main)"/><rect x="3" y="7" width="1" height="1" fill="var(--main)"/><rect x="5" y="7" width="1" height="1" fill="var(--second)" fill-opacity="0.55"/><rect x="13" y="7" width="1" height="1" fill="var(--main)"/><rect x="3" y="8" width="1" height="1" fill="var(--main)"/><rect x="13" y="8" width="1" height="1" fill="var(--main)"/><rect x="3" y="9" width="1" height="1" fill="var(--main)"/><rect x="13" y="9" width="1" height="1" fill="var(--main)"/><rect x="4" y="10" width="1" height="1" fill="var(--main)"/><rect x="12" y="10" width="1" height="1" fill="var(--main)"/><rect x="4" y="11" width="1" height="1" fill="var(--main)"/><rect x="12" y="11" width="1" height="1" fill="var(--main)"/><rect x="5" y="12" width="2" height="1" fill="var(--main)"/><rect x="10" y="12" width="2" height="1" fill="var(--main)"/><rect x="7" y="13" width="3" height="1" fill="var(--main)"/><rect x="12" y="13" width="2" height="1" fill="var(--main)"/><rect x="13" y="14" width="2" height="1" fill="var(--main)"/><rect x="14" y="15" width="2" height="1" fill="var(--main)"/><rect x="15" y="16" width="2" height="1" fill="var(--main)"/></g></g>"#,
    ),
    (
        "zoom",
        30,
        r#"<g transform="translate(3,3)"><circle class="s1" stroke-width="2" cx="10" cy="10" r="6"/><path class="s2" stroke-width="1" d="M6.5 10 A3.5 3.5 0 0 1 10 6.5"/><path class="s1" stroke-width="3" d="M14.5 14.5 L20 20"/></g>"#,
    ),
    (
        "zoom",
        36,
        r#"<g transform="translate(2,2)"><circle class="s1" stroke-width="2" cx="12.5" cy="12.5" r="7.5"/><path class="s2" stroke-width="1.5" d="M8 12.5 A4.5 4.5 0 0 1 12.5 8"/><path class="s1" stroke-width="3.5" d="M18 18 L25 25"/></g>"#,
    ),
    (
        "pick",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(4,3)"><rect x="7" y="0" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="8" y="0" width="1" height="1" fill="var(--main)"/><rect x="9" y="0" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="7" y="1" width="3" height="1" fill="var(--main)"/><rect x="7" y="2" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="8" y="2" width="1" height="1" fill="var(--main)"/><rect x="9" y="2" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="5" y="3" width="1" height="1" fill="var(--main)"/><rect x="6" y="4" width="1" height="1" fill="var(--main)"/><rect x="5" y="5" width="1" height="1" fill="var(--second)"/><rect x="7" y="5" width="1" height="1" fill="var(--main)"/><rect x="4" y="6" width="1" height="1" fill="var(--second)"/><rect x="3" y="7" width="1" height="1" fill="var(--second)"/><rect x="2" y="8" width="1" height="1" fill="var(--second)"/><rect x="1" y="9" width="1" height="1" fill="var(--main)"/><rect x="0" y="10" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g>"#,
    ),
    (
        "pick",
        20,
        r#"<g transform="translate(2,3)"><g shape-rendering="crispEdges"><rect x="11" y="2" width="2" height="1" fill="var(--main)"/><rect x="10" y="3" width="4" height="1" fill="var(--main)"/><rect x="10" y="4" width="4" height="1" fill="var(--main)"/><rect x="8" y="5" width="1" height="1" fill="var(--main)"/><rect x="11" y="5" width="2" height="1" fill="var(--main)"/><rect x="9" y="6" width="1" height="1" fill="var(--main)"/><rect x="7" y="7" width="2" height="1" fill="var(--second)"/><rect x="10" y="7" width="1" height="1" fill="var(--main)"/><rect x="6" y="8" width="2" height="1" fill="var(--second)"/><rect x="5" y="9" width="2" height="1" fill="var(--second)"/><rect x="4" y="10" width="2" height="1" fill="var(--second)"/><rect x="3" y="11" width="2" height="1" fill="var(--second)"/><rect x="2" y="12" width="1" height="1" fill="var(--main)"/><rect x="3" y="12" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g></g>"#,
    ),
    (
        "pick",
        24,
        r#"<g transform="translate(2,3)"><g shape-rendering="crispEdges"><rect x="14" y="3" width="2" height="1" fill="var(--main)"/><rect x="13" y="4" width="4" height="1" fill="var(--main)"/><rect x="13" y="5" width="4" height="1" fill="var(--main)"/><rect x="10" y="6" width="1" height="1" fill="var(--main)"/><rect x="14" y="6" width="2" height="1" fill="var(--main)"/><rect x="11" y="7" width="1" height="1" fill="var(--main)"/><rect x="12" y="8" width="1" height="1" fill="var(--main)"/><rect x="9" y="9" width="3" height="1" fill="var(--second)"/><rect x="13" y="9" width="1" height="1" fill="var(--main)"/><rect x="8" y="10" width="3" height="1" fill="var(--second)"/><rect x="7" y="11" width="3" height="1" fill="var(--second)"/><rect x="6" y="12" width="3" height="1" fill="var(--second)"/><rect x="5" y="13" width="3" height="1" fill="var(--second)"/><rect x="4" y="14" width="3" height="1" fill="var(--second)"/><rect x="3" y="15" width="1" height="1" fill="var(--main)"/><rect x="4" y="15" width="1" height="1" fill="var(--main)" fill-opacity="0.6"/></g></g>"#,
    ),
    (
        "pick",
        30,
        r#"<g transform="translate(3,3)"><g transform="rotate(-45 12 12)"><rect class="f1" x="15.5" y="9" width="6" height="6" rx="2.5"/><rect class="f1" x="13" y="8" width="1.5" height="8"/><rect class="f2" x="5" y="10.75" width="7" height="2.5"/><path class="f1" d="M2.5 12 L5 10.75 V13.25 Z"/></g></g>"#,
    ),
    (
        "pick",
        36,
        r#"<g transform="translate(2,2)"><g transform="rotate(-45 15.5 15.5)"><rect class="f1" x="19.5" y="12" width="7.5" height="7" rx="3"/><rect class="f1" x="16.5" y="10.5" width="2" height="10"/><rect class="f2" x="6.5" y="14" width="9" height="3"/><path class="f1" d="M3 15.5 L6.5 14 V17 Z"/></g></g>"#,
    ),
    (
        "gear",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(3,3)"><rect x="4" y="0" width="2" height="1" fill="var(--main)"/><rect x="3" y="1" width="4" height="1" fill="var(--main)"/><rect x="1" y="2" width="8" height="1" fill="var(--main)"/><rect x="1" y="3" width="3" height="1" fill="var(--main)"/><rect x="6" y="3" width="3" height="1" fill="var(--main)"/><rect x="0" y="4" width="3" height="1" fill="var(--main)"/><rect x="7" y="4" width="3" height="1" fill="var(--main)"/><rect x="0" y="5" width="3" height="1" fill="var(--main)"/><rect x="7" y="5" width="3" height="1" fill="var(--main)"/><rect x="1" y="6" width="3" height="1" fill="var(--main)"/><rect x="6" y="6" width="3" height="1" fill="var(--main)"/><rect x="1" y="7" width="8" height="1" fill="var(--main)"/><rect x="3" y="8" width="4" height="1" fill="var(--main)"/><rect x="4" y="9" width="2" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "gear",
        20,
        r#"<g transform="translate(2,2)"><g transform="translate(2,2)"><g shape-rendering="crispEdges"><rect x="5" y="0" width="2" height="1" fill="var(--main)"/><rect x="2" y="1" width="1" height="1" fill="var(--main)"/><rect x="5" y="1" width="2" height="1" fill="var(--main)"/><rect x="9" y="1" width="1" height="1" fill="var(--main)"/><rect x="1" y="2" width="10" height="1" fill="var(--main)"/><rect x="2" y="3" width="8" height="1" fill="var(--main)"/><rect x="2" y="4" width="2" height="1" fill="var(--main)"/><rect x="4" y="4" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="7" y="4" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="8" y="4" width="2" height="1" fill="var(--main)"/><rect x="0" y="5" width="4" height="1" fill="var(--main)"/><rect x="8" y="5" width="4" height="1" fill="var(--main)"/><rect x="0" y="6" width="4" height="1" fill="var(--main)"/><rect x="8" y="6" width="4" height="1" fill="var(--main)"/><rect x="2" y="7" width="2" height="1" fill="var(--main)"/><rect x="4" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="7" y="7" width="1" height="1" fill="var(--main)" fill-opacity="0.3"/><rect x="8" y="7" width="2" height="1" fill="var(--main)"/><rect x="2" y="8" width="8" height="1" fill="var(--main)"/><rect x="1" y="9" width="10" height="1" fill="var(--main)"/><rect x="2" y="10" width="1" height="1" fill="var(--main)"/><rect x="5" y="10" width="2" height="1" fill="var(--main)"/><rect x="9" y="10" width="1" height="1" fill="var(--main)"/><rect x="5" y="11" width="2" height="1" fill="var(--main)"/></g></g></g>"#,
    ),
    (
        "gear",
        24,
        r#"<g transform="translate(2,2)"><g transform="translate(3,3)"><g shape-rendering="crispEdges"><rect x="5" y="0" width="4" height="1" fill="var(--main)"/><rect x="5" y="1" width="4" height="1" fill="var(--main)"/><rect x="2" y="2" width="2" height="1" fill="var(--main)"/><rect x="5" y="2" width="4" height="1" fill="var(--main)"/><rect x="10" y="2" width="2" height="1" fill="var(--main)"/><rect x="2" y="3" width="10" height="1" fill="var(--main)"/><rect x="3" y="4" width="3" height="1" fill="var(--main)"/><rect x="8" y="4" width="3" height="1" fill="var(--main)"/><rect x="0" y="5" width="5" height="1" fill="var(--main)"/><rect x="9" y="5" width="5" height="1" fill="var(--main)"/><rect x="0" y="6" width="4" height="1" fill="var(--main)"/><rect x="10" y="6" width="4" height="1" fill="var(--main)"/><rect x="0" y="7" width="4" height="1" fill="var(--main)"/><rect x="10" y="7" width="4" height="1" fill="var(--main)"/><rect x="0" y="8" width="5" height="1" fill="var(--main)"/><rect x="9" y="8" width="5" height="1" fill="var(--main)"/><rect x="3" y="9" width="3" height="1" fill="var(--main)"/><rect x="8" y="9" width="3" height="1" fill="var(--main)"/><rect x="2" y="10" width="10" height="1" fill="var(--main)"/><rect x="2" y="11" width="2" height="1" fill="var(--main)"/><rect x="5" y="11" width="4" height="1" fill="var(--main)"/><rect x="10" y="11" width="2" height="1" fill="var(--main)"/><rect x="5" y="12" width="4" height="1" fill="var(--main)"/><rect x="5" y="13" width="4" height="1" fill="var(--main)"/></g></g></g>"#,
    ),
    (
        "gear",
        30,
        r#"<g transform="translate(3,3)"><circle class="s1" stroke-width="3" cx="12" cy="12" r="5"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(0 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(45 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(90 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(135 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(180 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(225 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(270 12 12)"/><rect class="f1" x="10.5" y="2.5" width="3" height="3.5" transform="rotate(315 12 12)"/><circle class="f2" cx="12" cy="12" r="1.5"/></g>"#,
    ),
    (
        "gear",
        36,
        r#"<g transform="translate(2,2)"><circle class="s1" stroke-width="3.5" cx="15.5" cy="15.5" r="6.25"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(0 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(45 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(90 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(135 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(180 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(225 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(270 15.5 15.5)"/><rect class="f1" x="13.5" y="4" width="4" height="4" transform="rotate(315 15.5 15.5)"/><circle class="f2" cx="15.5" cy="15.5" r="2"/></g>"#,
    ),
    (
        "arrangement",
        16,
        r#"<rect class="f1" x="3" y="4" width="7" height="2" rx="0.5"/><rect class="f1" x="6" y="7" width="7" height="2" rx="0.5"/><rect class="f1" x="3" y="10" width="5" height="2" rx="0.5"/>"#,
    ),
    (
        "arrangement",
        20,
        r#"<rect class="f1" x="4" y="5" width="8" height="2" rx="1"/><rect class="f1" x="8" y="9" width="8" height="2" rx="1"/><rect class="f1" x="4" y="13" width="5" height="2" rx="1"/>"#,
    ),
    (
        "arrangement",
        24,
        r#"<rect class="f1" x="5" y="6" width="9" height="2" rx="1"/><rect class="f1" x="10" y="11" width="9" height="2" rx="1"/><rect class="f1" x="5" y="16" width="6" height="2" rx="1"/>"#,
    ),
    (
        "arrangement",
        30,
        r#"<rect class="f1" x="7" y="8" width="10" height="3" rx="1.5"/><rect class="f1" x="12" y="14" width="11" height="3" rx="1.5"/><rect class="f1" x="7" y="20" width="7" height="3" rx="1.5"/>"#,
    ),
    (
        "arrangement",
        36,
        r#"<rect class="f1" x="7" y="10" width="13" height="4" rx="2"/><rect class="f1" x="14" y="16" width="15" height="4" rx="2"/><rect class="f1" x="7" y="22" width="9" height="4" rx="2"/>"#,
    ),
    (
        "pianoRoll",
        16,
        r#"<path class="s1" stroke-width="1" d="M3.5 4.5 H12.5 V11.5 H3.5 Z"/><rect class="f1" x="6" y="8" width="1" height="3"/><rect class="f1" x="9" y="8" width="1" height="3"/><rect class="f1" x="5" y="5" width="2" height="3"/><rect class="f1" x="9" y="5" width="2" height="3"/>"#,
    ),
    (
        "pianoRoll",
        20,
        r#"<path class="s1" stroke-width="1" d="M3.5 5.5 H15.5 V14.5 H3.5 Z"/><rect class="f1" x="7" y="10" width="1" height="4"/><rect class="f1" x="11" y="10" width="1" height="4"/><rect class="f1" x="6" y="6" width="3" height="4"/><rect class="f1" x="10" y="6" width="3" height="4"/>"#,
    ),
    (
        "pianoRoll",
        24,
        r#"<path class="s1" stroke-width="1" d="M4.5 6.5 H19.5 V17.5 H4.5 Z"/><rect class="f1" x="9" y="12" width="1" height="5"/><rect class="f1" x="14" y="12" width="1" height="5"/><rect class="f1" x="8" y="7" width="3" height="5"/><rect class="f1" x="13" y="7" width="3" height="5"/>"#,
    ),
    (
        "pianoRoll",
        30,
        r#"<path class="s1" stroke-width="2" stroke-linejoin="round" d="M6 7 H24 V23 H6 Z"/><rect class="f1" x="11" y="16" width="2" height="7"/><rect class="f1" x="17" y="16" width="2" height="7"/><rect class="f1" x="10" y="7" width="4" height="9"/><rect class="f1" x="16" y="7" width="4" height="9"/>"#,
    ),
    (
        "pianoRoll",
        36,
        r#"<path class="s1" stroke-width="2" stroke-linejoin="round" d="M6 8 H30 V28 H6 Z"/><rect class="f1" x="13" y="18" width="2" height="10"/><rect class="f1" x="21" y="18" width="2" height="10"/><rect class="f1" x="12" y="8" width="4" height="10"/><rect class="f1" x="20" y="8" width="4" height="10"/>"#,
    ),
    (
        "rack",
        16,
        r#"<rect class="f1" x="2" y="6" width="2" height="1"/><rect class="f1" x="5" y="5" width="2" height="2"/><rect class="f2" x="8" y="5" width="2" height="2"/><rect class="f1" x="11" y="5" width="2" height="2"/><rect class="f1" x="2" y="10" width="2" height="1"/><rect class="f2" x="5" y="9" width="2" height="2"/><rect class="f1" x="8" y="9" width="2" height="2"/><rect class="f2" x="11" y="9" width="2" height="2"/>"#,
    ),
    (
        "rack",
        20,
        r#"<rect class="f1" x="3" y="7" width="2" height="1"/><rect class="f1" x="6" y="6" width="3" height="3"/><rect class="f2" x="10" y="6" width="3" height="3"/><rect class="f1" x="14" y="6" width="3" height="3"/><rect class="f1" x="3" y="12" width="2" height="1"/><rect class="f2" x="6" y="11" width="3" height="3"/><rect class="f1" x="10" y="11" width="3" height="3"/><rect class="f2" x="14" y="11" width="3" height="3"/>"#,
    ),
    (
        "rack",
        24,
        r#"<rect class="f1" x="5" y="8" width="3" height="1" rx="0.5"/><rect class="f1" x="9" y="7" width="3" height="3" rx="0.5"/><rect class="f2" x="13" y="7" width="3" height="3" rx="0.5"/><rect class="f1" x="17" y="7" width="3" height="3" rx="0.5"/><rect class="f1" x="5" y="15" width="3" height="1" rx="0.5"/><rect class="f2" x="9" y="14" width="3" height="3" rx="0.5"/><rect class="f1" x="13" y="14" width="3" height="3" rx="0.5"/><rect class="f2" x="17" y="14" width="3" height="3" rx="0.5"/>"#,
    ),
    (
        "rack",
        30,
        r#"<rect class="f1" x="6" y="10" width="3" height="2" rx="0.5"/><rect class="f1" x="11" y="9" width="4" height="4" rx="1"/><rect class="f2" x="16" y="9" width="4" height="4" rx="1"/><rect class="f1" x="21" y="9" width="4" height="4" rx="1"/><rect class="f1" x="6" y="18" width="3" height="2" rx="0.5"/><rect class="f2" x="11" y="17" width="4" height="4" rx="1"/><rect class="f1" x="16" y="17" width="4" height="4" rx="1"/><rect class="f2" x="21" y="17" width="4" height="4" rx="1"/>"#,
    ),
    (
        "rack",
        36,
        r#"<rect class="f1" x="7" y="12" width="4" height="2" rx="0.5"/><rect class="f1" x="14" y="11" width="4" height="4" rx="1"/><rect class="f2" x="20" y="11" width="4" height="4" rx="1"/><rect class="f1" x="26" y="11" width="4" height="4" rx="1"/><rect class="f1" x="7" y="22" width="4" height="2" rx="0.5"/><rect class="f2" x="14" y="21" width="4" height="4" rx="1"/><rect class="f1" x="20" y="21" width="4" height="4" rx="1"/><rect class="f2" x="26" y="21" width="4" height="4" rx="1"/>"#,
    ),
    (
        "mixer",
        16,
        r#"<rect class="f2" x="3" y="3" width="1" height="10"/><rect class="f2" x="7" y="3" width="1" height="10"/><rect class="f2" x="11" y="3" width="1" height="10"/><rect class="f1" x="2" y="8" width="3" height="2"/><rect class="f1" x="6" y="4" width="3" height="2"/><rect class="f1" x="10" y="10" width="3" height="2"/>"#,
    ),
    (
        "mixer",
        20,
        r#"<rect class="f2" x="5" y="4" width="1" height="12"/><rect class="f2" x="9" y="4" width="1" height="12"/><rect class="f2" x="13" y="4" width="1" height="12"/><rect class="f1" x="4" y="10" width="3" height="2"/><rect class="f1" x="8" y="6" width="3" height="2"/><rect class="f1" x="12" y="12" width="3" height="2"/>"#,
    ),
    (
        "mixer",
        24,
        r#"<rect class="f2" x="6" y="5" width="1" height="14"/><rect class="f2" x="11" y="5" width="1" height="14"/><rect class="f2" x="16" y="5" width="1" height="14"/><rect class="f1" x="5" y="12" width="3" height="3" rx="0.5"/><rect class="f1" x="10" y="7" width="3" height="3" rx="0.5"/><rect class="f1" x="15" y="14" width="3" height="3" rx="0.5"/>"#,
    ),
    (
        "mixer",
        30,
        r#"<rect class="f2" x="8" y="7" width="2" height="16"/><rect class="f2" x="14" y="7" width="2" height="16"/><rect class="f2" x="20" y="7" width="2" height="16"/><rect class="f1" x="7" y="14" width="4" height="3" rx="1"/><rect class="f1" x="13" y="9" width="4" height="3" rx="1"/><rect class="f1" x="19" y="17" width="4" height="3" rx="1"/>"#,
    ),
    (
        "mixer",
        36,
        r#"<rect class="f2" x="10" y="9" width="2" height="18"/><rect class="f2" x="17" y="9" width="2" height="18"/><rect class="f2" x="24" y="9" width="2" height="18"/><rect class="f1" x="8" y="17" width="6" height="4" rx="1"/><rect class="f1" x="15" y="11" width="6" height="4" rx="1"/><rect class="f1" x="22" y="21" width="6" height="4" rx="1"/>"#,
    ),
    (
        "browser",
        16,
        r#"<path class="s1" stroke-width="1" d="M3.5 4.5 H6.5 L7.5 5.5 H12.5 V11.5 H3.5 Z"/>"#,
    ),
    (
        "browser",
        20,
        r#"<path class="s1" stroke-width="1" d="M4.5 5.5 H8.5 L9.5 6.5 H15.5 V14.5 H4.5 Z"/>"#,
    ),
    (
        "browser",
        24,
        r#"<path class="s1" stroke-width="1" d="M5.5 6.5 H10.5 L11.5 7.5 H18.5 V17.5 H5.5 Z"/><path class="s1" stroke-width="1" d="M5.5 9.5 H18.5"/>"#,
    ),
    (
        "browser",
        30,
        r#"<path class="s1" stroke-width="2" stroke-linejoin="round" d="M7 8 H12 L14 10 H23 V22 H7 Z"/><path class="s1" stroke-width="2" d="M7 13 H23"/>"#,
    ),
    (
        "browser",
        36,
        r#"<path class="s1" stroke-width="2" stroke-linejoin="round" d="M8 10 H15 L17 12.5 H28 V26 H8 Z"/><path class="s1" stroke-width="2" d="M8 16 H28"/>"#,
    ),
    (
        "plugins",
        16,
        r#"<g shape-rendering="crispEdges" transform="translate(4,3)"><rect x="1" y="0" width="1" height="1" fill="var(--main)"/><rect x="6" y="0" width="1" height="1" fill="var(--main)"/><rect x="1" y="1" width="1" height="1" fill="var(--main)"/><rect x="6" y="1" width="1" height="1" fill="var(--main)"/><rect x="0" y="2" width="8" height="1" fill="var(--main)"/><rect x="0" y="3" width="8" height="1" fill="var(--main)"/><rect x="1" y="4" width="6" height="1" fill="var(--main)"/><rect x="2" y="5" width="4" height="1" fill="var(--main)"/><rect x="3" y="6" width="2" height="1" fill="var(--main)"/><rect x="3" y="7" width="2" height="1" fill="var(--main)"/><rect x="3" y="8" width="2" height="1" fill="var(--main)"/><rect x="3" y="9" width="2" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "plugins",
        20,
        r#"<g shape-rendering="crispEdges" transform="translate(5,4)"><rect x="2" y="0" width="1" height="1" fill="var(--main)"/><rect x="7" y="0" width="1" height="1" fill="var(--main)"/><rect x="2" y="1" width="1" height="1" fill="var(--main)"/><rect x="7" y="1" width="1" height="1" fill="var(--main)"/><rect x="2" y="2" width="1" height="1" fill="var(--main)"/><rect x="7" y="2" width="1" height="1" fill="var(--main)"/><rect x="0" y="3" width="10" height="1" fill="var(--main)"/><rect x="0" y="4" width="10" height="1" fill="var(--main)"/><rect x="1" y="5" width="8" height="1" fill="var(--main)"/><rect x="2" y="6" width="6" height="1" fill="var(--main)"/><rect x="3" y="7" width="4" height="1" fill="var(--main)"/><rect x="4" y="8" width="2" height="1" fill="var(--main)"/><rect x="4" y="9" width="2" height="1" fill="var(--main)"/><rect x="4" y="10" width="2" height="1" fill="var(--main)"/><rect x="4" y="11" width="2" height="1" fill="var(--main)"/></g>"#,
    ),
    (
        "plugins",
        24,
        r#"<rect class="f1" x="9" y="5" width="1" height="3"/><rect class="f1" x="14" y="5" width="1" height="3"/><path class="s1" stroke-width="1" d="M6.5 8.5 H17.5 V10 A5.5 5.5 0 0 1 6.5 10 Z"/><rect class="f1" x="11" y="16" width="2" height="3"/>"#,
    ),
    (
        "plugins",
        30,
        r#"<rect class="f1" x="11" y="6" width="2" height="4"/><rect class="f1" x="17" y="6" width="2" height="4"/><path class="s1" stroke-width="2" stroke-linejoin="round" d="M8 11 H22 V13 A7 7 0 0 1 8 13 Z"/><rect class="f1" x="14" y="20" width="2" height="4"/>"#,
    ),
    (
        "plugins",
        36,
        r#"<rect class="f1" x="13" y="7" width="2" height="5"/><rect class="f1" x="21" y="7" width="2" height="5"/><path class="s1" stroke-width="2" stroke-linejoin="round" d="M10 13 H26 V15.5 A8 8 0 0 1 10 15.5 Z"/><rect class="f1" x="17" y="24" width="2" height="5"/>"#,
    ),
    (
        "inspector",
        16,
        r#"<path class="s1" stroke-width="1" d="M3.5 4.5 H12.5 V11.5 H3.5 Z"/><rect class="f1" x="9" y="5" width="3" height="6"/>"#,
    ),
    (
        "inspector",
        20,
        r#"<path class="s1" stroke-width="1" d="M4.5 5.5 H15.5 V14.5 H4.5 Z"/><path class="s1" stroke-width="1" d="M10.5 6 V14"/><rect class="f1" x="12" y="8" width="2" height="1"/><rect class="f1" x="12" y="10" width="2" height="1"/><rect class="f1" x="12" y="12" width="2" height="1"/>"#,
    ),
    (
        "inspector",
        24,
        r#"<path class="s1" stroke-width="1" d="M5.5 6.5 H18.5 V17.5 H5.5 Z"/><path class="s1" stroke-width="1" d="M13.5 7 V17"/><rect class="f1" x="15" y="9" width="2" height="1"/><rect class="f1" x="15" y="11" width="2" height="1"/><rect class="f1" x="15" y="13" width="2" height="1"/>"#,
    ),
    (
        "inspector",
        30,
        r#"<path class="s1" stroke-width="2" stroke-linejoin="round" d="M7 8 H23 V22 H7 Z"/><path class="s1" stroke-width="2" d="M17 8 V22"/><rect class="f1" x="19" y="11" width="2" height="2"/><rect class="f1" x="19" y="14" width="2" height="2"/><rect class="f1" x="19" y="17" width="2" height="2"/>"#,
    ),
    (
        "inspector",
        36,
        r#"<path class="s1" stroke-width="2" stroke-linejoin="round" d="M8 10 H28 V26 H8 Z"/><path class="s1" stroke-width="2" d="M21 10 V26"/><rect class="f1" x="23" y="13" width="3" height="2"/><rect class="f1" x="23" y="17" width="3" height="2"/><rect class="f1" x="23" y="21" width="3" height="2"/>"#,
    ),
];
