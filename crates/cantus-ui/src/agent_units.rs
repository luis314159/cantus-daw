//! The machine-readable catalogue of every unit (round 34, builder `tools`, ADR 0082):
//! each native instrument and effect with its parameters — keys, English names, groups,
//! ranges, units, tapers, defaults, the names of a stepped one's choices — its factory
//! presets and what they set, its starting preset, latency, what it is for and the
//! model's limits. **Generated from the parameter tables** (`cantus_core::fx::*`,
//! `cantus_core::inst::*`, the pads), the names from the English locale the editors use;
//! the test fails when a kind or a parameter is missing.

use crate::controllers_words::effect_choice_words;
use crate::i18n::{t_in, Locale};
use cantus_ai::tools_project::args::{effect_names, native_id, INSTRUMENTS};
use cantus_core::pads::{KitField, PadField, PadVoice};
use cantus_core::starting::{start_of, Neutral, Start};
use cantus_core::{
    EffectKind, EffectParamInfo, EffectSlot, EffectSpec, FirmusParam, InstrumentKind,
};
use serde_json::{json, Value};

fn en(key: &str) -> String {
    t_in(Locale::En, key)
}

/// What each effect is for, in a sentence a model can choose by (exhaustive: a new kind
/// does not compile until it is described).
pub fn effect_use(kind: EffectKind) -> &'static str {
    use EffectKind as K;
    match kind {
        K::Eq => "Parametric EQ, 24 bands (bell, shelves, cuts, notch, tilt), stereo/mid/side, zero-latency, natural or linear phase, a dynamic mode per band. Tone shaping, cleaning lows, carving masking frequencies, mastering EQ.",
        K::Compressor => "Compressor with styles (clean, vintage, opto, FET, bus…), sidechain from another strip and a filtered key. Control dynamics, glue a bus, duck with a sidechain.",
        K::Limiter => "Brickwall limiter with true peak and lookahead. The last unit on the master or a safety on a bus.",
        K::Space => "Algorithmic reverb with rooms, halls, plates and a decay EQ. A send return for space, or an insert for character.",
        K::Multiband => "Multiband dynamics (up to six bands): compress or expand each band. Mastering glue, de-mud, tame a resonant band.",
        K::DeEsser => "De-esser: tames sibilance (4–10 kHz) of vocals and cymbals, split-band or wideband.",
        K::Gate => "Gate and expander, keyed from its own signal, another strip or a track's notes. Tighten drums, rhythmic gating.",
        K::Saturator => "Saturation and colour: tape, tube, overdrive, a drawn waveshaper, oversampled. Warmth, grit, harmonics, loudness without peaks.",
        K::Stereo => "Stereo control: width, mid/side balance, mono bass below a frequency, Haas widening, panning. Mono compatibility, a wider chorus.",
        K::Maximizer => "Mastering maximizer: multiband dynamics into a limiter, loud and clean. The last unit on the master for streaming loudness.",
        K::Delay => "Stereo delay, 1 ms to 2 s (set in ms; no tempo sync), with feedback, tone and ping-pong. Echoes, slapback, throws on a send.",
        K::Reverb => "The simple reverb (older projects); Spatium is the full one.",
        K::External => "A hosted CLAP/VST3 plug-in, chosen by the user in Cantus.",
        K::Chorus => "Chorus (Juno-style, Dimension-style, modern): width and motion on pads, keys, guitars, a doubled voice.",
        K::Flanger => "Through-zero flanger: jet sweeps, synced sweeps on leads and drums.",
        K::Phaser => "Phaser (Phase 90 and more): swirling motion on guitars, keys, pads.",
        K::Tremolo => "Tremolo and auto-pan, synced to the song: pulsing amplitude or a moving stereo image.",
        K::Vibrato => "Vibrato and tape wow/flutter: pitch motion, lo-fi drift.",
        K::RingMod => "Ring modulator: metallic, bell-like, inharmonic tones; a shimmer at low mix.",
        K::FreqShift => "Frequency shifter: barber-pole risers, detuned stereo, inharmonic movement.",
        K::VolumeShaper => "Volume shaper (LFO Tool style): sidechain-style pumping on quarters, gating rhythms, synced to the song.",
        K::Clipper => "Clipper with true peak: shave peaks before a limiter, drive drums, a soft master clip.",
        K::Transient => "Transient shaper: more or less attack and sustain — punchier kicks, softer snares.",
        K::TransientSplit => "Transient splitter: separate gain and processing of the attack and the body.",
        K::Analyzer => "Analyser (spectrum, loudness, stereo): passes its sound through unchanged; for looking.",
        K::AutoTune => "Real-time pitch correction to a key and scale, from natural to hard-tuned, with created vibrato.",
        // r39:conv
        K::Aula => "Convolution reverb: a real room from its impulse response — Cantus's own rooms, halls, plates and springs, a WAV the user loads (mono, stereo, true stereo), or a free pack; shape it with stretch, length, decay, damping and cuts, pre-delay and width.",
        K::AutoFilter => "Auto filter: two analog-modelled filters (SVF, transistor ladder, OTA cascade, diode ladder, Sallen-Key; low/band/high-pass, notch, peak, vowel; 12 or 24 dB/oct) in series or parallel, moved by a song-synced or free LFO (7 waves), an envelope follower on the input or a sidechain, and a drawn curve — each with a depth onto cutoff (octaves), resonance and vowel. Sweeps, wobbles, auto-wah, pump filters, talking vowels, telephone. Starts on a bar-long sweep; mix 0 is a wire.", // fx:cribrum
        K::Pedal => "A pedalboard of up to three distortion and fuzz pedals in any order, each modelled from its circuit: a green overdrive (soft, mid-forward), an op-amp distortion (thick, with a Filter), a silicon distortion (hard clipping, scooped), a germanium fuzz (round; its Bias starves the battery into a sputtering, gated fuzz) and a triangle muff (endless sustain, scooped mids); a noise gate before them, mix and mono. Slots s1..s3 hold 0 (empty) or 1..5 in that order; each pedal has its footswitch and three knobs.", // fx:fragor
        K::TimeShaper => "Time shaper, Gross Beat class: two banks of 36 drawn patterns played together — time patterns (how far behind the music it reads: tape stop and start, half and quarter time, stutters and rolls, beat repeats, reverse, rewind, backspin, scratches) and volume patterns (gates, trance gates, pumps, fades, tremolos, cuts) — on cycles of 1/4 to 4 bars synced to the song, chosen by tslot/vslot (automatable) or by notes (time from C4, volume from C1; switch or hold, optional restart). Both on Through (slot 1) is a wire; starts on Half time.", // fx:lapsus
        K::LoFi => "Lo-fi: six modules, each switched on with its key (bits.on, rate.on, wob.on, age.on, noise.on, tone.on) — bits (1..24, rounding/truncation/mu-law, dither), sample rate (1..48 kHz, raw with its aliasing, filtered, or smoothed), wow and flutter in cents (wow synced or free, part of it drifting), age (tape dropouts and wear), noise (vinyl crackle and pops, tape or vinyl hiss, 50/60 Hz hum, optionally following the music) and tone (the band of tape, radio, telephone or gramophone) — and Character (char), which takes every module that is on towards its oldest. Cassettes, 8-bit, dusty drums, AM radio, telephone, gramophone, VHS. Starts as a worn cassette; every module off is a wire.", // fx:vetus
        K::Utility => "Utility: gain (out, ±36 dB), balance (pan, −1..1, the master's balance law), width (0..2, 0 is mono), the channel operation (chan: 0 stereo, 1 swap, 2 mono, 3 left to both, 4 right to both, 5 mid, 6 side), polarity per channel (pol.l, pol.r), DC removal (dc), a delay per channel in ms applied in whole samples (delay.l, delay.r — to align two microphones), mute; its window shows a tuner (tuner.ref is its A, 415..466 Hz), a goniometer and the correlation. Mono checks, phase flips, alignment, a pad, listening to mid or side. Inserts as a wire.", // fx:libra
        // fx3:contract — wave 3 (each builder words its own once its unit is built).
        K::Board => "A pedalboard: Cantus's own effects as pedals (any of them but a hosted plug-in, Speculum and the old reverb), up to sixteen, in any order along Pre, then an optional A/B split (lane A beside lane B, mixed by ab: 0 A only, 1 B only), then Post. Each pedal is that unit exactly, its parameters addressed p{id}.{key} (p2.time_ms); a footswitch never changes the latency. Eight macros m1..m8 each move pedals' parameters between two values; in/out (dB), mix (dry/wet, the dry aligned to the latency). It starts on a guitar rig: green overdrive, chorus, delay, spring reverb. Its factory presets are whole boards (guitar, bass, synth, drums, vocals).", // fx3:tabula
        K::PitchShift => "Pitch: four modes (mode) — 0 Shift: two voices at fixed intervals (st1/st2 semitones ±24, ct1/ct2 cents), 1 Harmony: two voices deg1/deg2 scale degrees from the note heard (hsrc 0 in key/scale, 1 in the chord held on MIDI, 2 singing the MIDI melody; key 0..11, scale 0..12), 2 Octave: one down/two down/one up (sub1, sub2, up1; ostyle 0 analog divider with no latency, 1 clean), 3 Whammy: one voice swept by pedal 0..1 from heel to toe (semitones), glided (glide ms), or pressed by held MIDI notes (wsrc 1). engine 0 mono (one note at a time, formants kept, 2.7 ms latency) or 1 poly (chords and whole mixes, 85 ms) for Shift and Whammy; each voice's level (lvl1/lvl2) and balance (pan1/pan2), dry, formant keep/move, throat, a4, out. Starts on Octave up, twelve-string (poly); defaults are the input delayed, a null.", // fx3:saltus
        K::Rotary => "Rotary speaker: horn and drum turning, two microphones. speed 0 stop (the brake parks both rotors), 1 slow (chorale), 2 fast (tremolo); each rotor ramps to it at its own pace (ramp scales the times, 0.25..4x). hslow/hfast horn rpm (20..80, 300..500), dslow/dfast drum rpm (20..80, 250..450); balance horn against drum dB (-12..12); spread the microphones' angle 0..180 degrees, dist their distance 10..300 cm; open 1 an open back; drive the cabinet's valve amp 0..1; notes 1: a note played on the track toggles slow and fast; mix 0..1 (defaults 0, a null), out dB. No latency. Starts on Chorale (slow, mix 1).", // fx3:rota
        K::Amp => "Amp and cabinet, valves modelled from their circuits: model 0 none, 1 clean (2x6V6, valve rectifier), 2 crunch (2xEL34), 3 chime (4xEL84 class A, presence works as a cut), 4 high gain (four stages, 4x6L6), 5 bass (2x5881); gain, bass, mid, treble, presence, master (1 = as built), sag (supply softness, 0.5 the amp's own) all 0..1; cab 0 none, 1 1x12 open, 2 2x12 open, 3 4x12 vintage, 4 4x12 modern, 5 4x10 open, 6 the loaded impulse-response file; mic 0 dynamic, 1 ribbon, 2 condenser, micpos 0 cap..1 edge, micdist 1..30 cm; os 0 2x, 1 4x, 2 8x oversampling (latency 1.0..1.2 ms while an amp is chosen); mix 0..1, out dB. Defaults: no amp, no cabinet (a null). Starts on Clean combo. For guitar, bass, keys and voice.", // fx3:arca
        K::Vocoder => "Vocoder: the track's own sound is the voice; the carrier (src) 0 the sidechain (another strip routed as the slot's key), 1 the built-in synthesiser played by the MIDI notes the slot listens to, 2 both; swap 1: the sidechain speaks through this track. bands 8..64, order 0..2 (slopes of order 4, 6, 8), lo/hi the band range Hz, width 0.5..2, att/rel ms; formant -24..24 st (moves the formants, not the notes), bshift -16..16 bands, tilt dB/oct, flat 0..1 (flatten the carrier), sib/sibhz/sibgate (the sibilance pass-through), unv (noise on consonants), hold 1 (freeze the vowel), spread, cpass (the raw carrier through); the synthesiser: saw, pulse, pw, noise, uni 1..7 copies, detune ct, uspread, oct -3..3, satt/srel ms, slevel dB; robot 1 drones rnote (MIDI 24..96) while the voice is above rgate dB; b0..b63 each band's level dB (-24..12, the bars the user draws on the bank); obj the object drawn in its window, 0 gooseneck, 1 talk-box tube, 2 desk microphone (a look, never heard; presets keep it); mix 0..1 (default 0, a null), out dB. No latency. Fifteen presets, both carriers (talk-box and sidechain carrier use the key; classic, choir, string machine the notes). Starts on Robot (the drone, 16 bands, mix 1, out +0.5 dB).", // fxa:loquax
        K::Glitch => "Glitch sequencer and grain cloud: ten lanes painted in time with the song (0 repeat, 1 reverse, 2 tape stop, 3 shuffle, 4 pitch jump, 5 crush, 6 filter sweep, 7 gate, 8 delay throw, 9 the cloud); the lanes are the spec's pattern, not parameters: set_effect's lanes paints them by name (blocks {at, len, amount, chance}; a block's amount is the module's mix for that event). step 0..6 (1/4, 1/8, 1/8T, 1/16, 1/16T, 1/32, 1/64), bars 1..4, swing, xfade ms; trig 0 the steps, 1 held notes (key + lane), 2 both; quant 1 a note waits for the next step; seed; repeat rsize (note 0..13: 1/2 .. 1/128), rramp 0..3, rpitch st, rdecay dB, rduty; reverse vslice (0 the step, else a note); tape tkind 0 stop 1 start, ttime, tcurve; shuffle sslice, srange 1..8, srev; pitch psemi ±24, pwin ms, pstyle 0 classic warble 1 clean; crush kbits, krate Hz; filter fkind 0 low 1 high 2 band, ffrom/fto Hz, fres, fcurve; gate grate (0 shut), gduty, gdepth; throw wtime, wfb, wtone Hz, wpp, wlvl; cloud clevel (beside the chain), cpos/cspray/csize ms, cdens grains/s, cpitch/crand st, cscale 0..6, cspread, crev, cwin 0..4, cfreeze; mix, out dB. Nothing painted and clevel 0: a null. No latency. Eighteen presets: stutters and builds, tape stops and starts, reverses, gates, filter sweeps, delay throws, shuffles, lo-fi fills, grain pads and a cloud to freeze, and sets played by notes. Starts on Last beat stutter.", // fxa:fractus
        K::Shimmer => "Shimmer reverb: size 0..1, decay 0.3..60 s, inf 1 infinite (ishim 0 the shimmer fades as it climbs, 1 sustains; ilevel 0 8 dB under an 8 s tail, 1 as loud), freeze 1, bloom ms (a swell), pre ms or psync 1..8 (1/32, 1/16, 1/8T, 1/8, 1/8., 1/4, 1/4., 1/2 at the song's tempo), diff, hhz/hdec high damping (Hz, x the decay), lhz/ldec low damping, mdepth/mrate modulation; shim 0..1 the shimmer, pa/pb its two voices st (-36..36), pblvl the second's level, slo/shi the loop's tone Hz; duck dB (keyed by the sidechain when routed), dthr, datt, drel; lcut/hcut the wet's tone, width 0..1.5, lines 0 eight 1 sixteen; mix 0..1 (default 0, a null), out dB. No latency. Starts on Octave shimmer.", // fxa:caelum
        // fxc — wave C's words (ADRs 0145–0146, provisional).
        K::Repair => "Repair (restoration as it plays): three modules, each off by default (a null). De-hum hon 1, hmode 0 fixed 1 followed (±2 Hz) 2 found (50 or 60 Hz, then followed), hhz 40..70 the fundamental, hharm 1..16 harmonics, hdepth 0..80 dB, hwidth 0.5..20 Hz the notches' width, hgentle 1 subtracts the learned hum held under a bass note (adds 50 ms), hmix, hout dB, hdelta 1 hears what is removed. De-click con 1, csens 0..0.7 (capped: higher harms the music), crackle 0..1 (0 off), cmix, cout, cdelta (adds 16 ms). De-reverb von 1, vamount 0..30 dB, vdecay 0 set 1 estimated, vt60 0.1..10 s, vdirect -10..20 dB the direct sound (default +6, a close voice; 0 a far microphone), vsmooth 0..1, vkeep 5..200 ms early reflections kept, vmix, vout, vdelta (adds 21.3 ms at 48 kHz). out dB. Starts on Mains hum 50 Hz (the de-hum on, followed).", // fxc:purus
        K::Reference => "Reference comparator, for the master's last insert: a reference track (a file set on the slot, by path) heard at the mix's loudness. listen 0 your mix (the default: untouched) 1 the reference 2 X hidden (ABX) 3/4 the blind preference's two choices; match 0 integrated 1 short term 2 by hand, offset -24..24 dB, refl 0 the reference's loudness over what has played 1 over its whole file, fade 1..500 ms the switch's crossfade, sync 0 its own playhead 1 locked to the song (soff seconds ahead) 2 a loop (lstart, lend seconds), seed and trial the blind test's order, res 0 thirds 1 sixths of an octave (the display), eqamt 0..1 and eqmax 1..12 dB the match EQ's suggestion (how far toward the reference, the most it boosts or cuts; the user sends it to an Aequalis before the unit). Up to eight references in a list, one compared. Exports and renders hear the mix, never the reference. No latency.", // fxc:comparatio
    }
}

