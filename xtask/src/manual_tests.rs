//! The manual: the converter, and the real pages — both languages present with the same
//! chapters, every picture there, every in-page link pointing at a heading.

use crate::manual::{self, body, href, html_name, inline, slug};

#[test]
fn the_markdown_the_manual_uses_becomes_html() {
    assert_eq!(
        inline("Press `Ctrl+S` to **save** *now*"),
        "Press <code>Ctrl+S</code> to <strong>save</strong> <em>now</em>"
    );
    assert_eq!(
        inline("[the mixer](#5-mix) and <https://lalmazan.com>"),
        "<a href=\"#5-mix\">the mixer</a> and \
         <a href=\"https://lalmazan.com\">https://lalmazan.com</a>"
    );
    assert_eq!(
        inline("![The window](../img/a.png)"),
        "<img src=\"../img/a.png\" alt=\"The window\" loading=\"lazy\">"
    );
    assert_eq!(inline("a < b & c"), "a &lt; b &amp; c");
    let html = body(
        "# Title\n\nOne\nline.\n\n## 2. Set up\n\n- a\n- b\n  more\n\n1. one\n2. two\n\n\
         | K | V |\n|---|---|\n| `x` | y |\n\n> quote\n",
    );
    assert!(html.contains("<h1 id=\"title\">Title</h1>"));
    assert!(html.contains("<p>One line.</p>"));
    assert!(html.contains("<h2 id=\"2-set-up\">2. Set up</h2>"));
    assert!(html.contains("<ul>\n<li>a</li>\n<li>b more</li>\n</ul>"));
    assert!(html.contains("<ol>\n<li>one</li>\n<li>two</li>\n</ol>"));
    assert!(
        html.contains("<tr><th>K</th><th>V</th></tr>\n<tr><td><code>x</code></td><td>y</td></tr>")
    );
    assert!(html.contains("<blockquote><p>quote</p></blockquote>"));
}

#[test]
fn headings_get_github_s_anchors_and_links_find_the_installed_copies() {
    assert_eq!(
        slug("5. Mix: the mixer, inserts"),
        "5-mix-the-mixer-inserts"
    );
    assert_eq!(slug("Grabar audio y MIDI"), "grabar-audio-y-midi");
    assert_eq!(slug("Configuración"), "configuración");
    assert_eq!(href("../../../PRIVACY.md"), "../../PRIVACY.txt");
    assert_eq!(
        href("../../../THIRD-PARTY-NOTICES.md"),
        "../../THIRD-PARTY-NOTICES.txt"
    );
    assert_eq!(
        href("../../../docs/BETA.md"),
        "https://github.com/luis314159/cantus-daw/blob/master/docs/BETA.md"
    );
    assert_eq!(href("../es/README.md"), "../es/index.html");
    assert_eq!(href("#x"), "#x");
    // The other pages of the manual are built beside the index (round 34).
    assert_eq!(href("stems.md"), "stems.html");
    assert_eq!(href("README.md#ai-agents-mcp"), "index.html#ai-agents-mcp");
    assert_eq!(
        href("../es/audio-to-midi.md#x"),
        "../es/audio-to-midi.html#x"
    );
    assert_eq!(href("../img/a.png"), "../img/a.png");
    assert_eq!(html_name("README.md"), "index.html");
    assert_eq!(html_name("../en/ai-tools.md"), "../en/ai-tools.html");
}

