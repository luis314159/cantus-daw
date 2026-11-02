//! A dancer for your songs: the family, imported characters, the companion for streams and
//! the clip as a video (round 35).

use yew::prelude::*;

use crate::lang::use_lang;

#[function_component(Dance)]
pub fn dance() -> Html {
    let l = use_lang().lang;
    let points = [
        (l.t("To the beat, by itself", "Al compás, solo"),
         l.t("Add a Saltator track and it dances on the beat with no automation. Draw automation, add a modulator, learn a knob or play notes from C3 and it changes moves exactly on your bars.",
             "Agrega una pista Saltator y baila al compás sin automatización. Dibuja automatización, agrega un modulador, aprende una perilla o toca notas desde C3 y cambia de paso justo en tus compases.")),
        (l.t("Six of ours, or yours", "Seis nuestros, o los tuyos"),
         l.t("Trino, Miso, Lana, Tic, Corchea and Sol — or your own Lottie, Aseprite, TexturePacker or DragonBones file, read by Cantus's own players with no script run and its licence shown.",
             "Trino, Miso, Lana, Tic, Corchea y Sol — o tu propio archivo Lottie, Aseprite, TexturePacker o DragonBones, leído por los reproductores de Cantus sin ejecutar ningún script y con su licencia a la vista.")),
        (l.t("On stream, all day", "En tu transmisión, todo el día"),
         l.t("The companion dances above everything with true transparency; streaming mode gives OBS a clean window. About 3 % of one core, measured for an hour.",
             "La ventana de compañía baila encima de todo con transparencia real; el modo streaming le da a OBS una ventana limpia. Cerca del 3 % de un núcleo, medido durante una hora.")),
        (l.t("A clip for social media", "Un clip para redes"),
         l.t("MP4, WebM, or a transparent MOV for your editor, with the song's audio — square, vertical or wide, in sync to the sample.",
             "MP4, WebM, o un MOV transparente para tu editor, con el audio de la canción — cuadrado, vertical o ancho, sincronizado a la muestra.")),
    ];
    let rows = points.into_iter().map(|(title, text)| {
        html! { <li><h3>{ title }</h3><p>{ text }</p></li> }
    });
    html! {
        <section class="section dance-wrap" id="dance">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("A dancer for your songs", "Un bailarín para tus canciones") }</h2>
                    <p>{ l.t("A character that dances over your workspace to whatever plays — for the fun of it while you work, for your stream, or for a clip to post. It makes no sound: every sample of your song stays the same, bit for bit.",
                             "Un personaje que baila sobre tu espacio de trabajo con lo que suene — por gusto mientras trabajas, para tu transmisión o para un clip que publicar. No hace ningún sonido: cada muestra de tu canción queda igual, bit por bit.") }</p>
                </header>
                // Written as HTML so the parser makes the video muted from the start (an
                // attribute set afterwards does not mute it, and browsers then refuse autoplay).
                <div class="dance-hero">{ Html::from_html_unchecked(AttrValue::from(format!(
                    r#"<video src="dance/family.mp4" poster="dance/family.webp" width="1440" height="318" autoplay muted loop playsinline preload="metadata" aria-label="{}"></video>"#,
                    l.t("Trino, Miso, Lana, Tic, Corchea and Sol dancing together on one stage", "Trino, Miso, Lana, Tic, Corchea y Sol bailando juntos en un escenario"),
                ))) }</div>
                <div class="score-more">
                    <figure>
                        <img src="dance/window.webp" width="1040" height="680" loading="lazy"
                             alt={ l.t("Saltator's window: the stage, the six characters, sixteen move pads, and Dance, Body, Look and Ears", "La ventana de Saltator: el escenario, los seis personajes, dieciséis pads de pasos, y Baile, Cuerpo, Aspecto y Oído") } />
                        <figcaption>{ l.t("Saltator: pick who dances, play its moves from the pads, shape the dance.", "Saltator: elige quién baila, toca sus pasos desde los pads, dale forma al baile.") }</figcaption>
                    </figure>
                    <figure>
                        <img src="dance/companion.webp" width="500" height="560" loading="lazy"
                             alt={ l.t("The companion window over the desktop with true transparency: Trino, the song and tempo, bar and beat, the master's level", "La ventana de compañía sobre el escritorio con transparencia real: Trino, la canción y el tempo, compás y pulso, el nivel del máster") } />
                        <figcaption>{ l.t("The companion: always on top, see-through, ready for OBS.", "La compañía: siempre encima, transparente, lista para OBS.") }</figcaption>
                    </figure>
                </div>
                <div class="score-more dance-pair">
                    <figure>
                        <img src="dance/video.webp" width="899" height="718" loading="lazy"
                             alt={ l.t("Export video: the preview of a moment, vertical 1080 × 1920, the background, a title and the encoders it will use", "Exportar video: la vista previa de un momento, vertical 1080 × 1920, el fondo, un título y los codificadores que usará") } />
                        <figcaption>{ l.t("Export video: what you see is the file's own frame.", "Exportar video: lo que ves es el cuadro del propio archivo.") }</figcaption>
                    </figure>
                    <figure>
                        <img src="dance/import.webp" width="1400" height="820" loading="lazy"
                             alt={ l.t("Importing a Lottie character: its licence, what it holds, its animations mapped to moves", "Importar un personaje Lottie: su licencia, lo que contiene, sus animaciones asignadas a pasos") } />
                        <figcaption>{ l.t("Your own character: its licence first, its animations mapped to our moves.", "Tu propio personaje: primero su licencia, sus animaciones asignadas a nuestros pasos.") }</figcaption>
                    </figure>
                </div>
                <ul class="score-points">{ for rows }</ul>
            </div>
        </section>
    }
}
