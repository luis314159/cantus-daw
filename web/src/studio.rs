//! The professional studio (round 36): the recording mode, told whole with its measured
//! numbers and what it does not do; then VCA faders and snapshots, the audio editor,
//! external gear with its latency ping, Mackie Control and HUI, and ARA 2 hosting.

use yew::prelude::*;

use crate::lang::{use_lang, Lang};
use crate::units::shade_now;

/// The recording mode's pictures, in the page's theme and language.
fn shot(what: &str, l: Lang) -> String {
    let theme = if shade_now() == "light" {
        "paper"
    } else {
        "night"
    };
    format!("/studio/record-{what}-{theme}-{}.webp", l.code())
}

#[function_component(RecordMode)]
pub fn record_mode() -> Html {
    let l = use_lang().lang;
    let tab = use_state(|| 0usize);
    let views = [
        (l.t("In the mode", "En el modo"), "on",
         l.t("Neon Tide in the recording mode: every strip that feeds the master is baked, the armed vocal take stays live, and the bar across the window says so. (The CPU figures in this picture are staged for it; the measured ones are below.)",
             "Neon Tide en el modo de grabación: cada canal que alimenta al máster está pre-renderizado, la toma de voz armada sigue en vivo, y la barra a lo ancho de la ventana lo dice. (Las cifras de CPU de esta imagen son de muestra; las medidas están abajo.)")),
        (l.t("Before it starts", "Antes de empezar"), "ask",
         l.t("Prepare to record asks first: what will be rendered, how much disk it may take, what stays live, the master live or baked, the mix controls live or locked — and the honest sentence about what it buys.",
             "Preparar para grabar pregunta primero: qué se va a renderizar, cuánto disco puede ocupar, qué sigue en vivo, el máster en vivo o pre-renderizado, los controles de mezcla en vivo o bloqueados — y la frase honesta de lo que consigue.")),
    ];
    let (_, what, caption) = views[*tab];
    let buttons = views.iter().enumerate().map(|(i, (name, ..))| {
        let tab = tab.clone();
        html! {
            <button type="button" role="tab" class="tab" aria-selected={(i == *tab).to_string()}
                    onclick={Callback::from(move |_| tab.set(i))}>{ *name }</button>
        }
    });
    let points = [
        (l.t("Everything baked but your take", "Todo pre-renderizado menos tu toma"),
         l.t("One click renders every strip that feeds the master — its effects, sends and automation, before its fader — and plays the renders back as streams. Armed tracks stay live with their own effects; the master chain can stay live or be baked too.",
             "Un clic renderiza cada canal que alimenta al máster — sus efectos, envíos y automatización, antes de su fader — y reproduce los renders como flujos. Las pistas armadas siguen en vivo con sus propios efectos; la cadena del máster puede seguir en vivo o pre-renderizarse también.")),
        (l.t("Mix while you record", "Mezcla mientras grabas"),
         l.t("Fader, pan, mute and solo keep working on every baked strip that goes straight to the master, and fader and pan on one that feeds others only before its fader; the question says how many stay locked. Or lock them all. Anything else waits behind the line Recording mode — leave it to edit.",
             "Fader, paneo, mute y solo siguen funcionando en cada canal pre-renderizado que va directo al máster, y fader y paneo en uno que alimenta a otros solo antes de su fader; la pregunta dice cuántos quedan bloqueados. O bloquéalos todos. Lo demás espera tras el aviso Modo de grabación — sal de él para editar.")),
        (l.t("A bar you cannot miss", "Una barra imposible de pasar por alto"),
         l.t("The buffer, the round trip, the CPU before and now, how many strips are baked, what monitoring leaves out, and Leave — always on screen while the mode is on.",
             "El búfer, el viaje de ida y vuelta, la CPU antes y ahora, cuántos canales están pre-renderizados, qué deja fuera el monitoreo, y Salir — siempre a la vista mientras el modo está activo.")),
        (l.t("Leave it, and nothing changed", "Sal de él, y nada cambió"),
         l.t("The baked mix equals the live mix to the bit — the tests hold it at every block size. The mode is never an undo step and never written into your project; going back in renders only what changed since.",
             "La mezcla pre-renderizada es igual a la mezcla en vivo bit a bit — las pruebas lo sostienen en cada tamaño de bloque. El modo nunca es un paso de deshacer ni se escribe en tu proyecto; volver a entrar renderiza solo lo que cambió.")),
        (l.t("Optional, never automatic", "Opcional, nunca automático"),
         l.t("It never switches on by itself. A plug-in that cannot render offline is named and stays live, or stops the preparation — you choose before it starts.",
             "Nunca se activa solo. Un plug-in que no puede renderizar sin conexión se nombra y sigue en vivo, o detiene la preparación — tú eliges antes de empezar.")),
    ];
    let rows = points
        .into_iter()
        .map(|(t, p)| html! { <li><h3>{ t }</h3><p>{ p }</p></li> });
    html! {
        <section class="section record-wrap" id="record">
            <div class="wrap">
                <header class="sec-head">
                    <span class="kicker">{ l.t("New · the recording mode", "Nuevo · el modo de grabación") }</span>
                    <h2>{ l.t("Prepare to record: the whole song baked, only your take live", "Preparar para grabar: toda la canción pre-renderizada, solo tu toma en vivo") }</h2>
                    <p>{ l.t("A heavy song and a low buffer do not mix: every plug-in must finish before the next block. Cantus renders the song you are recording over, so while you play, the computer only reads and sums — and the buffer can go down without dropouts.",
                             "Una canción pesada y un búfer bajo no se llevan: cada plug-in debe terminar antes del siguiente bloque. Cantus renderiza la canción sobre la que grabas, así mientras tocas la computadora solo lee y suma — y el búfer puede bajar sin cortes.") }</p>
                </header>
                <div class="tabs" role="tablist" aria-label={l.t("The recording mode", "El modo de grabación")}>{ for buttons }</div>
                <figure class="record-stage">
                    <div class="backdrop"><div class="window">
                        <img src={shot(what, l)} width="1920" height="1080" loading="lazy"
                             alt={l.t("Cantus in the recording mode", "Cantus en el modo de grabación")} />
                    </div></div>
                    <figcaption>{ caption }</figcaption>
                </figure>

                <div class="record-numbers">
                    <h3>{ l.t("Measured: 64 tracks with effects", "Medido: 64 pistas con efectos") }</h3>
                    <ul class="stats">
                        <li><span class="stat-k">{ l.t("Live", "En vivo") }</span>
                            <strong>{ "224–590 %" }</strong>
                            <span>{ l.t("of each block's time: it could not play live at any buffer from 32 to 512 frames", "del tiempo de cada bloque: no pudo sonar en vivo con ningún búfer de 32 a 512 muestras") }</span></li>
                        <li class="is-good"><span class="stat-k">{ l.t("In the mode", "En el modo") }</span>
                            <strong>{ "2–4 %" }</strong>
                            <span>{ l.t("mean, the 64 strips baked and streamed from disk, same buffers", "de media, los 64 canales pre-renderizados y leídos del disco, mismos búferes") }</span></li>
                        <li class="is-good"><span class="stat-k">{ l.t("Master baked too", "Con el máster pre-renderizado") }</span>
                            <strong>{ "0.4–0.9 %" }</strong>
                            <span>{ l.t("mean: the whole mix as one stream", "de media: toda la mezcla como un solo flujo") }</span></li>
                        <li class="is-good"><span class="stat-k">{ l.t("Disk underruns", "Cortes del disco") }</span>
                            <strong>{ "0" }</strong>
                            <span>{ l.t("in one-minute real-time runs at every buffer from 32 to 1024 frames, with 64 and with 128 streams", "en pasadas de un minuto en tiempo real con cada búfer de 32 a 1024 muestras, con 64 y con 128 flujos") }</span></li>
                        <li><span class="stat-k">{ l.t("Lowest clean buffer here", "Búfer limpio más bajo aquí") }</span>
                            <strong>{ "64" }</strong>
                            <span>{ l.t("frames (1.3 ms) for 64 streams with no block over budget in a minute — 128 frames for 128 streams", "muestras (1,3 ms) para 64 flujos sin ningún bloque fuera de tiempo en un minuto — 128 muestras para 128 flujos") }</span></li>
                    </ul>
                    <p class="stats-how">{ l.t("Each track a synth into EQ, compressor, delay and reverb; processor time per audio block against the block's length, one thread, on the development machine (Windows, 16 logical cores). The live runs shared the machine with other builds; the mode's ran on a quiet one, paced at the device's rate. Preparing 64 one-minute tracks took 236 s. On your interface the numbers will differ. Method and every figure: docs/round36/record.md.",
                                               "Cada pista un sinte hacia EQ, compresor, delay y reverb; tiempo de procesador por bloque de audio frente a la duración del bloque, un hilo, en la máquina de desarrollo (Windows, 16 núcleos lógicos). Las pasadas en vivo compartieron la máquina con otras compilaciones; las del modo corrieron en una tranquila, al ritmo del dispositivo. Preparar 64 pistas de un minuto tomó 236 s. Con tu interfaz las cifras serán otras. Método y cada cifra: docs/round36/record.md.") }</p>
                </div>

                <ul class="score-points record-points">{ for rows }</ul>

                <aside class="honest">
                    <h3>{ l.t("What it does not do", "Lo que no hace") }</h3>
                    <p>{ l.t("It does not shorten your interface's own round trip — the converters and the driver set that, and the bar shows it as the driver reports it. What it lowers is the work per block, so a lower buffer holds. The lowest stable buffer is your machine's to find: Cantus gives you the numbers to find it.",
                             "No acorta el viaje de ida y vuelta de tu interfaz — eso lo fijan los conversores y el driver, y la barra lo muestra como lo reporta el driver. Lo que baja es el trabajo por bloque, para que un búfer más bajo aguante. El búfer estable más bajo es cosa de tu máquina: Cantus te da los números para encontrarlo.") }</p>
                </aside>
            </div>
        </section>
    }
}