pub fn instrument_use(id: &str) -> &'static str {
    INSTRUMENTS
        .iter()
        .find(|(i, ..)| *i == id)
        .map(|(_, _, _, w)| *w)
        .unwrap_or("")
}

fn taper(i: &EffectParamInfo) -> &'static str {
    if i.stepped {
        "stepped"
    } else if i.logarithmic {
        "log"
    } else {
        "linear"
    }
}

/// `b3.gain` → `Band 4`: a banded key's group, numbered from 1 as the editors show it.
pub fn group_of(key: &str) -> Option<String> {
    let (head, _) = key.split_once('.')?;
    let letters: String = head
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let digits = &head[letters.len()..];
    let n: usize = digits.parse().ok()?;
    let word = match letters.as_str() {
        "b" => "Band",
        "sc" => "Key band",
        "d" => "Decay band",
        "e" => "Post band",
        "p" => "Point",
        "osc" => "Oscillator",
        "env" => "Envelope",
        "lfo" => "LFO",
        "mod" => "Matrix slot",
        "macro" => "Macro",
        "z" => "Zone",
        "op" => "Operator",
        "filter" | "f" => "Filter",
        other => other,
    };
    Some(format!("{word} {}", n + 1))
}

fn row(key: &str, i: &EffectParamInfo, choices: Option<Vec<String>>) -> Value {
    let mut v = json!({
        "key": key, "name": en(i.label_key), "unit": i.unit,
        "min": i.min, "max": i.max, "default": i.default, "taper": taper(i),
    });
    if let Some(g) = group_of(key) {
        v["group"] = json!(g);
    }
    if let Some(c) = choices.filter(|c| !c.is_empty()) {
        let lo = i.min.round() as i64;
        v["choices"] = json!(c
            .into_iter()
            .enumerate()
            .map(|(k, name)| json!({"value": lo + k as i64, "name": name}))
            .collect::<Vec<_>>());
    }
    v
}

