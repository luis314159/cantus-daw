//! Day and night. The choice is remembered; without one, the system's preference decides.
//! (`index.html` applies it before the first paint, so there is no flash.)

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Night,
    Day,
}

const KEY: &str = "cantus-theme";

impl Theme {
    pub fn detect() -> Self {
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

    /// Sets `data-theme` on the page and remembers the choice.
    pub fn apply(self, remember: bool) {
        let Some(win) = web_sys::window() else { return };
        if let Some(root) = win.document().and_then(|d| d.document_element()) {
            let _ = root.set_attribute("data-theme", self.name());
        }
        if remember {
            if let Ok(Some(store)) = win.local_storage() {
                let _ = store.set_item(KEY, self.name());
            }
        }
    }
}
