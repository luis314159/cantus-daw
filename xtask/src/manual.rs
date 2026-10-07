//! The quick-start manual as pages a browser opens offline: `docs/manual/{en,es}/README.md`
//! (read on GitHub as they are) turned into `manual/{en,es}/index.html` beside
//! `cantus.exe`, with the pictures in `manual/img/`. *Help ▸ Manual* opens the page in the
//! interface's language (round 22).
//!
//! The Markdown is the plain subset the manual is written in — headings, paragraphs,
//! lists, tables, quotes, pictures, `code`, **bold**, *italic* and links — so a small
//! converter is enough and no crate is added. Links to the repository's own documents
//! become the copies installed beside the program (`PRIVACY.txt`,
//! `THIRD-PARTY-NOTICES.txt`) or their pages on GitHub.

use std::path::Path;

pub const LANGS: &[&str] = &["en", "es"];
const REPO: &str = "https://github.com/luis314159/cantus/blob/master/";

/// Build every language into `out`; returns the files written, relative to `out`, with
/// `/` separators.
pub fn build(src: &Path, out: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    let _ = std::fs::remove_dir_all(out);
    for lang in LANGS {
        let md = std::fs::read_to_string(src.join(lang).join("README.md"))
            .map_err(|e| format!("{lang}/README.md: {e}"))?;
        let html = page(lang, &md);
        let rel = format!("{lang}/index.html");
        write(&out.join(&rel), html.as_bytes())?;
        written.push(rel);
    }
    let img = src.join("img");
    let mut pics: Vec<String> = std::fs::read_dir(&img)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".png"))
        .collect();
    pics.sort();
    for p in pics {
        let rel = format!("img/{p}");
        let bytes = std::fs::read(img.join(&p)).map_err(|e| format!("{rel}: {e}"))?;
        write(&out.join(&rel), &bytes)?;
        written.push(rel);
    }
    Ok(written)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// A whole page: the template around [`body`].
pub fn page(lang: &str, md: &str) -> String {
    let title = md
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("Cantus");
    format!(
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>{CSS}</style>\n</head>\n<body>\n<main>\n{}</main>\n\
         </body>\n</html>\n",
        escape(title),
        body(md)
    )
}

const CSS: &str = "
:root{--bg:#f7f5f0;--fg:#22211f;--dim:#6b665d;--line:#d9d4c8;--gold:#a8741a;--code:#ece8de}
@media (prefers-color-scheme: dark){:root{--bg:#17181b;--fg:#e8e6e1;--dim:#a19c92;\
--line:#34363b;--gold:#e0b04f;--code:#24262b}}
body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.6 'Segoe UI',system-ui,sans-serif}
main{max-width:860px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:2em;margin:.2em 0 .4em;color:var(--gold)}
h2{margin-top:2em;padding-top:.6em;border-top:1px solid var(--line)}
h3{margin-top:1.4em}
a{color:var(--gold)}
code{background:var(--code);padding:.1em .35em;border-radius:4px;font-size:.92em}
kbd{border:1px solid var(--line);border-bottom-width:2px;border-radius:4px;padding:0 .35em;\
font:.9em Consolas,monospace}
img{max-width:100%;height:auto;border:1px solid var(--line);border-radius:6px}
table{border-collapse:collapse;width:100%;margin:1em 0;font-size:.95em}
th,td{border:1px solid var(--line);padding:.35em .6em;text-align:left;vertical-align:top}
th{background:var(--code)}
blockquote{margin:1em 0;padding:.2em 1em;border-left:3px solid var(--gold);color:var(--dim)}
";

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// GitHub's heading anchors: lower case, punctuation dropped, spaces to hyphens.
pub fn slug(heading: &str) -> String {
    heading
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// Where a link in the manual points once it is installed.
pub fn href(url: &str) -> String {
    if url.starts_with('#') || url.contains("://") || url.starts_with("mailto:") {
        return url.to_owned();
    }
    if let Some(rest) = url.strip_prefix("../../../") {
        return match rest {
            "PRIVACY.md" => "../../PRIVACY.txt".into(),
            "THIRD-PARTY-NOTICES.md" => "../../THIRD-PARTY-NOTICES.txt".into(),
            other => format!("{REPO}{other}"),
        };
    }
    // `../es/README.md` is the other language's page.
    url.replace("README.md", "index.html")
}

/// Inline Markdown: code, pictures, links, autolinks, bold, italic; the rest escaped.
pub fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        let tail = &rest[c.len_utf8()..];
        if c == '`' {
            if let Some(end) = tail.find('`') {
                out.push_str(&format!("<code>{}</code>", escape(&tail[..end])));
                rest = &tail[end + 1..];
                continue;
            }
        }
        let image = c == '!' && tail.starts_with('[');
        if c == '[' || image {
            let t = if image { &tail[1..] } else { tail };
            if let Some((text, url, after)) = link_parts(t) {
                if image {
                    out.push_str(&format!(
                        "<img src=\"{}\" alt=\"{}\" loading=\"lazy\">",
                        escape(&href(url)),
                        escape(text)
                    ));
                } else {
                    out.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        escape(&href(url)),
                        inline(text)
                    ));
                }
                rest = after;
                continue;
            }
        }
        if c == '<' {
            if let Some(end) = tail.find('>') {
                let u = &tail[..end];
                if u.starts_with("https://") || u.starts_with("http://") {
                    out.push_str(&format!("<a href=\"{0}\">{0}</a>", escape(u)));
                    rest = &tail[end + 1..];
                    continue;
                }
            }
        }
        if rest.starts_with("**") {
            if let Some(end) = rest[2..].find("**") {
                out.push_str(&format!("<strong>{}</strong>", inline(&rest[2..2 + end])));
                rest = &rest[4 + end..];
                continue;
            }
        }
        if c == '*' && !tail.starts_with(' ') {
            if let Some(end) = tail.find('*') {
                out.push_str(&format!("<em>{}</em>", inline(&tail[..end])));
                rest = &tail[end + 1..];
                continue;
            }
        }
        out.push_str(&escape(&c.to_string()));
        rest = tail;
    }
    out
}

