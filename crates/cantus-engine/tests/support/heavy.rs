//! The bench's *heavy* settings (`cantus-bench`'s `kernels::heavy`, copied here until the
//! bench crate merges into the tests' reach), its test signal, and what the contract tests
//! draw at random: parameter values across each kind's whole range and block splits.
//!
//! Keep [`heavy`] in step with `crates/cantus-bench/src/kernels.rs`: these are the settings
//! the per-node segmentation and at-rest suspension of ADR 0031 are proven at.

use cantus_core::{EffectKind, EffectParamInfo, EffectSpec, InstrumentKind, NativeSpec};

pub const SAMPLE_RATE: f32 = 48_000.0;

/// Heavy settings per kind: its most expensive modes (oversampling, linear phase, true
/// peak, many bands), as a mastering engineer would push them. A copy of the bench's.
pub fn heavy(kind: EffectKind) -> EffectSpec {
    let mut s = EffectSpec::new(kind);
    let set = |s: &mut EffectSpec, kv: &[(&str, f32)]| {
        for (k, v) in kv {
            s.set(k, *v);
        }
    };
    match kind {
        EffectKind::Eq => {
            // Eight bands in use, two of them dynamic, in linear phase at its finest.
            set(&mut s, &[("mode", 2.0), ("lp_res", 3.0)]);
            for b in 0..8 {
                let f = 60.0 * cantus_dsp::math::powi(2f32, b as i32);
                let gain = if b % 2 == 0 { 4.0 } else { -3.0 };
                for (field, v) in [("used", 1.0), ("freq", f), ("gain", gain), ("q", 1.2)] {
                    s.set(&cantus_core::eq_key(b, field), v);
                }
            }
            s.set(&cantus_core::eq_key(2, "dyn"), 1.0);
            s.set(&cantus_core::eq_key(5, "dyn"), 1.0);
        }
        EffectKind::Compressor => set(&mut s, &[("os", 2.0), ("thr", -30.0), ("ratio", 8.0)]),
        EffectKind::Limiter => set(&mut s, &[("tp", 1.0), ("look", 5.0), ("dither", 3.0)]),
        EffectKind::Space => set(&mut s, &[("decay", 8.0), ("predelay", 120.0)]),
        EffectKind::Multiband => {
            // Four bands working, split in linear phase at the steepest slope.
            set(&mut s, &[("mode", 2.0), ("slope", 48.0)]);
            for (b, from) in [0.0f32, 200.0, 1_000.0, 5_000.0].into_iter().enumerate() {
                for (field, v) in [("used", 1.0), ("from", from.max(20.0)), ("thr", -40.0)] {
                    s.set(&format!("b{b}.{field}"), v);
                }
            }
        }
        EffectKind::DeEsser => set(&mut s, &[("mode", 1.0), ("proc", 2.0), ("thr", -50.0)]),
        EffectKind::Gate => set(&mut s, &[("style", 5.0)]),
        EffectKind::Saturator => set(&mut s, &[("os", 3.0)]),
        EffectKind::Stereo => {}
        EffectKind::Maximizer => set(&mut s, &[("bands", 3.0), ("tp", 1.0), ("look", 5.0)]),
        EffectKind::Delay => set(&mut s, &[("time_ms", 1500.0), ("feedback", 0.9)]),
        EffectKind::Reverb => set(&mut s, &[("size", 1.0)]),
        EffectKind::External => {}
        // Round 28: each builder sets its own heavy line.
        EffectKind::Chorus => set(
            &mut s,
            &[
                ("mode", 7.0),
                ("mix", 0.5),
                ("voices", 4.0),
                ("delay", 30.0),
                ("depth", 8.0),
                ("tone", 4_000.0),
            ],
        ),
        EffectKind::Flanger => set(
            &mut s,
            &[
                ("mode", 1.0),
                ("tz", 1.0),
                ("mix", 0.5),
                ("fb", 0.95),
                ("depth", 10.0),
                ("spread", 90.0),
            ],
        ),
        EffectKind::Phaser => set(
            &mut s,
            &[
                ("stages", 12.0),
                ("mix", 0.5),
                ("fb", 0.95),
                ("shape", 5.0),
                ("spread", 90.0),
                ("lo", 20.0),
                ("hi", 16_000.0),
            ],
        ),
        EffectKind::Tremolo => set(
            &mut s,
            &[
                ("mode", 2.0),
                ("depth", 1.0),
                ("shape", 5.0),
                ("swing", 0.5),
                ("spread", 90.0),
            ],
        ),
        EffectKind::Vibrato => set(
            &mut s,
            &[
                ("depth", 100.0),
                ("rate", 0.2),
                ("rise", 500.0),
                ("spread", 90.0),
                ("mix", 0.5),
            ],
        ),
        EffectKind::RingMod => set(
            &mut s,
            &[
                ("wave", 2.0),
                ("lfo", 24.0),
                ("rate", 20.0),
                ("mix", 0.5),
                ("spread", 90.0),
                ("freq", 5_000.0),
            ],
        ),
        EffectKind::FreqShift => set(
            &mut s,
            &[
                ("shift", -2_000.0),
                ("side", 2.0),
                ("fb", 0.95),
                ("lfo", 1_000.0),
                ("rate", 10.0),
                ("mix", 0.5),
            ],
        ),
        EffectKind::VolumeShaper => set(
            &mut s,
            &[
                ("depth", 1.0),
                ("div", 14.0),
                ("smooth", 0.0),
                ("p2.on", 1.0),
                ("p2.x", 0.7),
                ("p2.s", 4.0),
                ("p2.t", 1.0),
                ("p3.on", 1.0),
                ("p3.x", 0.9),
                ("p3.s", 1.0),
            ],
        ),
        // Tonsor: 16x with the true-peak guard, driven hard into the analog shoulder.
        EffectKind::Clipper => set(
            &mut s,
            &[
                ("drive", 12.0),
                ("ceil", -1.0),
                ("knee", 3.0),
                ("curve", 2.0),
                ("os", 4.0),
                ("tp", 1.0),
                ("match", 1.0),
            ],
        ),
        // Impetus: both amounts at work on the highs, the shortest onset, a long tail.
        EffectKind::Transient => set(
            &mut s,
            &[
                ("attack", 1.0),
                ("sustain", -0.7),
                ("speed", 2.0),
                ("length", 900.0),
                ("band", 1.0),
                ("xover", 600.0),
            ],
        ),
        // Discrimen: the look-ahead, both parts compressed and tilted.
        EffectKind::TransientSplit => set(
            &mut s,
            &[
                ("mode", 1.0),
                ("sens", 0.8),
                ("speed", 3.0),
                ("t.gain", 6.0),
                ("t.comp", 0.8),
                ("t.tilt", 4.0),
                ("b.gain", -4.0),
                ("b.comp", 0.8),
                ("b.tilt", -4.0),
            ],
        ),
        // Speculum: a pass-through, whatever is shown (the largest FFT, both channels).
        EffectKind::Analyzer => set(
            &mut s,
            &[("view", 1.0), ("fft", 4.0), ("chan", 4.0), ("sync", 1.0)],
        ),
        // Tonus: the hard effect an octave up with the throat down, humanised, a created
        // vibrato — every grain path working at once.
        EffectKind::AutoTune => set(
            &mut s,
            &[
                ("speed", 0.0),
                ("trans", 12.0),
                ("throat", -6.0),
                ("vdepth", 40.0),
                ("human", 1.0),
            ],
        ),
        // r39:conv — Aula: the cathedral stretched half again (about 11 s, true stereo, four
        // levels of partitions), damped, cut, wide, half wet.
        EffectKind::Aula => set(
            &mut s,
            &[
                ("ir", 9.0),
                ("stretch", 1.5),
                ("damp", 0.4),
                ("low", 80.0),
                ("predelay", 30.0),
                ("width", 1.3),
                ("mix", 0.5),
            ],
        ),
        // fx:cribrum — both filters (a driven ladder at 24 dB/oct into three formants), 4×
        // oversampling, every source on every target.
        EffectKind::AutoFilter => set(
            &mut s,
            &[
                ("mix", 1.0),
                ("os", 2.0),
                ("a.model", 1.0),
                ("a.slope", 1.0),
                ("a.cut", 800.0),
                ("a.res", 0.7),
                ("a.drive", 12.0),
                ("b.on", 1.0),
                ("b.mode", 5.0),
                ("b.res", 0.5),
                ("spread", 0.5),
                ("lfo.shape", 6.0),
                ("lfo.stereo", 90.0),
                ("lfo.cut", 2.0),
                ("lfo.res", 0.2),
                ("lfo.vowel", 1.0),
                ("env.cut", 1.5),
                ("cur.cut", -1.0),
                ("cur.vowel", 1.0),
                ("cur.smooth", 10.0),
                ("out", -3.0),
            ],
        ),
        // fx:fragor — three pedals (the muff, the fuzz on a tired battery, the silicon
        // distortion), 8x, stereo, the gate on, half wet.
        EffectKind::Pedal => set(
            &mut s,
            &[
                ("s1", 5.0),
                ("s2", 4.0),
                ("s3", 3.0),
                ("mf.sustain", 0.8),
                ("ge.fuzz", 0.9),
                ("ge.bias", 0.5),
                ("si.dist", 0.8),
                ("gate", -70.0),
                ("os", 2.0),
                ("mix", 0.5),
                ("out", -6.0),
            ],
        ), // fx:fragor
        // fx:lapsus — the roll build (fifteen jumps a bar, each a join) under a trance gate,
        // mixed with the dry, a long crossfade and glide, and lookahead (long kernels).
        EffectKind::TimeShaper => set(
            &mut s,
            &[
                ("tslot", 16.0),
                ("vslot", 5.0),
                ("mix", 0.8),
                ("xfade", 6.0),
                ("smooth", 4.0),
                ("ahead", 5.0),
                ("out", 6.0),
            ],
        ),
        // fx:vetus — every module on: dithered mu-law bits, the hold behind both its filters,
        // a synced wow, dropouts and wear, every noise (following the music, so it rests),
        // the gramophone's band, Character half way.
        EffectKind::LoFi => set(
            &mut s,
            &[
                ("char", 0.5),
                ("bits.on", 1.0),
                ("bits.depth", 6.0),
                ("bits.dither", 1.0),
                ("bits.mode", 2.0),
                ("rate.on", 1.0),
                ("rate.hz", 9_000.0),
                ("rate.filter", 2.0),
                ("wob.on", 1.0),
                ("wow", 30.0),
                ("wow.sync", 1.0),
                ("flut", 8.0),
                ("age.on", 1.0),
                ("age.drop", 0.7),
                ("age.wear", 0.6),
                ("noise.on", 1.0),
                ("crackle", 0.8),
                ("hiss", 0.5),
                ("hum", 0.3),
                ("noise.follow", 1.0),
                ("tone.on", 1.0),
                ("tone.medium", 3.0),
                ("mix", 0.8),
                ("out", -3.0),
            ],
        ), // fx:vetus
        // fx:libra — every stage at work: DC removal, the sides swapped, a channel inverted,
        // both delayed, wider, balanced, gained.
        EffectKind::Utility => set(
            &mut s,
            &[
                ("dc", 1.0),
                ("chan", 1.0),
                ("pol.l", 1.0),
                ("delay.l", 12.5),
                ("delay.r", 40.0),
                ("width", 1.6),
                ("pan", -0.4),
                ("out", 6.0),
            ],
        ), // fx:libra
        // fx3:contract — wave 3's stubs: the placeholder's gain; each builder replaces its
        // own line with its unit's heaviest settings (tabula: a full board).
        // fx3:tabula — a full board: drive and a linear-phase EQ before a split (a chorus on
        // A; a phaser and an oversampled saturator on B, so the lanes' latencies differ),
        // then a linear-phase multiband and a room; the board's own levels moved too.
        EffectKind::Board => {
            use cantus_core::fx::tabula::{Board, Path, Pedal, PedalId};
            let pedal = |id: u16, kind: EffectKind, path: Path| {
                let mut p = Pedal::new(PedalId(id), heavy(kind));
                p.path = path;
                p
            };
            let mut room = Pedal::new(PedalId(9), EffectSpec::new(EffectKind::Space));
            room.path = Path::Post;
            room.spec.set("mix", 0.3);
            s.board = Some(Box::new(Board {
                pedals: vec![
                    pedal(0, EffectKind::Pedal, Path::Pre),
                    pedal(1, EffectKind::Eq, Path::Pre),
                    pedal(2, EffectKind::Chorus, Path::A),
                    pedal(3, EffectKind::Phaser, Path::B),
                    pedal(5, EffectKind::Saturator, Path::B),
                    pedal(7, EffectKind::Multiband, Path::Post),
                    room,
                ],
                macros: Vec::new(),
            }));
            set(
                &mut s,
                &[("in", -3.0), ("out", 2.0), ("mix", 0.85), ("ab", 0.4)],
            );
        }
        // fx3:saltus — the poly engine (its dearest) with both voices, spread, the dry
        // under them, formants moved and the throat shifted.
        EffectKind::PitchShift => set(
            &mut s,
            &[
                ("engine", 1.0),
                ("st1", 7.0),
                ("pan1", -0.5),
                ("st2", -12.0),
                ("ct2", 15.0),
                ("lvl2", 0.8),
                ("pan2", 0.6),
                ("dry", 0.5),
                ("formant", 0.0),
                ("throat", -3.0),
                ("out", 6.0),
            ],
        ), // fx3:saltus
        // fx3:rota — fast, driven, all wet, the drum ramping (a note's work).
        EffectKind::Rotary => set(
            &mut s,
            &[("mix", 1.0), ("speed", 2.0), ("drive", 0.6), ("out", 6.0)],
        ), // fx3:rota
        // fx3:arca — the dearest: the high gain at 8× into a cabinet.
        EffectKind::Amp => set(
            &mut s,
            &[
                ("model", 4.0),
                ("gain", 0.8),
                ("cab", 4.0),
                ("os", 2.0),
                ("out", 6.0),
            ],
        ), // fx3:arca
        // fxa:loquax — 64 bands of order 8, the synthesiser in seven copies and the
        // sidechain both carrying, formant and band shifted, spread, the consonants' help.
        EffectKind::Vocoder => set(
            &mut s,
            &[
                ("mix", 1.0),
                ("src", 2.0),
                ("bands", 64.0),
                ("order", 2.0),
                ("formant", 3.0),
                ("bshift", 0.5),
                ("spread", 0.7),
                ("uni", 7.0),
                ("robot", 1.0),
                ("cpass", 0.2),
                ("tilt", 2.0),
                ("out", 6.0),
            ],
        ), // fxa:loquax
        // fxa:fractus — every lane painted, the stutter build, swing, the cloud beside the
        // chain at a high density with pitched grains, a throw ringing.
        EffectKind::Glitch => {
            use cantus_core::fx::fractus::{Block, Pattern};
            let mut p = Pattern::default();
            for lane in 0..10u8 {
                let at = (u16::from(lane) * 3) % 14;
                p.replace(lane, &[Block::new(at, 2 + u16::from(lane % 3))]);
            }
            s.fractus = Some(Box::new(p));
            set(
                &mut s,
                &[
                    ("swing", 0.4),
                    ("rramp", 2.0),
                    ("rpitch", 1.0),
                    ("clevel", 0.6),
                    ("cdens", 120.0),
                    ("cpitch", 7.0),
                    ("crand", 12.0),
                    ("crev", 0.3),
                    ("wfb", 0.6),
                    ("out", 3.0),
                ],
            );
        } // fxa:fractus
        // fxa:caelum — sixteen lines, the vast size, both shimmer voices, bloom, ducking
        // keyed, modulated deep (finite: an infinite or frozen tank never rests).
        EffectKind::Shimmer => set(
            &mut s,
            &[
                ("mix", 0.6),
                ("size", 0.95),
                ("decay", 12.0),
                ("shim", 0.8),
                ("pblvl", 0.6),
                ("bloom", 1_500.0),
                ("duck", 12.0),
                ("mdepth", 0.8),
                ("width", 1.3),
                ("out", 6.0),
            ],
        ), // fxa:caelum
        // fxb:vinculum — the vari-mu with its colour at 2× working hard, the side chain's
        // high-pass, parallel, made up, the output raised (time position 2, 0.4 ms / 2 s:
        // the programme-dependent positions hold 10–15 s and rest in minutes, not 30 s).
        EffectKind::BusCompressor => set(
            &mut s,
            &[
                ("model", 2.0),
                ("char", 1.0),
                ("time", 1.0),
                ("thr", -30.0),
                ("hpf", 120.0),
                ("colour", 1.0),
                ("makeup", 6.0),
                ("mix", 0.8),
                ("out", 3.0),
            ],
        ), // fxb:vinculum
        // fxb:lenis — the most work: Precise, Mid/Side (two detectors), Hard at full depth
        // and low selectivity, every focus band on, half the mix.
        EffectKind::ResonanceSuppressor => {
            set(
                &mut s,
                &[
                    ("depth", 1.0),
                    ("select", 0.2),
                    ("qual", 2.0),
                    ("chan", 2.0),
                    ("curve", 1.0),
                    ("att", 1.0),
                    ("mix", 0.8),
                    ("out", 3.0),
                ],
            );
            for b in 0..cantus_core::fx::lenis::BANDS {
                let key = |w| cantus_core::fx::lenis::band_key(b, w);
                set(&mut s, &[(key("on"), 1.0), (key("amt"), 0.5)]);
            }
        } // fxb:lenis
        // fxb:cedo — the sidechain through a band-pass key, RMS, the longest lookahead, the
        // lows split at linear phase, half linked, deep.
        EffectKind::Ducker => set(
            &mut s,
            &[
                ("depth", 30.0),
                ("thr", -36.0),
                ("kfilt", 2.0),
                ("khz", 80.0),
                ("det", 1.0),
                ("look", 10.0),
                ("band", 1.0),
                ("xover", 1.0),
                ("xhz", 150.0),
                ("link", 0.5),
                ("mix", 0.9),
                ("out", 3.0),
            ],
        ), // fxb:cedo
        // fxc:purus — all three modules: the gentle de-hum tracking sixteen harmonics, the
        // de-click at its cap with the de-crackle, the de-reverb estimating its decay; every
        // module's level moved (the latency is all three's, 87 ms at 48 kHz).
        EffectKind::Repair => set(
            &mut s,
            &[
                ("hon", 1.0),
                ("hmode", 1.0),
                ("hharm", 16.0),
                ("hgentle", 1.0),
                ("hout", -1.0),
                ("con", 1.0),
                ("csens", 0.7),
                ("crackle", 0.5),
                ("von", 1.0),
                ("vamount", 18.0),
                ("vdecay", 1.0),
                ("vmix", 0.8),
                ("out", 2.0),
            ],
        ), // fxc:purus
        // fxc:comparatio — the reference asked for, matched short-term with an offset (no
        // reference loaded here: the mix, to the bit).
        EffectKind::Reference => set(
            &mut s,
            &[
                ("listen", 1.0),
                ("match", 1.0),
                ("offset", -3.0),
                ("sync", 1.0),
            ],
        ), // fxc:comparatio
        // r41:nitor — every part on: the Loud character at full amount (its four bands
        // squeezed from above and below), the tonic sub, the bass's harmonics, the tightening,
        // the lift, the three exciter bands at 4×, the level match.
        EffectKind::Enhancer => set(
            &mut s,
            &[
                ("amount", 1.0),
                ("char", 4.0),
                ("sub", 0.6),
                ("submode", 1.0),
                ("harm", 0.5),
                ("tight", 0.6),
                ("lift", 0.7),
                ("xover", 120.0),
                ("body", 0.5),
                ("pres", 0.6),
                ("air", 0.7),
                ("color", 0.3),
                ("key", 8.0),
            ],
        ), // r41:nitor
        EffectKind::FragorOverdrive
        | EffectKind::FragorOpAmp
        | EffectKind::FragorSilicon
        | EffectKind::FragorFuzz
        | EffectKind::FragorMuff => {
            // r41:pedals — the circuit in, its gain and level high and its tone low (the
            // fuzz's battery tired), 8x, stereo, the gate on, half wet: Fragor's heavy
            // board's settings for one circuit.
            let i = kind.fragor_circuit().unwrap_or(0);
            let k = cantus_core::fx::fragor_pedal::knobs(i);
            set(
                &mut s,
                &[
                    (cantus_core::fx::fragor_pedal::switch(i), 1.0),
                    (k[0], 0.9),
                    (k[1], 0.3),
                    (k[2], 0.6),
                    ("gate", -70.0),
                    ("os", 2.0),
                    ("mix", 0.5),
                    ("out", -6.0),
                ],
            );
        } // r41:pedals
    }
    s
}