#[function_component(Studio)]
pub fn studio() -> Html {
    let l = use_lang().lang;
    let theme = if shade_now() == "light" {
        "paper"
    } else {
        "night"
    };
    // The Spanish page shows the program in Spanish.
    let es = l.t("", "-es");
    let card = |id: &'static str,
                title: &'static str,
                text: &'static str,
                note: &'static str,
                img: String,
                alt: &'static str,
                (w, h): (u32, u32)| {
        html! {
            <article class="studio-card" id={id}>
                if !img.is_empty() {
                    <img src={img} width={w.to_string()} height={h.to_string()} loading="lazy" alt={alt} />
                }
                <div class="studio-text">
                    <h3>{ title }</h3>
                    <p>{ text }</p>
                    if !note.is_empty() { <p class="studio-note">{ note }</p> }
                </div>
            </article>
        }
    };
    html! {
        <section class="section studio-wrap" id="studio">
            <div class="wrap">
                <header class="sec-head">
                    <h2>{ l.t("The professional studio", "El estudio profesional") }</h2>
                    <p>{ l.t("A console's VCAs and scenes, an audio editor, your outboard gear on time, a control surface under your hands, and ARA plug-ins on your clips.",
                             "Los VCA y las escenas de una consola, un editor de audio, tu equipo externo a tiempo, una superficie de control bajo tus manos, y plug-ins ARA en tus clips.") }</p>
                </header>
                <div class="studio-grid">
                    { card("vca",
                        l.t("VCA faders, snapshots and scene cues", "Faders VCA, snapshots y cues de escena"),
                        l.t("A VCA strip moves its members' gain at the gain stage: their own faders and automation stay where you left them, and a member can follow two VCAs, or a VCA another. Snapshots take the whole desk or chosen channels and recall them in one undo step, with recall-safe and solo-safe strips and an A/B compare; put one on the timeline and the mix changes on that bar.",
                            "Un canal VCA mueve la ganancia de sus miembros en la etapa de ganancia: sus propios faders y su automatización quedan donde los dejaste, y un miembro puede seguir a dos VCA, o un VCA a otro. Los snapshots toman la consola entera o los canales que elijas y los recuperan en un paso de deshacer, con canales a salvo del recall y del solo y una comparación A/B; pon uno en la línea de tiempo y la mezcla cambia en ese compás."),
                        l.t("A VCA at 0 dB touches no sample: a null test holds it.", "Un VCA a 0 dB no toca ninguna muestra: una prueba nula lo sostiene."),
                        format!("/studio/mix-snap-{theme}-{}.webp", l.code()), l.t("The mixer with two VCA strips and the Snapshots menu open", "El mezclador con dos canales VCA y el menú Snapshots abierto"), (1920, 1080)) }
                    { card("editor",
                        l.t("An audio editor, opened from the clip", "Un editor de audio, abierto desde el clip"),
                        l.t("Edit in audio editor… on any clip: cut, paste, silence, fades with five curves, normalise to peak, RMS or LUFS, reverse, remove silence — each edit a new file and one undo step, the original kept. A spectrogram where you select a region in time and frequency and turn it down; noise reduction by a learned profile; declick and repair. Markers, regions and loop points travel with the file.",
                            "Editar en el editor de audio… en cualquier clip: cortar, pegar, silencio, fundidos con cinco curvas, normalizar a pico, RMS o LUFS, invertir, quitar silencios — cada edición un archivo nuevo y un paso de deshacer, el original conservado. Un espectrograma donde seleccionas una región en tiempo y frecuencia y la atenúas; reducción de ruido por perfil aprendido; declick y reparación. Marcadores, regiones y puntos de loop viajan con el archivo."),
                        l.t("In its tests: hiss 12 dB down or more, a voice within 1 dB.", "En sus pruebas: el siseo 12 dB más bajo o más, una voz a menos de 1 dB."),
                        format!("/studio/editor-{theme}{es}.webp"), l.t("The audio editor: the waveform over its spectrogram, a selection, markers and the noise reduction controls", "El editor de audio: la onda sobre su espectrograma, una selección, marcadores y los controles de reducción de ruido"),
                        if theme == "night" && es.is_empty() { (1800, 1080) } else { (1500, 900) }) }
                    { card("external",
                        l.t("Outboard gear, on time by itself", "Equipo externo, a tiempo por sí solo"),
                        l.t("External instrument and External effect units send MIDI and audio out of your interface and bring the result back. Ping sends a burst out and listens for it to return: the trip is measured to the sample, stored with the unit, and delay compensation uses it. An interface that is unplugged is said, never loaded blindly.",
                            "Las unidades Instrumento externo y Efecto externo envían MIDI y audio por tu interfaz y traen el resultado de vuelta. Ping envía una ráfaga y la escucha volver: el viaje se mide a la muestra, se guarda con la unidad y la compensación de retardo lo usa. Una interfaz desconectada se avisa, nunca se carga a ciegas."),
                        l.t("Hardware cannot be rendered offline: export and freeze refuse a track with outboard gear and say why.", "El hardware no se puede renderizar sin conexión: exportar y congelar rechazan una pista con equipo externo y dicen por qué."),
                        format!("/studio/external-{theme}{es}.webp"), l.t("Settings, MIDI: an external instrument with its measured latency, and an external effect waiting for its ping", "Ajustes, MIDI: un instrumento externo con su latencia medida, y un efecto externo esperando su ping"), (1500, 414)) }
                    { card("surfaces",
                        l.t("Mackie Control and HUI", "Mackie Control y HUI"),
                        l.t("A Mackie Control with its extenders, or a HUI, drives the mixer: motor faders that never fight your hand, V-Pots, transport and jog, banks and flip, scribble strips, meters and the timecode display. Two clicks to set one up; generic MIDI mapping stays beside it.",
                            "Un Mackie Control con sus extensores, o un HUI, maneja el mezclador: faders motorizados que nunca pelean con tu mano, V-Pots, transporte y jog, bancos y flip, scribble strips, medidores y la pantalla de código de tiempo. Dos clics para configurarlo; el mapeo MIDI genérico sigue al lado."),
                        l.t("Tested byte for byte against a built-in emulation of both protocols, written from their published notes; trying it on real hardware is still to come.", "Probado byte a byte contra una emulación integrada de ambos protocolos, escrita a partir de sus notas publicadas; falta probarlo con hardware real."),
                        format!("/studio/surfaces-{theme}{es}.webp"), l.t("Settings, MIDI: a Mackie Control, an extender and a HUI, with the last messages sent", "Ajustes, MIDI: un Mackie Control, un extensor y un HUI, con los últimos mensajes enviados"), (1500, 372)) }
                    { card("ara",
                        l.t("ARA 2 plug-ins on your clips", "Plug-ins ARA 2 en tus clips"),
                        l.t("Cantus hosts ARA 2 plug-ins on audio clips: bind a clip, open the plug-in's editor from it, and each edit gesture is one undo step. The plug-in renders the region and Cantus plays the render, so fades, clip gain and offline export keep working.",
                            "Cantus aloja plug-ins ARA 2 en clips de audio: vincula un clip, abre el editor del plug-in desde él, y cada gesto de edición es un paso de deshacer. El plug-in renderiza la región y Cantus reproduce el render, así los fundidos, la ganancia del clip y la exportación siguen funcionando."),
                        l.t("ARA plug-ins run inside Cantus's own process, not isolated like other plug-ins — the clip menu says so.", "Los plug-ins ARA corren dentro del proceso de Cantus, no aislados como los demás plug-ins — el menú del clip lo dice."),
                        "/studio/ara-clips.webp".to_string(), l.t("Audio clips in the arrangement, one rendered by an ARA plug-in", "Clips de audio en el arreglo, uno renderizado por un plug-in ARA"), (1500, 420)) }
                </div>
            </div>
        </section>
    }
}
