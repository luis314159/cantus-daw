//! Which systems the latest release has a file for, asked of GitHub from the visitor's
//! browser (ADR 0067 §4). The download card says "Available" for Linux or macOS only when
//! the latest release holds that system's file under its stable name; until the answer
//! comes, when it fails, or when GitHub is unreachable, it says "Coming soon" — never a
//! link that would answer 404. The answer is kept an hour in the browser (GitHub allows 60
//! unauthenticated requests an hour per address), and nothing is sent but the request.

use wasm_bindgen::JsCast;
use yew::prelude::*;

use crate::top::{LATEST_API, STABLE_APPIMAGE, STABLE_DMG};

/// The systems beside Windows whose file the latest release holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shelf {
    pub linux: bool,
    pub macos: bool,
}

/// How long an answer is trusted, in milliseconds.
pub const KEEP_MS: f64 = 3_600_000.0;
const KEY: &str = "cantus-latest-shelf";

impl Shelf {
    /// From the names of the latest release's files.
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Shelf {
        let mut s = Shelf::default();
        for n in names {
            s.linux |= n == STABLE_APPIMAGE;
            s.macos |= n == STABLE_DMG;
        }
        s
    }

    /// `<when, ms>|<linux 0/1>|<macos 0/1>`, as kept in the browser.
    pub fn encode(self, now: f64) -> String {
        format!("{}|{}|{}", now as u64, self.linux as u8, self.macos as u8)
    }

