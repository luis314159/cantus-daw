//! The pre-renderer (round 39, P1-1): run natively after `trunk build`, it renders every page
//! of [`Page::all`] with Yew's `ServerRenderer` into the HTML Trunk wrote, so each address is a
//! static file whose text paints before the WebAssembly arrives and that crawlers and link
//! previews can read. It also writes `sitemap.xml`.
//!
//! ```text
//! trunk build --release
//! cargo run --features ssr -- prerender dist [--lastmod YYYY-MM-DD]
//! ```
//!
//! In the browser the app renders the same page afresh into `#app` in one task (`main.rs`), so
//! nothing has to match tag for tag: a component may read the browser (the theme, the
//! system) when it is first drawn.

use std::path::{Path, PathBuf};

use crate::app::{App, AppProps};
use crate::head;
use crate::route::{Page, Route};

const HEAD_OPEN: &str = "<!--page-head-->";
const HEAD_CLOSE: &str = "<!--/page-head-->";
const MOUNT: &str = r#"<div id="app"></div>"#;

/// Renders one page's body.
fn body(page: Page) -> String {
    futures::executor::block_on(
        yew::ServerRenderer::<App>::with_props(move || AppProps { page })
            .hydratable(false)
            .render(),
    )
}

/// Trunk's loader starts the WebAssembly at once, and preloads it. On a pre-rendered page that
/// only competes with what the visitor sees first (the hero's picture and the fonts, on a
/// slow phone the 270 KB of WebAssembly took LCP from 0.45 s observed to 4.7 s simulated):
/// the app starts once the page has loaded and the browser has reported the first picture
/// (`fetchpriority="high"`) as painted — so the app's own copy of it paints at once, from the
/// decoded image — or as soon as something larger than it has painted (a wide screen, where
/// the headline is the largest), or after 2.5 s at the latest; the preload goes. The text and
/// the links work meanwhile; the players and the rack wait.
///
/// The app's script waits with it (round 41, the site budget): Trunk imports it statically and
/// preloads it with its snippet, two requests that shared the slow phone's line with the
/// picture; here it is imported once the picture has painted, and the faces no first paint
/// uses ([`LATE_FACES`]) are declared then too.
const START_NOW: &str = "const wasm = await init(";
const START_AFTER_PAINT: &str = concat!(
    "await new Promise((go) => {\n",
    "  let done = false;\n",
    "  const start = () => {\n",
    "    if (done) return;\n",
    "    done = true;\n",
    "    // the faces the first paint does not use, now that it is done (prerender.rs)\n",
    "    const late = document.getElementById(\"late-faces\");\n",
    "    if (late) document.head.insertAdjacentHTML(\"beforeend\", late.textContent);\n",
    "    setTimeout(go, 0);\n",
    "  };\n",
    "  setTimeout(start, 2500);\n",
    "  const first = document.querySelector('img[fetchpriority=\"high\"]');\n",
    "  const painted = () => {\n",
    "    if (!first || !window.PerformanceObserver) return start();\n",
    "    // the picture's share of the screen: the largest paint is it, or something larger\n",
    "    const r = first.getBoundingClientRect();\n",
    "    const area = Math.max(0, Math.min(r.right, innerWidth) - Math.max(r.left, 0)) *\n",
    "      Math.max(0, Math.min(r.bottom, innerHeight) - Math.max(r.top, 0));\n",
    "    if (area === 0) return start();\n",
    "    try {\n",
    "      new PerformanceObserver((list) => {\n",
    "        if (list.getEntries().some((e) => e.element === first || e.size >= area)) start();\n",
    "      }).observe({ type: \"largest-contentful-paint\", buffered: true });\n",
    "    } catch (e) { start(); }\n",
    "  };\n",
    "  if (document.readyState === \"complete\") painted(); else addEventListener(\"load\", painted, { once: true });\n",
    "});\n",
);
/// Trunk's static import of the app's script: `import init, * as bindings from '/….js';`.
const IMPORT_OPEN: &str = "import init, * as bindings from '";

