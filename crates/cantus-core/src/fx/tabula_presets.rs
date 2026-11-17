//! Tabula's boards as presets (ADR 0134 §1): a board written as a flat map of numbers, so a
//! board preset travels every path a preset does (the menus, the browser, A/B, the agents,
//! the user's own presets) — and the factory's boards, made with care, for guitar, bass,
//! synths, drums and vocals.
//!
//! **The flat map** ([`to_flat`] / [`from_flat`]): the board's own keys as they are; each
//! pedal's parameters as `p{id}.{key}`; and its structure under keys no parameter can have
//! (they hold an `@`): `p{id}.@kind` (its kind's place in `EffectKind::ALL`, which never
//! changes), `p{id}.@at` (its place in the signal order), `p{id}.@path` (0 Pre, 1 A, 2 B,
//! 3 Post), `p{id}.@on` (0 when its footswitch is off), `p{id}.@shape` (1 … 5 when the user
//! chose one); a macro's maps `m{n}.@from{jj}.p{id}.{key}` and `m{n}.@to{jj}.p{id}.{key}`
//! (`jj` its place in the macro's list, two digits), its name
//! `m{n}.@name` (an index into [`MACRO_NAMES`], words the interface translates).

use crate::effect::{EffectKind, EffectSpec};
use crate::fx::tabula::{
    pedal_key, split_key, Board, Macro, MacroMap, Path, Pedal, PedalId, PedalShape, MACROS,
};
use std::collections::BTreeMap;

/// The names a factory board gives its macros: locale keys (the interface translates a
/// name that is one of them).
pub const MACRO_NAMES: &[&str] = &[
    "tabula.macro.drive",
    "tabula.macro.tone",
    "tabula.macro.echo",
    "tabula.macro.space",
    "tabula.macro.motion",
    "tabula.macro.depth",
    "tabula.macro.width",
    "tabula.macro.punch",
    "tabula.macro.grit",
    "tabula.macro.blend",
];

const PATHS: [Path; 4] = [Path::Pre, Path::A, Path::B, Path::Post];
const SHAPES: [PedalShape; 5] = [
    PedalShape::Mini,
    PedalShape::Standard,
    PedalShape::Wide,
    PedalShape::Double,
    PedalShape::Round,
];

/// The board `spec` as a flat map of numbers (the module's note).
pub fn to_flat(spec: &EffectSpec) -> BTreeMap<String, f32> {
    let mut m = spec.params.clone();
    let b = crate::fx::tabula::board(spec);
    for (at, p) in b.pedals.iter().enumerate() {
        let kind = EffectKind::ALL.iter().position(|k| *k == p.spec.kind);
        let Some(kind) = kind else { continue };
        m.insert(pedal_key(p.id, "@kind"), kind as f32);
        m.insert(pedal_key(p.id, "@at"), at as f32);
        if p.path != Path::Pre {
            let i = PATHS.iter().position(|x| *x == p.path).unwrap_or(0);
            m.insert(pedal_key(p.id, "@path"), i as f32);
        }
        if !p.on {
            m.insert(pedal_key(p.id, "@on"), 0.0);
        }
        if let Some(s) = p.shape {
            let i = SHAPES.iter().position(|x| *x == s).unwrap_or(1);
            m.insert(pedal_key(p.id, "@shape"), i as f32 + 1.0);
        }
        for (k, v) in &p.spec.params {
            m.insert(pedal_key(p.id, k), *v);
        }
    }
    for (n, mac) in b.macros.iter().enumerate().take(MACROS) {
        if let Some(i) = mac
            .name
            .as_deref()
            .and_then(|x| MACRO_NAMES.iter().position(|k| *k == x))
        {
            m.insert(format!("m{}.@name", n + 1), i as f32);
        }
        for (j, x) in mac.maps.iter().enumerate() {
            let t = pedal_key(x.pedal, &x.key);
            m.insert(format!("m{}.@from{j:02}.{t}", n + 1), x.from);
            m.insert(format!("m{}.@to{j:02}.{t}", n + 1), x.to);
        }
    }
    m
}

/// Whether a flat map describes a board's structure (a board preset), not only values.
pub fn is_board_map(map: &BTreeMap<String, f32>) -> bool {
    map.keys().any(|k| k.ends_with(".@kind"))
}

