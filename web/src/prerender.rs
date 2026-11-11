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
use crate::route::Page;

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
/// the headline is the largest), or after 2.5 s at the latest; the preload goes. The text and the links work meanwhile; the players and the rack wait.
const START_NOW: &str = "const wasm = await init(";
const START_AFTER_PAINT: &str = concat!(
    "await new Promise((go) => {
",
    "  let done = false;
",
    "  const start = () => { if (!done) { done = true; setTimeout(go, 0); } };
",
    "  setTimeout(start, 2500);
",
    "  const first = document.querySelector('img[fetchpriority=\"high\"]');
",
    "  const painted = () => {
",
    "    if (!first || !window.PerformanceObserver) return start();
",
    "    // the picture's share of the screen: the largest paint is it, or something larger
",
    "    const r = first.getBoundingClientRect();
",
    "    const area = Math.max(0, Math.min(r.right, innerWidth) - Math.max(r.left, 0)) *
",
    "      Math.max(0, Math.min(r.bottom, innerHeight) - Math.max(r.top, 0));
",
    "    if (area === 0) return start();
",
    "    try {
",
    "      new PerformanceObserver((list) => {
",
    "        if (list.getEntries().some((e) => e.element === first || e.size >= area)) start();
",
    "      }).observe({ type: \"largest-contentful-paint\", buffered: true });
",
    "    } catch (e) { start(); }
",
    "  };
",
    "  if (document.readyState === \"complete\") painted(); else addEventListener(\"load\", painted, { once: true });
",
    "});
",
    "const wasm = await init("
);

/// The template with the app's start deferred (see [`START_AFTER_PAINT`]).
pub fn defer_start(template: &str) -> Result<String, String> {
    if !template.contains(START_NOW) {
        return Err("no trunk loader (`const wasm = await init(`) in the template".into());
    }
    let mut t = template.replacen(START_NOW, START_AFTER_PAINT, 1);
    // `<link rel="preload" href="/cantus-web-….wasm" … as="fetch" type="application/wasm">`
    while let Some(i) = t.find(r#"<link rel="preload" href="/cantus-web-"#) {
        let end = t[i..]
            .find('>')
            .map(|j| i + j + 1)
            .ok_or("an unclosed preload")?;
        if !t[i..end].contains("application/wasm") {
            break;
        }
        t.replace_range(i..end, "");
    }
    Ok(t)
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
    let template = defer_start(&template)?;
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
        let t = concat!(
            "<head><script type=\"module\">
import init from '/cantus-web-1.js';
",
            "const wasm = await init({ module_or_path: '/cantus-web-1_bg.wasm' });
</script>",
            "<link rel=\"modulepreload\" href=\"/cantus-web-1.js\">",
            "<link rel=\"preload\" href=\"/cantus-web-1_bg.wasm\" as=\"fetch\" type=\"application/wasm\"></head>"
        );
        let d = defer_start(t).unwrap();
        assert!(d.contains("addEventListener(\"load\"") && d.contains("largest-contentful-paint"));
        assert!(d.find("addEventListener").unwrap() < d.find("const wasm = await init(").unwrap());
        assert!(!d.contains("application/wasm"));
        assert!(d.contains("modulepreload"));
        assert!(defer_start("<html></html>").is_err());
    }

    #[test]
    fn today_is_a_date() {
        let t = today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
