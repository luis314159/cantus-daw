//! English and Spanish. The language is the address's (`/` or `/es/`, `route.rs`); a choice is
//! remembered, and the home at `/` sends a visitor who chose Spanish, or whose browser speaks
//! it and who never chose, to `/es/` before the first paint (`index.html`).

use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lang {
    En,
    Es,
}

const KEY: &str = "cantus-lang";

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    pub fn remember(self) {
        if !cfg!(target_arch = "wasm32") {
            return;
        }
        if let Some(win) = web_sys::window() {
            if let Ok(Some(store)) = win.local_storage() {
                let _ = store.set_item(KEY, self.code());
            }
            if let Some(root) = win.document().and_then(|d| d.document_element()) {
                let _ = root.set_attribute("lang", self.code());
            }
        }
    }

    /// Picks the text for this language: `l.t("English", "Español")`.
    pub fn t(self, en: &'static str, es: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::Es => es,
        }
    }
}

/// The language and a way to change it (to the same page in that language), shared by every
/// component.
#[derive(Clone, PartialEq)]
pub struct LangCtx {
    pub lang: Lang,
    pub set: Callback<Lang>,
}

#[hook]
pub fn use_lang() -> LangCtx {
    use_context::<LangCtx>().expect("LangCtx is provided by <App/>")
}
