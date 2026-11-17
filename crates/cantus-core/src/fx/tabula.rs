//! Tabula, the pedalboard (ADR 0134, provisional): its model. Owned by tabula; nothing else
//! edits this file. **The fx side round's wave-3 contract** declared it whole: the board,
//! its pedals, the A/B split, the macros, how a pedal's parameter is addressed, what may be
//! a pedal, and the board's own twelve parameters.
//!
//! A board is a slot's [`EffectSpec`] of kind [`EffectKind::Board`] whose
//! [`EffectSpec::board`] holds the pedals: each **any native unit** (`pedal_allowed`), its
//! own spec inside, so a pedal is bit-identical to that unit inserted alone. The `Vec`'s
//! order is the signal's: **Pre → (A ∥ B) → Post**; a split exists only while some pedal is
//! on A or B. `board: None` is an empty board — a wire.
//!
//! **Addressing.** A pedal's parameter is `p{id}.{key}` (`p3.time_ms`, `p2.b3.gain`):
//! automation, MIDI learn, modulation and the AI use that key unchanged, through the routed
//! spec API ([`EffectSpec::param`], `get`, `set`, `clear`, `value_mut`, `ensure`). Ids are
//! board-local, so a board preset, a copy and a plug-in state are each one self-contained
//! spec. The board's own keys never start with `p` and a digit.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `m1` … `m8` | the macros (each mapped to pedals' parameters) | 0 … 100 % | 0 |
//! | `in` | the level into the board | −24 … 24 dB | 0 |
//! | `out` | the level out of the board | −24 … 24 dB | 0 |
//! | `mix` | dry/wet, the dry aligned to the board's latency | 0 … 100 % | 100 % |
//! | `ab` | the A/B balance of the split (0: A only, 100 %: B only) | 0 … 100 % | 50 % |
//!
//! **Every default leaves the signal untouched, to the bit**: an empty board is a wire, and
//! so is an empty split at `ab` 50 % (both paths are wires, summed half and half).

use crate::effect::{p, EffectKind, EffectParamInfo, EffectSpec};
use serde::{Deserialize, Serialize};

pub const NAME_KEY: &str = "effect.tabula";
/// Dense: the twelve keys are written (the pedals' own follow their kinds' rules).
pub const SPARSE: bool = false;
/// The kind itself takes none; a board takes a key when one of its pedals does
/// ([`EffectSpec::takes_sidechain`]).
pub const TAKES_SIDECHAIN: bool = false;
/// As [`TAKES_SIDECHAIN`]: a board hears notes when one of its pedals does
/// ([`EffectSpec::takes_notes`]).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key: none.
pub const CHOICES: &[(&str, &str)] = &[];

/// The most pedals a board holds.
pub const MAX_PEDALS: usize = 16;
/// The macros on the board's rail (`m1` … `m8`): the exported plug-in's fixed parameters.
pub const MACROS: usize = 8;
/// Each macro's key, by index (`MACRO_KEYS[0]` is `m1`): no allocation to name one.
pub const MACRO_KEYS: [&str; MACROS] = ["m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8"];

const fn macro_param(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, 0.0, "%", false, false)
}

pub const PARAMS: &[EffectParamInfo] = &[
    macro_param("m1", "fx.tabula.m1"),
    macro_param("m2", "fx.tabula.m2"),
    macro_param("m3", "fx.tabula.m3"),
    macro_param("m4", "fx.tabula.m4"),
    macro_param("m5", "fx.tabula.m5"),
    macro_param("m6", "fx.tabula.m6"),
    macro_param("m7", "fx.tabula.m7"),
    macro_param("m8", "fx.tabula.m8"),
    p("in", "fx.tabula.in", -24.0, 24.0, 0.0, "dB", false, false),
    p("out", "fx.tabula.out", -24.0, 24.0, 0.0, "dB", false, false),
    p("mix", "fx.tabula.mix", 0.0, 1.0, 1.0, "%", false, false),
    p("ab", "fx.tabula.ab", 0.0, 1.0, 0.5, "%", false, false),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// A pedal's id, unique on its board (never across boards: a board is self-contained).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct PedalId(pub u16);

/// Where a pedal sits in the flow Pre → (A ∥ B) → Post.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Path {
    #[default]
    Pre,
    A,
    B,
    Post,
}

