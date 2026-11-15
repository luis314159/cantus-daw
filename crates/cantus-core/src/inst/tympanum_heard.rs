//! What a microphone hears of a piece — one estimate the engine's virtual mics and the
//! views (*What it hears*, the matrix) share (round 40 phase 3, asked by `tymp-ui`): for a
//! modelled kit, the direct path the engine renders (1/r from where the mic stands now,
//! its pattern at the angle the sound arrives from, the head's radiation towards it, the
//! mic's level against its own piece), and the bleed cell and the kit's `bleed` on top; for
//! a recorded kit, the kit's own default cell.

use super::{KitDef, MicDef, PieceDef, PieceKind};
use crate::inst::tympanum::MicPattern;
use crate::NativeSpec;

/// The pattern's gain at `cos` of the angle off its axis (the engine's law).
pub fn pattern_gain(p: MicPattern, cos: f32) -> f32 {
    let a = match p {
        MicPattern::Omni => 1.0,
        MicPattern::Cardioid => 0.5,
        MicPattern::Supercardioid => 0.37,
        MicPattern::Hypercardioid => 0.25,
        MicPattern::Figure8 => 0.0,
    };
    a + (1.0 - a) * cos
}

/// How much of a piece's sound is a dipole along its head's normal (the engine's).
pub fn dipole(kind: PieceKind) -> f32 {
    match kind {
        PieceKind::Kick => 0.55,
        PieceKind::Snare | PieceKind::RackTom | PieceKind::FloorTom => 0.7,
        _ => 0.85,
    }
}

/// Where mic `i` stands (cm), where it points and its pattern, with the `m{i}.*` moves.
pub fn mic_now(spec: &NativeSpec, i: usize, m: &MicDef) -> ([f32; 3], [f32; 3], MicPattern) {
    let k = |f: &str| spec.get(&format!("m{i}.{f}"));
    let a = m.place.at();
    let pos = [a[0] + k("x"), a[1] + k("y"), a[2] + k("z")];
    let mut place = m.place;
    place.azimuth += k("az");
    place.elevation = (place.elevation + k("el")).clamp(-90.0, 90.0);
    let pv = k("pattern");
    let pattern = if pv >= 1.0 {
        MicPattern::ALL[((pv as usize) - 1).min(MicPattern::ALL.len() - 1)]
    } else {
        m.pattern
    };
    (pos, place.dir(), pattern)
}

/// The direct path's gain from `piece` to a capsule at `pos` (cm) aimed `aim`.
pub fn direct_gain(piece: &PieceDef, pos: [f32; 3], aim: [f32; 3], pattern: MicPattern) -> f32 {
    let s = piece.place.at();
    let v = [
        (pos[0] - s[0]) / 100.0,
        (pos[1] - s[1]) / 100.0,
        (pos[2] - s[2]) / 100.0,
    ];
    let r = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(0.03);
    let u = [v[0] / r, v[1] / r, v[2] / r];
    let pat = pattern_gain(pattern, -(aim[0] * u[0] + aim[1] * u[1] + aim[2] * u[2]));
    let n = piece.place.dir();
    let c = -(n[0] * u[0] + n[1] * u[1] + n[2] * u[2]);
    let dp = dipole(piece.kind);
    let rad = (1.0 - dp) + dp * c.abs();
    (pat * rad / r).abs()
}

/// What mic `m` (output `o{n}`) hears of `piece`, dB: 0 is the level of the mic's own
/// piece where the kit puts it.
pub fn heard_db(def: &KitDef, spec: &NativeSpec, piece: u16, m: &MicDef) -> f32 {
    let Some(pd) = def.pieces.iter().find(|x| x.index == piece) else {
        return -96.0;
    };
    let n = m.output.0;
    let cell = format!("b{piece}.{n}.");
    if spec.get(&format!("{cell}on")) < 0.5 {
        return -96.0;
    }
    let own = m.main_for.is_empty() || m.main_for.contains(&piece);
    let kit_bleed = if own { 0.0 } else { spec.get("bleed") };
    let base = if def.modelled {
        let i = def
            .mics
            .iter()
            .position(|x| x.output == m.output)
            .unwrap_or(0);
        let (pos, aim, pat) = mic_now(spec, i, m);
        let g = direct_gain(pd, pos, aim, pat);
        // Its own piece (or the kit's middle) where the kit puts the mic: the reference.
        let reference = m
            .main_for
            .first()
            .and_then(|p| def.pieces.iter().find(|x| x.index == *p))
            .or_else(|| def.pieces.iter().find(|x| x.kind == PieceKind::Snare))
            .map_or(1.0, |p| {
                direct_gain(p, m.place.at(), m.place.dir(), m.pattern)
            });
        crate::linear_to_db((g / reference.max(1e-6)).max(1e-5)) + m.level_db
    } else {
        def.bleed
            .cells
            .iter()
            .find(|c| c.0 == piece && c.1 == m.output)
            .map_or(0.0, |c| c.2)
    };
    (base + kit_bleed + spec.get(&format!("{cell}db"))).clamp(-96.0, 24.0)
}