/// The board a flat map describes, sanitized (`Board::sanitize`); `None` when the map
/// holds no structure (it then only sets values). Keys it does not know are left out.
pub fn from_flat(map: &BTreeMap<String, f32>) -> Option<EffectSpec> {
    if !is_board_map(map) {
        return None;
    }
    let mut spec = EffectSpec::new(EffectKind::Board);
    // The pedals first, in their order.
    let mut pedals: Vec<(f32, Pedal)> = Vec::new();
    for (k, v) in map {
        let Some((id, rest)) = split_key(k) else {
            continue;
        };
        if rest != "@kind" {
            continue;
        }
        let Some(kind) = EffectKind::ALL.get(*v as usize).copied() else {
            continue;
        };
        let get = |what: &str| map.get(&pedal_key(id, what)).copied();
        let mut p = Pedal::new(id, EffectSpec::new(kind));
        p.path = get("@path")
            .and_then(|i| PATHS.get(i as usize).copied())
            .unwrap_or(Path::Pre);
        p.on = get("@on").is_none_or(|v| v >= 0.5);
        p.shape = get("@shape").and_then(|i| SHAPES.get((i as usize).wrapping_sub(1)).copied());
        pedals.push((get("@at").unwrap_or(f32::MAX), p));
    }
    pedals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut board = Board {
        pedals: pedals.into_iter().map(|x| x.1).collect(),
        macros: Vec::new(),
    };
    // Values: the board's own and the pedals'.
    for (k, v) in map {
        if k.contains('@') {
            continue;
        }
        match split_key(k) {
            Some((id, rest)) => {
                if let Some(p) = board.pedal_mut(id) {
                    let _ = p.spec.set(rest, *v);
                }
            }
            None => {
                let _ = spec.set(k, *v);
            }
        }
    }
    // Macros: names and maps.
    for n in 0..MACROS {
        let pre = format!("m{}.@", n + 1);
        let mut mac = Macro::default();
        for (k, v) in map.range(pre.clone()..) {
            let Some(rest) = k.strip_prefix(&pre) else {
                break;
            };
            if rest == "name" {
                mac.name = MACRO_NAMES.get(*v as usize).map(|x| x.to_string());
            } else if let Some(jt) = rest.strip_prefix("from") {
                let (j, t) = (jt.get(..2).unwrap_or(""), jt.get(3..).unwrap_or(""));
                if let Some((id, key)) = split_key(t) {
                    let to = map.get(&format!("{pre}to{j}.{t}")).copied().unwrap_or(*v);
                    mac.maps.push(MacroMap {
                        pedal: id,
                        key: key.to_string(),
                        from: *v,
                        to,
                    });
                }
            }
        }
        if mac != Macro::default() {
            if board.macros.len() < n {
                board.macros.resize_with(n, Macro::default);
            }
            board.macros.push(mac);
        }
    }
    board.sanitize();
    if !board.pedals.is_empty() || !board.macros.is_empty() {
        spec.board = Some(Box::new(board));
    }
    Some(spec)
}

// ---- the factory's boards ------------------------------------------------------------------

/// One pedal of a factory board: its kind, its lane and its values (every key one its unit
/// has — `factory_boards_are_sound` holds it).
struct P(EffectKind, Path, &'static [(&'static str, f32)]);

