//! English and Spanish. The language follows the browser's, is remembered, and switches live.

use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

    /// The saved choice, else the browser's language, else English.
    pub fn detect() -> Self {
        let Some(win) = web_sys::window() else {
            return Lang::En;
        };
        if let Ok(Some(store)) = win.local_storage() {
            match store.get_item(KEY) {
                Ok(Some(v)) if v == "es" => return Lang::Es,
                Ok(Some(v)) if v == "en" => return Lang::En,
                _ => {}
            }
        }
        match win.navigator().language() {
            Some(l) if l.to_lowercase().starts_with("es") => Lang::Es,
            _ => Lang::En,
        }
    }

    pub fn remember(self) {
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

/// The language and a way to change it, shared by every component.
#[derive(Clone, PartialEq)]
pub struct LangCtx {
    pub lang: Lang,
    pub set: Callback<Lang>,
}

#[hook]
pub fn use_lang() -> LangCtx {
    use_context::<LangCtx>().expect("LangCtx is provided by <App/>")
}