/// The template with the app's start deferred (see [`START_AFTER_PAINT`]).
pub fn defer_start(template: &str) -> Result<String, String> {
    if !template.contains(START_NOW) {
        return Err("no trunk loader (`const wasm = await init(`) in the template".into());
    }
    let i = template
        .find(IMPORT_OPEN)
        .ok_or("no trunk import (`import init, * as bindings from '…'`) in the template")?;
    let from = i + IMPORT_OPEN.len();
    let to = template[from..]
        .find("';")
        .map(|j| from + j)
        .ok_or("an unclosed trunk import")?;
    let script = template[from..to].to_string();
    let mut line_end = to + 2;
    while template[line_end..].starts_with(['\r', '\n']) {
        line_end += 1;
    }
    let mut t = template[..i].to_string() + &template[line_end..];
    let start = format!(
        "{START_AFTER_PAINT}const bindings = await import('{script}');\nconst init = bindings.default;\n{START_NOW}"
    );
    t = t.replacen(START_NOW, &start, 1);
    // `<link rel="preload" href="/cantus-web-….wasm" … as="fetch" type="application/wasm">`,
    // and `<link rel="modulepreload" href="/cantus-web-….js" …>` with its snippets'.
    for open in [
        r#"<link rel="preload" href="/cantus-web-"#,
        r#"<link rel="modulepreload" href="/cantus-web-"#,
        r#"<link rel="modulepreload" href="/snippets/"#,
    ] {
        while let Some(i) = t.find(open) {
            let end = t[i..]
                .find('>')
                .map(|j| i + j + 1)
                .ok_or("an unclosed preload")?;
            if open.contains("\"preload\"") && !t[i..end].contains("application/wasm") {
                break;
            }
            t.replace_range(i..end, "");
        }
    }
    Ok(t)
}

/// The faces no page's first paint uses (round 41, the site budget): Light (300) sets only the
/// rack's tempo, below the first screen, yet the first layout asked for it beside the hero's
/// picture. Their `@font-face` rules leave the inlined style for `<noscript id="late-faces">`:
/// without script it applies as it is; with script, its text is declared once the first
/// picture has painted ([`START_AFTER_PAINT`]) — the same face, from the same file.
const LATE_FACES: &[&str] = &["/fonts/barlow-semi-condensed-light.woff2"];

/// The template with [`LATE_FACES`] declared after the first paint.
pub fn defer_faces(template: &str) -> Result<String, String> {
    let mut t = template.to_string();
    let mut moved = String::new();
    for url in LATE_FACES {
        let style = t.find("<style").ok_or("no <style> in the template")?;
        let at = t[style..]
            .find(url)
            .map(|j| style + j)
            .ok_or_else(|| format!("no @font-face for {url} in the template's style"))?;
        let open = t[..at]
            .rfind("@font-face")
            .ok_or_else(|| format!("{url} is not in an @font-face"))?;
        let close = t[at..]
            .find('}')
            .map(|j| at + j + 1)
            .ok_or("an unclosed @font-face")?;
        if t[open..at].contains('}') {
            return Err(format!("{url} is not in an @font-face"));
        }
        moved.push_str(&t[open..close]);
        t.replace_range(open..close, "");
    }
    let head = t.find("</head>").ok_or("no </head> in the template")?;
    t.insert_str(
        head,
        &format!("<noscript id=\"late-faces\"><style>{moved}</style></noscript>"),
    );
    Ok(t)
}

/// Every inlined `<style>` of the template minified ([`minify_css`]): Trunk inlines the style
/// sheets as written, comments and all, and the page's first 14 KB, 29 KB and 44 KB each cost
/// a slow phone one more round trip before anything else can start (TCP's slow start: the home
/// at 44.1 KB took 3, the beta.16 one 2 — the site budget's LCP, 1.88 s then, 2.11 s now).
pub fn minify_styles(template: &str) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(i) = rest.find("<style>") {
        let from = i + "<style>".len();
        let to = rest[from..]
            .find("</style>")
            .map(|j| from + j)
            .ok_or("an unclosed <style>")?;
        out.push_str(&rest[..from]);
        out.push_str(&minify_css(&rest[from..to]));
        rest = &rest[to..];
    }
    out.push_str(rest);
    Ok(out)
}