/// A factory macro: its name's place in [`MACRO_NAMES`] and its maps (pedal, key, from, to).
type FactoryMacro<'a> = (usize, &'a [(u16, &'a str, f32, f32)]);

fn build(pedals: &[P], own: &[(&str, f32)], macros: &[FactoryMacro<'_>]) -> EffectSpec {
    let mut spec = EffectSpec::new(EffectKind::Board);
    let mut board = Board::default();
    for (i, P(kind, path, values)) in pedals.iter().enumerate() {
        let mut s = crate::starting::starting_spec(*kind);
        for (k, v) in values.iter() {
            let _ = s.set(k, *v);
        }
        let mut p = Pedal::new(PedalId(i as u16), s);
        p.path = *path;
        board.pedals.push(p);
    }
    for (n, (name, maps)) in macros.iter().enumerate() {
        board.macros.push(Macro {
            name: Some(MACRO_NAMES[*name].to_string()),
            maps: maps
                .iter()
                .map(|(id, key, from, to)| MacroMap {
                    pedal: PedalId(*id),
                    key: key.to_string(),
                    from: *from,
                    to: *to,
                })
                .collect(),
        });
        let _ = n;
    }
    spec.board = Some(Box::new(board));
    for (k, v) in own {
        let _ = spec.set(k, *v);
    }
    spec
}

use EffectKind as K;
use Path::{Post, Pre, A, B};

/// The factory's boards: (the name's locale key, the board).
pub fn factory() -> Vec<(&'static str, EffectSpec)> {
    let rig = {
        let mut s = crate::fx::tabula::guitar_rig();
        if let Some(b) = s.board.as_mut() {
            b.macros = vec![
                Macro {
                    name: Some(MACRO_NAMES[0].into()),
                    maps: vec![MacroMap {
                        pedal: PedalId(0),
                        key: "od.drive".into(),
                        from: 0.15,
                        to: 0.8,
                    }],
                },
                Macro {
                    name: Some(MACRO_NAMES[2].into()),
                    maps: vec![
                        MacroMap {
                            pedal: PedalId(2),
                            key: "mix".into(),
                            from: 0.05,
                            to: 0.45,
                        },
                        MacroMap {
                            pedal: PedalId(2),
                            key: "feedback".into(),
                            from: 0.15,
                            to: 0.6,
                        },
                    ],
                },
                Macro {
                    name: Some(MACRO_NAMES[3].into()),
                    maps: vec![MacroMap {
                        pedal: PedalId(3),
                        key: "mix".into(),
                        from: 0.05,
                        to: 0.5,
                    }],
                },
            ];
        }
        s.set("m1", 0.25);
        s.set("m2", 0.4);
        s.set("m3", 0.35);
        s
    };
    vec![
        ("preset.tabula.guitar_rig", rig),
        (
            "preset.tabula.blues_crunch",
            build(
                &[
                    P(K::Compressor, Pre, &[("thr", -26.0), ("ratio", 3.0)]),
                    P(
                        K::Pedal,
                        Pre,
                        &[
                            ("s1", 1.0),
                            ("od.drive", 0.55),
                            ("od.tone", 0.5),
                            ("od.level", 0.6),
                        ],
                    ),
                    P(K::Space, Post, &[("style", 3.0), ("mix", 0.25)]),
                ],
                &[("m1", 0.5)],
                &[(0, &[(1, "od.drive", 0.25, 0.85)])],
            ),
        ),
        (
            "preset.tabula.ambient_swells",
            build(
                &[
                    P(K::Compressor, Pre, &[("thr", -30.0), ("ratio", 4.0)]),
                    P(K::Chorus, Pre, &[("mix", 0.3)]),
                    P(
                        K::Delay,
                        Pre,
                        &[("time_ms", 600.0), ("feedback", 0.55), ("mix", 0.45)],
                    ),
                    P(K::Space, Post, &[("decay", 6.0), ("mix", 0.5)]),
                ],
                &[("m1", 0.5), ("m2", 0.5)],
                &[
                    (2, &[(2, "feedback", 0.3, 0.75), (2, "mix", 0.25, 0.6)]),
                    (3, &[(3, "mix", 0.25, 0.75)]),
                ],
            ),
        ),
        (
            "preset.tabula.fuzz_lead",
            build(
                &[
                    P(
                        K::Pedal,
                        Pre,
                        &[("s1", 4.0), ("ge.fuzz", 0.8), ("ge.vol", 0.5)],
                    ),
                    P(
                        K::Delay,
                        Pre,
                        &[("time_ms", 380.0), ("feedback", 0.3), ("mix", 0.25)],
                    ),
                    P(K::Space, Post, &[("mix", 0.2)]),
                ],
                &[("m1", 0.6)],
                &[(0, &[(0, "ge.fuzz", 0.4, 1.0)])],
            ),
        ),
        (
            "preset.tabula.phaser_funk",
            build(
                &[
                    P(K::Compressor, Pre, &[("thr", -28.0), ("ratio", 6.0)]),
                    P(K::Phaser, Pre, &[("rate", 0.6), ("fb", 0.5), ("mix", 0.5)]),
                    P(K::Tremolo, Pre, &[("rate", 5.0), ("depth", 0.2)]),
                ],
                &[("m1", 0.4)],
                &[(4, &[(1, "rate", 0.2, 3.0), (2, "depth", 0.0, 0.6)])],
            ),
        ),
        (
            "preset.tabula.wet_dry_wet",
            build(
                &[
                    P(
                        K::Pedal,
                        Pre,
                        &[("s1", 1.0), ("od.drive", 0.4), ("od.level", 0.55)],
                    ),
                    P(
                        K::Delay,
                        A,
                        &[("time_ms", 450.0), ("feedback", 0.4), ("mix", 1.0)],
                    ),
                    P(K::Space, B, &[("decay", 4.0), ("mix", 1.0)]),
                ],
                &[("ab", 0.5), ("mix", 0.6)],
                &[(9, &[])],
            ),
        ),
        (
            "preset.tabula.bass_parallel_drive",
            build(
                &[
                    P(K::Compressor, Pre, &[("thr", -24.0), ("ratio", 4.0)]),
                    P(
                        K::Pedal,
                        A,
                        &[("s1", 2.0), ("op.dist", 0.6), ("op.filter", 0.4)],
                    ),
                ],
                &[("ab", 0.45)],
                &[(8, &[(1, "op.dist", 0.2, 0.9)])],
            ),
        ),
        (
            "preset.tabula.bass_chorus",
            build(
                &[
                    P(K::Compressor, Pre, &[("thr", -22.0), ("ratio", 3.0)]),
                    P(K::Chorus, Pre, &[("mix", 0.35)]),
                ],
                &[],
                &[(5, &[(1, "mix", 0.1, 0.6)])],
            ),
        ),
        (
            "preset.tabula.synth_space",
            build(
                &[
                    P(K::Phaser, Pre, &[("rate", 0.25), ("fb", 0.4), ("mix", 0.4)]),
                    P(
                        K::Delay,
                        Pre,
                        &[
                            ("time_ms", 375.0),
                            ("feedback", 0.45),
                            ("mix", 0.3),
                            ("ping_pong", 1.0),
                        ],
                    ),
                    P(K::Space, Post, &[("decay", 5.0), ("mix", 0.35)]),
                ],
                &[],
                &[(3, &[(2, "mix", 0.1, 0.6)])],
            ),
        ),
        (
            "preset.tabula.synth_tape",
            build(
                &[
                    P(K::LoFi, Pre, &[("wow", 0.3), ("flut", 0.2), ("mix", 0.8)]),
                    P(K::Chorus, Pre, &[("mix", 0.3)]),
                    P(
                        K::Delay,
                        Pre,
                        &[("time_ms", 420.0), ("feedback", 0.35), ("mix", 0.25)],
                    ),
                ],
                &[],
                &[(4, &[(0, "wow", 0.0, 0.8)])],
            ),
        ),
        (
            "preset.tabula.drum_crush",
            build(
                &[
                    P(K::Transient, Pre, &[("attack", 0.4)]),
                    P(K::Saturator, A, &[("drive", 0.6), ("mix", 1.0)]),
                    P(K::Compressor, B, &[("thr", -36.0), ("ratio", 8.0)]),
                ],
                &[("ab", 0.4)],
                &[
                    (7, &[(0, "attack", 0.0, 0.8)]),
                    (8, &[(1, "drive", 0.2, 0.9)]),
                ],
            ),
        ),
        (
            "preset.tabula.drum_room",
            build(
                &[
                    P(K::Transient, Pre, &[("attack", 0.25)]),
                    P(K::Space, Pre, &[("decay", 1.0), ("mix", 0.22)]),
                ],
                &[],
                &[(3, &[(1, "mix", 0.05, 0.45)])],
            ),
        ),
        (
            "preset.tabula.vocal_chain",
            build(
                &[
                    P(K::DeEsser, Pre, &[]),
                    P(K::Compressor, Pre, &[("thr", -24.0), ("ratio", 3.0)]),
                    P(
                        K::Delay,
                        Pre,
                        &[("time_ms", 120.0), ("feedback", 0.1), ("mix", 0.15)],
                    ),
                    P(K::Space, Post, &[("mix", 0.2)]),
                ],
                &[],
                &[(3, &[(3, "mix", 0.05, 0.45)]), (2, &[(2, "mix", 0.0, 0.4)])],
            ),
        ),
        (
            "preset.tabula.vocal_doubler",
            build(
                &[
                    P(K::Compressor, Pre, &[("thr", -22.0), ("ratio", 3.0)]),
                    P(K::Chorus, A, &[("mix", 1.0)]),
                    P(
                        K::Delay,
                        B,
                        &[("time_ms", 28.0), ("feedback", 0.0), ("mix", 1.0)],
                    ),
                ],
                &[("ab", 0.5), ("mix", 0.45)],
                &[(6, &[])],
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every factory board is sane, its keys all its units', and it survives the flat map.
    #[test]
    fn factory_boards_are_sound_and_round_trip() {
        let all = factory();
        assert!(all.len() >= 12);
        let mut keys = std::collections::BTreeSet::new();
        for (key, spec) in &all {
            assert!(keys.insert(*key), "{key} twice");
            let b = crate::fx::tabula::board(spec);
            assert!(!b.pedals.is_empty(), "{key}");
            let mut c = b.clone();
            assert_eq!(c.sanitize(), 0, "{key}: a map or a pedal refused");
            let back = from_flat(&to_flat(spec)).expect("a board");
            assert_eq!(&back, spec, "{key}");
        }
    }

    /// Every value a factory board sets is one its unit has (a typo would be dropped).
    #[test]
    fn factory_boards_set_only_real_keys() {
        // `build` drops an unknown key silently; check the source lists themselves.
        let spec = &factory()[1].1;
        assert_eq!(spec.get("p1.od.drive"), 0.55);
        for (key, spec) in factory() {
            for p in spec.pedals() {
                for k in p.spec.params.keys() {
                    assert!(p.spec.kind.param(k).is_some(), "{key}: {k}");
                }
            }
        }
    }

    /// A map of values alone is no board.
    #[test]
    fn plain_values_are_not_a_board() {
        let mut m = BTreeMap::new();
        m.insert("mix".to_string(), 0.5);
        assert!(from_flat(&m).is_none());
    }
}
