//! A unit's own page (round 39): `/instruments/<name>` and `/effects/<name>` — its window
//! large in either theme, what it does, what stands out, its presets heard (`units_sounds.rs`)
//! and, for the effects that have one, the same excerpt with and without it; the previous and
//! the next unit, and the shelf of the rest. [`UnitIndex`] is `/instruments` and `/effects`:
//! the cards of every unit (`units.rs`).

use yew::prelude::*;

use crate::fx::AbPlayer;
use crate::lang::use_lang;
use crate::nav::href;
use crate::sounds::PresetSounds;
use crate::units::{pic_lang, shade_now, Unit};

/// The unit named `file` (the URL's last part), its group (0 instruments, 1 effects), its
/// index, the group's units and path.
pub fn find(file: &str) -> Option<(usize, usize, &'static [Unit], &'static str)> {
    for g in 0..2 {
        let (units, base) = crate::spotlight::group(g);
        if let Some(i) = units.iter().position(|u| u.file == file) {
            return Some((g, i, units, base));
        }
    }
    None
}

#[derive(Properties, PartialEq)]
pub struct UnitProps {
    /// The unit's name in the URL (`unda`, `fervor`…): `units_data.rs`'s `file`.
    pub file: AttrValue,
}

#[function_component(UnitPage)]
pub fn unit_page(p: &UnitProps) -> Html {
    let l = use_lang().lang;
    let shade = use_state(|| None::<&'static str>);
    let Some((g, i, units, base)) = find(&p.file) else {
        return html! {
            <section class="page-band lapis unit-missing">
                <div class="wrap">
                    <h1>{ l.t("No such instrument or effect", "No hay tal instrumento ni efecto") }</h1>
                    <p><a href={href(l, "/#units")}>{ l.t("See them all →", "Verlos todos →") }</a></p>
                </div>
            </section>
        };
    };
    let u = &units[i];
    let prev = &units[(i + units.len() - 1) % units.len()];
    let next = &units[(i + 1) % units.len()];
    let shown = shade.unwrap_or_else(shade_now);
    let pick = |s: &'static str| {
        let shade = shade.clone();
        Callback::from(move |_| shade.set(Some(s)))
    };
    let big = format!("/units/{}-{}{}.webp", u.file, shown, pic_lang(l));
    let feats = crate::unit_text::features(u, l).into_iter().map(|f| {
        html! {
            <li>{ crate::icons::check() }<span>{ f }</span></li>
        }
    });
    let presets = u.presets.map(|n| {
        if u.file == "pulsus" {
            format!("{n} {}", l.t("factory kits", "kits de fábrica"))
        } else {
            format!("{n} {}", l.t("factory presets", "presets de fábrica"))
        }
    });
    let group_name = if g == 0 {
        l.t("Instruments", "Instrumentos")
    } else {
        l.t("Effects", "Efectos")
    };
    let shelf = units.iter().map(|s| {
        let here = std::ptr::eq(s, u);
        html! {
            <li><a class={classes!("shelf-item", here.then_some("is-on"))} href={href(l, &format!("{base}{}", s.file))}
                   aria-current={here.then_some("page")}>
                <img src={format!("/units/{}-thumb{}.webp", s.file, pic_lang(l))} width={s.thumb.0.to_string()} height={s.thumb.1.to_string()} loading="lazy" alt="" />
                <span>{ s.name }</span>
            </a></li>
        }
    });
    html! {
        <article class="unit-full">
            <div class="page-band lapis"><div class="wrap">
                <nav class="crumbs" aria-label={l.t("Where you are", "Dónde estás")}>
                    <a href={href(l, "/")}>{ "Cantus" }</a><span aria-hidden="true">{ "/" }</span>
                    <a href={href(l, base.trim_end_matches('/'))}>{ group_name }</a><span aria-hidden="true">{ "/" }</span>
                    <span aria-current="page">{ u.name }</span>
                </nav>
                <header class="uf-head">
                    <span class="eyebrow">{ l.t(u.kind.0, u.kind.1) }</span>
                    <h1>{ u.name }</h1>
                    <p class="lede">{ l.t(u.pitch.0, u.pitch.1) }</p>
                </header>
            </div></div>
            <div class="section uf-body"><div class="wrap">
                <figure class="uf-shot">
                    <a href={big.clone()} target="_blank" rel="noopener"
                       aria-label={format!("{} — {}", u.name, l.t("open the picture full size", "abrir la imagen a tamaño completo"))}>
                        <img key={big.clone()} src={big} width={u.big.0.to_string()} height={u.big.1.to_string()}
                             alt={format!("{} — {}", u.name, l.t("its window in Cantus", "su ventana en Cantus"))} />
                    </a>
                    <figcaption>
                        <div class="ab-seg" role="group" aria-label={l.t("Theme of the picture", "Tema de la imagen")}>
                            <button type="button" aria-pressed={(shown == "dark").to_string()} onclick={pick("dark")}>{ l.t("Dark theme", "Tema oscuro") }</button>
                            <button type="button" aria-pressed={(shown == "light").to_string()} onclick={pick("light")}>{ l.t("Light theme", "Tema claro") }</button>
                        </div>
                        <span>{ l.t("The program's own window, drawn headless.", "La ventana del propio programa, dibujada sin pantalla.") }</span>
                    </figcaption>
                </figure>
                <div class="up-body">
                    <div class="up-main">
                        <p class="up-what">{ crate::unit_text::what(u, l) }</p>
                        if let Some((file, song)) = u.ab {
                            <div class="up-ab">
                                <h2 class="h4">{ l.t("Hear it, with and without", "Escúchalo, con y sin") }</h2>
                                <AbPlayer key={file} file={file} song={song} />
                            </div>
                        }
                        <PresetSounds key={u.file} unit={u.file} />
                    </div>
                    <div class="up-side">
                        <h2 class="h4">{ l.t("What stands out", "Lo que destaca") }</h2>
                        <ul class="up-feats">{ for feats }</ul>
                        <ul class="up-meta">
                            if let Some(p) = presets { <li>{ p }</li> }
                            if u.plugin {
                                <li>{ l.t("Also a CLAP and VST3 plug-in for other DAWs, with the same face", "También plug-in CLAP y VST3 para otros DAW, con la misma cara") }</li>
                            } else {
                                <li>{ l.t("Every change one undo step, saved with the song", "Cada cambio, un paso de deshacer, guardado con la canción") }</li>
                            }
                            <li>{ l.t("Free, inside Cantus", "Gratis, dentro de Cantus") }</li>
                        </ul>
                        <a class="btn btn-primary" href={href(l, "/download")}>{ crate::icons::arrow_down() }{ l.t("Get Cantus, free", "Consigue Cantus, gratis") }</a>
                    </div>
                </div>
                <nav class="uf-pager" aria-label={l.t("Next and previous", "Siguiente y anterior")}>
                    <a href={href(l, &format!("{base}{}", prev.file))}><small>{ l.t("← Previous", "← Anterior") }</small><b>{ prev.name }</b></a>
                    <a href={href(l, &format!("{base}{}", next.file))}><small>{ l.t("Next →", "Siguiente →") }</small><b>{ next.name }</b></a>
                </nav>
                <ul class="spot-shelf" aria-label={group_name}>{ for shelf }</ul>
            </div></div>
        </article>
    }
}

/// `/instruments` and `/effects`: every unit's card (the cards open their page in place).
#[function_component(UnitIndex)]
pub fn unit_index() -> Html {
    let l = use_lang().lang;
    html! {
        <>
            <div class="page-band lapis"><div class="wrap">
                <span class="eyebrow">{ l.t("Instruments & effects", "Instrumentos y efectos") }</span>
                <h1>{ l.t("All its own, all free", "Todos propios, todos gratis") }</h1>
            </div></div>
            <crate::units::Units />
        </>
    }
}
