//! Each page's `<head>` (round 39, P1-2/P1-7): title, description, canonical address,
//! hreflang alternates, Open Graph with its locale and the Spanish share image, the releases
//! feed, and the JSON-LD that tells search engines what Cantus is (free, its systems, its
//! version — read from the workspace at build time, never typed). Written by the
//! pre-renderer between `<!--page-head-->` and `<!--/page-head-->` in `index.html`.

use crate::lang::Lang;
use crate::route::{Page, Route, ORIGIN};
use crate::top::{DOWNLOAD, GITHUB, VERSION};

/// `&`, `<`, `>` and `"` escaped, for text and attribute values.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A JSON string's contents (quotes, backslashes, `<` so `</script>` cannot close the block).
fn json(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '<' => o.push_str("\\u003c"),
            '\n' => o.push_str("\\n"),
            c => o.push(c),
        }
    }
    o
}

/// The Open Graph locale of a language.
pub fn og_locale(l: Lang) -> &'static str {
    match l {
        Lang::En => "en_US",
        Lang::Es => "es_MX",
    }
}

/// The JSON-LD: Cantus as a `SoftwareApplication` at price 0.
pub fn json_ld(page: Page) -> String {
    let l = page.lang;
    let home = Page::new(l, Route::Home).url();
    let desc = Page::new(l, Route::Home).description();
    format!(
        concat!(
            r#"{{"@context":"https://schema.org","@type":"SoftwareApplication","#,
            r#""name":"Cantus","alternateName":"Cantus DAW","url":"{home}","#,
            r#""description":"{desc}","applicationCategory":"MultimediaApplication","#,
            r#""applicationSubCategory":"Digital audio workstation","#,
            r#""operatingSystem":"Windows 11, Linux, macOS (preview)","#,
            r#""softwareVersion":"{ver}","downloadUrl":"{dl}","installUrl":"{dlpage}","#,
            r#""license":"https://www.gnu.org/licenses/gpl-3.0.html","isAccessibleForFree":true,"#,
            r#""inLanguage":["en","es"],"image":"{origin}/og.png","#,
            r#""author":{{"@type":"Person","name":"Luis Ángel Almazán"}},"#,
            r#""codeRepository":"{gh}","#,
            r#""offers":{{"@type":"Offer","price":"0","priceCurrency":"USD"}}}}"#
        ),
        home = json(&home),
        desc = json(&desc),
        ver = json(VERSION),
        dl = json(DOWNLOAD),
        dlpage = json(&Page::new(l, Route::Download).url()),
        origin = ORIGIN,
        gh = json(GITHUB),
    )
}

/// The block that goes between the markers, for `page`.
pub fn block(page: Page) -> String {
    let title = esc(&page.title());
    let desc = esc(&page.description());
    let url = page.url();
    let en = page.in_lang(Lang::En).url();
    let es = page.in_lang(Lang::Es).url();
    let (image, alt) = match page.lang {
        Lang::En => ("og.png", Lang::Es),
        Lang::Es => ("og-es.png", Lang::En),
    };
    format!(
        concat!(
            "<title>{title}</title>\n",
            "    <meta name=\"description\" content=\"{desc}\" />\n",
            "    <link rel=\"canonical\" href=\"{url}\" />\n",
            "    <link rel=\"alternate\" hreflang=\"en\" href=\"{en}\" />\n",
            "    <link rel=\"alternate\" hreflang=\"es\" href=\"{es}\" />\n",
            "    <link rel=\"alternate\" hreflang=\"x-default\" href=\"{en}\" />\n",
            "    <link rel=\"alternate\" type=\"application/atom+xml\" title=\"Cantus releases\" href=\"/releases.xml\" />\n",
            "    <meta property=\"og:title\" content=\"{title}\" />\n",
            "    <meta property=\"og:description\" content=\"{desc}\" />\n",
            "    <meta property=\"og:type\" content=\"website\" />\n",
            "    <meta property=\"og:site_name\" content=\"Cantus\" />\n",
            "    <meta property=\"og:url\" content=\"{url}\" />\n",
            "    <meta property=\"og:locale\" content=\"{loc}\" />\n",
            "    <meta property=\"og:locale:alternate\" content=\"{alt}\" />\n",
            "    <meta property=\"og:image\" content=\"{origin}/{image}\" />\n",
            "    <meta property=\"og:image:width\" content=\"1200\" />\n",
            "    <meta property=\"og:image:height\" content=\"630\" />\n",
            "    <meta name=\"twitter:card\" content=\"summary_large_image\" />\n",
            "    <script type=\"application/ld+json\">{ld}</script>\n",
            "    "
        ),
        title = title,
        desc = desc,
        url = url,
        en = en,
        es = es,
        loc = og_locale(page.lang),
        alt = og_locale(alt),
        origin = ORIGIN,
        image = image,
        ld = json_ld(page),
    )
}

/// The sitemap: every page in both languages with its alternates, then the static pages
/// (`extra`: paths such as `/manual/en/index.html`), all with `lastmod`.
pub fn sitemap(lastmod: &str, extra: &[String]) -> String {
    let mut s = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" ",
        "xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n"
    ));
    for p in Page::all() {
        s.push_str(&format!(
            concat!(
                "  <url><loc>{}</loc><lastmod>{}</lastmod>",
                "<xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"{}\"/>",
                "<xhtml:link rel=\"alternate\" hreflang=\"es\" href=\"{}\"/>",
                "<xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\"/></url>\n"
            ),
            p.url(),
            lastmod,
            p.in_lang(Lang::En).url(),
            p.in_lang(Lang::Es).url(),
            p.in_lang(Lang::En).url(),
        ));
    }
    for e in extra {
        s.push_str(&format!(
            "  <url><loc>{ORIGIN}{}</loc><lastmod>{lastmod}</lastmod></url>\n",
            esc(e)
        ));
    }
    s.push_str("</urlset>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_names_both_languages_and_its_own_address() {
        let p = Page::new(Lang::Es, Route::Download);
        let b = block(p);
        assert!(b.contains(r#"<link rel="canonical" href="https://cantusdaw.com/es/download/" />"#));
        assert!(b.contains(r#"hreflang="en" href="https://cantusdaw.com/download/""#));
        assert!(b.contains(r#"hreflang="x-default" href="https://cantusdaw.com/download/""#));
        assert!(b.contains(r#"og:locale" content="es_MX""#));
        assert!(b.contains(r#"og:locale:alternate" content="en_US""#));
        assert!(b.contains("/og-es.png"));
        assert!(b.contains(r#"href="/releases.xml""#));
    }

    #[test]
    fn the_json_ld_is_free_and_carries_the_workspace_version() {
        let ld = json_ld(Page::new(Lang::En, Route::Home));
        assert!(ld.contains(r#""price":"0""#));
        assert!(ld.contains(&format!(r#""softwareVersion":"{VERSION}""#)));
        assert!(!ld.contains("</"));
        // Balanced braces: a hand-written JSON that a typo would break.
        assert_eq!(ld.matches('{').count(), ld.matches('}').count());
    }

    #[test]
    fn the_sitemap_lists_every_page_with_lastmod() {
        let s = sitemap("2026-11-09", &["/press/index.html".into()]);
        assert_eq!(s.matches("<url>").count(), Page::all().len() + 1);
        assert_eq!(
            s.matches("<lastmod>2026-11-09</lastmod>").count(),
            Page::all().len() + 1
        );
        assert!(s.contains("<loc>https://cantusdaw.com/es/instruments/unda/</loc>"));
    }
}