/// The bench's heavy instrument load: this many voices held at once
/// (`cantus-bench`'s `instrument_metrics`, `inst.*.v32`).
pub const HEAVY_VOICES: usize = 32;

/// The bench's test signal: noise and two tones, swelling and falling at 2 Hz, −12 dBFS
/// peak. A copy of `kernels::signal`.
pub fn signal(frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut seed = 0x2545_F491u32;
    let mut noise = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let (mut l, mut r) = (Vec::with_capacity(frames), Vec::with_capacity(frames));
    for i in 0..frames {
        let t = i as f32 / SAMPLE_RATE;
        let env = 0.5 + 0.5 * cantus_dsp::math::sin(std::f32::consts::TAU * 2.0 * t);
        let tone = cantus_dsp::math::sin(std::f32::consts::TAU * 220.0 * t) * 0.5
            + cantus_dsp::math::sin(std::f32::consts::TAU * 6_300.0 * t) * 0.2;
        l.push((noise() * 0.3 + tone) * env * 0.25);
        r.push((noise() * 0.3 + tone * 0.9) * env * 0.25);
    }
    (l, r)
}

/// A small deterministic generator (xorshift64*): the tests' splits, values and seeds.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as usize
    }
    /// Uniform in `0..1`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    /// A block length: mostly short, sometimes up to 4096, sometimes a single sample — the
    /// splits per-node segmentation can produce.
    pub fn block(&mut self) -> usize {
        match self.range(0, 9) {
            0 => 1,
            1..=4 => self.range(1, 64),
            5..=7 => self.range(1, 512),
            _ => self.range(1, 4096),
        }
    }
}