/// Every link of every built page — between pages, to a heading, to a picture, to the
/// documents installed beside the program — reaches a file the build wrote (and, with an
/// anchor, a heading of that page). A link to a page that is not built fails here
/// (round 34: `ai-tools.md`, `stems.md` and `audio-to-midi.md` led nowhere offline).
#[test]
fn every_link_of_the_built_manual_reaches_a_built_page() {
    let ws = crate::workspace();
    let src = ws.join("docs/manual");
    let out = tempfile::tempdir().unwrap();
    let files = manual::build(&src, out.path()).expect("the manual builds");
    let read = |rel: &str| std::fs::read_to_string(out.path().join(rel)).unwrap();
    let mut pages = 0;
    for lang in manual::LANGS {
        for name in manual::pages(&src.join(lang)).unwrap() {
            let rel = format!("{lang}/{}", html_name(&name));
            assert!(files.contains(&rel), "{lang}/{name} was not built as {rel}");
            pages += 1;
            let html = read(&rel);
            for attr in ["href=\"", "src=\""] {
                let mut rest = html.as_str();
                while let Some(at) = rest.find(attr) {
                    let tail = &rest[at + attr.len()..];
                    let url = tail[..tail.find('"').unwrap()].replace("&amp;", "&");
                    rest = tail;
                    if url.contains("://") || url.starts_with("mailto:") {
                        continue;
                    }
                    let (path, anchor) = url.split_at(url.find('#').unwrap_or(url.len()));
                    let anchor = anchor.trim_start_matches('#');
                    if path.starts_with("../../") {
                        // Beside the program, not in the manual's folder.
                        assert!(
                            ["../../PRIVACY.txt", "../../THIRD-PARTY-NOTICES.txt"].contains(&path),
                            "{rel}: {url} is no document the release carries"
                        );
                        continue;
                    }
                    let target = if path.is_empty() {
                        rel.clone()
                    } else {
                        resolve(lang, path)
                    };
                    assert!(
                        files.contains(&target),
                        "{rel}: {url} points at {target}, which the manual does not build"
                    );
                    if !anchor.is_empty() {
                        assert!(
                            read(&target).contains(&format!(" id=\"{anchor}\"")),
                            "{rel}: {url} — {target} has no heading #{anchor}"
                        );
                    }
                }
            }
        }
    }
    assert!(pages > manual::LANGS.len(), "only the indexes were built");
}

/// A link relative to `{lang}/`, as a path under the manual's folder.
fn resolve(lang: &str, path: &str) -> String {
    let mut parts = vec![lang];
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

#[test]
fn the_manual_builds_in_both_languages_and_its_links_hold() {
    let ws = crate::workspace();
    let src = ws.join("docs/manual");
    let out = tempfile::tempdir().unwrap();
    let files = manual::build(&src, out.path()).expect("the manual builds");
    for lang in manual::LANGS {
        assert!(files.contains(&format!("{lang}/index.html")));
    }
    let chapters = |lang: &str| -> Vec<String> {
        let md = std::fs::read_to_string(src.join(lang).join("README.md")).unwrap();
        md.lines()
            .filter_map(|l| l.strip_prefix("## "))
            .map(|h| h.split('.').next().unwrap_or("").to_owned())
            .collect()
    };
    assert_eq!(
        chapters("en"),
        chapters("es"),
        "the same chapters, in order"
    );
    assert!(chapters("en").len() >= 10, "{:?}", chapters("en"));
    for lang in manual::LANGS {
        let md = std::fs::read_to_string(src.join(lang).join("README.md")).unwrap();
        let html = manual::page(lang, &md);
        // Every picture exists, and every in-page link has its heading.
        let mut rest = html.as_str();
        while let Some(at) = rest.find("src=\"") {
            let tail = &rest[at + 5..];
            let url = &tail[..tail.find('"').unwrap()];
            let file = src.join(lang).join(url.replace("&amp;", "&"));
            assert!(file.is_file(), "{lang}: missing picture {url}");
            rest = tail;
        }
        let mut rest = html.as_str();
        while let Some(at) = rest.find("href=\"#") {
            let tail = &rest[at + 7..];
            let id = &tail[..tail.find('"').unwrap()];
            assert!(
                html.contains(&format!(" id=\"{id}\"")),
                "{lang}: no heading for #{id}"
            );
            rest = tail;
        }
        // Small pictures: the whole manual stays light enough to ship.
        let size: u64 = std::fs::read_dir(src.join("img"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        assert!(size < 4 << 20, "pictures are {size} bytes");
    }
}
