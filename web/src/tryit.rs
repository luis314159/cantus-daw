//! Try it (fx side round): a unit's presets on the same take, dry or wet, switched without a
//! gap and at the same loudness, with the editor's picture for each preset.
//!
//! Each unit has a manifest, `assets/tryit/<unit>.json` (fetched when the panel shows that
//! unit: nothing is in the WebAssembly), naming the dry take, each preset's wet take, its
//! name and use in both languages, the level the preset leaves it at (the takes themselves
//! are matched to one loudness) and its picture. `cargo run --release -p cantus-ui --example
//! render_tryit` writes them from the factory presets (docs/round-fx/site.md).
//!
//! The audio is Web Audio: both takes start together on the same clock and loop, each
//! through its own gain; Dry and Wet move the gains (a 20 ms crossfade), so the music never
//! moves. A new preset starts its pair at the same point of the loop, crossfaded. Nothing is
//! downloaded before the first Play but the manifest and the picture (`loading="lazy"`).

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use wasm_bindgen::{JsCast, JsValue};
use yew::prelude::*;

use crate::lang::{use_lang, Lang};
use crate::units::{pic_lang, shade_now};
use crate::units_data::EFFECTS;

/// The units with a manifest, in the order the panel lists them: the guitar units first
/// (the owner's order), then the units heard on their own sources.
pub const UNITS: &[&str] = &[
    "fragor", "tabula", "saltus", "arca", "rota", "cribrum", "vetus", "lapsus", "caelum",
    "fractus", "phasis", "loquax", "vinculum", "lenis", "cedo", "purus",
];

/// Whether `file` has a manifest.
pub fn has(file: &str) -> bool {
    UNITS.contains(&file)
}

/// One preset of a manifest.
#[derive(Clone, PartialEq, Debug)]
pub struct Preset {
    pub id: String,
    pub name: (String, String),
    pub what: (String, String),
    pub wet: String,
    /// How much louder (+) or quieter the preset leaves the take than the dry, dB, before
    /// the two were matched (none for takes that were not rendered from a preset as it is).
    pub level_db: Option<f64>,
    /// The pictures' stem (`<pic>-dark.webp`, `-light`, `-es`), or the unit's own.
    pub pic: Option<String>,
}

/// A unit's manifest.
#[derive(Clone, PartialEq, Debug)]
pub struct Manifest {
    /// Where the takes come from and how they were matched, in both languages.
    pub note: (String, String),
    pub source: (String, String),
    pub credit: (String, String),
    pub dry: String,
    pub presets: Vec<Preset>,
}

fn get(v: &JsValue, k: &str) -> JsValue {
    js_sys::Reflect::get(v, &k.into()).unwrap_or(JsValue::UNDEFINED)
}

fn text(v: &JsValue, k: &str) -> String {
    get(v, k).as_string().unwrap_or_default()
}

fn pair(v: &JsValue, k: &str) -> (String, String) {
    let a = get(v, k);
    let at = |i| {
        js_sys::Reflect::get_u32(&a, i)
            .ok()
            .and_then(|s| s.as_string())
    };
    (at(0).unwrap_or_default(), at(1).unwrap_or_default())
}

/// The manifest from its JSON (`JSON.parse`d by the browser).
pub fn parse(v: &JsValue) -> Option<Manifest> {
    let list = get(v, "presets").dyn_into::<js_sys::Array>().ok()?;
    let presets = list
        .iter()
        .map(|p| Preset {
            id: text(&p, "id"),
            name: pair(&p, "name"),
            what: pair(&p, "what"),
            wet: text(&p, "wet"),
            level_db: get(&p, "level_db").as_f64(),
            pic: get(&p, "pic").as_string(),
        })
        .collect::<Vec<_>>();
    (!presets.is_empty()).then(|| Manifest {
        note: pair(v, "note"),
        source: pair(v, "source"),
        credit: pair(v, "credit"),
        dry: text(v, "dry"),
        presets,
    })
}

fn say(l: Lang, p: &(String, String)) -> String {
    match l {
        Lang::En => p.0.clone(),
        Lang::Es => p.1.clone(),
    }
}

thread_local! {
    /// Manifests already fetched, by unit.
    static MANIFESTS: RefCell<Vec<(String, Rc<Manifest>)>> = const { RefCell::new(Vec::new()) };
}