/// How a pedal is drawn on the board; `None` on a pedal is its unit's natural shape.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PedalShape {
    Mini,
    Standard,
    Wide,
    Double,
    Round,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_pre(p: &Path) -> bool {
    *p == Path::Pre
}

/// One pedal: a native unit's whole spec, its footswitch, its place and its shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pedal {
    pub id: PedalId,
    pub spec: EffectSpec,
    /// The footswitch: off is a soft bypass that never changes the board's latency.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub on: bool,
    #[serde(default, skip_serializing_if = "is_pre")]
    pub path: Path,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<PedalShape>,
}

impl Pedal {
    /// A pedal of `spec`, on, before any split, in its natural shape.
    pub fn new(id: PedalId, spec: EffectSpec) -> Self {
        Pedal {
            id,
            spec,
            on: true,
            path: Path::Pre,
            shape: None,
        }
    }
}

/// One target of a macro: a pedal's parameter, swept from `from` (macro at 0) to `to`
/// (macro at 100 %) in the target's own units, along its taper.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroMap {
    pub pedal: PedalId,
    /// The pedal's own key (`time_ms`), not the board's `p{id}.` form.
    pub key: String,
    pub from: f32,
    pub to: f32,
}

/// One macro of the rail: its name (`None`: "Macro n") and what it moves.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Macro {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maps: Vec<MacroMap>,
}

/// The board: its pedals in signal order and its macros (`macros[i]` is `m{i+1}`; the list
/// may be shorter than [`MACROS`] — a macro past its end has no name and no maps).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Board {
    #[serde(default)]
    pub pedals: Vec<Pedal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<Macro>,
}

/// The board of a spec that holds none: empty, a wire.
static EMPTY: Board = Board {
    pedals: Vec::new(),
    macros: Vec::new(),
};

/// A spec's board (an empty one when it holds none).
pub fn board(spec: &EffectSpec) -> &Board {
    spec.board.as_deref().unwrap_or(&EMPTY)
}

/// Whether `kind` may be a pedal: every native unit but the board itself (no nesting), a
/// hosted plug-in (delivered by its host, isolated per ADR 0063, and it cannot live in an
/// exported Tabula), the older Reverb (no longer offered) and Speculum (its scope rings are
/// keyed by the slot's `EffectId`).
pub fn pedal_allowed(kind: EffectKind) -> bool {
    !matches!(
        kind,
        EffectKind::Board
            | EffectKind::External
            | EffectKind::Reverb
            | EffectKind::Analyzer
            // fxa:contract — wave A's three join the board with their faces (phase B).
            // fxa:loquax — Loquax stays off: a board has no key and hears no notes, so a
            // vocoder pedal could only ever drone its robot (ADR 0139, phase B).
            | EffectKind::Vocoder
            // fxa:fractus — Fractus stays off too: its lanes are its whole instrument and
            // `SetFractusLane` addresses an insert, not a pedal (no lane could be painted on
            // a board), and a board hears no notes, so its keys and the presets they play
            // would do nothing (ADR 0140, phase B).
            // fxa:caelum — Caelum is a pedal: a reverb pedal is natural. Its ducking hears
            // the board's key when the slot is keyed (a board takes a key when a pedal does),
            // its own input otherwise; its pre-delay syncs to the song through the board.
            | EffectKind::Glitch
    )
}

/// The board's key of a pedal's parameter: `pedal_key(PedalId(3), "time_ms")` is
/// `p3.time_ms`. Allocates: off the audio thread.
pub fn pedal_key(id: PedalId, key: &str) -> String {
    format!("p{}.{key}", id.0)
}

