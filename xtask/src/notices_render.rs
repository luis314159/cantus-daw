//! Writing `THIRD-PARTY-NOTICES.md` (see `notices.rs`): the hand-written head, the crate
//! table, and each distinct licence text once with the crates that carry it.

use crate::notices::Crate;
use cantus_content::credits::{self, Courtesy};
use cantus_content::Manifest;
use std::collections::BTreeMap;
use std::path::Path;

/// Licence files outside a crate's top folder that the generic search does not find.
const EXTRA: &[(&str, &[&str])] = &[(
    "epaint_default_fonts",
    &[
        "fonts/OFL.txt",
        "fonts/UFL.txt",
        "fonts/Hack-Regular.txt",
        "fonts/emoji-icon-font-mit-license.txt",
    ],
)];

/// Written where a standard text is needed and `xtask/notices/` lacks it; the check fails.
pub const MISSING: &str = "**Missing standard text:** add";

/// The marker that replaces the standard Apache-2.0 text inside a crate's file.
const APACHE_REF: &str = "[The Apache License, Version 2.0 — its standard text is under \
\"Standard texts\" at the end of this file.]";

fn is_licence_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["licen", "copying", "notice", "copyright", "unlicense"]
        .iter()
        .any(|p| n.starts_with(p))
}

/// A crate's licence files: `(path inside the crate, text)`, sorted by path.
pub fn licence_files(c: &Crate) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut add = |rel: String| {
        let p = c.dir.join(&rel);
        if p.is_file() {
            if let Ok(bytes) = std::fs::read(&p) {
                out.push((rel, String::from_utf8_lossy(&bytes).into_owned()));
            }
        }
    };
    let mut top: Vec<String> = std::fs::read_dir(&c.dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_licence_name(n))
        .collect();
    // A `LICENSES` folder by its real name, found by listing the crate: joining both
    // spellings counted the one folder twice on a case-insensitive file system (Windows),
    // so the notices differed between Windows and Linux.
    let subs: Vec<String> = std::fs::read_dir(&c.dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.eq_ignore_ascii_case("licenses"))
        .collect();
    for sub in subs {
        top.extend(
            std::fs::read_dir(c.dir.join(&sub))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| format!("{sub}/{}", e.file_name().to_string_lossy())),
        );
    }
    if let Some((_, extra)) = EXTRA.iter().find(|(n, _)| *n == c.name) {
        top.extend(extra.iter().map(|s| s.to_string()));
    }
    top.sort();
    top.dedup();
    for rel in top {
        add(rel);
    }
    out
}

/// `text` with any copy of the standard Apache-2.0 terms replaced by [`APACHE_REF`] (with
/// the "how to apply" appendix when it is the unfilled template), and trimmed. Copyright
/// lines, NOTICE text and anything else around it are kept.
pub fn collapse_apache(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let Some(v) = text.find("Version 2.0, January 2004") else {
        return text.trim().to_string();
    };
    let Some(head) = text[..v].rfind("Apache License") else {
        return text.trim().to_string();
    };
    let start = text[..head].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end_marker = "END OF TERMS AND CONDITIONS";
    let Some(e) = text[v..].find(end_marker) else {
        return text.trim().to_string();
    };
    let mut end = v + e + end_marker.len();
    // The appendix is a template unless someone filled it in.
    let rest = &text[end..];
    if let Some(a) = rest.find("APPENDIX: How to apply the Apache License") {
        let tail = "limitations under the License.";
        if let Some(t) = rest[a..].find(tail) {
            let appendix = &rest[a..a + t];
            if appendix.contains("[yyyy]") || appendix.contains("{yyyy}") {
                end += a + t + tail.len();
            }
        }
    }
    let mut out = String::new();
    out.push_str(text[..start].trim_end());
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(APACHE_REF);
    let after = text[end..].trim();
    if !after.is_empty() {
        out.push_str("\n\n");
        out.push_str(after);
    }
    out.trim().to_string()
}

