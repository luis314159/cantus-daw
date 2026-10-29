//! The system's own fonts, after the bundled ones (round 26): names people type — a track
//! in Japanese, a clip in Korean, a strip in Hindi, a sample folder in Arabic, a plug-in
//! from China — in scripts IBM Plex and the emoji face do not cover.
//!
//! - **Never bundled, never distributed**: the files are read from the operating system's
//!   font folder at run time (Windows: `%WINDIR%\Fonts`; Linux: the usual Noto/DejaVu
//!   locations; macOS: `/System/Library/Fonts`), so their licences are the system's, not
//!   ours to carry.
//! - **Lazy, per character**: nothing is read until a string the kit lays out (or text
//!   typed, pasted or composed in an input method) holds a character no bundled font has.
//!   A loader thread then finds the faces (once; on Linux by walking the font folders),
//!   reads that character's script's faces one at a time until one maps it — keeping each
//!   face read with its character map, so a later character it covers needs no new read —
//!   and the fonts are reinstalled on the next frame. On the interface thread a miss costs
//!   a few set lookups. A Latin-only session never reads a system font.
//! - **Off unless armed**: only the real window arms it ([`arm`], from `boot`'s font stage);
//!   tests, the shot examples and the benchmarks see the bundled fonts alone, so their
//!   pixels do not depend on the machine.
//! - Never on the audio thread: it lives in the interface's text path only.
//!
//! What egui does not do, whatever the font: shaping. Arabic and Hebrew show their letters
//! unjoined and left to right; Devanagari, Thai and other scripts that reorder or join
//! letters show them without the reordering. The release notes and the manual say so.

use crate::design;
use egui::Context;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// The script groups a missing character is looked up in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Script {
    /// Kana and CJK ideographs, Japanese faces first.
    Japanese,
    /// CJK ideographs not in the Japanese faces.
    Chinese,
    Hangul,
    /// Devanagari, Bengali, Tamil and the other Indic scripts.
    Indic,
    Thai,
    /// Greek, Cyrillic, Armenian, Hebrew, Arabic, Syriac…
    Letters,
    /// Arrows, maths, technical and other symbols.
    Symbols,
    Emoji,
}

/// The group a character belongs to.
pub fn script_of(c: char) -> Script {
    match c as u32 {
        0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF65..=0xFF9F => Script::Japanese,
        0x2E80..=0x2FDF | 0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF => Script::Japanese,
        0xF900..=0xFAFF | 0xFF00..=0xFF64 | 0x20000..=0x3134F => Script::Chinese,
        0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xAC00..=0xD7FF => Script::Hangul,
        0x0900..=0x0DFF | 0xA8E0..=0xA8FF => Script::Indic,
        0x0E00..=0x0EFF => Script::Thai,
        0x0370..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF => Script::Letters,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F => Script::Emoji,
        _ => Script::Symbols,
    }
}

/// A system face: its file, the face inside a collection, and the name it gets in egui.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub path: PathBuf,
    pub index: u32,
    pub key: String,
}

/// The faces to try for `script` on this system, in order (those whose file exists).
/// It looks at the disk (on Linux it walks the font folders): the loader thread calls it,
/// once per script, never the interface thread.
pub fn faces_for(script: Script) -> Vec<Face> {
    candidates(script)
        .into_iter()
        .filter(|f| f.path.is_file())
        .collect()
}

#[cfg(any(windows, target_os = "macos"))]
fn face(dir: &std::path::Path, file: &str, index: u32) -> Face {
    Face {
        path: dir.join(file),
        index,
        key: format!("system:{file}#{index}"),
    }
}

#[cfg(windows)]
fn candidates(script: Script) -> Vec<Face> {
    let dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    let list: &[(&str, u32)] = match script {
        Script::Japanese => &[("YuGothR.ttc", 0), ("meiryo.ttc", 0), ("msyh.ttc", 0)],
        Script::Chinese => &[("msyh.ttc", 0), ("simsun.ttc", 0), ("YuGothR.ttc", 0)],
        Script::Hangul => &[("malgun.ttf", 0)],
        Script::Indic => &[("Nirmala.ttc", 0), ("Nirmala.ttf", 0)],
        Script::Thai => &[("LeelawUI.ttf", 0), ("tahoma.ttf", 0)],
        Script::Letters => &[("segoeui.ttf", 0), ("arial.ttf", 0)],
        Script::Symbols => &[("seguisym.ttf", 0), ("segoeui.ttf", 0)],
        Script::Emoji => &[("seguiemj.ttf", 0), ("seguisym.ttf", 0)],
    };
    list.iter().map(|(f, i)| face(&dir, f, *i)).collect()
}

