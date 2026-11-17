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
use std::collections::HashMap;
use std::rc::Rc;

use gloo_timers::callback::Interval;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode};
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
    /// the two were matched.
    pub level_db: f64,
    /// The pictures' stem (`<pic>-dark.webp`, `-light`, `-es`), or the unit's own.
    pub pic: Option<String>,
}

/// A unit's manifest.
#[derive(Clone, PartialEq, Debug)]
pub struct Manifest {
    /// `presets`: rendered from the factory presets; `dev`: the unit's development renders.
    pub origin: String,
    pub source: (String, String),
    pub credit: (String, String),
    pub lufs: f64,
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
            level_db: get(&p, "level_db").as_f64().unwrap_or(0.0),
            pic: get(&p, "pic").as_string(),
        })
        .collect::<Vec<_>>();
    (!presets.is_empty()).then(|| Manifest {
        origin: text(v, "origin"),
        source: pair(v, "source"),
        credit: pair(v, "credit"),
        lufs: get(v, "lufs").as_f64().unwrap_or(0.0),
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
    static MANIFESTS: RefCell<HashMap<String, Rc<Manifest>>> = RefCell::new(HashMap::new());
}

async fn fetch(url: &str) -> Option<web_sys::Response> {
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
    if let Some(m) = MANIFESTS.with(|c| c.borrow().get(&unit).cloned()) {
        return Some(m);
    }
    let r = fetch(&format!("/tryit/{unit}.json")).await?;
    let v = wasm_bindgen_futures::JsFuture::from(r.json().ok()?)
        .await
        .ok()?;
    let m = Rc::new(parse(&v)?);
    MANIFESTS.with(|c| c.borrow_mut().insert(unit, m.clone()));
    Some(m)
}

/// The two takes playing: both run, only the gains say which is heard.
struct Voice {
    dry: (AudioBufferSourceNode, GainNode),
    wet: (AudioBufferSourceNode, GainNode),
    /// The context's time at which the loop's start played.
    t0: f64,
    len: f64,
}

/// The panel's sound: one context (made on the first Play, inside the click), the decoded
/// takes by URL, and what is playing.
#[derive(Default)]
struct Engine {
    ctx: Option<AudioContext>,
    takes: HashMap<String, AudioBuffer>,
    voice: Option<Voice>,
    /// Bumped by every request: a take that arrives after another request is not played.
    ask: u32,
}

/// The crossfade's time constant, s (about 20 ms to silence).
const FADE: f64 = 0.005;

fn ramp(g: &GainNode, to: f32, at: f64) {
    let _ = g.gain().cancel_scheduled_values(at);
    let _ = g.gain().set_target_at_time(to, at, FADE);
}

fn stop_source(s: &AudioBufferSourceNode, at: f64) {
    let base: &web_sys::AudioScheduledSourceNode = s.as_ref();
    let _ = base.stop_with_when(at);
}

impl Engine {
    fn ctx(&mut self) -> Option<AudioContext> {
        if self.ctx.is_none() {
            self.ctx = AudioContext::new().ok();
        }
        let c = self.ctx.clone()?;
        let _ = c.resume();
        Some(c)
    }

    fn now(&self) -> f64 {
        self.ctx.as_ref().map(|c| c.current_time()).unwrap_or(0.0)
    }

    /// Where in the loop the music is, 0…1.
    fn pos(&self) -> f64 {
        self.voice
            .as_ref()
            .map(|v| ((self.now() - v.t0) / v.len).rem_euclid(1.0))
            .unwrap_or(0.0)
    }

    /// Fade out what plays.
    fn release(&mut self) {
        let now = self.now();
        if let Some(v) = self.voice.take() {
            for (s, g) in [&v.dry, &v.wet] {
                ramp(g, 0.0, now);
                stop_source(s, now + 0.12);
            }
        }
    }

    fn source(
        &self,
        ctx: &AudioContext,
        take: &AudioBuffer,
        gain: f32,
        when: f64,
        offset: f64,
    ) -> Option<(AudioBufferSourceNode, GainNode)> {
        let s = ctx.create_buffer_source().ok()?;
        s.set_buffer(Some(take));
        s.set_loop(true);
        let g = ctx.create_gain().ok()?;
        g.gain().set_value(0.0);
        let _ = g.gain().set_target_at_time(gain, when, FADE);
        s.connect_with_audio_node(&g).ok()?;
        g.connect_with_audio_node(&ctx.destination()).ok()?;
        #[allow(deprecated)]
        s.start_with_when_and_grain_offset(when, offset).ok()?;
        Some((s, g))
    }

    /// Start `dry` and `wet` together, at the point of the loop that plays now (from the
    /// start when nothing plays), crossfaded over what played.
    fn start(&mut self, dry: &AudioBuffer, wet: &AudioBuffer, wet_on: bool) {
        let Some(ctx) = self.ctx.clone() else {
            return;
        };
        let len = dry.duration().min(wet.duration()).max(0.1);
        let when = ctx.current_time() + 0.04;
        let offset = self
            .voice
            .as_ref()
            .map(|v| ((when - v.t0) / v.len).rem_euclid(1.0) * len)
            .unwrap_or(0.0);
        self.release();
        let (d, w) = if wet_on { (0.0, 1.0) } else { (1.0, 0.0) };
        let (Some(dry), Some(wet)) = (
            self.source(&ctx, dry, d, when, offset),
            self.source(&ctx, wet, w, when, offset),
        ) else {
            return;
        };
        self.voice = Some(Voice {
            dry,
            wet,
            t0: when - offset,
            len,
        });
    }

    /// Dry or wet, on the running loop.
    fn choose(&self, wet_on: bool) {
        if let Some(v) = &self.voice {
            let now = self.now();
            ramp(&v.dry.1, if wet_on { 0.0 } else { 1.0 }, now);
            ramp(&v.wet.1, if wet_on { 1.0 } else { 0.0 }, now);
        }
    }
}

/// Decode `url` into the engine's takes (once).
async fn take(engine: Rc<RefCell<Engine>>, url: String) -> Option<AudioBuffer> {
    if let Some(t) = engine.borrow().takes.get(&url).cloned() {
        return Some(t);
    }
    let ctx = engine.borrow().ctx.clone()?;
    let r = fetch(&url).await?;
    let bytes = wasm_bindgen_futures::JsFuture::from(r.array_buffer().ok()?)
        .await
        .ok()?;
    let bytes: js_sys::ArrayBuffer = bytes.dyn_into().ok()?;
    let decoded = wasm_bindgen_futures::JsFuture::from(ctx.decode_audio_data(&bytes).ok()?)
        .await
        .ok()?;
    let t: AudioBuffer = decoded.dyn_into().ok()?;
    engine.borrow_mut().takes.insert(url, t.clone());
    Some(t)
}

/// Fetch the pair and play it; `done(true)` when it plays, `done(false)` when it could not.
fn play(
    engine: Rc<RefCell<Engine>>,
    dry: String,
    wet: String,
    wet_on: Rc<RefCell<bool>>,
    done: Callback<bool>,
) {
    let ask = {
        let mut e = engine.borrow_mut();
        e.ask += 1;
        e.ask
    };
    wasm_bindgen_futures::spawn_local(async move {
        let d = take(engine.clone(), dry).await;
        let w = take(engine.clone(), wet).await;
        if engine.borrow().ask != ask {
            return; // another request came after this one
        }
        match (d, w) {
            (Some(d), Some(w)) => {
                engine.borrow_mut().start(&d, &w, *wet_on.borrow());
                done.emit(true);
            }
            _ => done.emit(false),
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
    let engine = use_mut_ref(Engine::default);
    let file = list.get(*which).copied().unwrap_or_default();

    // A unit shown: its manifest (fetched once), what played stopped.
    {
        let (man, failed, sel, engine, playing) = (
            man.clone(),
            failed.clone(),
            sel.clone(),
            engine.clone(),
            playing.clone(),
        );
        use_effect_with(file, move |f| {
            engine.borrow_mut().release();
            engine.borrow_mut().ask += 1;
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
        let (engine, pos) = (engine.clone(), pos.clone());
        use_effect_with(*playing, move |on| {
            let timer = on.then(|| Interval::new(100, move || pos.set(engine.borrow().pos())));
            move || drop(timer)
        });
    }
    // Leaving the page silences it.
    {
        let engine = engine.clone();
        use_effect_with((), move |_| {
            move || {
                let mut e = engine.borrow_mut();
                e.release();
                e.ask += 1;
                if let Some(c) = e.ctx.take() {
                    let _ = c.close();
                }
            }
        });
    }

    let m = (*man).clone().filter(|_| unit_of(file).is_some());
    let preset = m.as_ref().and_then(|m| {
        m.presets
            .get((*sel).min(m.presets.len().saturating_sub(1)))
            .cloned()
    });

    // Play the pair of preset `i` (from where the loop is, if it plays).
    let start = {
        let (engine, wet_now, playing, loading, m) = (
            engine.clone(),
            wet_now.clone(),
            playing.clone(),
            loading.clone(),
            m.clone(),
        );
        Callback::from(move |i: usize| {
            let Some(m) = m.as_ref() else { return };
            let Some(p) = m.presets.get(i) else { return };
            if engine.borrow_mut().ctx().is_none() {
                return;
            }
            loading.set(true);
            let (playing, loading) = (playing.clone(), loading.clone());
            play(
                engine.clone(),
                m.dry.clone(),
                p.wet.clone(),
                wet_now.clone(),
                Callback::from(move |ok: bool| {
                    loading.set(false);
                    playing.set(ok);
                }),
            );
        })
    };
    let toggle = {
        let (engine, playing, start, sel, pos) = (
            engine.clone(),
            playing.clone(),
            start.clone(),
            sel.clone(),
            pos.clone(),
        );
        Callback::from(move |_: MouseEvent| {
            if *playing {
                let mut e = engine.borrow_mut();
                e.release();
                e.ask += 1;
                playing.set(false);
                pos.set(0.0);
            } else {
                start.emit(*sel);
            }
        })
    };
    let choose = |want: bool| {
        let (engine, wet_on, wet_now) = (engine.clone(), wet_on.clone(), wet_now.clone());
        Callback::from(move |_: MouseEvent| {
            *wet_now.borrow_mut() = want;
            wet_on.set(want);
            engine.borrow().choose(want);
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
            let origin = if m.origin == "presets" {
                l.t("Rendered by Cantus from its factory presets on the same take, each matched to one loudness by a gain alone.",
                    "Renderizado por Cantus desde sus presets de fábrica sobre la misma toma, cada uno igualado a una sonoridad solo con una ganancia.")
            } else {
                l.t("Sound examples from the unit's own renders on the same take, each matched to one loudness by a gain alone.",
                    "Ejemplos de sonido de los propios renders de la unidad sobre la misma toma, cada uno igualado a una sonoridad solo con una ganancia.")
            };
            let level = match l {
                Lang::En => format!("As the preset leaves it: {} against the dry. Matched here, so you hear the sound, not the level.", db(l, pr.level_db)),
                Lang::Es => format!("Tal como lo deja el preset: {} frente a la señal seca. Igualado aquí, para que oigas el sonido y no el volumen.", db(l, pr.level_db)),
            };
            html! {
                <div class="try-body">
                    <figure class="try-shot">
                        <img key={pic.clone()} src={pic} width={u.big.0.to_string()} height={u.big.1.to_string()} loading="lazy"
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
                        <p class="try-level">{ level }</p>
                        <p class="try-note">{ say(l, &m.source) }{ ". " }{ origin }{ " " }<span class="try-credit">{ say(l, &m.credit) }{ "." }</span></p>
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
        (l.t("Try it", "Pruébalo"),
         l.t("Choose a preset and play: switch between dry and wet while it plays — the music does not move, only the effect comes and goes.",
             "Elige un preset y reproduce: cambia entre seco y con efecto mientras suena — la música no se mueve, solo entra y sale el efecto."))
    } else {
        (l.t("Try them on a guitar", "Pruébalos en una guitarra"),
         l.t("One take through each effect's presets, the editor beside it. Switch between dry and wet while it plays — the music does not move, only the effect comes and goes.",
             "Una misma toma a través de los presets de cada efecto, con su editor al lado. Cambia entre seco y con efecto mientras suena — la música no se mueve, solo entra y sale el efecto."))
    };
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
