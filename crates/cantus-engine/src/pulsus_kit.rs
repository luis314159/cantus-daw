//! Pulsus's side of the engine: a kit from the project model to the DSP's fixed tables.
//!
//! Owned by Pulsus (round 17) so the processor only calls [`configure`] and never needs
//! to know what a pad holds. Runs on the audio thread, once per snapshot, allocation-free:
//! every table it writes is preallocated by `cantus_dsp::Pulsus::new`.

use cantus_core::{db_to_linear, DrumPad, PadVoice, PulsusParams, SampleData};
use cantus_dsp::pulsus::{LayerSettings, MAX_LAYERS};
use cantus_dsp::{DrumVoice, PadSettings, Pulsus, SampleRef, MAX_PADS};
use std::sync::Arc;

/// One pad as the DSP sees it.
pub fn settings(pad: &DrumPad, bus: u8) -> PadSettings {
    PadSettings {
        voice: match pad.voice {
            PadVoice::Kick => DrumVoice::Kick,
            PadVoice::Snare => DrumVoice::Snare,
            PadVoice::HiHat => DrumVoice::HiHat,
            PadVoice::Sample => DrumVoice::Sample,
            PadVoice::Clap => DrumVoice::Clap,
            PadVoice::Tom => DrumVoice::Tom,
        },
        tune: pad.tune,
        decay: pad.decay,
        pitch_drop: pad.pitch_drop,
        pitch_decay: pad.pitch_decay,
        click: pad.click,
        drive: pad.drive,
        noise_decay: pad.noise_decay,
        tone: pad.tone,
        mix: pad.mix,
        hp_cutoff: pad.hp_cutoff,
        gain: db_to_linear(pad.level_db),
        pan: pad.pan,
        // A muted pad is already gone from the snapshot's events (`Snapshot::compile`).
        choke: pad.choke,
        id: pad.id.0,
        model: pad.model_index() as u8,
        bus,
        attack: pad.attack_ms / 1000.0,
        hold: pad.hold_ms / 1000.0,
        curve: pad.curve,
        pitch_env: pad.pitch_env,
        pitch_env_time: pad.pitch_env_ms / 1000.0,
        filter: pad.filter,
        cutoff: pad.cutoff,
        resonance: pad.resonance,
        filter_env: pad.filter_env,
        trans_attack: pad.trans_attack,
        trans_sustain: pad.trans_sustain,
        drive_mode: pad.drive_mode,
        phase: pad.phase,
        vel_level: pad.vel_level,
        vel_tone: pad.vel_tone,
        vel_pitch: pad.vel_pitch,
        vel_decay: pad.vel_decay,
        choke_fade: pad.choke_fade_ms / 1000.0,
        choke_on_off: pad.choke_on_off,
        polyphony: pad.polyphony,
        round_robin: pad.round_robin.index() as u8,
        pedal: pad.pedal,
        openness: pad.openness,
        open_decay: pad.open_decay,
        sigh: pad.sigh,
        wires: pad.wires,
        material: pad.material,
        modes: pad.modes,
        damping: pad.damping,
        position: pad.position,
        hardness: pad.hardness,
        glide: pad.glide,
        shell: pad.shell,
        band_mix: pad.band_mix,
        swell: pad.swell_ms / 1000.0,
        bursts: pad.bursts,
        spacing: pad.spacing_ms / 1000.0,
        q: pad.q,
        layer_count: pad.layers.len().min(MAX_LAYERS) as u8,
    }
}

fn sample_ref(d: &Arc<SampleData>) -> SampleRef {
    SampleRef {
        frames: d.frames.clone(),
        rate: d.rate as f32,
        channels: d.channels as usize,
    }
}

/// Hand a kit to the DSP. `samples` is what `SampleCache::attach` put in the snapshot:
/// one per pad, then every pad's layers in pad order.
pub fn configure(drums: &mut Pulsus, kit: &PulsusParams, samples: &[Option<Arc<SampleData>>]) {
    drums.set_kit(
        kit.version,
        kit.kit.seed,
        kit.kit.hat_cc,
        kit.kit.hat_invert,
    );
    drums.set_pad_count(kit.pads.len());
    let mut layer_at = kit.pads.len();
    for (i, pad) in kit.pads.iter().take(MAX_PADS).enumerate() {
        // Per-pad outputs are not routed yet: every pad plays into the kit's own strip.
        drums.set_pad(i, pad.pitch, &settings(pad, 0));
        // An `Arc` clone per pad and nothing more.
        let sample = samples.get(i).and_then(|s| s.as_ref()).map(sample_ref);
        drums.set_sample(i, sample);
        for (k, layer) in pad.layers.iter().take(MAX_LAYERS).enumerate() {
            let audio = samples
                .get(layer_at)
                .and_then(|s| s.as_ref())
                .map(sample_ref);
            layer_at += 1;
            let settings = LayerSettings {
                vel_lo: layer.vel_lo,
                vel_hi: layer.vel_hi,
                xfade: layer.xfade,
                tune: layer.tune,
                gain: db_to_linear(layer.level_db),
                pan: layer.pan,
                start: layer.start_ms / 1000.0,
                reverse: layer.reverse,
            };
            drums.set_layer(i, k, &settings, audio);
        }
    }
}