/// The pedal and its own key a board key addresses (`p12.b3.gain` → 12, `b3.gain`); `None`
/// for one of the board's own keys. Exact inverse of [`pedal_key`]: digits without a
/// leading zero (but `p0`), within `u16`, a dot, a key that is not empty. No allocation.
pub fn split_key(key: &str) -> Option<(PedalId, &str)> {
    let rest = key.strip_prefix('p')?;
    let dot = rest.find('.')?;
    let (digits, tail) = (&rest[..dot], &rest[dot + 1..]);
    if digits.is_empty()
        || tail.is_empty()
        || digits.len() > 5
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    let n: u32 = digits.parse().ok()?;
    let id = u16::try_from(n).ok()?;
    Some((PedalId(id), tail))
}

impl Board {
    /// The pedal with `id`.
    pub fn pedal(&self, id: PedalId) -> Option<&Pedal> {
        self.pedals.iter().find(|p| p.id == id)
    }

    pub fn pedal_mut(&mut self, id: PedalId) -> Option<&mut Pedal> {
        self.pedals.iter_mut().find(|p| p.id == id)
    }

    /// Where the pedal with `id` is in signal order.
    pub fn index_of(&self, id: PedalId) -> Option<usize> {
        self.pedals.iter().position(|p| p.id == id)
    }

    /// Whether the board splits: some pedal is on A or on B.
    pub fn has_split(&self) -> bool {
        self.pedals
            .iter()
            .any(|p| matches!(p.path, Path::A | Path::B))
    }

    /// The macro at `index` (`None` past the rail; a default one past the list's end).
    pub fn macro_at(&self, index: usize) -> Option<&Macro> {
        static NONE: Macro = Macro {
            name: None,
            maps: Vec::new(),
        };
        (index < MACROS).then(|| self.macros.get(index).unwrap_or(&NONE))
    }

    /// The macro mapped to a pedal's `key`, if any (one macro per target).
    pub fn macro_of(&self, pedal: PedalId, key: &str) -> Option<usize> {
        self.macros
            .iter()
            .position(|m| m.maps.iter().any(|x| x.pedal == pedal && x.key == key))
    }

    /// Make the board what a command would accept, after a load or a hand edit: pedals of a
    /// kind that may not be one, with an id already used, or past [`MAX_PEDALS`] are
    /// dropped, and nothing nests (a pedal holds no board); macros past [`MACROS`] are
    /// dropped, and so is a map to a pedal or key that is not there, one with a value that
    /// is not a number, or a second map to a target another macro already moves. Returns
    /// how many things it dropped (0 for a board a command made).
    pub fn sanitize(&mut self) -> usize {
        let mut dropped = 0;
        let mut seen: Vec<PedalId> = Vec::with_capacity(self.pedals.len());
        self.pedals.retain(|p| {
            let keep = pedal_allowed(p.spec.kind) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id);
            } else {
                dropped += 1;
            }
            keep
        });
        if self.pedals.len() > MAX_PEDALS {
            dropped += self.pedals.len() - MAX_PEDALS;
            self.pedals.truncate(MAX_PEDALS);
        }
        for p in &mut self.pedals {
            if p.spec.board.take().is_some() {
                dropped += 1;
            }
        }
        if self.macros.len() > MACROS {
            dropped += self.macros.len() - MACROS;
            self.macros.truncate(MACROS);
        }
        let mut targets: Vec<(PedalId, String)> = Vec::new();
        let pedals = &self.pedals;
        for m in &mut self.macros {
            m.maps.retain(|x| {
                let exists = pedals
                    .iter()
                    .find(|p| p.id == x.pedal)
                    .is_some_and(|p| p.spec.kind.param(&x.key).is_some());
                let fresh = !targets.iter().any(|(id, k)| *id == x.pedal && *k == x.key);
                let keep = exists && fresh && x.from.is_finite() && x.to.is_finite();
                if keep {
                    targets.push((x.pedal, x.key.clone()));
                } else {
                    dropped += 1;
                }
                keep
            });
        }
        while self.macros.last().is_some_and(|m| *m == Macro::default()) {
            self.macros.pop();
        }
        dropped
    }
}

