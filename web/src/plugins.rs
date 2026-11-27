//! Cantus's effects as plug-ins on their own (round 42, BACKLOG §6a): for people who come for
//! the effects and not the DAW. What `/plugins/` and `/plugins/<effect>/` link (the release's
//! zips, named as `cantus_update::release` names them; an xtask test keeps the two, the
//! release workflow and the folders below one), each file's size and SHA-256 from the release
//! the site is built against (`release.json`), and the pages' longer text.
//!
//! **A button is live only when the release holds its file** (`file`): until a release carries
//! the zips (0.1.0-beta.18 is the first meant to), every button is shown disabled with a note,
//! never a link that would answer 404.
//!
//! Neither the files' facts nor the longer text are compiled into the WebAssembly (its budget
//! is 250 KB gzipped, `tools/wasm_budget.py`): the pre-renderer writes them to `/plugins.json`
//! and into the pages' HTML, and the browser fetches that file only on these pages (`load`).

use crate::lang::Lang;
use crate::platform::Os;

/// Where every link points: the latest release's file of that name (ADR 0048 §7).
pub const DOWNLOADS: &str = "https://github.com/luis314159/cantus-daw/releases/latest/download/";

/// Where the pre-renderer writes the facts and the text, and the browser reads them.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const URL: &str = "/plugins.json";

/// A system the plug-ins are built for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sys {
    Windows,
    Linux,
    Mac,
}

impl Sys {
    pub const ALL: [Sys; 3] = [Sys::Windows, Sys::Linux, Sys::Mac];

    /// The system's part of the file names (`cantus_update::release::Target::platform`).
    pub fn platform(self) -> &'static str {
        match self {
            Sys::Windows => "windows-x64",
            Sys::Linux => "linux-x86_64",
            Sys::Mac => "macos-universal",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sys::Windows => "Windows",
            Sys::Linux => "Linux",
            Sys::Mac => "macOS",
        }
    }

    /// The visitor's, as far as the browser says (a phone: Windows, the first one listed).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the browser ticks it
    pub fn of(os: Os) -> Sys {
        match os {
            Os::Linux => Sys::Linux,
            Os::MacOs => Sys::Mac,
            Os::Windows | Os::Mobile => Sys::Windows,
        }
    }

    /// What it runs on, in a few words.
    pub fn needs(self, l: Lang) -> &'static str {
        match self {
            Sys::Windows => l.t("Windows 10 or 11 · x64", "Windows 10 u 11 · x64"),
            Sys::Linux => l.t(
                "x86-64 · glibc 2.35 or later (Ubuntu 22.04 on)",
                "x86-64 · glibc 2.35 o posterior (Ubuntu 22.04 en adelante)",
            ),
            Sys::Mac => l.t(
                "macOS 11 on · Apple silicon and Intel · preview",
                "macOS 11 en adelante · Apple silicon e Intel · vista previa",
            ),
        }
    }

    /// The command that prints a file's SHA-256.
    pub fn hash_cmd(self) -> &'static str {
        match self {
            Sys::Windows => "Get-FileHash",
            Sys::Linux => "sha256sum",
            Sys::Mac => "shasum -a 256",
        }
    }

    /// The folders each standard is read from — (standard, for you, for everyone). The same
    /// as the zips' README (`xtask/src/plugin_zips.rs`) and the installer's.
    pub fn folders(self) -> [(&'static str, &'static str, &'static str); 2] {
        match self {
            Sys::Windows => [
                (
                    "CLAP",
                    r"%LOCALAPPDATA%\Programs\Common\CLAP",
                    r"C:\Program Files\Common Files\CLAP",
                ),
                (
                    "VST3",
                    r"%LOCALAPPDATA%\Programs\Common\VST3",
                    r"C:\Program Files\Common Files\VST3",
                ),
            ],
            Sys::Linux => [
                ("CLAP", "~/.clap", "/usr/lib/clap"),
                ("VST3", "~/.vst3", "/usr/lib/vst3"),
            ],
            Sys::Mac => [
                (
                    "CLAP",
                    "~/Library/Audio/Plug-Ins/CLAP",
                    "/Library/Audio/Plug-Ins/CLAP",
                ),
                (
                    "VST3",
                    "~/Library/Audio/Plug-Ins/VST3",
                    "/Library/Audio/Plug-Ins/VST3",
                ),
            ],
        }
    }
}