/// Whitespace-insensitive identity of a text, to print each distinct one once.
fn key(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A fence longer than any run of backticks in `text`.
fn fence(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in text.chars() {
        run = if ch == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat((longest + 1).max(3))
}

/// The standard text of an SPDX id that some crate declares without shipping it, from the
/// repository (`xtask/notices/<id>.txt`, or the root's `LICENSE-APACHE`).
fn standard(ws: &Path, id: &str) -> Option<String> {
    let path = match id {
        "Apache-2.0" => ws.join("LICENSE-APACHE"),
        other => ws.join("xtask/notices").join(format!("{other}.txt")),
    };
    std::fs::read_to_string(path)
        .ok()
        .map(|t| t.replace("\r\n", "\n").trim().to_string())
}

/// The SPDX ids an expression names.
fn ids(expr: &str) -> Vec<String> {
    expr.replace(['/', '(', ')'], " ")
        .split_whitespace()
        .filter(|w| !matches!(*w, "OR" | "AND" | "WITH"))
        .map(Into::into)
        .collect()
}

/// Markdown table cell text: no pipes, no line breaks.
fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// The *Content* section (ADR 0042 §3), generated from the content manifest: every CC BY
/// pack with its credit line word for word, every CC0 pack credited as a courtesy.
pub fn content(m: &Manifest) -> String {
    let lang = "en";
    let list = credits::list(m, &[]);
    let mut s = String::new();
    s.push_str(
        "## Content\n\n\
         No samples are bundled with Cantus. *Sound packs…* (in the sample browser) \
         downloads a pack only when you click *Install*, and installs it only with its \
         licence file, which stays in the pack's folder \
         (`%LOCALAPPDATA%\\Cantus\\Content\\<pack>`) beside a receipt \
         (`cantus-pack.json`) that keeps its credit. Content is accepted under **CC0 1.0** \
         and **CC BY 3.0 / 4.0** only (ADR 0042); nothing non-commercial, no-derivatives \
         or share-alike. Music you make with these sounds is yours. *Help ▸ About Cantus* \
         shows the same credits.\n\n",
    );
    s.push_str("### Under CC BY — the credit the licence asks for\n\n");
    let by: Vec<_> = list
        .iter()
        .filter(|c| c.courtesy == Courtesy::Required)
        .collect();
    if by.is_empty() {
        s.push_str("None of the packs offered now is under CC BY.\n\n");
    } else {
        s.push_str(
            "CC BY's one condition is credit to the author. Each pack's credit, as its \
             licensor asks:\n\n",
        );
        for c in by {
            let p = m.pack(&c.id).expect("listed from the manifest");
            s.push_str(&format!(
                "- **{}** ({}) — {}\n  Licence file in the pack: `{}`.\n",
                p.name.get(lang),
                c.licence,
                c.line,
                p.licence_file
            ));
            if !c.note.is_empty() {
                s.push_str(&format!(
                    "  About crediting, in the author's words: *\u{201c}{}\u{201d}*\n",
                    c.note
                ));
            }
        }
        s.push('\n');
    }
    s.push_str("### Under CC0 — no credit asked, given gladly\n\n");
    let cc0: Vec<_> = list
        .iter()
        .filter(|c| c.courtesy == Courtesy::Courtesy)
        .collect();
    if cc0.is_empty() {
        s.push_str("None of the packs offered now is under CC0.\n\n");
    } else {
        s.push_str("| Pack | Made by | Source |\n|---|---|---|\n");
        for c in cc0 {
            let p = m.pack(&c.id).expect("listed from the manifest");
            let mut urls: Vec<String> = Vec::new();
            for cr in &p.credits {
                let u = format!("<{}>", cr.url);
                if !urls.contains(&u) {
                    urls.push(u);
                }
            }
            s.push_str(&format!(
                "| {} | {} | {} |\n",
                cell(p.name.get(lang)),
                cell(&c.line),
                urls.join(", ")
            ));
        }
        s.push('\n');
    }
    s.push_str(
        "The list, with every file's URL and SHA-256, is \
         `crates/cantus-content/manifest/content.json`.\n\n",
    );
    s
}

/// Where the head's Fonts section takes the bundled fonts' licence texts.
const FONT_LICENCES: &str = "<!-- font-licences -->";

/// The licence file of each font bundled in `crates/cantus-ui/assets/fonts/`
/// (`LICENSE-*.txt`), each printed whole under its file name.
fn font_licences(ws: &Path) -> String {
    let dir = ws.join("crates/cantus-ui/assets/fonts");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("LICENSE-") && n.ends_with(".txt"))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let mut out = String::new();
    for p in files {
        let body = std::fs::read_to_string(&p)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let body = body.trim_end();
        let f = fence(body);
        out.push_str(&format!(
            "### `{}`\n\n{f}text\n{body}\n{f}\n\n",
            p.file_name().unwrap().to_string_lossy()
        ));
    }
    out
}

pub fn render(ws: &Path, crates: &[Crate], manifest: &Manifest) -> String {
    let head = std::fs::read_to_string(ws.join("xtask/notices/head.md"))
        .expect("xtask/notices/head.md")
        .replace("\r\n", "\n");
    // Each distinct text, numbered in the order the sorted crates first use it.
    let mut texts: Vec<(String, Vec<String>)> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut refs: Vec<Vec<usize>> = Vec::new();
    let mut declared_only: Vec<&Crate> = Vec::new();
    let mut standard_ids: Vec<String> = vec!["Apache-2.0".into()];
    for c in crates {
        let files = licence_files(c);
        if files.is_empty() {
            declared_only.push(c);
            for id in ids(&c.license) {
                if !standard_ids.contains(&id) {
                    standard_ids.push(id);
                }
            }
        }
        let mut mine = Vec::new();
        for (rel, raw) in files {
            let body = collapse_apache(&raw);
            let k = key(&body);
            let n = *index.entry(k).or_insert_with(|| {
                texts.push((body, Vec::new()));
                texts.len() - 1
            });
            texts[n]
                .1
                .push(format!("{} {} (`{rel}`)", c.name, c.version));
            if !mine.contains(&n) {
                mine.push(n);
            }
        }
        refs.push(mine);
    }

    let mut s = String::new();
    s.push_str(
        "<!-- Generated by `cargo run -p xtask -- notices` from xtask/notices/head.md and \
         Cargo.lock; `cargo run -p xtask -- notices --check` (and `cargo test -p xtask`) \
         fails when it is stale. Edit the head, not this file. -->\n\n",
    );
    s.push_str(head.replace(FONT_LICENCES, &font_licences(ws)).trim_end());
    s.push_str("\n\n");
    s.push_str(&content(manifest));
    let inside = crates.iter().filter(|c| !c.build_only).count();
    s.push_str(&format!(
        "## Rust crates\n\n\
         {inside} crates from crates.io are compiled into `cantus.exe` and the Cantus \
         plug-ins (Windows x64); {} more are used only to build them (marked *build*). \
         Each is listed with the licence its author declared and the licence files it \
         ships, printed once each under [Licence texts](#licence-texts). Where a crate \
         offers a choice (`MIT OR Apache-2.0`), Cantus takes it under either, as its \
         author allows. The patched `cpal` and `asio-sys` are under \
         [Patched crates](#patched-crates-cpal-and-asio-sys) above.\n\n",
        crates.len() - inside
    ));
    s.push_str("| Crate | Version | Licence | Texts |\n|---|---|---|---|\n");
    for (c, mine) in crates.iter().zip(&refs) {
        let texts_cell = if mine.is_empty() {
            "declared only — [standard texts](#standard-texts)".to_string()
        } else {
            mine.iter()
                .map(|n| format!("[T{}](#t{})", n + 1, n + 1))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let build = if c.build_only { " *build*" } else { "" };
        s.push_str(&format!(
            "| {} | {} | {}{build} | {texts_cell} |\n",
            c.name,
            c.version,
            c.license.replace('|', "\\|")
        ));
    }
    s.push_str("\n## Licence texts\n\n");
    for (i, (body, users)) in texts.iter().enumerate() {
        let f = fence(body);
        s.push_str(&format!(
            "<a id=\"t{n}\"></a>\n\n### T{n}\n\nIn: {}.\n\n{f}text\n{body}\n{f}\n\n",
            users.join(", "),
            n = i + 1
        ));
    }
    s.push_str("## Standard texts\n\n");
    s.push_str(
        "The standard text of each licence a crate above declares without shipping its \
         own file, and of the Apache License 2.0 that the texts above refer to.\n\n",
    );
    if !declared_only.is_empty() {
        s.push_str("Crates that ship no licence file, with the authors their manifest names:\n\n");
        for c in &declared_only {
            let who = if c.authors.is_empty() {
                c.repository.clone().unwrap_or_else(|| "its authors".into())
            } else {
                c.authors.join(", ")
            };
            s.push_str(&format!(
                "- {} {} — `{}` — Copyright (c) {who}\n",
                c.name, c.version, c.license
            ));
        }
        s.push('\n');
    }
    for id in &standard_ids {
        match standard(ws, id) {
            Some(text) => {
                let f = fence(&text);
                s.push_str(&format!("### {id}\n\n{f}text\n{text}\n{f}\n\n"));
            }
            None => s.push_str(&format!(
                "### {id}\n\n{MISSING} `xtask/notices/{id}.txt`\n\n"
            )),
        }
    }
    s.trim_end().to_string() + "\n"
}