/// A fresh pedal id for `board`: one past the highest of its pedals' ids and of every id
/// `referenced` elsewhere (automation lanes, macro maps, modulators naming `p{n}.`), so a
/// leftover lane never attaches to another unit. `None` when no id is left.
pub fn next_id(board: &Board, referenced: impl IntoIterator<Item = PedalId>) -> Option<PedalId> {
    let top = board
        .pedals
        .iter()
        .map(|p| p.id)
        .chain(referenced)
        .map(|id| id.0)
        .max();
    match top {
        None => Some(PedalId(0)),
        Some(n) => n.checked_add(1).map(PedalId),
    }
}

/// A macro's target value at position `v` (0…1): `from` → `to` along the target's taper
/// (logarithmic where both ends are positive), a stepped one rounded, clamped to the range.
pub fn macro_value(info: &EffectParamInfo, from: f32, to: f32, v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    let x = if info.logarithmic && from > 0.0 && to > 0.0 {
        from * cantus_dsp::math::powf(to / from, v)
    } else {
        from + (to - from) * v
    };
    let x = if info.stepped { x.round() } else { x };
    x.clamp(info.min, info.max)
}

/// Store every key a macro moves, so [`apply_macros`] only writes. Allocates: where the
/// snapshot clones the slots and where the automation runtime copies a board, never on the
/// audio thread.
pub fn ensure_macro_keys(spec: &mut EffectSpec) {
    let Some(b) = spec.board.as_deref_mut() else {
        return;
    };
    let Board { pedals, macros } = b;
    for x in macros.iter().flat_map(|m| m.maps.iter()) {
        if let Some(p) = pedals.iter_mut().find(|p| p.id == x.pedal) {
            if p.spec.kind.param(&x.key).is_some() && !p.spec.params.contains_key(&x.key) {
                let v = p.spec.get(&x.key);
                p.spec.params.insert(x.key.clone(), v);
            }
        }
    }
}

/// Write each macro's position into what it moves: a mapped target follows its macro (the
/// macro wins over automation of that target). Writes only keys already stored
/// ([`ensure_macro_keys`]): no allocation, for the snapshot's compile and the audio thread.
pub fn apply_macros(spec: &mut EffectSpec) {
    if spec.kind != EffectKind::Board {
        return;
    }
    let mut at = [0.0f32; MACROS];
    for (i, k) in MACRO_KEYS.iter().enumerate() {
        at[i] = spec.get(k);
    }
    let Some(b) = spec.board.as_deref_mut() else {
        return;
    };
    let Board { pedals, macros } = b;
    for (i, m) in macros.iter().enumerate().take(MACROS) {
        for x in &m.maps {
            let Some(p) = pedals.iter_mut().find(|p| p.id == x.pedal) else {
                continue;
            };
            let Some(info) = p.spec.kind.param(&x.key) else {
                continue;
            };
            if let Some(slot) = p.spec.params.get_mut(x.key.as_str()) {
                *slot = macro_value(info, x.from, x.to, at[i]);
            }
        }
    }
}

/// The slots as the engine is sent them (the snapshot): each board's macro targets stored
/// ([`ensure_macro_keys`]) and set from their macros ([`apply_macros`]), so a mapped knob
/// plays where its macro puts it.
pub fn prepared(mut slots: Vec<crate::effect::EffectSlot>) -> Vec<crate::effect::EffectSlot> {
    for e in &mut slots {
        if e.spec.kind == EffectKind::Board && e.spec.board.is_some() {
            ensure_macro_keys(&mut e.spec);
            apply_macros(&mut e.spec);
        }
    }
    slots
}

