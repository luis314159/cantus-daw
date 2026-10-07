//! One sounding drum: the state every model needs, how a hit starts it, and how it
//! renders a sample.
//!
//! Model 1 kits and the three Pulsus I shapes render through **the same arithmetic, in the
//! same order,** as Pulsus I did (`render_kick`, `render_snare`, `render_hat`,
//! `render_sample` with `legacy`), which is what the golden tests pin. Everything Pulsus II
//! added runs only when it is switched away from its neutral value.

use super::modal::Bank;
use super::shape::{decay_coef, semitones, Band, Noise, Oversampler};
use super::{DrumVoice, SampleRef};
use crate::filter::{FilterMode, Svf};
use crate::math;
use crate::osc::{PolyBlepOsc, Wave};

/// Below this an envelope is inaudible and the voice is freed. −80 dB.
pub(crate) const ENV_FLOOR: f32 = 1.0e-4;

#[derive(Clone, Debug)]
pub(crate) struct Voice {
    pub(crate) active: bool,
    pub(crate) kind: DrumVoice,
    pub(crate) model: u8,
    /// Rendered exactly as Pulsus I did: a model 1 kit.
    pub(crate) legacy: bool,
    pub(crate) pad_id: u16,
    pub(crate) note_id: u64,
    pub(crate) bus: u8,
    pub(crate) choke: u8,
    /// Follows the hi-hat pedal.
    pub(crate) pedal: bool,
    /// The pad's own hit (not a layer riding along): what polyphony counts.
    pub(crate) primary: bool,
    /// Monotonic start counter, for stealing and polyphony.
    pub(crate) age: u64,
    pub(crate) gain_l: f32,
    pub(crate) gain_r: f32,
    /// Main amplitude envelope and its per-sample coefficient, and the value the cores
    /// read this sample (the amp itself unless the decay has a curve).
    pub(crate) amp: f32,
    pub(crate) amp_coef: f32,
    pub(crate) env: f32,
    /// Second and third envelopes: the kick's click, the snare's noise, the cymbal's
    /// high band, the clap's current hand.
    pub(crate) aux: f32,
    pub(crate) aux_coef: f32,
    pub(crate) aux2: f32,
    pub(crate) aux2_coef: f32,
    /// Pitch envelope, 1 → 0: the kick's fall, the snare's slight drop at the hit.
    pub(crate) pitch_env: f32,
    pub(crate) pitch_coef: f32,
    /// The beater's knock on a Pulsus kick, gone in ~8 ms.
    pub(crate) knock: f32,
    pub(crate) knock_coef: f32,
    pub(crate) phase_a: f32,
    pub(crate) inc_a: f32,
    pub(crate) phase_b: f32,
    pub(crate) inc_b: f32,
    pub(crate) pitch_span: f32,
    pub(crate) base_hz: f32,
    /// The in-voice saturation of a Pulsus I kick (model 1 kits only).
    pub(crate) drive: f32,
    pub(crate) mix: f32,
    pub(crate) hats: [PolyBlepOsc; 6],
    pub(crate) band: Svf,
    pub(crate) high: Svf,
    pub(crate) noise: Noise,
    pub(crate) choking: bool,
    pub(crate) choke_coef: f32,
    /// This voice's own pad's choke fade, captured at the hit.
    pub(crate) own_choke_coef: f32,
    /// Voices whose sound is not one amp envelope (modal, clap, cymbals) fade here.
    pub(crate) uses_fade: bool,
    pub(crate) fade: f32,
    pub(crate) sample: Option<SampleRef>,
    pub(crate) pos: f64,
    pub(crate) inc: f64,
    pub(crate) hermite: bool,
    pub(crate) reverse: bool,
    // ---- the per-pad section --------------------------------------------------------
    /// Samples since the hit.
    pub(crate) t: u32,
    /// Samples the amp envelope stays at 1 (attack + hold).
    pub(crate) freeze: u32,
    pub(crate) attack_len: u32,
    pub(crate) curve: f32,
    pub(crate) lin: f32,
    pub(crate) lin_step: f32,
    pub(crate) amp2: f32,
    pub(crate) amp2_coef: f32,
    /// Generic pitch envelope (voices without a sweep of their own): value and depth.
    pub(crate) penv: f32,
    pub(crate) penv_coef: f32,
    pub(crate) penv_st: f32,
    pub(crate) filter: u8,
    pub(crate) fl: Band,
    pub(crate) fr: Band,
    pub(crate) cutoff: f32,
    pub(crate) filter_q: f32,
    pub(crate) filter_env: f32,
    pub(crate) gdrive: f32,
    pub(crate) gmode: u8,
    pub(crate) os_l: Oversampler,
    pub(crate) os_r: Oversampler,
    pub(crate) trans: bool,
    pub(crate) trans_a: f32,
    pub(crate) trans_s: f32,
    /// Anything in the section is on: the post chain runs.
    pub(crate) post: bool,
    // ---- model state ----------------------------------------------------------------
    pub(crate) lp: Band,
    pub(crate) b2: Band,
    pub(crate) b3: Band,
    pub(crate) bank: Bank,
    pub(crate) sigh: f32,
    pub(crate) wires: f32,
    pub(crate) follow: f32,
    pub(crate) follow_coef: f32,
    pub(crate) delay: u32,
    pub(crate) bursts_left: u8,
    pub(crate) next_burst: u32,
    pub(crate) spacing: u32,
    pub(crate) swell_len: u32,
    pub(crate) glide: f32,
    pub(crate) retune_every: u32,
    /// Metal: the decays and the band centre at closed and open, for a moving pedal.
    pub(crate) closed_decay: f32,
    pub(crate) open_decay: f32,
    pub(crate) tone_hz: f32,
    pub(crate) openness: f32,
    pub(crate) sample_rate: f32,
}