fn latency_at_48k(spec: EffectSpec) -> usize {
    let slot = EffectSlot {
        id: cantus_core::EffectId(1),
        bypass: false,
        spec,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
    };
    cantus_engine::effects::EffectUnit::new(&slot, 48_000.0).latency()
}

fn sets(values: &[(&str, f32)]) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
    )
}

/// The full entry of an effect kind.
pub fn effect_entry(kind: EffectKind) -> Value {
    let (id, name) = effect_names(kind);
    let params: Vec<Value> = kind
        .params()
        .iter()
        .map(|i| row(i.key, i, effect_choice_words(kind, i.key, Locale::En)))
        .collect();
    let presets: Vec<Value> = crate::views::fx::factory_presets(kind)
        .iter()
        .map(|(label, values)| {
            // fxa:fractus — a Fractus preset paints its lanes too: its values, and its lanes
            // as `set_effect`'s `lanes` takes them.
            if kind == EffectKind::Glitch {
                let own: Vec<(&str, f32)> = values
                    .iter()
                    .filter(|(k, _)| kind.param(k).is_some())
                    .copied()
                    .collect();
                let map: std::collections::BTreeMap<String, f32> =
                    values.iter().map(|(k, v)| (k.to_string(), *v)).collect();
                let mut lanes = serde_json::Map::new();
                if let Some(p) = cantus_core::fx::fractus::pattern_of_flat(&map) {
                    for (l, blocks) in &p.lanes {
                        let name = cantus_core::fx::fractus::LANE_KEYS[usize::from(*l)];
                        let list: Vec<Value> = blocks
                            .iter()
                            .map(|b| {
                                json!({"at": b.at, "len": b.len, "amount": b.amount,
                                    "chance": b.prob})
                            })
                            .collect();
                        lanes.insert(name.to_string(), Value::Array(list));
                    }
                }
                return json!({"name": en(label), "sets": sets(&own), "lanes": lanes});
            }
            if kind != EffectKind::Board {
                return json!({"name": en(label), "sets": sets(values)});
            }
            // fx3:tabula — a board preset is a whole board: its own values, and its pedals
            // in order (loaded whole: what `sets` cannot say).
            let own: Vec<(&str, f32)> = values
                .iter()
                .filter(|(k, _)| kind.param(k).is_some())
                .copied()
                .collect();
            let map: std::collections::BTreeMap<String, f32> =
                values.iter().map(|(k, v)| (k.to_string(), *v)).collect();
            let pedals: Vec<Value> = cantus_core::fx::tabula_presets::from_flat(&map)
                .map(|s| {
                    s.pedals()
                        .iter()
                        .map(|p| {
                            json!({"id": p.id.0, "unit": effect_names(p.spec.kind).0,
                                "lane": format!("{:?}", p.path).to_lowercase()})
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({"name": en(label), "sets": sets(&own), "pedals": pedals})
        })
        .collect();
    let user: Vec<String> = crate::views::fx::user_presets(kind)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let starting = match start_of(kind) {
        Start::Preset(p) => json!({"preset": en(p.label_key), "sets": sets(p.params)}),
        // fx3:contract — a whole spec (Tabula's rig): its name.
        Start::Spec(s) => json!({"preset": en(s.label_key)}),
        Start::Neutral(n) => json!({"neutral": match n {
            Neutral::Flat => "flat by design: the curve or image is yours to make",
            Neutral::Sounds => "its defaults already do the job audibly",
            Neutral::MustNotSurprise => "a safety at the end of a chain: changes nothing until asked",
            Neutral::Measures => "it only measures",
            Neutral::Hosted => "a hosted plug-in's own initial state",
        }}),
    };
    json!({
        "id": id, "name": name, "type": "effect", "what": effect_use(kind),
        "sidechain": kind.takes_sidechain(), "hears_notes": kind.takes_notes(),
        "latency_samples_at_48k": latency_at_48k(cantus_core::starting::starting_spec(kind)),
        "starting_preset": starting,
        "params": params, "presets": presets, "user_presets": user,
        "limits": {"per_strip": cantus_core::model::MAX_INSERTS},
    })
}

fn firmus_rows() -> Vec<Value> {
    FirmusParam::ALL
        .iter()
        .map(|p| {
            let i = p.info();
            let info = EffectParamInfo {
                key: "",
                label_key: i.label_key,
                min: i.min,
                max: i.max,
                default: i.default,
                unit: i.unit,
                logarithmic: i.logarithmic,
                stepped: i.stepped,
            };
            let key = cantus_ai::tools_project::json::firmus_key(*p);
            let waves = matches!(p, FirmusParam::Osc1Wave | FirmusParam::Osc2Wave).then(|| {
                crate::inst_choices::firmus_waves()
                    .iter()
                    .map(|w| w.show_in(Locale::En))
                    .collect()
            });
            row(&key, &info, waves)
        })
        .collect()
}

fn pulsus_rows() -> Vec<Value> {
    let mut out = Vec::new();
    for f in KitField::ALL {
        let (label, min, max, default) = f.range();
        out.push(json!({"key": format!("kit.{}", cantus_ai::tools_project::json::snake(&format!("{f:?}"))),
            "name": en(label), "min": min, "max": max, "default": default, "taper": "stepped"}));
    }
    for voice in PadVoice::ALL {
        for f in PadField::for_voice(voice) {
            let i = f.info(voice);
            let field = cantus_ai::tools_project::json::snake(&format!("{f:?}"));
            out.push(json!({
                "key": format!("pad<note>.{field}"), "voice": en(voice.label_key()), "name": en(i.label_key),
                "unit": i.unit, "min": i.min, "max": i.max, "default": i.default,
                "taper": if i.stepped { "stepped" } else if i.logarithmic { "log" } else { "linear" },
            }));
        }
    }
    out
}

/// The full entry of an instrument, by its id (`unda`, `chorda`, `firmus`, `pulsus`…).
pub fn instrument_entry(id: &str) -> Option<Value> {
    let (_, name, choice, what) = INSTRUMENTS.iter().find(|(i, ..)| *i == id)?;
    use cantus_ai::tools_project::args::InstrumentChoice as C;
    let (params, presets): (Vec<Value>, Vec<Value>) = match choice {
        C::Native(kind) => (
            kind.params()
                .iter()
                .map(|i| {
                    let words = crate::inst_choices::words(*kind, i.key)
                        .map(|w| w.iter().map(|x| x.show_in(Locale::En)).collect());
                    let mut r = row(i.key, i, words);
                    r["automatable"] = json!(kind.automatable(i.key));
                    r
                })
                .collect(),
            crate::views::inst::factory_presets(*kind)
                .iter()
                .map(|p| json!({"name": en(p.label_key), "tags": p.tags, "sets": sets(p.values),
                    "text": Value::Object(p.text.iter().map(|(k, v)| (k.to_string(), json!(v))).collect())}))
                .chain(crate::views::inst::user_presets(*kind).into_iter().map(|(n, _)| json!({"name": n, "user": true})))
                .collect(),
        ),
        C::Firmus => (firmus_rows(), Vec::new()),
        C::Pulsus => (
            pulsus_rows(),
            vec![json!({"name": "Default kit", "what": "seven pads on the General MIDI drum map"}),
                 json!({"name": "Studio kit", "what": "the full Pulsus II kit"})],
        ),
    };
    let mut v = json!({
        "id": id, "name": name, "type": "instrument", "what": what,
        "params": params, "presets": presets,
    });
    if let C::Native(kind) = choice {
        v["playable"] = match range_words(*kind) {
            Some(r) => json!(r),
            None => json!("the whole MIDI range"),
        };
        if *kind == InstrumentKind::Sampler {
            v["note"] = json!("silent until an instrument (SFZ, Decent Sampler, SF2) is loaded into it by the user or from a file in the project's folder");
        }
        if *kind == InstrumentKind::Dancer {
            crate::dance_live::describe_for_agents(&mut v); // r35:dancer
        }
        let _ = native_id(*kind);
    }
    if id == "pulsus" {
        v["note"] = json!("pads are addressed by their note: pad36 is the kick on the General MIDI map; read get_track for a kit's pads and names");
    }
    Some(v)
}

fn range_words(kind: InstrumentKind) -> Option<String> {
    let spec = cantus_core::InstrumentSpec::Native(cantus_core::NativeSpec::new(kind));
    cantus_ai::tools_project::music::playable(&spec).map(|(_, s)| s)
}

/// Every unit, short: what list_units answers and `cantus://units` holds.
pub fn list() -> Value {
    let mut out: Vec<Value> = INSTRUMENTS
        .iter()
        .map(|(id, name, _, what)| json!({"id": id, "name": name, "type": "instrument", "what": what}))
        .collect();
    for kind in EffectKind::ALL
        .into_iter()
        .filter(|k| *k != EffectKind::External && k.offered())
    // r37:fixes
    {
        let (id, name) = effect_names(kind);
        out.push(json!({"id": id, "name": name, "type": "effect", "what": effect_use(kind),
            "params": kind.params().len(), "presets": crate::views::fx::factory_presets(kind).len()}));
    }
    json!({"units": out, "limits": cantus_ai::tools_project::json::limits_json()})
}

/// One unit by id, or `None`.
pub fn entry(id: &str) -> Option<Value> {
    instrument_entry(id).or_else(|| {
        cantus_ai::tools_project::args::any_effect_kind(id) // r37:fixes — reverb_legacy too
            .filter(|k| *k != EffectKind::External)
            .map(effect_entry)
    })
}