/// The inverse of [`EffectSpec::flat_params`]: `flat` becomes the spec's whole parameter
/// map — for a board, its own keys its own and each pedal's `p{id}.{key}` that pedal's
/// (a pedal `flat` names nothing of holds nothing). What an exact undo writes back.
pub fn put_flat_params(spec: &mut EffectSpec, flat: &std::collections::BTreeMap<String, f32>) {
    if spec.kind != EffectKind::Board || spec.board.is_none() {
        spec.params = flat.clone();
        return;
    }
    spec.params = flat
        .iter()
        .filter(|(k, _)| split_key(k).is_none())
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    if let Some(b) = spec.board.as_deref_mut() {
        for p in &mut b.pedals {
            p.spec.params = flat
                .iter()
                .filter_map(|(k, v)| match split_key(k) {
                    Some((id, key)) if id == p.id => Some((key.to_string(), *v)),
                    _ => None,
                })
                .collect();
        }
    }
}

/// Repair every board of a project after a load ([`Board::sanitize`]), and take a board off
/// any slot that is not one. Returns how many things it dropped.
pub fn repair_project(project: &mut crate::model::Project) -> usize {
    let mut dropped = 0;
    for e in project.all_effects_mut() {
        if e.spec.kind != EffectKind::Board {
            if e.spec.board.take().is_some() {
                dropped += 1;
            }
            continue;
        }
        if let Some(b) = e.spec.board.as_deref_mut() {
            dropped += b.sanitize();
        }
    }
    dropped
}