/// One effect's zip for a system: `Cantus-plugin-aequalis-windows-x64.zip`.
pub fn zip_name(slug: &str, sys: Sys) -> String {
    format!(
        "Cantus-plugin-{slug}-{platform}.zip",
        platform = sys.platform()
    )
}

/// Every effect in one zip for a system: `Cantus-plugins-all-windows-x64.zip`.
pub fn all_name(sys: Sys) -> String {
    format!(
        "Cantus-plugins-all-{platform}.zip",
        platform = sys.platform()
    )
}

/// One file of the release, as the pages show it.
#[derive(Clone, PartialEq, Debug)]
pub struct Got {
    pub href: String,
    /// The name the browser saves it under.
    pub name: String,
    pub size: u64,
    /// Lower-case hex; empty when GitHub gave none.
    pub sha256: String,
}

/// The release's file `name`, linked through `releases/latest/download/`, or `None` while
/// the release the site was built against does not hold it.
pub fn file(name: &str) -> Option<Got> {
    let (size, sha256) = facts(name)?;
    Some(Got {
        href: format!("{DOWNLOADS}{name}"),
        name: name.to_string(),
        size,
        sha256,
    })
}

/// Every effect at once for `sys`: the zip with all; on Windows, while the release has none
/// yet, the plug-ins zip every Windows release has carried beside the installer (versioned,
/// linked by its own address, as the download page links the portable zip).
pub fn all(sys: Sys) -> Option<Got> {
    file(&all_name(sys)).or_else(|| {
        if sys != Sys::Windows {
            return None;
        }
        let f = crate::release::file("plugins-windows-x64.zip")?;
        Some(Got {
            href: f.url.to_string(),
            name: f.name.to_string(),
            size: f.size,
            sha256: f.sha256.to_string(),
        })
    })
}

