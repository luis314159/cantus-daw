//! The score: a part engraved beside the piano roll, a pane of its own, a printed page.

use yew::prelude::*;

use crate::lang::use_lang;

#[function_component(Score)]
pub fn score() -> Html {
    let l = use_lang().lang;
    let points = [
        (l.t("Read, not moved", "Leída, no movida"),
         l.t("The notes are read for the score only — each beat picks straight values or triplets — and what you hear never moves.",
             "Las notas se leen solo para la partitura —cada pulso elige figuras normales o tresillos— y lo que suena nunca se mueve.")),
        (l.t("Engraved like the notation programs", "Grabada como en los programas de notación"),
         l.t("Ties that show the beat, beams by the metre, a grand staff that follows the hands, accidentals in the song's key, dynamics from velocity, in the SMuFL font Bravura.",
             "Ligaduras que muestran el pulso, barras según el compás, un sistema de piano que sigue a las manos, alteraciones en la tonalidad de la canción y dinámicas desde la velocidad, en la fuente SMuFL Bravura.")),
        (l.t("Edit on the staff", "Edita en el pentagrama"),
         l.t("Click a note, move it a step of the key, or type notes by letter and value. Every edit is one undo step.",
             "Haz clic en una nota, muévela un grado de la tonalidad o escribe notas por letra y figura. Cada cambio es un paso de deshacer.")),
        (l.t("MusicXML, PDF, print", "MusicXML, PDF, impresión"),
         l.t("Send a part or the whole song to any notation program as MusicXML 4.0, export a vector PDF on A4 or Letter, or print it.",
             "Lleva una parte o la canción entera a cualquier programa de notación como MusicXML 4.0, exporta un PDF vectorial en A4 o Carta, o imprímela.")),
    ];
    let rows = points.into_iter().map(|(title, text)| {
        html! { <li><h3>{ title }</h3><p>{ text }</p></li> }
    });
    html! {
        <section class="section score-wrap" id="score">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("The score, from the piano roll", "La partitura, desde el piano roll") }</h2>
                    <p>{ l.t("Press Alt+S and the clip you are writing appears as an engraved staff under the piano roll, following the playhead. A whole track gets a pane of its own.",
                             "Pulsa Alt+S y el clip que escribes aparece como un pentagrama grabado bajo el piano roll, siguiendo la reproducción. Una pista entera tiene su propio panel.") }</p>
                </header>
                <div class="score-stage"><div class="backdrop">
                    <div class="window">
                        <div class="window-bar"><i></i><i></i><i></i><span>{ l.t("Cantus — piano roll and score", "Cantus — piano roll y partitura") }</span></div>
                        <img src="score/roll.webp" width="1400" height="760" loading="lazy"
                             alt={ l.t("The piano roll with the engraved staff under it", "El piano roll con el pentagrama grabado debajo") } />
                    </div>
                </div></div>
                <div class="score-more">
                    <figure>
                        <img src="score/pane.webp" width="900" height="400" loading="lazy"
                             alt={ l.t("The Score pane, in the light theme", "El panel Partitura, en el tema claro") } />
                        <figcaption>{ l.t("The Score pane, systems wrapped to its width.", "El panel Partitura, con los sistemas ajustados a su ancho.") }</figcaption>
                    </figure>
                    <figure class="paper">
                        <img src="score/page.webp" width="830" height="452" loading="lazy"
                             alt={ l.t("The top of a printed A4 page", "La parte de arriba de una página A4 impresa") } />
                        <figcaption>{ l.t("A page as the PDF prints it.", "Una página como la imprime el PDF.") }</figcaption>
                    </figure>
                </div>
                <ul class="score-points">{ for rows }</ul>
            </div>
        </section>
    }
}