/// A response that answered OK (the plug-in pages fetch their parts with it too).
pub(crate) async fn fetch(url: &str) -> Option<web_sys::Response> {
    use wasm_bindgen_futures::JsFuture;
    let win = web_sys::window()?;
    let r: web_sys::Response = JsFuture::from(win.fetch_with_str(url))
        .await
        .ok()?
        .dyn_into()
        .ok()?;
    r.ok().then_some(r)
}

async fn manifest(unit: String) -> Option<Rc<Manifest>> {
    if let Some(m) = MANIFESTS.with(|c| {
        c.borrow()
            .iter()
            .find(|(u, _)| *u == unit)
            .map(|(_, m)| m.clone())
    }) {
        return Some(m);
    }
    let r = fetch(&format!("/tryit/{unit}.json")).await?;
    let v = wasm_bindgen_futures::JsFuture::from(r.json().ok()?)
        .await
        .ok()?;
    let m = Rc::new(parse(&v)?);
    MANIFESTS.with(|c| c.borrow_mut().push((unit, m.clone())));
    Some(m)
}

// The player is a few lines of Web Audio in JavaScript (wasm-bindgen's inline snippet): the
// same calls made from Rust cost the WebAssembly budget ~4 KB of glue for nothing.
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
let ctx = null, takes = new Map(), voice = null, ask = 0;
const FADE = 0.005;
function ramp(g, to, at) { g.gain.cancelScheduledValues(at); g.gain.setTargetAtTime(to, at, FADE); }
function release() {
  if (!voice) return;
  const now = ctx.currentTime;
  for (const [s, g] of [voice.dry, voice.wet]) { ramp(g, 0, now); s.stop(now + 0.12); }
  voice = null;
}
async function take(url) {
  if (takes.has(url)) return takes.get(url);
  const r = await fetch(url);
  if (!r.ok) throw new Error(url);
  const b = await ctx.decodeAudioData(await r.arrayBuffer());
  takes.set(url, b);
  return b;
}
function source(buf, gain, when, offset) {
  const s = ctx.createBufferSource(), g = ctx.createGain();
  s.buffer = buf; s.loop = true;
  g.gain.value = 0; g.gain.setTargetAtTime(gain, when, FADE);
  s.connect(g); g.connect(ctx.destination);
  s.start(when, offset);
  return [s, g];
}
export function try_play(dry, wet, wetOn) {
  if (!ctx) { try { ctx = new AudioContext(); } catch (e) { return Promise.resolve(false); } }
  ctx.resume();
  const mine = ++ask;
  return Promise.all([take(dry), take(wet)]).then(([d, w]) => {
    if (mine !== ask) return null;
    const len = Math.max(0.1, Math.min(d.duration, w.duration));
    const when = ctx.currentTime + 0.04;
    const offset = voice ? (((when - voice.t0) / voice.len) % 1 + 1) % 1 * len : 0;
    release();
    const on = wetOn();
    voice = { dry: source(d, on ? 0 : 1, when, offset), wet: source(w, on ? 1 : 0, when, offset), t0: when - offset, len };
    return true;
  }, () => false);
}
export function try_choose(wetOn) {
  if (!voice) return;
  const now = ctx.currentTime;
  ramp(voice.dry[1], wetOn ? 0 : 1, now);
  ramp(voice.wet[1], wetOn ? 1 : 0, now);
}
export function try_stop() { ask++; if (ctx) release(); }
export function try_pos() { return voice ? (((ctx.currentTime - voice.t0) / voice.len) % 1 + 1) % 1 : 0; }
export function try_close() { try_stop(); if (ctx) { ctx.close(); ctx = null; takes.clear(); } }
"#)]
extern "C" {
    /// Fetch and decode the pair (once each), then start both at the point of the loop that
    /// plays now (from the start when nothing plays), crossfaded over what played; resolves
    /// to whether it plays (`null` when a later request overtook it). `wet_on` is asked
    /// when the takes are in, so a switch made while they load is kept.
    fn try_play(dry: &str, wet: &str, wet_on: &js_sys::Function) -> js_sys::Promise;
    /// Dry or wet on the running loop (a 20 ms crossfade).
    fn try_choose(wet_on: bool);
    fn try_stop();
    /// Where in the loop the music is, 0…1.
    fn try_pos() -> f64;
    /// Stop and free the context and the takes (leaving the page).
    fn try_close();
}