impl Voice {
    pub(crate) fn new(sample_rate: f32) -> Self {
        let mut band = Svf::new(sample_rate);
        band.set_mode(FilterMode::Band);
        let mut high = Svf::new(sample_rate);
        high.set_mode(FilterMode::High);
        let mut hats = [
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
            PolyBlepOsc::new(sample_rate),
        ];
        for h in &mut hats {
            h.set_wave(Wave::Square);
        }
        Voice {
            active: false,
            kind: DrumVoice::Kick,
            model: 0,
            legacy: true,
            pad_id: 0,
            note_id: 0,
            bus: 0,
            choke: 0,
            pedal: false,
            primary: true,
            age: 0,
            gain_l: 0.0,
            gain_r: 0.0,
            amp: 0.0,
            amp_coef: 0.0,
            env: 0.0,
            aux: 0.0,
            aux_coef: 0.0,
            aux2: 0.0,
            aux2_coef: 0.0,
            pitch_env: 0.0,
            pitch_coef: 0.0,
            knock: 0.0,
            knock_coef: 0.0,
            phase_a: 0.0,
            inc_a: 0.0,
            phase_b: 0.0,
            inc_b: 0.0,
            pitch_span: 0.0,
            base_hz: 0.0,
            drive: 0.0,
            mix: 0.5,
            hats,
            band,
            high,
            noise: Noise(1),
            choking: false,
            choke_coef: 0.0,
            own_choke_coef: 0.0,
            uses_fade: false,
            fade: 1.0,
            sample: None,
            pos: 0.0,
            inc: 0.0,
            hermite: false,
            reverse: false,
            t: 0,
            freeze: 0,
            attack_len: 0,
            curve: 0.0,
            lin: 0.0,
            lin_step: 0.0,
            amp2: 0.0,
            amp2_coef: 0.0,
            penv: 0.0,
            penv_coef: 0.0,
            penv_st: 0.0,
            filter: 0,
            fl: Band::default(),
            fr: Band::default(),
            cutoff: 1_000.0,
            filter_q: 0.7,
            filter_env: 0.0,
            gdrive: 0.0,
            gmode: 0,
            os_l: Oversampler::default(),
            os_r: Oversampler::default(),
            trans: false,
            trans_a: 1.0,
            trans_s: 1.0,
            post: false,
            lp: Band::default(),
            b2: Band::default(),
            b3: Band::default(),
            bank: Bank::new(sample_rate),
            sigh: 0.0,
            wires: 0.0,
            follow: 0.0,
            follow_coef: 0.0,
            delay: 0,
            bursts_left: 0,
            next_burst: 0,
            spacing: 0,
            swell_len: 0,
            glide: 0.0,
            retune_every: 16,
            closed_decay: 0.1,
            open_decay: 1.0,
            tone_hz: 9_500.0,
            openness: 0.0,
            sample_rate,
        }
    }

