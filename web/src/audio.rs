//! A tiny synthesiser for the hero rack: kick, clap, hat and bass, made of Web Audio nodes.
//! Nothing is downloaded and nothing leaves the page.

use wasm_bindgen::JsValue;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioNode, BiquadFilterNode,
    BiquadFilterType, GainNode, OscillatorNode, OscillatorType,
};

type R<T> = Result<T, JsValue>;

pub struct Synth {
    ctx: AudioContext,
    noise: AudioBuffer,
    out: AudioNode,
}

impl Synth {
    /// Must run inside a user gesture (a click), or the browser keeps the context suspended.
    pub fn new() -> R<Self> {
        let ctx = AudioContext::new()?;
        let _ = ctx.resume()?;
        let rate = ctx.sample_rate();
        let len = rate as u32;
        let noise = ctx.create_buffer(1, len, rate)?;
        // A fixed-seed xorshift: the same hats and claps every visit.
        let mut s: u32 = 0x9e37_79b9;
        let mut data: Vec<f32> = (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();
        noise.copy_to_channel(&mut data, 0)?;

        let master = ctx.create_gain()?;
        master.gain().set_value(0.8);
        let limiter = ctx.create_dynamics_compressor()?;
        limiter.threshold().set_value(-8.0);
        limiter.ratio().set_value(12.0);
        master.connect_with_audio_node(&limiter)?;
        limiter.connect_with_audio_node(&ctx.destination())?;
        Ok(Self { ctx, noise, out: master.into() })
    }

    pub fn now(&self) -> f64 {
        self.ctx.current_time()
    }

    fn gain(&self, t: f64, peak: f32, attack: f64, decay: f64) -> R<GainNode> {
        let g = self.ctx.create_gain()?;
        let p = g.gain();
        p.set_value_at_time(0.0001, t)?;
        p.linear_ramp_to_value_at_time(peak, t + attack)?;
        p.exponential_ramp_to_value_at_time(0.0001, t + attack + decay)?;
        g.connect_with_audio_node(&self.out)?;
        Ok(g)
    }

    fn osc(&self, kind: OscillatorType, hz: f32) -> R<OscillatorNode> {
        let o = self.ctx.create_oscillator()?;
        o.set_type(kind);
        o.frequency().set_value(hz);
        Ok(o)
    }

    fn noise_src(&self, t: f64) -> R<AudioBufferSourceNode> {
        let n = self.ctx.create_buffer_source()?;
        n.set_buffer(Some(&self.noise));
        n.set_loop(true);
        n.start_with_when(t)?;
        Ok(n)
    }

    fn filter(&self, kind: BiquadFilterType, hz: f32, q: f32) -> R<BiquadFilterNode> {
        let f = self.ctx.create_biquad_filter()?;
        f.set_type(kind);
        f.frequency().set_value(hz);
        f.q().set_value(q);
        Ok(f)
    }

    pub fn kick(&self, t: f64) -> R<()> {
        let o = self.osc(OscillatorType::Sine, 150.0)?;
        o.frequency().set_value_at_time(150.0, t)?;
        o.frequency().exponential_ramp_to_value_at_time(44.0, t + 0.12)?;
        let g = self.gain(t, 1.0, 0.002, 0.34)?;
        o.connect_with_audio_node(&g)?;
        o.start_with_when(t)?;
        o.stop_with_when(t + 0.4)
    }

    pub fn clap(&self, t: f64) -> R<()> {
        let n = self.noise_src(t)?;
        let f = self.filter(BiquadFilterType::Bandpass, 1500.0, 1.1)?;
        let g = self.ctx.create_gain()?;
        let p = g.gain();
        p.set_value_at_time(0.0001, t)?;
        for (i, off) in [0.0, 0.011, 0.023].iter().enumerate() {
            let peak = 0.55 - i as f32 * 0.08;
            p.linear_ramp_to_value_at_time(peak, t + off + 0.001)?;
            p.exponential_ramp_to_value_at_time(0.05, t + off + 0.009)?;
        }
        p.linear_ramp_to_value_at_time(0.5, t + 0.026)?;
        p.exponential_ramp_to_value_at_time(0.0001, t + 0.2)?;
        n.connect_with_audio_node(&f)?;
        f.connect_with_audio_node(&g)?;
        g.connect_with_audio_node(&self.out)?;
        stop(&n, t + 0.25)
    }

    pub fn hat(&self, t: f64, open: bool) -> R<()> {
        let n = self.noise_src(t)?;
        let f = self.filter(BiquadFilterType::Highpass, 7200.0, 0.7)?;
        let decay = if open { 0.18 } else { 0.045 };
        let g = self.gain(t, 0.28, 0.001, decay)?;
        n.connect_with_audio_node(&f)?;
        f.connect_with_audio_node(&g)?;
        stop(&n, t + decay + 0.05)
    }

    pub fn bass(&self, t: f64, hz: f32, len: f64) -> R<()> {
        let o = self.osc(OscillatorType::Sawtooth, hz)?;
        let f = self.filter(BiquadFilterType::Lowpass, 900.0, 4.0)?;
        f.frequency().set_value_at_time(1400.0, t)?;
        f.frequency().exponential_ramp_to_value_at_time(180.0, t + len)?;
        let g = self.gain(t, 0.42, 0.006, len)?;
        o.connect_with_audio_node(&f)?;
        f.connect_with_audio_node(&g)?;
        o.start_with_when(t)?;
        o.stop_with_when(t + len + 0.05)
    }
}

impl Drop for Synth {
    fn drop(&mut self) {
        let _ = self.ctx.close();
    }
}

/// `stop` lives on the scheduled-source base class; the node's own copy is deprecated.
fn stop(n: &AudioBufferSourceNode, t: f64) -> R<()> {
    let base: &web_sys::AudioScheduledSourceNode = n.as_ref();
    base.stop_with_when(t)
}
