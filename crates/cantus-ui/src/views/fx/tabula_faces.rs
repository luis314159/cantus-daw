//! Tabula's pedal faces (ADR 0131, its faces table): what each native unit shows as a pedal
//! on the board — up to four knobs, an enclosure shape, a motif, a letter and a texture; the
//! finish comes from the unit's own skin (and the colourway the user chose for it).
//!
//! **The wave-3 contract pre-filled one row per kind that may be a pedal**
//! (`cantus_core::fx::tabula::pedal_allowed`); the knobs are placeholders a builder refines.
//! Each unit's builder edits only its own row — the new units' rows are marked
//! `// fx3:<stem>`; the tabula builder owns the rest of the file and the drawing.

use cantus_core::fx::tabula::PedalShape;
use cantus_core::EffectKind;

/// The small picture printed on a pedal's face, above its knobs.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Motif {
    None,
    /// A sine: modulation.
    Wave,
    /// Repeats fading: delays.
    Echoes,
    /// Rings spreading: spaces.
    Rings,
    /// A clipped wave: drive.
    Clip,
    /// A needle: dynamics.
    Needle,
    /// A curve: tone.
    Curve,
    /// Steps: time and rhythm.
    Steps,
    /// A spiral: rotation.
    Spiral,
    /// Arcs leaping over a keyboard: pitch. // fx3:saltus
    Leap,
    /// A horn rotor turning, its sound thrown in arcs: the rotary speaker. // fx3:rota
    Wheel,
    /// A valve before a speaker's cone: the amp. // fx3:arca
    Chest,
}

/// The enclosure's surface.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Texture {
    Smooth,
    Hammered,
    Brushed,
    Wrinkle,
}

/// One unit's pedal face.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Face {
    pub kind: EffectKind,
    /// The knobs on the face, by the unit's own keys (at most four).
    pub knobs: &'static [&'static str],
    pub shape: PedalShape,
    pub motif: Motif,
    /// The big letter stamped on the face (the unit's initial, or a sign).
    pub letter: &'static str,
    pub texture: Texture,
}

const fn face(
    kind: EffectKind,
    knobs: &'static [&'static str],
    shape: PedalShape,
    motif: Motif,
    letter: &'static str,
    texture: Texture,
) -> Face {
    Face {
        kind,
        knobs,
        shape,
        motif,
        letter,
        texture,
    }
}

use EffectKind as K;
use Motif as M;
use PedalShape as S;
use Texture as T;

