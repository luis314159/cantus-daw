//! Honest social proof (round 39, P2-4): GitHub's own counts, asked at build time by
//! `web/tools/site_data.py` into `stats.json` beside the page, and the songs people made
//! with Cantus. Nothing here is typed by hand or estimated: a number shows only when the
//! file holds it and it has passed [`STARS_FROM`] / [`DOWNLOADS_FROM`]; when the file is
//! missing (a local build), unreadable, or the numbers are too small to say anything, the
//! whole strip stays hidden. The "made with Cantus" row stays hidden while [`MADE_WITH`] is
//! empty. The file comes from the site's own origin, so the visitor's browser asks no one
//! else.

use wasm_bindgen::JsCast;
use yew::prelude::*;

use crate::lang::{use_lang, Lang};
use crate::top::{GITHUB, RELEASES};

/// Where the build puts GitHub's counts (absolute: the same file for `/` and `/es/`).
pub const STATS_URL: &str = "/stats.json";
/// Stars are shown from this many on — a smaller count proves nothing.
pub const STARS_FROM: u64 = 50;
/// Downloads are shown from this many on.
pub const DOWNLOADS_FROM: u64 = 500;

/// GitHub's counts on the day of the build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub stars: Option<u64>,
    pub downloads: Option<u64>,
    /// `YYYY-MM-DD`, the day GitHub was asked.
    pub as_of: Option<String>,
}

impl Stats {
    /// The stars worth showing, if any.
    pub fn stars_shown(&self) -> Option<u64> {
        self.stars.filter(|&n| n >= STARS_FROM)
    }

    /// The downloads worth showing, if any.
    pub fn downloads_shown(&self) -> Option<u64> {
        self.downloads.filter(|&n| n >= DOWNLOADS_FROM)
    }

    /// Whether the strip has anything to say — and a date to say it with.
    pub fn any(&self) -> bool {
        self.as_of.is_some() && (self.stars_shown().is_some() || self.downloads_shown().is_some())
    }
}

/// A song made with Cantus, listed only with its author's permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Song {
    pub title: &'static str,
    pub artist: &'static str,
    /// Where it is heard (the author's own page).
    pub url: &'static str,
}

/// Songs from beta users who said yes to being listed. Empty until there are some: the row
/// stays hidden, never filled with examples.
pub const MADE_WITH: &[Song] = &[];

/// `12 345` — grouped by thin no-break spaces, as the rest of the site writes numbers.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// `2026-11-09` as "9 November 2026" / "9 de noviembre de 2026"; anything else as it is.
pub fn day(iso: &str, l: Lang) -> String {
    const EN: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const ES: [&str; 12] = [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ];
    let mut it = iso.splitn(3, '-');
    let parsed = (|| {
        let y: u32 = it.next()?.parse().ok()?;
        let m: usize = it.next()?.parse().ok()?;
        let d: u32 = it.next()?.parse().ok()?;
        (1..=12).contains(&m).then_some((y, m - 1, d))
    })();
    match (parsed, l) {
        (Some((y, m, d)), Lang::En) => format!("{d} {} {y}", EN[m]),
        (Some((y, m, d)), Lang::Es) => format!("{d} de {} de {y}", ES[m]),
        (None, _) => iso.to_string(),
    }
}

fn num(v: &wasm_bindgen::JsValue, key: &str) -> Option<u64> {
    let n = js_sys::Reflect::get(v, &key.into()).ok()?.as_f64()?;
    (n.is_finite() && n >= 0.0).then_some(n as u64)
}

async fn load() -> Option<Stats> {
    use wasm_bindgen_futures::JsFuture;
    let win = web_sys::window()?;
    let resp: web_sys::Response = JsFuture::from(win.fetch_with_str(STATS_URL))
        .await
        .ok()?
        .dyn_into()
        .ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let as_of = js_sys::Reflect::get(&json, &"as_of".into())
        .ok()?
        .as_string();
    Some(Stats {
        stars: num(&json, "stars"),
        downloads: num(&json, "downloads"),
        as_of,
    })
}