    pub(crate) fn silence(&mut self) {
        self.active = false;
        self.amp = 0.0;
        self.aux = 0.0;
        self.choking = false;
        self.band.reset();
        self.high.reset();
        // Let go of the buffer, so a replaced sample is freed by whoever holds the last
        // reference — which is never this thread (see `SampleRef`).
        self.sample = None;
        self.bank.clear();
    }

    /// Start fading this voice out over its own pad's choke time.
    pub(crate) fn choke_now(&mut self) {
        self.choking = true;
        self.choke_coef = self.own_choke_coef;
    }

    /// The level the voice is freed on: its master envelope, and its fade.
    #[inline]
    fn level(&self) -> f32 {
        if self.uses_fade {
            self.amp * self.fade
        } else {
            self.env_value()
        }
    }

    /// The amplitude envelope with its curve: exactly `amp` when the curve is 0.
    #[inline]
    pub(crate) fn env_value(&self) -> f32 {
        if self.curve == 0.0 {
            self.amp
        } else if self.curve > 0.0 {
            (1.0 - self.curve) * self.amp + self.curve * self.lin.max(0.0)
        } else {
            let c = -self.curve;
            (1.0 - c) * self.amp + c * self.amp2
        }
    }

    /// Move the envelopes on by one sample. Returns `false` when the voice fell silent
    /// and was freed. For a model 1 voice this is Pulsus I's arithmetic exactly.
    #[inline]
    pub(crate) fn advance(&mut self) -> bool {
        if self.freeze > 0 {
            self.freeze -= 1;
        } else {
            self.amp *= self.amp_coef;
            if self.curve != 0.0 {
                self.lin -= self.lin_step;
                self.amp2 *= self.amp2_coef;
            }
        }
        if self.choking {
            if self.uses_fade {
                self.fade *= self.choke_coef;
            } else {
                self.amp *= self.choke_coef;
                self.aux *= self.choke_coef;
                self.amp2 *= self.choke_coef;
                self.lin *= self.choke_coef;
            }
        }
        self.t = self.t.saturating_add(1);
        if self.level() < ENV_FLOOR {
            self.silence();
            return false;
        }
        true
    }

    /// A moving hi-hat pedal reaches a ringing metal voice: its decay slides between
    /// closed and open, and its band a little lower as it opens.
    pub(crate) fn set_openness(&mut self, openness: f32) {
        let o = openness.clamp(0.0, 1.0);
        self.openness = o;
        let d = decay_at(self.closed_decay, self.open_decay, o);
        self.amp_coef = decay_coef(d, self.sample_rate);
        if self.model == 0 {
            self.band
                .set(open_tone(self.tone_hz, o).clamp(200.0, 18_000.0), 0.5);
        }
    }
}

/// The decay at an openness: logarithmic between closed and open.
#[inline]
pub(crate) fn decay_at(closed: f32, open: f32, openness: f32) -> f32 {
    if openness <= 0.0 {
        return closed;
    }
    closed * math::powf(open.max(0.001) / closed.max(0.001), openness)
}

/// A hat's band centre drops up to three semitones as it opens.
#[inline]
pub(crate) fn open_tone(tone: f32, openness: f32) -> f32 {
    if openness <= 0.0 {
        return tone;
    }
    semitones(tone, -3.0 * openness)
}