/// `text](url)rest` → (text, url, rest).
fn link_parts(s: &str) -> Option<(&str, &str, &str)> {
    let close = s.find("](")?;
    let after = &s[close + 2..];
    let end = after.find(')')?;
    Some((&s[..close], &after[..end], &after[end + 1..]))
}

fn cells(line: &str) -> Vec<&str> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(str::trim).collect()
}

/// Block Markdown to HTML.
pub fn body(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, out: &mut String| {
        if !para.is_empty() {
            out.push_str(&format!("<p>{}</p>\n", inline(&para.join(" "))));
            para.clear();
        }
    };
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut out);
            i += 1;
            continue;
        }
        if let Some(level) = (1..=4).find(|n| t.starts_with(&format!("{} ", "#".repeat(*n)))) {
            flush(&mut para, &mut out);
            let text = &t[level + 1..];
            out.push_str(&format!(
                "<h{level} id=\"{}\">{}</h{level}>\n",
                slug(text),
                inline(text)
            ));
            i += 1;
            continue;
        }
        if t == "---" {
            flush(&mut para, &mut out);
            out.push_str("<hr>\n");
            i += 1;
            continue;
        }
        if t.starts_with('|') && lines.get(i + 1).is_some_and(|n| n.trim().starts_with("|-") || n.trim().starts_with("|:") || n.trim().starts_with("| -")) {
            flush(&mut para, &mut out);
            out.push_str("<table>\n<tr>");
            for c in cells(t) {
                out.push_str(&format!("<th>{}</th>", inline(c)));
            }
            out.push_str("</tr>\n");
            i += 2;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                out.push_str("<tr>");
                for c in cells(lines[i]) {
                    out.push_str(&format!("<td>{}</td>", inline(c)));
                }
                out.push_str("</tr>\n");
                i += 1;
            }
            out.push_str("</table>\n");
            continue;
        }
        let bullet = |l: &str| l.trim_start().strip_prefix("- ").map(str::to_owned);
        let number = |l: &str| {
            let l = l.trim_start();
            let dot = l.find(". ")?;
            l[..dot].chars().all(|c| c.is_ascii_digit()).then(|| l[dot + 2..].to_owned())
        };
        if bullet(line).is_some() || number(line).is_some() {
            flush(&mut para, &mut out);
            let ordered = number(line).is_some();
            let tag = if ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>\n"));
            let mut items: Vec<String> = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                let item = if ordered { number(l) } else { bullet(l) };
                if let Some(item) = item {
                    items.push(item);
                } else if !l.trim().is_empty() && l.starts_with("  ") && !items.is_empty() {
                    let last = items.last_mut().expect("an item");
                    last.push(' ');
                    last.push_str(l.trim());
                } else {
                    break;
                }
                i += 1;
            }
            for it in items {
                out.push_str(&format!("<li>{}</li>\n", inline(&it)));
            }
            out.push_str(&format!("</{tag}>\n"));
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            flush(&mut para, &mut out);
            let mut quote = vec![q.trim()];
            i += 1;
            while i < lines.len() {
                match lines[i].trim().strip_prefix('>') {
                    Some(q) => quote.push(q.trim()),
                    None => break,
                }
                i += 1;
            }
            out.push_str(&format!("<blockquote><p>{}</p></blockquote>\n", inline(&quote.join(" "))));
            continue;
        }
        para.push(t);
        i += 1;
    }
    flush(&mut para, &mut out);
    out
}