/// CSS without its comments and the white space nothing reads: each run of white space becomes
/// one space, and none is kept beside `{`, `}` or `;`, nor a `;` before `}`. Strings and
/// `url(…)` are copied as they are, and so is the space beside a comma (a custom property's
/// value keeps it as written). What the browser parses is unchanged: its CSSOM is the same.
pub fn minify_css(css: &str) -> String {
    let b = css.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut space = false;
    let tight = |c: u8| matches!(c, b'{' | b'}' | b';');
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // a comment is no white space: `.a/**/.b` stays one compound selector
            i = css[i + 2..].find("*/").map_or(b.len(), |j| i + 2 + j + 2);
            continue;
        }
        if c.is_ascii_whitespace() {
            space = true;
            i += 1;
            continue;
        }
        if space && !out.is_empty() && !tight(c) && !tight(*out.last().unwrap()) {
            out.push(b' ');
        }
        space = false;
        if c == b'}' && out.last() == Some(&b';') {
            out.pop();
        }
        let raw_end = if c == b'"' || c == b'\'' {
            // a string, to its closing quote (a backslash escapes the next byte)
            let mut j = i + 1;
            while j < b.len() && b[j] != c {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            Some((j + 1).min(b.len()))
        } else if b[i..].starts_with(b"url(") {
            // an unquoted address may hold `/*`; a quoted one is a string just after
            let j = i + 4;
            if matches!(b.get(j), Some(b'"' | b'\'')) {
                None
            } else {
                Some(css[j..].find(')').map_or(b.len(), |k| j + k + 1))
            }
        } else {
            None
        };
        match raw_end {
            Some(end) => {
                out.extend_from_slice(&b[i..end]);
                i = end;
            }
            None => {
                out.push(c);
                i += 1;
            }
        }
    }
    // Only ASCII bytes were dropped or added, so the bytes are still UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| css.to_string())
}