/// Play the pair; `done(true)` when it plays, `done(false)` when it could not (never when a
/// later request overtook it).
fn play(dry: &str, wet: &str, wet_now: Rc<RefCell<bool>>, done: Callback<bool>) {
    let ask = wasm_bindgen::closure::Closure::<dyn Fn() -> bool>::new(move || *wet_now.borrow());
    let p = try_play(dry, wet, ask.as_ref().unchecked_ref());
    wasm_bindgen_futures::spawn_local(async move {
        let v = wasm_bindgen_futures::JsFuture::from(p).await;
        drop(ask);
        // null: a later request overtook this one, and it answers instead.
        match v {
            Ok(v) if v.is_null() => {}
            Ok(v) => done.emit(v.as_bool().unwrap_or(false)),
            Err(_) => done.emit(false),
        }
    });
}

fn unit_of(file: &str) -> Option<&'static crate::units::Unit> {
    EFFECTS.iter().find(|u| u.file == file)
}

fn focus(id: &str) {
    if let Some(e) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = e.focus();
    }
}

/// "+2.1 dB" / "+2,1 dB".
fn db(l: Lang, v: f64) -> String {
    let s = format!(
        "{}{:.1} dB",
        if v >= 0.05 {
            "+"
        } else if v <= -0.05 {
            "−"
        } else {
            "±"
        },
        v.abs()
    );
    match l {
        Lang::En => s,
        Lang::Es => s.replace('.', ","),
    }
}

#[derive(Properties, PartialEq)]
pub struct TryProps {
    /// One unit (its page); none: every unit with a manifest, to choose from.
    #[prop_or_default]
    pub unit: Option<AttrValue>,
}