/// The starting preset (ADR 0071; the owner's choice): a guitar rig — Fragor's green
/// overdrive at a low drive, Concentus, Delay and Spatium in its spring style.
pub fn guitar_rig() -> EffectSpec {
    fn pedal(id: u16, kind: EffectKind, values: &[(&str, f32)]) -> Pedal {
        let mut s = EffectSpec::new(kind);
        for (k, v) in values {
            let _ = s.set(k, *v);
        }
        Pedal::new(PedalId(id), s)
    }
    let mut spec = EffectSpec::new(EffectKind::Board);
    spec.board = Some(Box::new(Board {
        pedals: vec![
            pedal(
                0,
                EffectKind::Pedal,
                &[
                    ("s1", 1.0),
                    ("od.drive", 0.3),
                    ("od.tone", 0.55),
                    ("od.level", 0.55),
                ],
            ),
            pedal(
                1,
                EffectKind::Chorus,
                &[("mode", 0.0), ("mix", 0.35), ("out", -1.5)],
            ),
            pedal(
                2,
                EffectKind::Delay,
                &[("time_ms", 375.0), ("feedback", 0.3), ("mix", 0.2)],
            ),
            pedal(3, EffectKind::Space, &[("style", 3.0), ("mix", 0.2)]),
        ],
        macros: Vec::new(),
    }));
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The null rule: every default is the value that leaves the signal untouched.
    #[test]
    fn the_boards_defaults_are_a_null() {
        assert_eq!(PARAMS.len(), 12);
        for (i, k) in MACRO_KEYS.iter().enumerate() {
            assert_eq!(PARAMS[i].key, *k);
            assert_eq!(PARAMS[i].default, 0.0);
        }
        let d = |k: &str| PARAMS.iter().find(|i| i.key == k).unwrap().default;
        assert_eq!((d("in"), d("out"), d("mix"), d("ab")), (0.0, 0.0, 1.0, 0.5));
        let s = EffectSpec::new(EffectKind::Board);
        assert!(s.board.is_none(), "a new board is empty");
        assert!(board(&s).pedals.is_empty());
    }

    /// A key and its pedal survive the round trip, and nothing else parses as a pedal's.
    #[test]
    fn a_pedal_key_round_trips_and_nothing_collides() {
        let k = pedal_key(PedalId(12), "b3.gain");
        assert_eq!(k, "p12.b3.gain");
        assert_eq!(split_key(&k), Some((PedalId(12), "b3.gain")));
        for id in [0u16, 1, 9, 10, 99, 4_096, u16::MAX] {
            for key in ["out", "time_ms", "b23.freq", "p0.on"] {
                let k = pedal_key(PedalId(id), key);
                assert_eq!(split_key(&k), Some((PedalId(id), key)), "{k}");
            }
        }
        // The board's own keys are never a pedal's.
        for i in PARAMS {
            assert!(split_key(i.key).is_none(), "{}", i.key);
            assert!(
                !(i.key.starts_with('p') && i.key[1..].starts_with(|c: char| c.is_ascii_digit())),
                "{}",
                i.key
            );
        }
        for bad in [
            "p",
            "p.",
            "p1",
            "p1.",
            "px.out",
            "p01.out",
            "p65536.out",
            "q1.out",
            "p-1.out",
            "p 1.out",
            "p123456.x",
        ] {
            assert!(split_key(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn no_board_nests_and_no_pedal_is_a_stranger() {
        for k in [
            EffectKind::Board,
            EffectKind::External,
            EffectKind::Reverb,
            EffectKind::Analyzer,
        ] {
            assert!(!pedal_allowed(k), "{k:?}");
        }
        for k in EffectKind::ALL {
            if !matches!(
                k,
                EffectKind::Board
                    | EffectKind::Reverb
                    | EffectKind::Analyzer
                    // fxa:contract — wave A joins the board in phase B.
                    | EffectKind::Vocoder
                    | EffectKind::Glitch
            ) {
                assert!(pedal_allowed(k), "{k:?}");
            }
        }
    }

    /// The load-time repair drops what a command would refuse, and keeps what it accepts.
    #[test]
    fn sanitize_drops_what_a_command_would_refuse() {
        let mut b = Board {
            pedals: vec![
                Pedal::new(PedalId(1), EffectSpec::new(EffectKind::Delay)),
                Pedal::new(PedalId(1), EffectSpec::new(EffectKind::Chorus)),
                Pedal::new(PedalId(2), EffectSpec::new(EffectKind::Board)),
                Pedal::new(PedalId(3), EffectSpec::new(EffectKind::Analyzer)),
                Pedal::new(PedalId(4), EffectSpec::new(EffectKind::Pedal)),
            ],
            macros: vec![Macro {
                name: Some("Drive".into()),
                maps: vec![
                    MacroMap {
                        pedal: PedalId(4),
                        key: "od.drive".into(),
                        from: 0.0,
                        to: 1.0,
                    },
                    MacroMap {
                        pedal: PedalId(4),
                        key: "od.drive".into(),
                        from: 0.2,
                        to: 0.5,
                    },
                    MacroMap {
                        pedal: PedalId(9),
                        key: "out".into(),
                        from: 0.0,
                        to: 1.0,
                    },
                    MacroMap {
                        pedal: PedalId(1),
                        key: "nope".into(),
                        from: 0.0,
                        to: 1.0,
                    },
                ],
            }],
        };
        assert_eq!(b.sanitize(), 6);
        let ids: Vec<u16> = b.pedals.iter().map(|p| p.id.0).collect();
        assert_eq!(ids, [1, 4]);
        assert_eq!(b.macros[0].maps.len(), 1);
        let mut again = b.clone();
        assert_eq!(again.sanitize(), 0, "a sane board is left as it is");
        assert_eq!(again, b);
        let mut many = Board {
            pedals: (0..20)
                .map(|i| Pedal::new(PedalId(i), EffectSpec::new(EffectKind::Tremolo)))
                .collect(),
            macros: vec![Macro::default(); 10],
        };
        assert_eq!(many.sanitize(), 4 + 2);
        assert_eq!(many.pedals.len(), MAX_PEDALS);
        assert!(many.macros.is_empty(), "empty macros are not kept");
    }

    #[test]
    fn a_fresh_id_skips_every_id_still_referenced() {
        let b = Board {
            pedals: vec![Pedal::new(PedalId(2), EffectSpec::new(EffectKind::Delay))],
            macros: Vec::new(),
        };
        assert_eq!(next_id(&Board::default(), []), Some(PedalId(0)));
        assert_eq!(next_id(&b, []), Some(PedalId(3)));
        assert_eq!(next_id(&b, [PedalId(7)]), Some(PedalId(8)));
        assert_eq!(next_id(&b, [PedalId(u16::MAX)]), None);
    }

    /// A macro moves its target along the target's taper, and only through stored keys.
    #[test]
    fn a_macro_moves_its_target_and_allocates_nothing_to_do_it() {
        let mut spec = guitar_rig();
        spec.board.as_mut().unwrap().macros = vec![Macro {
            name: None,
            maps: vec![
                MacroMap {
                    pedal: PedalId(2),
                    key: "time_ms".into(),
                    from: 100.0,
                    to: 400.0,
                },
                MacroMap {
                    pedal: PedalId(0),
                    key: "od.drive".into(),
                    from: 0.0,
                    to: 1.0,
                },
            ],
        }];
        spec.set("m1", 0.5).unwrap();
        ensure_macro_keys(&mut spec);
        apply_macros(&mut spec);
        assert!((spec.get("p2.time_ms") - 200.0).abs() < 1e-3, "logarithmic");
        assert!((spec.get("p0.od.drive") - 0.5).abs() < 1e-6, "linear");
        spec.set("m1", 0.0).unwrap();
        apply_macros(&mut spec);
        assert_eq!(spec.get("p2.time_ms"), 100.0);
        assert_eq!(spec.get("p0.od.drive"), 0.0);
    }

    /// A board's flat map put back is the board it was taken from (an exact undo of a mix
    /// snapshot's recall), and the snapshot's slots carry their macros applied.
    #[test]
    fn the_flat_map_goes_back_into_the_pedals() {
        let mut spec = guitar_rig();
        spec.set("p2.feedback", 0.6).unwrap();
        let flat = spec.flat_params().into_owned();
        let mut other = guitar_rig();
        other.set("p0.od.drive", 0.9).unwrap();
        other.set("mix", 0.3).unwrap();
        put_flat_params(&mut other, &flat);
        assert_eq!(other, spec);
        spec.board.as_mut().unwrap().macros = vec![Macro {
            name: None,
            maps: vec![MacroMap {
                pedal: PedalId(3),
                key: "mix".into(),
                from: 0.1,
                to: 0.5,
            }],
        }];
        spec.set("m1", 1.0).unwrap();
        let slot = crate::effect::EffectSlot {
            id: crate::model::EffectId(1),
            bypass: false,
            mix: 1.0,
            sidechain: None,
            midi_from: None,
            spec,
        };
        let out = prepared(vec![slot]);
        assert!((out[0].spec.get("p3.mix") - 0.5).abs() < 1e-6);
    }

    /// The starting rig is the owner's: four allowed pedals in order, a sane board.
    #[test]
    fn the_starting_rig_is_a_guitar_board() {
        let spec = guitar_rig();
        let b = board(&spec);
        let kinds: Vec<EffectKind> = b.pedals.iter().map(|p| p.spec.kind).collect();
        assert_eq!(
            kinds,
            [
                EffectKind::Pedal,
                EffectKind::Chorus,
                EffectKind::Delay,
                EffectKind::Space
            ]
        );
        assert_eq!(spec.get("p3.style"), 3.0, "Spatium's spring");
        assert_eq!(spec.get("p0.s1"), 1.0, "the green overdrive");
        assert!(spec.get("p0.od.drive") < 0.5, "at a low drive");
        let mut c = b.clone();
        assert_eq!(c.sanitize(), 0);
        let json = serde_json::to_string(&spec).unwrap();
        let back: EffectSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
    }
}