/// A value drawn across a parameter's whole range (log-spaced where the knob is).
fn draw(info: &EffectParamInfo, rng: &mut Rng) -> f32 {
    let u = rng.unit();
    let v = if info.logarithmic && info.min > 0.0 && info.max > info.min {
        info.min * cantus_dsp::math::powf(info.max / info.min, u)
    } else {
        info.min + (info.max - info.min) * u
    };
    if info.stepped {
        v.round()
    } else {
        v
    }
}

/// An effect with every parameter drawn at random over its range, with a probability
/// `p` per parameter (the others keep their defaults).
pub fn random_effect(kind: EffectKind, p: f32, rng: &mut Rng) -> EffectSpec {
    let mut s = EffectSpec::new(kind);
    for info in kind.params() {
        if rng.chance(p) {
            s.set(info.key, draw(info, rng));
        }
    }
    s
}

/// An instrument with every parameter drawn at random over its range, with a probability
/// `p` per parameter.
pub fn random_instrument(kind: InstrumentKind, p: f32, rng: &mut Rng) -> NativeSpec {
    let mut s = NativeSpec::new(kind);
    for info in kind.params() {
        // Chorda's sustain pedal (round 28) is drawn from a stream of its own, so every other
        // draw stays what it was before the parameter existed.
        if kind == InstrumentKind::Chorda && info.key == "pedal" {
            let mut side = Rng::new(rng.0 ^ 0x0005_EDA1);
            if side.chance(p) {
                s.set(info.key, draw(info, &mut side));
            }
            continue;
        }
        if rng.chance(p) {
            s.set(info.key, draw(info, rng));
        }
    }
    s
}

