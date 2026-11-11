//! Moving between pages without a reload (round 39, P1-3). Every page is also a static file
//! (`prerender.rs`), so a link is a plain `<a href="/es/download/">`: it works before the
//! WebAssembly arrives, for crawlers and in a new tab. Once the app runs, one listener on the
//! document turns a plain left click on a link to another of the site's pages into a history
//! entry and a render, and the back button into the page before. No component needs a
//! special link type.

use yew::prelude::*;

use crate::lang::Lang;
use crate::route::{Page, Route};

/// How a page was reached, which decides where it scrolls once drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Arrive {
    /// The first load: the browser has already put the static page where the address says.
    Loaded,
    /// A link: to the `#chapter` the address names, else to the top.
    Fresh,
    /// Back or forward: where the visitor left it.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the browser's only
    At(f64),
}

/// The page shown, how it was reached, and a count that makes every visit new (a link to the
/// page already shown still scrolls).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Visit {
    pub page: Page,
    pub arrive: Arrive,
    pub seq: u32,
}

/// The page shown, and a way to go to another.
#[derive(Clone, PartialEq)]
pub struct RouteCtx {
    pub page: Page,
    pub go: Callback<Page>,
}

#[hook]
pub fn use_page() -> Page {
    use_context::<RouteCtx>()
        .map(|c| c.page)
        .unwrap_or(Page::new(Lang::En, Route::Home))
}

#[hook]
pub fn use_route() -> RouteCtx {
    use_context::<RouteCtx>().expect("RouteCtx is provided by <App/>")
}

/// The browser's side: history, the click listener, the title. Nothing of it runs in the
/// pre-renderer (effects never do there).
#[cfg(target_arch = "wasm32")]
pub mod browser {
    use super::*;
    use wasm_bindgen::{closure::Closure, JsCast};

    /// Pushes `page` (with `hash`, if any) on the history, after noting in the entry being
    /// left how far it was scrolled (the back button returns there); the address already shown
    /// is not pushed twice.
    pub fn push(page: Page, hash: &str) {
        let Some(win) = web_sys::window() else { return };
        let Ok(history) = win.history() else { return };
        let y = wasm_bindgen::JsValue::from_f64(win.scroll_y().unwrap_or(0.0));
        let _ = history.replace_state(&y, "");
        let url = format!("{}{hash}", page.path());
        let loc = win.location();
        let now = format!(
            "{}{}",
            loc.pathname().unwrap_or_default(),
            loc.hash().unwrap_or_default()
        );
        if now != url {
            let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
        }
    }

    /// After a page is rendered: its title, its description, the document's language, and
    /// the scroll — to the `#chapter` the address names, else to the top.
    pub fn arrived(page: Page, arrive: Arrive) {
        let Some(win) = web_sys::window() else { return };
        let Some(doc) = win.document() else { return };
        doc.set_title(&page.title());
        if let Some(root) = doc.document_element() {
            let _ = root.set_attribute("lang", page.lang.code());
        }
        if let Ok(Some(m)) = doc.query_selector("meta[name=description]") {
            let _ = m.set_attribute("content", &page.description());
        }
        match arrive {
            Arrive::Loaded => return,
            Arrive::At(y) => return jump(&win, y),
            Arrive::Fresh => {}
        }
        let hash = win.location().hash().unwrap_or_default();
        let target = hash
            .strip_prefix('#')
            .filter(|h| !h.is_empty())
            .and_then(|h| doc.get_element_by_id(h));
        match target {
            Some(e) => {
                e.scroll_into_view();
                // The bands before it are laid out as they come near (`content-visibility`),
                // so their real heights may move it: aim again on the next frame.
                let again = Closure::once_into_js(move || e.scroll_into_view());
                let _ = win.request_animation_frame(again.unchecked_ref());
            }
            None => jump(&win, 0.0),
        }
    }

    /// To `y` at once — the page's smooth scrolling is for chapters, not for arriving.
    fn jump(win: &web_sys::Window, y: f64) {
        let o = web_sys::ScrollToOptions::new();
        o.set_top(y);
        o.set_behavior(web_sys::ScrollBehavior::Instant);
        win.scroll_to_with_scroll_to_options(&o);
    }

    /// The page a click on `e` should open in place, with its `#hash`, or `None` to let the
    /// browser do what it does (a new tab, another site, a file).
    fn claimed(e: &web_sys::MouseEvent) -> Option<(Page, String)> {
        if e.default_prevented()
            || e.button() != 0
            || e.ctrl_key()
            || e.meta_key()
            || e.shift_key()
            || e.alt_key()
        {
            return None;
        }
        let mut node = e.target()?.dyn_into::<web_sys::Element>().ok()?;
        let a = loop {
            if node.tag_name().eq_ignore_ascii_case("a") {
                break node;
            }
            node = node.parent_element()?;
        };
        if a.has_attribute("download") || a.get_attribute("target").is_some_and(|t| t != "_self") {
            return None;
        }
        let a: web_sys::HtmlAnchorElement = a.dyn_into().ok()?;
        let loc = web_sys::window()?.location();
        if a.origin() != loc.origin().ok()? {
            return None;
        }
        let path = a.pathname();
        let page = Page::parse(&path)?;
        // A chapter of this same page too: the app scrolls to it, and the entry it leaves keeps
        // where the visitor was, so the back button returns there (the browser's own
        // fragment jump would race the app's scroll).
        Some((page, a.hash()))
    }

    /// The document's click listener and the back button, for the app's whole life.
    pub fn listen(go: Callback<(Page, Arrive)>) {
        let Some(win) = web_sys::window() else { return };
        let Some(doc) = win.document() else { return };
        // The app puts each page where it belongs (`arrived`); the browser's own restoring
        // would race it on back and forward.
        if let Ok(h) = win.history() {
            let _ = h.set_scroll_restoration(web_sys::ScrollRestoration::Manual);
        }
        let go2 = go.clone();
        let click =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                if let Some((page, hash)) = claimed(&e) {
                    e.prevent_default();
                    push(page, &hash);
                    go2.emit((page, Arrive::Fresh));
                }
            });
        let _ = doc.add_event_listener_with_callback("click", click.as_ref().unchecked_ref());
        click.forget();
        let w3 = win.clone();
        let pop =
            Closure::<dyn FnMut(web_sys::PopStateEvent)>::new(move |e: web_sys::PopStateEvent| {
                let path = w3.location().pathname().unwrap_or_default();
                if let Some(page) = Page::parse(&path) {
                    let arrive = e.state().as_f64().map_or(Arrive::Fresh, Arrive::At);
                    go.emit((page, arrive));
                }
            });
        let _ = win.add_event_listener_with_callback("popstate", pop.as_ref().unchecked_ref());
        pop.forget();
    }
}