/// Trunk's `index.html` with `page`'s head and body in it.
pub fn fill(template: &str, page: Page) -> Result<String, String> {
    let a = template
        .find(HEAD_OPEN)
        .ok_or("no <!--page-head--> in the template")?;
    let b = template[a..]
        .find(HEAD_CLOSE)
        .map(|i| a + i)
        .ok_or("no <!--/page-head--> in the template")?;
    let mut out = String::with_capacity(template.len() + 64 * 1024);
    out.push_str(&template[..a + HEAD_OPEN.len()]);
    out.push_str(&head::block(page));
    out.push_str(&template[b..]);
    let out = out.replacen(
        "<html lang=\"en\">",
        &format!("<html lang=\"{}\">", page.lang.code()),
        1,
    );
    if !out.contains(MOUNT) {
        return Err(format!("no {MOUNT} in the template"));
    }
    Ok(out.replacen(MOUNT, &format!(r#"<div id="app">{}</div>"#, body(page)), 1))
}

/// The static pages beside the app's, for the sitemap: the manuals and the press kit.
fn static_pages(dist: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for lang in ["en", "es"] {
        let dir = dist.join("manual").join(lang);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".html"))
            .collect();
        names.sort();
        v.extend(names.into_iter().map(|n| format!("/manual/{lang}/{n}")));
    }
    if dist.join("press/index.html").is_file() {
        v.push("/press/index.html".into());
    }
    v
}

/// Today, `YYYY-MM-DD` (UTC), when no `--lastmod` is given.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut dist = PathBuf::from("dist");
    let mut lastmod = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lastmod" => lastmod = it.next().cloned(),
            other => dist = PathBuf::from(other),
        }
    }
    let lastmod = lastmod.unwrap_or_else(today);
    let index = dist.join("index.html");
    let template =
        std::fs::read_to_string(&index).map_err(|e| format!("read {}: {e}", index.display()))?;
    // Trunk's page is kept beside the build, so a second run (after the first overwrote
    // dist/index.html with the home) starts from it again.
    let kept_dir = dist.parent().unwrap_or(Path::new(".")).join("target");
    let kept = kept_dir.join("trunk-index.html");
    let template = if template.contains(MOUNT) {
        let _ = std::fs::create_dir_all(&kept_dir);
        let _ = std::fs::write(&kept, &template);
        template
    } else {
        std::fs::read_to_string(&kept).map_err(|_| {
            format!(
                "{} is already pre-rendered and Trunk's page is not kept: run `trunk build` first",
                index.display()
            )
        })?
    };
    let template = defer_start(&defer_faces(&minify_styles(&template)?)?)?;
    let pages = Page::all();
    for &page in &pages {
        let html = fill(&template, page)?;
        let dir = dist.join(page.path().trim_start_matches('/'));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join("index.html");
        std::fs::write(&file, html).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    // The proof strip asks for stats.json (web-extra's tools/site_data.py writes it after this
    // step); until then, or when GitHub did not answer, an empty one: the strip stays hidden
    // and the browser logs no 404.
    let stats = dist.join("stats.json");
    if !stats.exists() {
        std::fs::write(
            &stats, "{}
",
        )
        .map_err(|e| format!("{}: {e}", stats.display()))?;
    }
    // The units' long text, which the WebAssembly leaves out and fetches (unit_text.rs).
    let text = dist.join(crate::unit_text::URL.trim_start_matches('/'));
    std::fs::write(&text, crate::unit_text::json())
        .map_err(|e| format!("{}: {e}", text.display()))?;
    // Each plug-in page's body, which that page fetches (pages/plugins.rs, round 42).
    for &page in &pages {
        let file = match page.route {
            Route::Plugins => None,
            Route::Plugin(f) => Some(f),
            _ => continue,
        };
        let part =
            dist.join(crate::pages::plugins::part_url(page.lang, file).trim_start_matches('/'));
        if let Some(dir) = part.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&part, crate::pages::plugins::parts_json(page.lang, file))
            .map_err(|e| format!("{}: {e}", part.display()))?;
    }
    let extra = static_pages(&dist);
    let map = dist.join("sitemap.xml");
    std::fs::write(&map, head::sitemap(&lastmod, &extra))
        .map_err(|e| format!("{}: {e}", map.display()))?;
    println!(
        "pre-rendered {} pages and a sitemap of {} addresses into {} (lastmod {lastmod})",
        pages.len(),
        pages.len() + extra.len(),
        dist.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::route::Route;

    const TEMPLATE: &str = "<!doctype html><html lang=\"en\"><head><!--page-head--><title>x</title><!--/page-head--></head><body><div id=\"app\"></div></body></html>";

    #[test]
    fn every_page_renders_its_text_into_the_html() {
        for page in Page::all() {
            let html = fill(TEMPLATE, page).unwrap_or_else(|e| panic!("{page:?}: {e}"));
            assert!(html.contains(&format!("<html lang=\"{}\">", page.lang.code())));
            assert!(html.contains(&format!("<title>{}</title>", head::esc(&page.title()))));
            assert!(html.contains("<footer"), "{page:?}");
            assert_eq!(html.matches("<h1").count(), 1, "{page:?}: one <h1>");
        }
    }

    #[test]
    fn the_home_says_what_it_is_in_its_language() {
        let en = fill(TEMPLATE, Page::new(Lang::En, Route::Home)).unwrap();
        let es = fill(TEMPLATE, Page::new(Lang::Es, Route::Home)).unwrap();
        assert!(en.contains("Finish the song"));
        assert!(es.contains("Termina la canción"));
        assert!(!es.contains("Finish the song"));
    }

    #[test]
    fn the_app_starts_after_the_first_paint_and_is_not_preloaded() {
        // Trunk 0.21's loader and preloads, as it writes them.
        let t = concat!(
            "<head><link rel=\"modulepreload\" href=\"/cantus-web-1.js\" crossorigin=\"anonymous\" integrity=\"sha384-x\">",
            "<link rel=\"modulepreload\" href=\"/snippets/cantus-web-2/inline0.js\" crossorigin=\"anonymous\" integrity=\"sha384-y\">",
            "<link rel=\"preload\" href=\"/cantus-web-1_bg.wasm\" crossorigin=\"anonymous\" integrity=\"sha384-z\" as=\"fetch\" type=\"application/wasm\">",
            "<script type=\"module\">\nimport init, * as bindings from '/cantus-web-1.js';\n",
            "const wasm = await init({ module_or_path: '/cantus-web-1_bg.wasm' });\n",
            "window.wasmBindings = bindings;\n</script></head>"
        );
        let d = defer_start(t).unwrap();
        assert!(d.contains("addEventListener(\"load\"") && d.contains("largest-contentful-paint"));
        // nothing of the app's is asked for before the picture has painted
        assert!(!d.contains("application/wasm"));
        assert!(!d.contains("modulepreload"));
        assert!(!d.contains("import init"));
        let wait = d.find("addEventListener(\"load\"").unwrap();
        let import = d
            .find("const bindings = await import('/cantus-web-1.js');")
            .unwrap();
        let init = d.find("const init = bindings.default;").unwrap();
        let run = d
            .find("const wasm = await init({ module_or_path: '/cantus-web-1_bg.wasm' });")
            .unwrap();
        assert!(wait < import && import < init && init < run);
        assert!(d.contains("window.wasmBindings = bindings;"));
        // and the late faces are declared when it starts
        assert!(d.find("late-faces").unwrap() < wait);
        assert!(defer_start("<html></html>").is_err());
        assert!(defer_start("<script>const wasm = await init({});</script>").is_err());
    }

    #[test]
    fn the_light_face_waits_for_the_first_paint() {
        let t = concat!(
            "<head><style>@font-face{font-family:\"B\";src:url(\"/fonts/barlow-semi-condensed-regular.woff2\") format(\"woff2\");font-weight:400}",
            "@font-face{font-family:\"B\";src:url(\"/fonts/barlow-semi-condensed-light.woff2\") format(\"woff2\");font-weight:300}",
            ".tempo output{font:300 1.5rem/1 B}</style></head><body></body>"
        );
        let d = defer_faces(t).unwrap();
        let (head, body) = d.split_once("<noscript id=\"late-faces\">").unwrap();
        assert!(!head.contains("light.woff2") && head.contains("regular.woff2"));
        assert!(head.contains(".tempo output{font:300 1.5rem/1 B}"));
        assert_eq!(
            body,
            "<style>@font-face{font-family:\"B\";src:url(\"/fonts/barlow-semi-condensed-light.woff2\") format(\"woff2\");font-weight:300}</style></noscript></head><body></body>"
        );
        assert!(defer_faces("<head><style>a{}</style></head>").is_err());
    }

    #[test]
    fn the_inlined_style_loses_only_what_nothing_reads() {
        assert_eq!(
            minify_css("/* a comment */\r\n.a , .b > .c {\r\n  color : red ;\r\n  margin: 0 auto;\r\n}\r\n"),
            ".a , .b > .c{color : red;margin: 0 auto}"
        );
        assert_eq!(
            minify_css("@media (min-width: 52rem) and (prefers-reduced-motion: no-preference) {\n  .x { width: calc(100% - 2rem); }\n}"),
            "@media (min-width: 52rem) and (prefers-reduced-motion: no-preference){.x{width: calc(100% - 2rem)}}"
        );
        // strings and addresses as they are, comment-like text and spaces included
        assert_eq!(
            minify_css(".q::before { content: \"a  /* b */ ; }\"; background: url(data:image/svg+xml,%3Csvg/*x*/) , url('x  y.png'); }"),
            ".q::before{content: \"a  /* b */ ; }\";background: url(data:image/svg+xml,%3Csvg/*x*/) , url('x  y.png')}"
        );
        assert_eq!(
            minify_css("a { content: 'it\\'s  ok' }"),
            "a{content: 'it\\'s  ok'}"
        );
        // a comment is no white space, and white space beside one stays one space
        assert_eq!(minify_css(".a/**/.b{x:1}"), ".a.b{x:1}");
        assert_eq!(minify_css(".a /* c */ .b{x:1}"), ".a .b{x:1}");
        assert_eq!(minify_css("a{b:c}  "), "a{b:c}");
        assert_eq!(minify_css(".é ,\r\n  .ü{x:«1» }"), ".é , .ü{x:«1»}");
        let page = "<head><style>\n/* x */ a { b: c; }\n</style><style> d { e: f } </style></head><body> a  b </body>";
        assert_eq!(
            minify_styles(page).unwrap(),
            "<head><style>a{b: c}</style><style>d{e: f}</style></head><body> a  b </body>"
        );
    }

    #[test]
    fn today_is_a_date() {
        let t = today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