#[cfg(target_os = "macos")]
fn candidates(script: Script) -> Vec<Face> {
    let sys = PathBuf::from("/System/Library/Fonts");
    let sup = sys.join("Supplemental");
    let list: Vec<(PathBuf, &str)> = match script {
        Script::Japanese => vec![
            (sys.clone(), "Hiragino Sans GB.ttc"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
        Script::Chinese => vec![
            (sys.clone(), "Hiragino Sans GB.ttc"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
        Script::Hangul => vec![
            (sys.clone(), "AppleSDGothicNeo.ttc"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
        Script::Indic => vec![
            (sys.clone(), "Kohinoor.ttc"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
        Script::Thai => vec![
            (sys.clone(), "Thonburi.ttc"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
        Script::Letters => vec![
            (sys.clone(), "Helvetica.ttc"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
        Script::Symbols | Script::Emoji => vec![
            (sys.clone(), "Apple Symbols.ttf"),
            (sup.clone(), "Arial Unicode.ttf"),
        ],
    };
    list.iter().map(|(d, f)| face(d, f, 0)).collect()
}

/// Linux (ADR 0065): what fontconfig answers first — the faces the system's own font setup
/// gives for the script's language, whatever their file names (Fedora's variable Noto CJK,
/// WenQuanYi, a Flatpak's `/run/host/fonts`) — then the usual files by name in the font
/// folders, the user's own included.
#[cfg(all(unix, not(target_os = "macos")))]
fn candidates(script: Script) -> Vec<Face> {
    let mut out = fontconfig_faces(script);
    for face in named_faces(script) {
        if !out
            .iter()
            .any(|f| f.path == face.path && f.index == face.index)
        {
            out.push(face);
        }
    }
    out
}

/// fontconfig's language for a script (`fc-list :lang=<it>`), `None` for the mixed ones.
#[cfg(all(unix, not(target_os = "macos")))]
fn fontconfig_lang(script: Script) -> Option<&'static str> {
    match script {
        Script::Japanese => Some("ja"),
        Script::Chinese => Some("zh-cn"),
        Script::Hangul => Some("ko"),
        Script::Indic => Some("hi"),
        Script::Thai => Some("th"),
        Script::Letters | Script::Symbols | Script::Emoji => None,
    }
}

/// The faces `fc-list` names as covering the script's language (at most four; Noto first,
/// as the desktops' own settings prefer it). Not `fc-match`: it always answers with some
/// face, covering the language or not. Nothing when fontconfig's tools are not installed.
#[cfg(all(unix, not(target_os = "macos")))]
fn fontconfig_faces(script: Script) -> Vec<Face> {
    let Some(lang) = fontconfig_lang(script) else {
        return Vec::new();
    };
    let text = std::process::Command::new("fc-list")
        .args(["-f", "%{file}|%{index}\n", &format!(":lang={lang}")])
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut faces: Vec<Face> = parse_fontconfig(&text, lang)
        .into_iter()
        .filter(|f| f.path.is_file())
        .collect();
    faces.sort_by_key(|f| !f.path.to_string_lossy().contains("Noto"));
    faces.truncate(4);
    faces
}

/// `file|index` lines, as `fc-list -f '%{file}|%{index}\n'` prints them, as faces (the
/// first of each file and index kept, in order).
pub fn parse_fontconfig(text: &str, lang: &str) -> Vec<Face> {
    let mut out: Vec<Face> = Vec::new();
    for line in text.lines() {
        let Some((file, index)) = line.trim().rsplit_once('|') else {
            continue;
        };
        let Ok(index) = index.trim().parse::<u32>() else {
            continue;
        };
        let path = PathBuf::from(file);
        if !path.is_absolute() || out.iter().any(|f| f.path == path && f.index == index) {
            continue;
        }
        out.push(Face {
            key: format!("fontconfig:{lang}:{file}#{index}"),
            path,
            index,
        });
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn named_faces(script: Script) -> Vec<Face> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        // A Flatpak sees the host's fonts here.
        PathBuf::from("/run/host/fonts"),
        PathBuf::from("/run/host/user-fonts"),
    ];
    roots.extend(data.map(|d| d.join("fonts")));
    roots.extend(home.map(|h| h.join(".fonts")));
    let names: &[&str] = match script {
        Script::Japanese | Script::Chinese | Script::Hangul => &[
            "NotoSansCJK-Regular.ttc",
            "NotoSansCJKjp-Regular.otf",
            "NotoSansCJKsc-Regular.otf",
            "NotoSansCJKkr-Regular.otf",
            "DroidSansFallbackFull.ttf",
        ],
        Script::Indic => &["NotoSansDevanagari-Regular.ttf", "FreeSans.ttf"],
        Script::Thai => &["NotoSansThai-Regular.ttf", "FreeSans.ttf"],
        Script::Letters => &[
            "NotoSans-Regular.ttf",
            "DejaVuSans.ttf",
            "NotoSansArabic-Regular.ttf",
            "NotoSansHebrew-Regular.ttf",
        ],
        Script::Symbols | Script::Emoji => &["NotoSansSymbols2-Regular.ttf", "DejaVuSans.ttf"],
    };
    let mut out = Vec::new();
    for name in names {
        for root in &roots {
            if let Some(p) = find(root, name, 4) {
                out.push(Face {
                    key: format!("system:{name}#0"),
                    path: p,
                    index: 0,
                });
                break;
            }
        }
    }
    out
}

#[cfg(all(unix, not(target_os = "macos")))]
fn find(dir: &std::path::Path, name: &str, depth: u32) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
        if p.is_dir() {
            dirs.push(p);
        }
    }
    if depth == 0 {
        return None;
    }
    dirs.iter().find_map(|d| find(d, name, depth - 1))
}

#[cfg(not(any(windows, unix)))]
fn candidates(_: Script) -> Vec<Face> {
    Vec::new()
}

/// Whether the real window has armed the fallback.
static ARMED: AtomicBool = AtomicBool::new(false);
/// The score's music font is wanted (the app's start-up, or a score first drawn).
static MUSIC: AtomicBool = AtomicBool::new(false);

/// Ask for the score's music font in every definition from now on (`definitions`).
pub fn enable_music() {
    MUSIC.store(true, Ordering::Relaxed);
}

/// Whether the music font is part of the definitions.
pub fn music_enabled() -> bool {
    MUSIC.load(Ordering::Relaxed)
}

/// A system face that was read: what it covers, and its bytes once it is installed.
struct Read {
    face: Face,
    cmap: HashSet<u32>,
    installed: bool,
}

#[derive(Default)]
struct State {
    /// Every character an installed face maps: the interface thread's O(1) check.
    covered: HashSet<u32>,
    /// Characters no face on this system maps (asked once, never again).
    uncoverable: HashSet<u32>,
    /// Characters asked for and not yet resolved, and the queue the loader takes them from.
    pending: HashSet<u32>,
    queue: Vec<char>,
    /// Whether a loader thread is running.
    loading: bool,
    /// Every face read so far, once each (by file and index), installed or not: a face that
    /// covered nothing asked so far is kept with its character map for later characters.
    read: Vec<Read>,
    /// Faces whose file could not be read.
    unreadable: HashSet<String>,
    /// The installed faces, in order, and how many egui already has.
    installed: Vec<(Face, &'static [u8])>,
    given: usize,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State::default()))
}

/// Every candidate face on this system, its own script's first: found once, on the
/// loader thread (on Linux it walks the font folders), never on the interface thread.
fn catalog() -> &'static Vec<(Script, Vec<Face>)> {
    static C: OnceLock<Vec<(Script, Vec<Face>)>> = OnceLock::new();
    C.get_or_init(|| {
        use Script::*;
        [
            Japanese, Chinese, Hangul, Indic, Thai, Letters, Symbols, Emoji,
        ]
        .into_iter()
        .map(|s| (s, faces_for(s)))
        .collect()
    })
}

/// The faces to try for `c`, in order: its script's, then every other one (deduplicated).
fn order_for(c: char) -> Vec<Face> {
    let script = script_of(c);
    let mut out: Vec<Face> = Vec::new();
    let all = catalog();
    let own = all.iter().filter(|(s, _)| *s == script);
    let rest = all.iter().filter(|(s, _)| *s != script);
    for (_, faces) in own.chain(rest) {
        for f in faces {
            if !out.iter().any(|g| g.key == f.key) {
                out.push(f.clone());
            }
        }
    }
    out
}

/// Every character the bundled text faces map (IBM Plex and the emoji face — the faces in
/// the text families' chains, never the score's music font), read once from their
/// character maps.
fn bundled() -> &'static HashSet<u32> {
    static B: OnceLock<HashSet<u32>> = OnceLock::new();
    B.get_or_init(|| {
        let defs = design::font_definitions();
        let keys: HashSet<&String> = defs
            .families
            .iter()
            .filter(|(f, _)| **f != design::family_music())
            .flat_map(|(_, chain)| chain.iter())
            .collect();
        let mut set = HashSet::new();
        for k in keys {
            if let Some(data) = defs.font_data.get(k) {
                set.extend(cmap_of(&data.font, data.index));
            }
        }
        set
    })
}

/// The characters a font file (a face of it) maps.
pub fn cmap_of(bytes: &[u8], index: u32) -> HashSet<u32> {
    use skrifa::MetadataProvider;
    skrifa::FontRef::from_index(bytes, index)
        .map(|f| f.charmap().mappings().map(|(c, _)| c).collect())
        .unwrap_or_default()
}

/// Whether a bundled font has `c` (whitespace and controls count as covered).
pub fn bundled_has(c: char) -> bool {
    c.is_ascii() || c.is_whitespace() || c.is_control() || bundled().contains(&(c as u32))
}

/// Arm the fallback for the real window. Tests and shot examples never call it.
pub fn arm() {
    ARMED.store(true, Ordering::Relaxed);
}

/// The real window's fonts: arm the fallback, then install the fonts. `boot`'s font
/// stage calls this — the path the desktop app takes — and so does `CantusApp::new`.
pub fn install_for_window(ctx: &Context) {
    arm();
    enable_music();
    design::install_fonts(ctx);
}

/// Whether the fallback is armed (the real window).
pub fn armed() -> bool {
    ARMED.load(Ordering::Relaxed)
}

/// Whether `c` has been asked for (pending, covered by a system face, or known to have no
/// face here). For tests.
pub fn asked(c: char) -> bool {
    let st = state().lock().unwrap();
    let u = c as u32;
    st.pending.contains(&u) || st.covered.contains(&u) || st.uncoverable.contains(&u)
}

/// Text about to be laid out: queue each character no bundled face has and nothing has
/// answered yet. On the interface thread this is a few set lookups per character; the
/// faces are found, read and installed by a loader thread.
pub fn note_text(ctx: &Context, s: &str) {
    if !armed() || s.is_ascii() {
        return;
    }
    let mut st = state().lock().unwrap();
    for c in s.chars() {
        if bundled_has(c) {
            continue;
        }
        let u = c as u32;
        if st.covered.contains(&u) || st.uncoverable.contains(&u) || !st.pending.insert(u) {
            continue;
        }
        st.queue.push(c);
    }
    if st.queue.is_empty() || st.loading {
        return;
    }
    st.loading = true;
    drop(st);
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new()
        .name("cantus-font-fallback".into())
        .spawn(move || loader(&ctx));
    if spawned.is_err() {
        state().lock().unwrap().loading = false;
    }
}

/// The loader thread: resolve queued characters one by one until the queue is empty.
fn loader(ctx: &Context) {
    loop {
        let next = {
            let mut st = state().lock().unwrap();
            match st.queue.pop() {
                Some(c) => c,
                None => {
                    st.loading = false;
                    return;
                }
            }
        };
        if resolve(next, order_for(next)) {
            ctx.request_repaint();
        }
    }
}

/// Find a face for `c` among `order`: one already read (its character map kept), else the
/// next unread candidate. True when a face was installed.
fn resolve(c: char, order: Vec<Face>) -> bool {
    let u = c as u32;
    // Covered meanwhile by a face installed for an earlier character?
    {
        let mut st = state().lock().unwrap();
        if st.covered.contains(&u) {
            st.pending.remove(&u);
            return false;
        }
    }
    for f in order {
        // Already read: install it if it maps `c`, else try the next.
        let known = {
            let st = state().lock().unwrap();
            if st.unreadable.contains(&f.key) {
                continue;
            }
            st.read
                .iter()
                .find(|r| r.face.key == f.key)
                .map(|r| (r.cmap.contains(&u), r.installed))
        };
        match known {
            Some((false, _)) => continue,
            Some((true, true)) => {
                state().lock().unwrap().pending.remove(&u);
                return false;
            }
            Some((true, false)) | None => {}
        }
        let bytes = match std::fs::read(&f.path) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("font fallback: {}: {e}", f.path.display());
                state().lock().unwrap().unreadable.insert(f.key.clone());
                continue;
            }
        };
        let cmap = cmap_of(&bytes, f.index);
        let has = cmap.contains(&u);
        let mut st = state().lock().unwrap();
        let at = match st.read.iter().position(|r| r.face.key == f.key) {
            Some(i) => i,
            None => {
                st.read.push(Read {
                    face: f.clone(),
                    cmap,
                    installed: false,
                });
                st.read.len() - 1
            }
        };
        if !has {
            continue;
        }
        if !st.read[at].installed {
            st.read[at].installed = true;
            let covered: Vec<u32> = st.read[at].cmap.iter().copied().collect();
            st.covered.extend(covered);
            let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            st.installed.push((f, bytes));
        }
        st.pending.remove(&u);
        return true;
    }
    let mut st = state().lock().unwrap();
    st.pending.remove(&u);
    st.uncoverable.insert(u);
    false
}

/// `resolve` among given faces, as the loader thread does (for tests: their own process).
#[doc(hidden)]
pub fn resolve_among(c: char, faces: &[Face]) -> bool {
    state().lock().unwrap().pending.insert(c as u32);
    resolve(c, faces.to_vec())
}

/// The faces read so far, once each, and whether each is installed (for tests).
#[doc(hidden)]
pub fn faces_read() -> Vec<(String, bool)> {
    let st = state().lock().unwrap();
    st.read
        .iter()
        .map(|r| (r.face.key.clone(), r.installed))
        .collect()
}

/// Text typed, pasted or composed (an input method's preedit and commit — how Japanese,
/// Chinese and Korean are typed) this frame: the same check as [`note_text`], since a
/// rename field lays its text out without the kit.
pub fn note_input(ctx: &Context) {
    if !armed() {
        return;
    }
    let typed: Vec<String> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Text(t) | egui::Event::Paste(t) => Some(t.clone()),
                egui::Event::Ime(egui::ImeEvent::Commit(t)) => Some(t.clone()),
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => Some(text.clone()),
                _ => None,
            })
            .collect()
    });
    for t in typed {
        note_text(ctx, &t);
    }
}