/// `(size, sha256)` of a plug-in zip in the release, natively from the build's facts.
#[cfg(not(target_arch = "wasm32"))]
fn facts(name: &str) -> Option<(u64, String)> {
    crate::release::REL_PLUGINS
        .iter()
        .find(|f| f.name == name)
        .map(|f| (f.size, f.sha256.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn facts(name: &str) -> Option<(u64, String)> {
    browser::facts(name)
}

/// The pages' longer text: `(key, paragraphs)`, compiled only natively; the browser reads
/// `/plugins.json`'s `text[key][i]`.
#[cfg(not(target_arch = "wasm32"))]
pub const TEXT: &[(&str, &[(&str, &str)])] = &[
    ("lede", &[(
        "Every effect of Cantus as a CLAP and VST3 plug-in for the DAW you already use — REAPER, Bitwig, FL Studio, Ableton Live, Cubase or any other that loads them. Free and open source, with no account, no licence key and nothing that phones home.",
        "Cada efecto de Cantus como plug-in CLAP y VST3 para el DAW que ya usas — REAPER, Bitwig, FL Studio, Ableton Live, Cubase o cualquiera que los cargue. Libres y de código abierto, sin cuenta, sin clave de licencia y sin nada que llame a casa.",
    )]),
    ("face", &[(
        "On Windows each plug-in opens the same window it has in Cantus. On Linux and macOS your host shows its own controls for every parameter for now: the window comes to them later. The sound is the same everywhere, to the bit — the plug-ins run the engine's own code.",
        "En Windows cada plug-in abre la misma ventana que tiene en Cantus. En Linux y macOS, por ahora, tu DAW muestra sus propios controles para cada parámetro: la ventana les llega después. El sonido es el mismo en todas partes, bit a bit: los plug-ins corren el código del propio motor.",
    )]),
    ("soon", &[(
        "The downloads arrive with the next beta, 0.1.0-beta.18, the first release that zips every effect on its own for each system. Until then every effect is inside Cantus, free, and on Windows all of them come in one zip beside the installer.",
        "Las descargas llegan con la próxima beta, 0.1.0-beta.18, la primera versión que comprime cada efecto por separado para cada sistema. Hasta entonces cada efecto está dentro de Cantus, gratis, y en Windows vienen todos en un zip junto al instalador.",
    )]),
    ("steps-windows", &[
        ("Right-click the zip → Properties → tick Unblock → OK, then Extract All.", "Clic derecho en el zip → Propiedades → marca Desbloquear → Aceptar, y luego Extraer todo."),
        ("Copy each Cantus ….clap from CLAP into a CLAP folder, and each Cantus ….vst3 folder from VST3 into a VST3 folder:", "Copia cada Cantus ….clap de CLAP a una carpeta CLAP, y cada carpeta Cantus ….vst3 de VST3 a una carpeta VST3:"),
        ("Let your DAW look again for new plug-ins (below). One format is enough: CLAP where your DAW reads it, else VST3.", "Haz que tu DAW busque plug-ins nuevos (abajo). Basta un formato: CLAP donde tu DAW lo lea, si no VST3."),
    ]),
    ("steps-linux", &[
        ("Extract the zip (the libraries stay executable).", "Extrae el zip (las bibliotecas siguen siendo ejecutables)."),
        ("Copy each Cantus ….clap from CLAP into a CLAP folder, and each Cantus ….vst3 folder from VST3 into a VST3 folder:", "Copia cada Cantus ….clap de CLAP a una carpeta CLAP, y cada carpeta Cantus ….vst3 de VST3 a una carpeta VST3:"),
        ("Let your DAW look again for new plug-ins (below). One format is enough: CLAP where your DAW reads it, else VST3.", "Haz que tu DAW busque plug-ins nuevos (abajo). Basta un formato: CLAP donde tu DAW lo lea, si no VST3."),
    ]),
    ("steps-mac", &[
        ("Open the zip: Finder extracts it beside it.", "Abre el zip: Finder lo extrae a su lado."),
        ("Copy each Cantus ….clap bundle from CLAP into a CLAP folder, and each Cantus ….vst3 from VST3 into a VST3 folder (in Finder, Go → Go to Folder… opens one):", "Copia cada paquete Cantus ….clap de CLAP a una carpeta CLAP, y cada Cantus ….vst3 de VST3 a una carpeta VST3 (en Finder, Ir → Ir a la carpeta… abre una):"),
        ("Let your DAW look again for new plug-ins (below), then allow them once (Gatekeeper, below).", "Haz que tu DAW busque plug-ins nuevos (abajo) y permítelos una vez (Gatekeeper, abajo)."),
    ]),
    ("hosts", &[
        ("REAPER: Options → Preferences → Plug-ins → CLAP or VST → Re-scan.", "REAPER: Options → Preferences → Plug-ins → CLAP o VST → Re-scan."),
        ("Bitwig Studio: Settings → Locations: add the folder if it is not listed; Bitwig scans it.", "Bitwig Studio: Settings → Locations: añade la carpeta si no aparece; Bitwig la explora."),
        ("FL Studio: Options → Manage plugins → Find installed plugins.", "FL Studio: Options → Manage plugins → Find installed plugins."),
        ("Ableton Live (VST3): Settings → Plug-Ins → Use VST3 Plug-In System Folders on → Rescan.", "Ableton Live (VST3): Settings → Plug-Ins → activa Use VST3 Plug-In System Folders → Rescan."),
        ("Cubase and Nuendo (VST3): Studio → VST Plug-in Manager.", "Cubase y Nuendo (VST3): Studio → VST Plug-in Manager."),
    ]),
    ("unsigned-windows", &[
        ("The plug-ins carry no code-signing certificate yet. Your browser's SmartScreen may say the zip is not commonly downloaded: open the download's … menu and choose Keep, then Keep anyway. Unblock it before extracting (step 1) and your DAW loads the plug-ins without asking.", "Los plug-ins aún no llevan certificado de firma de código. El SmartScreen de tu navegador puede decir que el zip no se descarga con frecuencia: abre el menú … de la descarga y elige Conservar, y luego Conservar de todos modos. Desbloquéalo antes de extraerlo (paso 1) y tu DAW carga los plug-ins sin preguntar."),
    ]),
    ("unsigned-linux", &[
        ("Nothing to allow: Linux asks for no signature, and the zip keeps the libraries executable.", "Nada que permitir: Linux no pide firma, y el zip conserva las bibliotecas como ejecutables."),
    ]),
    ("unsigned-mac", &[
        ("They are signed ad hoc, not yet by a registered Apple developer, so Gatekeeper stops your DAW the first time it loads one. Open System Settings → Privacy & Security and choose Open Anyway, or clear the download mark once in Terminal:", "Van firmados ad hoc, aún no por un desarrollador registrado de Apple, así que Gatekeeper detiene a tu DAW la primera vez que carga uno. Abre Ajustes del Sistema → Privacidad y seguridad y elige Abrir igualmente, o quita la marca de descarga una vez en Terminal:"),
        ("A preview: built and checked on GitHub's Macs, not yet loaded in a DAW on a real Mac. AU, for Logic and GarageBand, waits for Apple's signing.", "Una vista previa: compilados y comprobados en las Mac de GitHub, aún no cargados en un DAW en una Mac real. AU, para Logic y GarageBand, espera a la firma de Apple."),
    ]),
    ("licence", &[
        ("Free software under the GPL-3.0-or-later, as Cantus is: use them in any song, commercial or not — what you make with them is yours — and share the zips freely. The source of every one is on GitHub.", "Software libre bajo la GPL-3.0-o-posterior, como Cantus: úsalos en cualquier canción, comercial o no — lo que hagas con ellos es tuyo — y comparte los zips libremente. El código de cada uno está en GitHub."),
        ("\"Cantus\" and its logos are trademarks of Luis Ángel Almazán: share the official zips as they are; a modified build is welcome under a name of its own.", "«Cantus» y sus logotipos son marcas de Luis Ángel Almazán: comparte los zips oficiales tal cual; una versión modificada es bienvenida con un nombre propio."),
    ]),
    ("instruments", &[
        ("Cantus's instruments are not plug-ins yet: they play inside Cantus, free. As CLAP and VST3 instruments for other DAWs they come later — there is no date, and no download until there is one that works.", "Los instrumentos de Cantus aún no son plug-ins: suenan dentro de Cantus, gratis. Como instrumentos CLAP y VST3 para otros DAW llegarán después — no hay fecha, y no habrá descarga hasta que haya una que funcione."),
    ]),
    ("daw", &[
        ("Want them with everything around them? Cantus is the whole DAW, free: these effects, its own instruments, a mixer, a piano roll and a recording mode — for Windows and Linux, macOS in preview.", "¿Los quieres con todo lo demás? Cantus es el DAW completo, gratis: estos efectos, sus propios instrumentos, un mezclador, un piano roll y un modo de grabación — para Windows y Linux, macOS en vista previa."),
    ]),
];

/// The `i`-th paragraph of `key`, or `None` past its last one.
pub fn text(key: &str, i: usize, l: Lang) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        browser::text(key, i as u32, l)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        TEXT.iter()
            .find(|p| p.0 == key)
            .and_then(|p| p.1.get(i))
            .map(|&(en, es)| l.t(en, es).to_string())
    }
}

/// Every paragraph of `key`.
pub fn paras(key: &str, l: Lang) -> Vec<String> {
    (0..).map_while(|i| text(key, i, l)).collect()
}

impl Sys {
    /// The system's key in the text's names and the page's ids: `windows`, `linux`, `mac`.
    pub fn key(self) -> &'static str {
        match self {
            Sys::Windows => "windows",
            Sys::Linux => "linux",
            Sys::Mac => "mac",
        }
    }
}

