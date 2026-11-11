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
    // The kit's own gain (ADR 0100 §8): exactly unity at 0 dB.
    drums.set_gain(if kit.kit.gain_db == 0.0 {
        1.0
    } else {
        db_to_linear(kit.kit.gain_db)
    });
    drums.set_pad_count(kit.pads.len());
    let mut layer_at = kit.pads.len();
    for (i, pad) in kit.pads.iter().take(MAX_PADS).enumerate() {
        // Each pad on its output (Pulsus III, ADR 0100): bus 0 the kit's own strip, bus
        // `k` the `k`-th strip its pads name — the snapshot's taps carry them.
        drums.set_pad(i, pad.pitch, &settings(pad, kit.bus_of(i)));
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

/// A track slot's drum machine (ADR 0107): none until a kit is assigned to its track.
///
/// A Pulsus holds ≈ 120 KB (its voice pool, its pad and layer tables); every one of the
/// 256 track slots held one whether its track was a drum track or not (31 MB on an empty
/// song). Now a kit is built off the audio thread (`crate::parts::kit`) and sent in
/// (`EngineMessage::AddPart`) before the snapshot that needs it, as the native instruments
/// are; the processor keeps the kits it was given and moves them between slots as tracks
/// start and stop hosting Pulsus — never allocating, never freeing. An empty slot plays
/// nothing and hears nothing, which is what a slot whose kit has not arrived must do.
#[derive(Clone, Debug, Default)]
pub struct SlotKit(Option<Box<Pulsus>>);

impl SlotKit {
    /// No kit.
    pub const fn empty() -> Self {
        SlotKit(None)
    }

    /// A kit, as `build` made it.
    pub fn with(kit: Box<Pulsus>) -> Self {
        SlotKit(Some(kit))
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// The kit, to be configured or played.
    #[inline]
    pub fn get(&mut self) -> Option<&mut Pulsus> {
        self.0.as_deref_mut()
    }

    pub fn take(&mut self) -> Option<Box<Pulsus>> {
        self.0.take()
    }

    pub fn put(&mut self, kit: Box<Pulsus>) -> Option<Box<Pulsus>> {
        self.0.replace(kit)
    }

    #[inline]
    pub fn note_on(&mut self, pitch: u8, velocity: u8, tag: u64) {
        if let Some(k) = self.get() {
            k.note_on(pitch, velocity, tag);
        }
    }

    #[inline]
    pub fn trigger(&mut self, pitch: u8, velocity: u8, hit: cantus_dsp::pulsus::Hit) {
        if let Some(k) = self.get() {
            k.trigger(pitch, velocity, hit);
        }
    }

    #[inline]
    pub fn note_off(&mut self, pitch: u8, tag: u64) {
        if let Some(k) = self.get() {
            k.note_off(pitch, tag);
        }
    }

    #[inline]
    pub fn controller(&mut self, cc: u8, value: f32) {
        if let Some(k) = self.get() {
            k.controller(cc, value);
        }
    }

    #[inline]
    pub fn all_notes_off(&mut self) {
        if let Some(k) = self.get() {
            k.all_notes_off();
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        if let Some(k) = self.get() {
            k.reset();
        }
    }

    #[inline]
    pub fn active_voices(&self) -> usize {
        self.0.as_ref().map_or(0, |k| k.active_voices())
    }

    /// Render, adding (`Pulsus::process_add`); nothing without a kit.
    #[inline]
    pub fn process_add(&mut self, left: &mut [f32], right: &mut [f32]) -> bool {
        self.get().is_some_and(|k| k.process_add(left, right))
    }

    /// Render bus by bus (`Pulsus::process_buses`); nothing without a kit.
    #[inline]
    pub fn process_buses(&mut self, n: usize, out: &mut cantus_dsp::pulsus::BusOut<'_>) -> bool {
        self.get().is_some_and(|k| k.process_buses(n, out))
    }
}