/// The proof strip: GitHub's counts and the songs made with Cantus. Renders nothing at all
/// until it has something true to show.
#[function_component(Proof)]
pub fn proof() -> Html {
    let l = use_lang().lang;
    let stats = use_state(Stats::default);
    {
        let stats = stats.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(s) = load().await {
                    stats.set(s);
                }
            });
        });
    }
    let counts = stats.any();
    if !counts && MADE_WITH.is_empty() {
        return Html::default();
    }
    let stars = stats.stars_shown().map(|n| {
        html! {
            <a class="proof-stat" href={format!("{GITHUB}/stargazers")}>
                <strong>{ grouped(n) }</strong>
                <span>{ l.t("stars on GitHub", "estrellas en GitHub") }</span>
            </a>
        }
    });
    let downloads = stats.downloads_shown().map(|n| {
        html! {
            <a class="proof-stat" href={RELEASES}>
                <strong>{ grouped(n) }</strong>
                <span>{ l.t("downloads of the release files", "descargas de los archivos publicados") }</span>
            </a>
        }
    });
    let as_of = stats
        .as_of
        .as_deref()
        .map(|d| day(d, l))
        .unwrap_or_default();
    html! {
        <section class="proof" aria-label={l.t("Who uses Cantus", "Quién usa Cantus")}>
            <div class="wrap">
                if counts {
                    <div class="proof-stats">
                        { for stars }
                        { for downloads }
                        <p class="proof-how">{ match l {
                            Lang::En => format!("GitHub's counts on {as_of}, when this page was built. The checksum files are not counted."),
                            Lang::Es => format!("Las cuentas de GitHub el {as_of}, cuando se construyó esta página. Los archivos de sumas de verificación no cuentan."),
                        } }</p>
                    </div>
                }
                <MadeWith />
            </div>
        </section>
    }
}

/// Songs made with Cantus, with their authors' permission. Hidden while there are none.
#[function_component(MadeWith)]
pub fn made_with() -> Html {
    let l = use_lang().lang;
    if MADE_WITH.is_empty() {
        return Html::default();
    }
    html! {
        <div class="proof-songs">
            <h3>{ l.t("Made with Cantus", "Hecho con Cantus") }</h3>
            <ul>
                { for MADE_WITH.iter().map(|s| html! {
                    <li><a href={s.url}><strong>{ s.title }</strong><span>{ s.artist }</span></a></li>
                }) }
            </ul>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_grouped_by_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1\u{202f}000");
        assert_eq!(grouped(1234567), "1\u{202f}234\u{202f}567");
    }

    #[test]
    fn days_read_in_each_language() {
        assert_eq!(day("2026-11-09", Lang::En), "9 November 2026");
        assert_eq!(day("2026-11-09", Lang::Es), "9 de noviembre de 2026");
        assert_eq!(day("soon", Lang::En), "soon");
        assert_eq!(day("2026-13-01", Lang::En), "2026-13-01");
    }

    #[test]
    fn nothing_shows_without_a_true_number() {
        assert!(!Stats::default().any());
        let small = Stats {
            stars: Some(4),
            downloads: Some(109),
            as_of: Some("2026-11-09".into()),
        };
        assert!(!small.any(), "too small to prove anything: hidden");
        let undated = Stats {
            stars: Some(500),
            downloads: None,
            as_of: None,
        };
        assert!(!undated.any(), "a count without its day is not shown");
        let some = Stats {
            stars: Some(STARS_FROM),
            downloads: Some(10),
            as_of: Some("2026-11-09".into()),
        };
        assert!(some.any());
        assert_eq!(some.stars_shown(), Some(STARS_FROM));
        assert_eq!(some.downloads_shown(), None);
    }

    #[test]
    fn no_song_is_listed_by_example() {
        for s in MADE_WITH {
            assert!(!s.title.is_empty() && !s.artist.is_empty() && s.url.starts_with("https://"));
        }
    }
}