/// Install the faces that arrived since the last frame (the interface thread): the bundled
/// chain, then every system face so far, each once.
pub fn poll(ctx: &Context) {
    {
        let mut st = state().lock().unwrap();
        if st.given == st.installed.len() {
            return;
        }
        st.given = st.installed.len();
    }
    ctx.set_fonts(definitions());
    ctx.request_repaint();
}

/// The bundled fonts plus the system faces installed so far, each family's chain ending
/// with them. Without any, exactly [`design::font_definitions`].
pub fn definitions() -> egui::FontDefinitions {
    let mut defs = design::font_definitions();
    if music_enabled() {
        design::add_music_font(&mut defs);
    }
    let st = state().lock().unwrap();
    with_faces(&mut defs, &st.installed);
    defs
}

/// Append `faces` to every family's chain of `defs`, each face once, each moved onto the
/// baseline of the proportional family's first face (Plex).
pub fn with_faces(defs: &mut egui::FontDefinitions, faces: &[(Face, &'static [u8])]) {
    let primary = defs
        .families
        .get(&egui::FontFamily::Proportional)
        .and_then(|chain| chain.first())
        .and_then(|k| defs.font_data.get(k))
        .and_then(|d| vertical(&d.font, d.index));
    for (f, bytes) in faces {
        if defs.font_data.contains_key(&f.key) {
            continue;
        }
        let mut data = egui::FontData::from_static(bytes);
        data.index = f.index;
        if let (Some(p), Some(own)) = (primary, vertical(bytes, f.index)) {
            data.tweak.y_offset_factor = baseline_shift(p, own);
        }
        defs.font_data
            .insert(f.key.clone(), std::sync::Arc::new(data));
        for chain in defs.families.values_mut() {
            chain.push(f.key.clone());
        }
    }
}

/// A face's ascent, descent (negative) and line gap, as fractions of its em: the metrics
/// egui places a glyph by.
fn vertical(bytes: &[u8], index: u32) -> Option<(f32, f32, f32)> {
    use skrifa::MetadataProvider;
    let f = skrifa::FontRef::from_index(bytes, index).ok()?;
    let m = f.metrics(
        skrifa::instance::Size::unscaled(),
        skrifa::instance::LocationRef::default(),
    );
    let em = f32::from(m.units_per_em.max(1));
    Some((m.ascent / em, m.descent / em, m.leading / em))
}

/// How far down (in ems) to move a fallback face's glyphs so they sit on the primary's
/// baseline. egui puts a glyph's baseline at its own face's ascent, plus half the
/// difference between the primary's row height and its face's; the shift undoes the rest.
fn baseline_shift(primary: (f32, f32, f32), face: (f32, f32, f32)) -> f32 {
    let row = |(a, d, g): (f32, f32, f32)| a - d + g;
    primary.0 - face.0 - 0.5 * (row(primary) - row(face))
}
