//! The units' long text — what each does and what stands out (`units_data.rs`'s `what` and
//! `features`) — kept out of the WebAssembly (round 39: it was a fifth of it, and the budget
//! is 250 KB gzipped). The pre-renderer, which has it all natively, writes it to
//! `/unit-text.json` beside the pages (and into their HTML); the browser fetches that once
//! before the app starts (`main.rs`), so the app's first render matches the static page.
//! Natively (the pre-renderer, the tests) the text is read straight from `units_data.rs`.

use crate::lang::Lang;
use crate::units::Unit;

/// Where the pre-renderer writes it and the browser reads it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const URL: &str = "/unit-text.json";

/// What the unit does, in two or three sentences.
pub fn what(u: &Unit, l: Lang) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        browser::get(u.file, "what", None, l).unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        l.t(u.what.0, u.what.1).to_string()
    }
}

/// What stands out, one line each.
pub fn features(u: &Unit, l: Lang) -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        (0..)
            .map_while(|i| browser::get(u.file, "features", Some(i), l))
            .collect()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        u.features
            .iter()
            .map(|f| l.t(f.0, f.1).to_string())
            .collect()
    }
}

/// The JSON the browser reads: `{"unda":{"what":["en","es"],"features":[["en","es"],…]},…}`.
#[cfg(feature = "ssr")]
pub fn json() -> String {
    fn s(t: &str) -> String {
        let mut o = String::from("\"");
        for c in t.chars() {
            match c {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                '\n' => o.push_str("\\n"),
                c => o.push(c),
            }
        }
        o.push('"');
        o
    }
    let all = crate::units_data::INSTRUMENTS
        .iter()
        .chain(crate::units_data::EFFECTS.iter());
    let units: Vec<String> = all
        .map(|u| {
            let feats: Vec<String> = u
                .features
                .iter()
                .map(|f| format!("[{},{}]", s(f.0), s(f.1)))
                .collect();
            format!(
                "{}:{{\"what\":[{},{}],\"features\":[{}]}}",
                s(u.file),
                s(u.what.0),
                s(u.what.1),
                feats.join(",")
            )
        })
        .collect();
    format!("{{{}}}\n", units.join(","))
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::RefCell;

    use wasm_bindgen::{JsCast, JsValue};

    use crate::lang::Lang;

    thread_local! {
        static TEXT: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    }

    /// `TEXT[file][field]` (or its `i`-th item), in the language's column.
    pub fn get(file: &str, field: &str, i: Option<u32>, l: Lang) -> Option<String> {
        TEXT.with(|t| {
            let t = t.borrow();
            let unit = js_sys::Reflect::get(&t, &file.into()).ok()?;
            let mut v = js_sys::Reflect::get(&unit, &field.into()).ok()?;
            if let Some(i) = i {
                v = js_sys::Reflect::get_u32(&v, i).ok()?;
            }
            let col = match l {
                Lang::En => 0,
                Lang::Es => 1,
            };
            js_sys::Reflect::get_u32(&v, col).ok()?.as_string()
        })
    }

    /// Fetches the text once; on any failure the units show their pitch and pictures only.
    pub async fn load() {
        use wasm_bindgen_futures::JsFuture;
        let got: Option<JsValue> = async {
            let win = web_sys::window()?;
            let resp: web_sys::Response = JsFuture::from(win.fetch_with_str(super::URL))
                .await
                .ok()?
                .dyn_into()
                .ok()?;
            if !resp.ok() {
                return None;
            }
            JsFuture::from(resp.json().ok()?).await.ok()
        }
        .await;
        if let Some(v) = got {
            TEXT.with(|t| *t.borrow_mut() = v);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::load;

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[test]
    fn the_json_holds_every_unit_with_its_text() {
        let j = json();
        for u in crate::units_data::INSTRUMENTS
            .iter()
            .chain(crate::units_data::EFFECTS.iter())
        {
            assert!(
                j.contains(&format!("\"{}\":{{\"what\":[", u.file)),
                "{}",
                u.file
            );
            assert!(!u.what.0.is_empty() && !u.features.is_empty(), "{}", u.file);
        }
        assert_eq!(j.matches('{').count(), j.matches('}').count());
    }
}