    /// A kept answer, if it is younger than [`KEEP_MS`] at `now`.
    pub fn decode(text: &str, now: f64) -> Option<Shelf> {
        let mut it = text.split('|');
        let when: f64 = it.next()?.parse::<u64>().ok()? as f64;
        let flag = |s: Option<&str>| match s {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        };
        let linux = flag(it.next())?;
        let macos = flag(it.next())?;
        (it.next().is_none() && now >= when && now - when < KEEP_MS)
            .then_some(Shelf { linux, macos })
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn kept(now: f64) -> Option<Shelf> {
    let text = storage()?.get_item(KEY).ok().flatten()?;
    Shelf::decode(&text, now)
}

/// Ask GitHub for the latest release's file names.
async fn ask() -> Option<Shelf> {
    use wasm_bindgen_futures::JsFuture;
    let win = web_sys::window()?;
    let resp: web_sys::Response = JsFuture::from(win.fetch_with_str(LATEST_API))
        .await
        .ok()?
        .dyn_into()
        .ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let assets = js_sys::Reflect::get(&json, &"assets".into()).ok()?;
    let assets: js_sys::Array = assets.dyn_into().ok()?;
    let names: Vec<String> = assets
        .iter()
        .filter_map(|a| js_sys::Reflect::get(&a, &"name".into()).ok()?.as_string())
        .collect();
    Some(Shelf::from_names(names.iter().map(String::as_str)))
}

/// One request per page, however many components want the answer: the first asks, the
/// others wait for the same answer, and later ones get it at once.
#[derive(Debug)]
pub struct Once<T> {
    asked: bool,
    waiting: Vec<T>,
    answer: Option<Shelf>,
}

/// What a component joining [`Once`] does.
#[derive(Debug, PartialEq, Eq)]
pub enum Join {
    /// Send the request (it is the first).
    Ask,
    /// A request is on its way: wait for it.
    Wait,
    /// The answer is known.
    Known(Shelf),
}

impl<T> Default for Once<T> {
    fn default() -> Self {
        Once {
            asked: false,
            waiting: Vec::new(),
            answer: None,
        }
    }
}

impl<T> Once<T> {
    pub fn join(&mut self, who: T) -> Join {
        if let Some(s) = self.answer {
            return Join::Known(s);
        }
        self.waiting.push(who);
        if self.asked {
            Join::Wait
        } else {
            self.asked = true;
            Join::Ask
        }
    }

    /// The answer came (`None`: the request failed — "Coming soon" stays, and it is not
    /// asked again this visit). Returns who to tell.
    pub fn answer(&mut self, s: Option<Shelf>) -> Vec<T> {
        self.answer = Some(s.unwrap_or_default());
        let told = std::mem::take(&mut self.waiting);
        if s.is_some() {
            told
        } else {
            Vec::new()
        }
    }
}

thread_local! {
    static ONCE: std::cell::RefCell<Once<UseStateSetter<Shelf>>> =
        std::cell::RefCell::new(Once::default());
}

/// The shelf, kept or asked for once per visit (one request for the whole page);
/// "nothing but Windows" until known.
#[hook]
pub fn use_shelf() -> Shelf {
    let now = js_sys::Date::now();
    let shelf = use_state(|| kept(now).unwrap_or_default());
    {
        let setter = shelf.setter();
        use_effect_with((), move |_| {
            if kept(now).is_none() {
                match ONCE.with(|o| o.borrow_mut().join(setter.clone())) {
                    Join::Known(s) => setter.set(s),
                    Join::Wait => {}
                    Join::Ask => wasm_bindgen_futures::spawn_local(async move {
                        let got = ask().await;
                        if let (Some(s), Some(store)) = (got, storage()) {
                            let _ = store.set_item(KEY, &s.encode(js_sys::Date::now()));
                        }
                        for who in ONCE.with(|o| o.borrow_mut().answer(got)) {
                            who.set(got.unwrap_or_default());
                        }
                    }),
                }
            }
            || ()
        });
    }
    *shelf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_is_available_only_when_its_stable_file_is_there() {
        let none = Shelf::from_names(["Cantus-windows-x64-setup.exe", "SHA256SUMS.txt"]);
        assert_eq!(none, Shelf::default());
        // The versioned file alone is not what the button links.
        let versioned = Shelf::from_names(["Cantus-0.2.0-linux-x86_64.AppImage"]);
        assert!(!versioned.linux);
        let both = Shelf::from_names([STABLE_APPIMAGE, STABLE_DMG]);
        assert_eq!(
            both,
            Shelf {
                linux: true,
                macos: true
            }
        );
        assert_eq!(
            Shelf::from_names([STABLE_DMG]),
            Shelf {
                linux: false,
                macos: true
            }
        );
    }

    #[test]
    fn the_page_asks_github_once_however_many_cards_want_to_know() {
        let mut once: Once<&str> = Once::default();
        assert_eq!(once.join("get"), Join::Ask);
        assert_eq!(once.join("ahead"), Join::Wait);
        let yes = Shelf {
            linux: true,
            macos: true,
        };
        assert_eq!(once.answer(Some(yes)), ["get", "ahead"]);
        // A card drawn later is told at once, with no request.
        assert_eq!(once.join("late"), Join::Known(yes));
        // A failed request: nobody is told anything new, and it is not asked again.
        let mut failed: Once<&str> = Once::default();
        assert_eq!(failed.join("get"), Join::Ask);
        assert!(failed.answer(None).is_empty());
        assert_eq!(failed.join("ahead"), Join::Known(Shelf::default()));
    }

    #[test]
    fn a_kept_answer_lasts_an_hour_and_nothing_malformed_is_believed() {
        let s = Shelf {
            linux: true,
            macos: false,
        };
        let text = s.encode(1_000.0);
        assert_eq!(text, "1000|1|0");
        assert_eq!(Shelf::decode(&text, 1_000.0), Some(s));
        assert_eq!(Shelf::decode(&text, 1_000.0 + KEEP_MS - 1.0), Some(s));
        assert_eq!(Shelf::decode(&text, 1_000.0 + KEEP_MS), None);
        // A clock set back: asked again.
        assert_eq!(Shelf::decode(&text, 999.0), None);
        for bad in ["", "1000", "1000|1", "1000|2|0", "x|1|1", "1000|1|1|1"] {
            assert_eq!(Shelf::decode(bad, 1_000.0), None, "{bad}");
        }
    }
}