#[function_component(TryIt)]
pub fn try_it(p: &TryProps) -> Html {
    let l = use_lang().lang;
    let list: Vec<&'static str> = match &p.unit {
        Some(u) => UNITS.iter().copied().filter(|f| *f == u.as_str()).collect(),
        None => UNITS.to_vec(),
    };
    let which = use_state(|| 0usize);
    let man = use_state(|| None::<Rc<Manifest>>);
    let failed = use_state(|| false);
    let sel = use_state(|| 0usize);
    let wet_on = use_state(|| true);
    let wet_now = use_mut_ref(|| true);
    let playing = use_state(|| false);
    let loading = use_state(|| false);
    let pos = use_state(|| 0.0f64);
    let file = list.get(*which).copied().unwrap_or_default();

    // A unit shown: its manifest (fetched once), what played stopped.
    {
        let (man, failed, sel, playing) =
            (man.clone(), failed.clone(), sel.clone(), playing.clone());
        use_effect_with(file, move |f| {
            try_stop();
            playing.set(false);
            sel.set(0);
            failed.set(false);
            let f = f.to_string();
            if !f.is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    match manifest(f).await {
                        Some(m) => man.set(Some(m)),
                        None => failed.set(true),
                    }
                });
            }
        });
    }
    // The progress line while it plays.
    {
        let pos = pos.clone();
        use_effect_with(*playing, move |on| {
            let timer = on.then(|| Interval::new(100, move || pos.set(try_pos())));
            move || drop(timer)
        });
    }
    // Leaving the page silences it.
    {
        use_effect_with((), move |_| try_close);
    }

    let m = (*man).clone().filter(|_| unit_of(file).is_some());
    let preset = m.as_ref().and_then(|m| {
        m.presets
            .get((*sel).min(m.presets.len().saturating_sub(1)))
            .cloned()
    });

    // Play the pair of preset `i` (from where the loop is, if it plays).
    let start = {
        let (wet_now, playing, loading, m) =
            (wet_now.clone(), playing.clone(), loading.clone(), m.clone());
        Callback::from(move |i: usize| {
            let Some(m) = m.as_ref() else { return };
            let Some(p) = m.presets.get(i) else { return };
            loading.set(true);
            let (playing, loading) = (playing.clone(), loading.clone());
            play(
                &m.dry,
                &p.wet,
                wet_now.clone(),
                Callback::from(move |ok: bool| {
                    loading.set(false);
                    playing.set(ok);
                }),
            );
        })
    };
    let toggle = {
        let (playing, start, sel, pos) = (playing.clone(), start.clone(), sel.clone(), pos.clone());
        Callback::from(move |_: MouseEvent| {
            if *playing {
                try_stop();
                playing.set(false);
                pos.set(0.0);
            } else {
                start.emit(*sel);
            }
        })
    };
    let choose = |want: bool| {
        let (wet_on, wet_now) = (wet_on.clone(), wet_now.clone());
        Callback::from(move |_: MouseEvent| {
            *wet_now.borrow_mut() = want;
            wet_on.set(want);
            try_choose(want);
        })
    };
    let pick = {
        let (sel, playing, start) = (sel.clone(), playing.clone(), start.clone());
        Callback::from(move |i: usize| {
            sel.set(i);
            if *playing {
                start.emit(i);
            }
        })
    };

    let n = m.as_ref().map(|m| m.presets.len()).unwrap_or(0);
    let keys = {
        let (sel, pick) = (sel.clone(), pick.clone());
        Callback::from(move |e: KeyboardEvent| {
            if n == 0 {
                return;
            }
            let i = match e.key().as_str() {
                "ArrowDown" | "ArrowRight" => (*sel + 1) % n,
                "ArrowUp" | "ArrowLeft" => (*sel + n - 1) % n,
                "Home" => 0,
                "End" => n - 1,
                _ => return,
            };
            e.prevent_default();
            pick.emit(i);
            focus(&format!("try-p-{i}"));
        })
    };

    let units = (p.unit.is_none()).then(|| {
        let chips = list.iter().enumerate().filter_map(|(i, f)| {
            let u = unit_of(f)?;
            let which = which.clone();
            Some(html! {
                <li><button type="button" class="try-unit" aria-pressed={(i == *which).to_string()}
                            onclick={Callback::from(move |_| which.set(i))}>
                    <strong>{ u.name }</strong><span>{ l.t(u.kind.0, u.kind.1) }</span>
                </button></li>
            })
        });
        html! { <ul class="try-units" aria-label={l.t("Effects to try", "Efectos para probar")}>{ for chips }</ul> }
    });

    let body = match (unit_of(file), m.as_ref(), preset.as_ref()) {
        (Some(u), Some(m), Some(pr)) => {
            let shade = shade_now();
            let pic = match &pr.pic {
                Some(stem) => format!("{stem}-{shade}{}.webp", pic_lang(l)),
                None => format!("/units/{}-{shade}{}.webp", u.file, pic_lang(l)),
            };
            let rows = m.presets.iter().enumerate().map(|(i, x)| {
                let on = i == (*sel).min(n - 1);
                let pick = pick.clone();
                html! {
                    <li><button type="button" id={format!("try-p-{i}")} class={classes!("try-preset", on.then_some("is-on"))}
                                aria-pressed={on.to_string()} tabindex={if on { "0" } else { "-1" }}
                                onclick={Callback::from(move |_| pick.emit(i))}>
                        <strong>{ say(l, &x.name) }</strong><span>{ say(l, &x.what) }</span>
                    </button></li>
                }
            });
            let icon = if *playing {
                html! { <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3" width="3" height="10" rx="1" fill="currentColor"/><rect x="9.5" y="3" width="3" height="10" rx="1" fill="currentColor"/></svg> }
            } else {
                html! { <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 2.5v11l9-5.5z" fill="currentColor"/></svg> }
            };
            let level = pr.level_db.map(|v| match l {
                Lang::En => format!(
                    "The preset's own level: {} against the dry; matched here.",
                    db(l, v)
                ),
                Lang::Es => format!(
                    "El nivel propio del preset: {} frente a la señal seca; igualado aquí.",
                    db(l, v)
                ),
            });
            html! {
                <div class="try-body">
                    <figure class="try-shot">
                        <img key={pic.clone()} src={pic} width={u.big_in(l).0.to_string()} height={u.big_in(l).1.to_string()} loading="lazy"
                             alt={format!("{} — {} ({})", u.name, say(l, &pr.name), l.t("its window in Cantus", "su ventana en Cantus"))} />
                    </figure>
                    <div class="try-side">
                        <h3 class="try-name">{ u.name }<span>{ l.t(u.kind.0, u.kind.1) }</span></h3>
                        <ul class="try-presets" aria-label={l.t("Presets", "Presets")} onkeydown={keys}>{ for rows }</ul>
                        <div class="ab try-ab" role="group" aria-label={l.t("Hear it dry or wet", "Escúchalo seco o con el efecto")}>
                            <button type="button" class={classes!("ab-play", playing.then_some("is-on"))} onclick={toggle}
                                    aria-busy={loading.to_string()}
                                    aria-label={if *playing { l.t("Stop", "Detener") } else { l.t("Play", "Reproducir") }}>{ icon }</button>
                            <div class="ab-seg">
                                <button type="button" aria-pressed={(!*wet_on).to_string()} onclick={choose(false)}>{ l.t("Dry", "Seco") }</button>
                                <button type="button" aria-pressed={wet_on.to_string()} onclick={choose(true)}>{ u.name }</button>
                            </div>
                            <div class="ab-bar" aria-hidden="true"><i style={format!("width:{:.1}%", *pos * 100.0)}></i></div>
                            if *loading { <span class="try-wait" role="status">{ l.t("Loading…", "Cargando…") }</span> }
                        </div>
                        if let Some(level) = level { <p class="try-level">{ level }</p> }
                        <p class="try-note">{ say(l, &m.source) }{ ". " }{ say(l, &m.note) }{ " " }<span class="try-credit">{ say(l, &m.credit) }{ "." }</span></p>
                    </div>
                </div>
            }
        }
        _ if *failed => {
            html! { <p class="try-note">{ l.t("The sound examples could not be loaded.", "No se pudieron cargar los ejemplos de sonido.") }</p> }
        }
        _ => html! { <div class="try-body is-waiting" aria-hidden="true"></div> },
    };

    let (title, lede) = if p.unit.is_some() {
        (
            l.t("Try it", "Pruébalo"),
            l.t("Choose a preset and play.", "Elige un preset y reproduce."),
        )
    } else {
        (
            l.t("Try them on a guitar", "Pruébalos en una guitarra"),
            l.t(
                "One take through each effect's presets.",
                "Una toma a través de los presets de cada efecto.",
            ),
        )
    };
    let lede = format!(
        "{lede} {}",
        l.t(
            "Switch dry and wet while it plays: only the effect comes and goes.",
            "Cambia entre seco y con efecto mientras suena: solo entra y sale el efecto."
        )
    );
    html! {
        <section class={classes!("try", p.unit.is_none().then_some("section"))} id="try" aria-labelledby="try-title">
            <div class={classes!(p.unit.is_none().then_some("wrap"))}>
                <header class="sec-head">
                    <h2 id="try-title">{ title }</h2>
                    <p>{ lede }</p>
                </header>
                { units.unwrap_or_default() }
                { body }
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every listed unit is a card with a manifest, and every take and picture a manifest
    /// names is on disk (the manifests are read as text: the browser parses them).
    #[test]
    fn every_manifest_names_files_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for f in UNITS {
            assert!(unit_of(f).is_some(), "{f}: no card in units_data.rs");
            let json = std::fs::read_to_string(root.join(format!("tryit/{f}.json")))
                .unwrap_or_else(|e| panic!("{f}: {e}"));
            let urls: Vec<&str> = json
                .split('"')
                .filter(|s| s.starts_with("/tryit/") && s.ends_with(".mp3"))
                .collect();
            assert!(urls.len() >= 2, "{f}: a dry and at least one wet take");
            for u in urls {
                let p = root.join(u.trim_start_matches('/'));
                let n = std::fs::metadata(&p)
                    .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
                    .len();
                assert!(n < 400_000, "{u}: {n} bytes, over the clip budget");
            }
            for stem in json
                .split("\"pic\": \"")
                .skip(1)
                .map(|s| s.split('"').next().unwrap_or(""))
            {
                for v in ["dark", "light", "dark-es", "light-es"] {
                    let p = root.join(format!("{}-{v}.webp", stem.trim_start_matches('/')));
                    assert!(p.exists(), "{}: missing", p.display());
                }
            }
        }
    }

    #[test]
    fn level_reads_in_each_language() {
        assert_eq!(db(Lang::En, 2.14), "+2.1 dB");
        assert_eq!(db(Lang::Es, -3.06), "−3,1 dB");
        assert_eq!(db(Lang::En, 0.01), "±0.0 dB");
    }
}