/// Every pedal's face, one row per kind that may be a pedal.
pub const FACES: &[Face] = &[
    face(
        K::Eq,
        &["b0.gain", "b1.gain", "b2.gain", "out"],
        S::Wide,
        M::Curve,
        "Æ",
        T::Brushed,
    ),
    face(
        K::Compressor,
        &["thr", "ratio", "att", "rel"],
        S::Standard,
        M::Needle,
        "C",
        T::Brushed,
    ),
    face(
        K::Limiter,
        &["gain", "rel"],
        S::Standard,
        M::Needle,
        "T",
        T::Brushed,
    ),
    face(
        K::Space,
        &["space", "decay", "mix"],
        S::Standard,
        M::Rings,
        "S",
        T::Smooth,
    ),
    face(K::Multiband, &["out"], S::Wide, M::Needle, "M", T::Brushed),
    face(
        K::DeEsser,
        &["thr", "range"],
        S::Mini,
        M::Needle,
        "S",
        T::Smooth,
    ),
    face(
        K::Gate,
        &["thr", "rel"],
        S::Mini,
        M::Needle,
        "P",
        T::Hammered,
    ),
    face(
        K::Saturator,
        &["drive", "tone", "mix"],
        S::Standard,
        M::Clip,
        "F",
        T::Hammered,
    ),
    face(K::Stereo, &["width"], S::Mini, M::Rings, "L", T::Smooth),
    face(
        K::Maximizer,
        &["gain"],
        S::Standard,
        M::Needle,
        "C",
        T::Brushed,
    ),
    face(
        K::Delay,
        &["time_ms", "feedback", "mix"],
        S::Standard,
        M::Echoes,
        "D",
        T::Hammered,
    ),
    face(
        K::Chorus,
        &["rate", "mix", "depth"],
        S::Standard,
        M::Wave,
        "C",
        T::Smooth,
    ),
    face(
        K::Flanger,
        &["rate", "fb", "mix"],
        S::Standard,
        M::Wave,
        "F",
        T::Smooth,
    ),
    face(
        K::Phaser,
        &["rate", "fb", "mix"],
        S::Round,
        M::Wave,
        "P",
        T::Hammered,
    ),
    face(
        K::Tremolo,
        &["rate", "depth"],
        S::Standard,
        M::Wave,
        "T",
        T::Wrinkle,
    ),
    face(
        K::Vibrato,
        &["rate", "mix", "depth"],
        S::Standard,
        M::Wave,
        "V",
        T::Smooth,
    ),
    face(
        K::RingMod,
        &["rate", "mix"],
        S::Mini,
        M::Wave,
        "A",
        T::Wrinkle,
    ),
    face(
        K::FreqShift,
        &["fb", "mix"],
        S::Mini,
        M::Wave,
        "T",
        T::Smooth,
    ),
    face(
        K::VolumeShaper,
        &["depth", "div"],
        S::Standard,
        M::Steps,
        "F",
        T::Smooth,
    ),
    face(
        K::Clipper,
        &["knee", "out"],
        S::Mini,
        M::Clip,
        "T",
        T::Brushed,
    ),
    face(
        K::Transient,
        &["attack", "sustain"],
        S::Mini,
        M::Needle,
        "I",
        T::Brushed,
    ),
    face(
        K::TransientSplit,
        &["t.gain", "b.gain"],
        S::Standard,
        M::Needle,
        "D",
        T::Brushed,
    ),
    face(
        K::AutoTune,
        &["key", "scale"],
        S::Standard,
        M::Steps,
        "T",
        T::Smooth,
    ),
    face(
        K::Aula,
        &["decay", "mix"],
        S::Standard,
        M::Rings,
        "A",
        T::Smooth,
    ),
    face(
        K::AutoFilter,
        &["a.cut", "a.res", "mix"],
        S::Standard,
        M::Curve,
        "C",
        T::Hammered,
    ),
    face(
        K::Pedal,
        &["od.drive", "od.tone", "od.level"],
        S::Standard,
        M::Clip,
        "F",
        T::Hammered,
    ),
    face(
        K::TimeShaper,
        &["tslot", "vslot", "mix"],
        S::Standard,
        M::Steps,
        "L",
        T::Smooth,
    ),
    face(
        K::LoFi,
        &["wow", "flut", "hiss", "mix"],
        S::Standard,
        M::Steps,
        "V",
        T::Wrinkle,
    ),
    face(
        K::Utility,
        &["out", "pan", "width"],
        S::Mini,
        M::Needle,
        "L",
        T::Brushed,
    ),
    face(
        K::PitchShift,
        &["st1", "lvl1", "dry", "pedal"],
        S::Standard,
        M::Leap,
        "S",
        T::Brushed,
    ), // fx3:saltus
    face(
        K::Rotary,
        &["speed", "drive", "balance", "mix"],
        S::Wide,
        M::Wheel,
        "R",
        T::Wrinkle,
    ), // fx3:rota
    face(
        K::Amp,
        &["gain", "bass", "treble", "master"],
        S::Wide,
        M::Chest,
        "A",
        T::Hammered,
    ), // fx3:arca
    // fxa:caelum — a shimmer pedal, as a big-box reverb is: the space's decay, the
    // shimmer's amount and first voice, the mix (Infinite and Freeze stay on its editor's
    // sill, reached from the pedal).
    face(
        K::Shimmer,
        &["decay", "shim", "pa", "mix"],
        S::Wide,
        M::Rings,
        "C",
        T::Smooth,
    ), // fxa:caelum
    // fxb:vinculum — a compressor pedal: the model, the sustain (the threshold), the level
    // (the make-up) and the blend (the mix). The Compress/Limit and the times stay on its
    // editor's faceplate, reached from the pedal.
    face(
        K::BusCompressor,
        &["model", "thr", "makeup", "mix"],
        S::Standard,
        M::Needle,
        "V",
        T::Smooth,
    ), // fxb:vinculum
    // fxb:lenis — a resonance tamer: depth, sharpness, selectivity and where its focus
    // starts (a guitar's body left alone below it). The curve, the quality and the focus's
    // bands stay on its editor's screen, reached from the pedal.
    face(
        K::ResonanceSuppressor,
        &["depth", "sharp", "select", "lo"],
        S::Standard,
        M::Curve,
        "L",
        T::Brushed,
    ), // fxb:lenis
    // fxb:cedo — a ducker on the board: its trigger, how deep, the cycle, the threshold (a
    // keyed board's key, the board's beat or its notes fire it). The shape, the envelope and
    // the band stay on its editor's screen, reached from the pedal.
    face(
        K::Ducker,
        &["trig", "depth", "div", "thr"],
        S::Standard,
        M::Steps,
        "C",
        T::Hammered,
    ), // fxb:cedo
];

/// A kind's pedal face; `None` for a kind that may not be a pedal.
pub fn of(kind: EffectKind) -> Option<&'static Face> {
    FACES.iter().find(|f| f.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::fx::tabula::pedal_allowed;

    /// One face per kind that may be a pedal, none for the others, and every knob a key of
    /// its unit — at most four.
    #[test]
    fn every_pedal_has_one_face_of_its_own_keys() {
        for kind in EffectKind::ALL {
            let n = FACES.iter().filter(|f| f.kind == kind).count();
            assert_eq!(n, usize::from(pedal_allowed(kind)), "{kind:?}");
        }
        for f in FACES {
            assert!(!f.knobs.is_empty() && f.knobs.len() <= 4, "{:?}", f.kind);
            for k in f.knobs {
                assert!(f.kind.param(k).is_some(), "{:?}: no key {k}", f.kind);
            }
            assert!(!f.letter.is_empty());
        }
    }
}