/// The JSON the browser reads: `{"files":{"<name>":[size,"sha"],…},"text":{"<key>":[["en","es"],…],…}}`.
#[cfg(feature = "ssr")]
pub fn json() -> String {
    fn s(t: &str) -> String {
        let mut o = String::from("\"");
        for c in t.chars() {
            match c {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                '\n' => o.push_str("\\n"),
                c => o.push(c),
            }
        }
        o.push('"');
        o
    }
    let files: Vec<String> = crate::release::REL_PLUGINS
        .iter()
        .map(|f| format!("{}:[{},{}]", s(f.name), f.size, s(f.sha256)))
        .collect();
    let text: Vec<String> = TEXT
        .iter()
        .map(|(key, paras)| {
            let p: Vec<String> = paras
                .iter()
                .map(|&(en, es)| format!("[{},{}]", s(en), s(es)))
                .collect();
            format!("{}:[{}]", s(key), p.join(","))
        })
        .collect();
    format!(
        "{{\"files\":{{{}}},\"text\":{{{}}}}}\n",
        files.join(","),
        text.join(",")
    )
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::{Cell, RefCell};

    use wasm_bindgen::{JsCast, JsValue};

    use crate::lang::Lang;

    thread_local! {
        static DATA: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
        static ASKED: Cell<bool> = const { Cell::new(false) };
    }

    fn at(path: &[&str]) -> Option<JsValue> {
        DATA.with(|d| {
            let mut v = d.borrow().clone();
            for k in path {
                v = js_sys::Reflect::get(&v, &(*k).into()).ok()?;
                if v.is_undefined() {
                    return None;
                }
            }
            Some(v)
        })
    }

    pub fn facts(name: &str) -> Option<(u64, String)> {
        let v = at(&["files", name])?;
        let size = js_sys::Reflect::get_u32(&v, 0).ok()?.as_f64()? as u64;
        let sha = js_sys::Reflect::get_u32(&v, 1).ok()?.as_string()?;
        Some((size, sha))
    }

    pub fn text(key: &str, i: u32, l: Lang) -> Option<String> {
        let v = js_sys::Reflect::get_u32(&at(&["text", key])?, i).ok()?;
        let col = match l {
            Lang::En => 0,
            Lang::Es => 1,
        };
        js_sys::Reflect::get_u32(&v, col).ok()?.as_string()
    }

    pub fn loaded() -> bool {
        DATA.with(|d| !d.borrow().is_undefined())
    }

    /// Fetches the file once; on any failure the pages show their buttons disabled and no
    /// longer text.
    pub async fn load() {
        use wasm_bindgen_futures::JsFuture;
        if loaded() || ASKED.with(|a| a.replace(true)) {
            return;
        }
        let got: Option<JsValue> = async {
            let win = web_sys::window()?;
            let resp: web_sys::Response = JsFuture::from(win.fetch_with_str(super::URL))
                .await
                .ok()?
                .dyn_into()
                .ok()?;
            if !resp.ok() {
                return None;
            }
            JsFuture::from(resp.json().ok()?).await.ok()
        }
        .await;
        if let Some(v) = got {
            DATA.with(|d| *d.borrow_mut() = v);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::load;

/// Whether the facts and text are here (natively they always are).
pub fn loaded() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        browser::loaded()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_the_release_s() {
        assert_eq!(
            zip_name("aequalis", Sys::Windows),
            "Cantus-plugin-aequalis-windows-x64.zip"
        );
        assert_eq!(all_name(Sys::Mac), "Cantus-plugins-all-macos-universal.zip");
        assert_eq!(
            file("Cantus-plugin-nope-linux-x86_64.zip"),
            None,
            "no button for a file the release lacks"
        );
    }

    #[test]
    fn a_button_is_live_only_for_a_file_the_release_holds() {
        for u in crate::units_data::EFFECTS.iter().filter(|u| u.plugin) {
            for sys in Sys::ALL {
                let name = zip_name(u.file, sys);
                let held = crate::release::REL_PLUGINS.iter().any(|f| f.name == name);
                assert_eq!(file(&name).is_some(), held, "{name}");
                if let Some(g) = file(&name) {
                    assert_eq!(g.href, format!("{DOWNLOADS}{name}"));
                }
            }
        }
        // The plug-in zips are never among the files the WebAssembly is built with.
        assert!(crate::release::REL_FILES
            .iter()
            .all(|f| !f.name.starts_with("Cantus-plugin")));
        // Windows' versioned zip with all stands in while the release has no stable one.
        if crate::release::REL_PLUGINS.is_empty() {
            let w = all(Sys::Windows).expect("beta.16 on carry the Windows plug-ins zip");
            assert!(w.name.ends_with("-plugins-windows-x64.zip") && w.sha256.len() == 64);
            assert_eq!(all(Sys::Linux), None);
        }
    }

    #[test]
    fn every_paragraph_is_in_both_languages() {
        for (key, paras) in TEXT {
            assert!(!paras.is_empty(), "{key}");
            for (en, es) in paras.iter() {
                assert!(!en.is_empty() && !es.is_empty(), "{key}: {en}");
            }
        }
        for sys in ["windows", "linux", "mac"] {
            assert_eq!(paras(&format!("steps-{sys}"), Lang::Es).len(), 3);
            assert!(!paras(&format!("unsigned-{sys}"), Lang::En).is_empty());
        }
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn the_json_holds_the_text_and_every_file() {
        let j = json();
        assert!(j.starts_with("{\"files\":{"));
        for f in crate::release::REL_PLUGINS {
            assert!(j.contains(&format!("\"{}\":[{},", f.name, f.size)));
        }
        assert!(j.contains("\"licence\":[[\"Free software under the GPL-3.0-or-later"));
        assert_eq!(j.matches('{').count(), j.matches('}').count());
    }
}
