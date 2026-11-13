//! Day and night. The choice is remembered; without one, the system's preference decides.
//! (`index.html` applies it before the first paint, so there is no flash.) The nav's and the
//! footer's switches share one [`ThemeCtx`], provided by `<App/>`.

use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Night,
    Day,
}

const KEY: &str = "cantus-theme";

impl Theme {
    /// The page's theme; night in the pre-renderer (the page's script corrects it before the
    /// first paint, and the app reads it again when it starts).
    pub fn detect() -> Self {
        if !cfg!(target_arch = "wasm32") {
            return Theme::Night;
        }
        let Some(win) = web_sys::window() else {
            return Theme::Night;
        };
        if let Ok(Some(store)) = win.local_storage() {
            match store.get_item(KEY) {
                Ok(Some(v)) if v == "light" => return Theme::Day,
                Ok(Some(v)) if v == "dark" => return Theme::Night,
                _ => {}
            }
        }
        match win.match_media("(prefers-color-scheme: light)") {
            Ok(Some(q)) if q.matches() => Theme::Day,
            _ => Theme::Night,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Theme::Day => "light",
            Theme::Night => "dark",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Theme::Day => Theme::Night,
            Theme::Night => Theme::Day,
        }
    }

    /// Sets `data-theme` on the page and remembers the choice. A switch the visitor flips
    /// (`remember`) cross-fades the page where the browser has view transitions (motion.css
    /// sets the fade, and takes it away with reduced motion); elsewhere it changes at once.
    pub fn apply(self, remember: bool) {
        if !cfg!(target_arch = "wasm32") {
            return;
        }
        let Some(win) = web_sys::window() else { return };
        let name = self.name();
        let set = move || {
            if let Some(root) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.document_element())
            {
                let _ = root.set_attribute("data-theme", name);
            }
        };
        let faded = remember
            && win.document().is_some_and(|doc| {
                use wasm_bindgen::JsCast;
                js_sys::Reflect::get(&doc, &"startViewTransition".into())
                    .ok()
                    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
                    .is_some_and(|f| {
                        let cb = wasm_bindgen::closure::Closure::once_into_js(set);
                        f.call1(&doc, &cb).is_ok()
                    })
            });
        if !faded {
            set();
        }
        if remember {
            if let Ok(Some(store)) = win.local_storage() {
                let _ = store.set_item(KEY, self.name());
            }
        }
    }
}

/// The theme, and a way to change it (applied and remembered), shared by every switch.
#[derive(Clone, PartialEq)]
pub struct ThemeCtx {
    pub theme: Theme,
    pub set: Callback<Theme>,
}

impl ThemeCtx {
    /// Day to night or night to day.
    pub fn flip(&self) -> Callback<MouseEvent> {
        let (set, next) = (self.set.clone(), self.theme.toggled());
        Callback::from(move |_| set.emit(next))
    }
}

#[hook]
pub fn use_theme() -> ThemeCtx {
    use_context::<ThemeCtx>().expect("ThemeCtx is provided by <App/>")
}
