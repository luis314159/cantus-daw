//! AI that works with you, on your computer: agents through MCP, stems, audio to MIDI and
//! the mastering assistant (round 34).

use yew::prelude::*;

use crate::lang::use_lang;

#[function_component(Ai)]
pub fn ai() -> Html {
    let l = use_lang().lang;
    let points = [
        (l.t("Your assistant, in your DAW", "Tu asistente, en tu DAW"),
         l.t("Claude Code, Claude Desktop, Cursor, VS Code, Gemini CLI and LM Studio drive Cantus through MCP: tracks, notes and chords, the mixer, effects, automation, export — about sixty tools made for music.",
             "Claude Code, Claude Desktop, Cursor, VS Code, Gemini CLI y LM Studio manejan Cantus por MCP: pistas, notas y acordes, el mezclador, efectos, automatización, exportar — unas sesenta herramientas hechas para la música.")),
        (l.t("Every change, one undo step", "Cada cambio, un paso de deshacer"),
         l.t("Edits ask first on a card; removing your material and files always ask. External agents are off until you allow them, on this computer only.",
             "Las ediciones preguntan primero en una tarjeta; quitar tu material y los archivos siempre preguntan. Los agentes externos están apagados hasta que los permites, solo en esta computadora.")),
        (l.t("Stems and audio to MIDI", "Stems y audio a MIDI"),
         l.t("Split a song into drums, bass, vocals and the rest with HT-Demucs — 8.80 dB SDR, the paper's own — or turn a melody, chords or a drum loop into notes.",
             "Separa una canción en batería, bajo, voces y el resto con HT-Demucs — 8,80 dB de SDR, lo mismo que su artículo — o convierte una melodía, acordes o un loop de batería en notas.")),
        (l.t("Mastering you can read", "Un mastering que se puede leer"),
         l.t("Pick a platform; the assistant proposes a chain of Cantus's own effects, says why, compares at the same loudness and leaves every knob yours.",
             "Elige una plataforma; el asistente propone una cadena con los efectos de Cantus, dice por qué, compara a la misma sonoridad y deja cada perilla en tus manos.")),
    ];
    let rows = points.into_iter().map(|(title, text)| {
        html! { <li><h3>{ title }</h3><p>{ text }</p></li> }
    });
    html! {
        <section class="section ai-wrap" id="ai">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("AI that works with you, on your computer", "IA que trabaja contigo, en tu computadora") }</h2>
                    <p>{ l.t("Bring the assistant you already use, or none at all. Cantus has no AI service of its own and sends nothing anywhere; models download from their makers only when you ask.",
                             "Trae el asistente que ya usas, o ninguno. Cantus no tiene un servicio de IA propio y no envía nada a ninguna parte; los modelos se descargan de sus autores solo cuando lo pides.") }</p>
                </header>
                <div class="score-stage"><div class="backdrop">
                    <div class="window">
                        <div class="window-bar"><i></i><i></i><i></i><span>{ l.t("Cantus — an agent at work", "Cantus — un agente trabajando") }</span></div>
                        <img src="ai/agent.webp" width="1400" height="875" loading="lazy"
                             alt={ l.t("Claude Code working in Cantus: the AI activity lists its calls, a card asks before a track is deleted", "Claude Code trabajando en Cantus: la actividad de IA lista sus llamadas, una tarjeta pregunta antes de borrar una pista") } />
                    </div>
                </div></div>
                <div class="score-more">
                    <figure>
                        <img src="ai/stems.webp" width="1400" height="860" loading="lazy"
                             alt={ l.t("A song separated into drums, bass, other and vocals, each on its own track under a bus", "Una canción separada en batería, bajo, otros y voces, cada una en su pista bajo un bus") } />
                        <figcaption>{ l.t("Separate stems: the parts on their own tracks, at the original's level, in one undo step.", "Separar stems: las partes en pistas propias, al nivel del original, en un solo paso de deshacer.") }</figcaption>
                    </figure>
                    <figure>
                        <img src="ai/mastering.webp" width="622" height="804" loading="lazy"
                             alt={ l.t("The mastering assistant: the target, the mix and the proposal measured, the octave balance and the proposed chain", "El asistente de mastering: el objetivo, la mezcla y la propuesta medidas, el balance por octavas y la cadena propuesta") } />
                        <figcaption>{ l.t("The mastering assistant: what it hears, what it proposes and why.", "El asistente de mastering: lo que oye, lo que propone y por qué.") }</figcaption>
                    </figure>
                </div>
                <div class="score-more ai-pair">
                    <figure>
                        <img src="ai/settings.webp" width="780" height="468" loading="lazy"
                             alt={ l.t("Settings ▸ AI: the AI area, external agents on 127.0.0.1 with a token, setups for six clients", "Ajustes ▸ IA: el área de IA, agentes externos en 127.0.0.1 con un token, configuración para seis clientes") } />
                        <figcaption>{ l.t("One switch for the whole area; a token per client; setups ready to paste.", "Un interruptor para toda el área; un token por cliente; configuraciones listas para pegar.") }</figcaption>
                    </figure>
                    <figure>
                        <img src="ai/midi.webp" width="608" height="648" loading="lazy"
                             alt={ l.t("Convert to MIDI: melody, notes and chords or drums, and the model it uses", "Convertir a MIDI: melodía, notas y acordes o batería, y el modelo que usa") } />
                        <figcaption>{ l.t("Convert to MIDI: a melody, chords or a drum loop, as notes on a new track.", "Convertir a MIDI: una melodía, acordes o un loop de batería, como notas en una pista nueva.") }</figcaption>
                    </figure>
                </div>
                <ul class="score-points">{ for rows }</ul>
            </div>
        </section>
    }
}