/// Denormals flushed to zero (FTZ and DAZ on x86-64, FZ on AArch64) while it lives, as the
/// engine's `process` will run (ADR 0031 §5.2): a copy of `cantus-plugins`' guard. Some
/// state decays by multiplication alone and stops in the subnormals without it — there a
/// unit can be exact and still never come to rest.
pub struct Flush {
    #[allow(dead_code)]
    saved: u64,
}

impl Flush {
    #[cfg(target_arch = "x86_64")]
    pub fn new() -> Flush {
        let mut csr: u32 = 0;
        // SAFETY: stmxcsr/ldmxcsr only read and write the SSE control register; the value
        // written is the current one with two mode bits set.
        unsafe {
            std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
            let flushed = csr | 0x8040;
            std::arch::asm!("ldmxcsr [{}]", in(reg) &flushed, options(nostack, preserves_flags));
        }
        Flush { saved: csr as u64 }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn new() -> Flush {
        let fpcr: u64;
        // SAFETY: reads and writes the floating-point control register only.
        unsafe {
            std::arch::asm!("mrs {}, fpcr", out(reg) fpcr, options(nomem, nostack, preserves_flags));
            std::arch::asm!("msr fpcr, {}", in(reg) fpcr | (1 << 24), options(nomem, nostack, preserves_flags));
        }
        Flush { saved: fpcr }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub fn new() -> Flush {
        Flush { saved: 0 }
    }
}

impl Default for Flush {
    fn default() -> Self {
        Flush::new()
    }
}

impl Drop for Flush {
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        {
            let csr = self.saved as u32;
            // SAFETY: restores the value read in `new`.
            unsafe {
                std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, preserves_flags));
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            // SAFETY: restores the value read in `new`.
            unsafe {
                std::arch::asm!("msr fpcr, {}", in(reg) self.saved, options(nomem, nostack, preserves_flags));
            }
        }
    }
}
