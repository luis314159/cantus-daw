//! r42:arcus-upper — the realistic musicians that may hold an Arcus instrument (the owner,
//! 2026-11-24): a woman in a black velvet long-sleeved concert dress and a man in a dark
//! suit, made from MakeHuman's CC0 assets offline (`tools/arcus-figures/build_figures.py`)
//! into `assets/figures/{woman,man}.fig`, posed here from the same skeleton the stylised
//! figure is posed from (`bowed_figure::skeleton`), skinned (four bones a vertex) and
//! handed to both pictures as smooth triangles (`Mesh::tris`).
//!
//! **Not uncanny** (the owner asked twice): realistic adult proportions, the eyelids lowered
//! over the eyes (a player's eyes on the instrument), small calm features, soft skin. **The
//! faces are original**: MakeHuman's generic adult averages, nothing fitted to a photograph
//! (the owner's reference shows a real person; only her posture, dress and light are taken).
//!
//! The arms are the live picture's, as the stylised figure's are: each triangle carries its
//! part (the body, baked; the left or the right arm, drawn live by
//! `views/inst/arcus_draw_flesh.rs` from the same vertices), a band round each shoulder in
//! both, so no seam opens when an arm moves.

use super::geo::{v, V3};
use std::sync::OnceLock;

/// The scene's bones, as `build_figures.py` lists them: the trunk, then each side's 24
/// (left first).
pub const HIPS: usize = 0;
pub const SPINE: usize = 1;
pub const CHEST: usize = 2;
pub const NECK: usize = 3;
pub const HEAD: usize = 4;
/// A side's bones (`side` 0 the left, 1 the right): its first bone's index.
pub const fn side(s: usize) -> usize {
    5 + 24 * s
}
pub const CLAV: usize = 0;
pub const UPPER1: usize = 1;
pub const UPPER2: usize = 2;
pub const FORE1: usize = 3;
pub const FORE2: usize = 4;
pub const HAND: usize = 5;
/// Finger `f` (0 the thumb … 4 the little finger), phalanx `k` (0…2).
pub const fn finger(f: usize, k: usize) -> usize {
    6 + f * 3 + k
}
pub const THIGH: usize = 21;
pub const SHIN: usize = 22;
pub const FOOT: usize = 23;
pub const BONES: usize = 53;

/// What a triangle is made of (`build_figures.py`'s numbers).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stuff {
    Skin,
    Lips,
    Hair,
    Velvet,
    Wool,
    Shirt,
    Tie,
    Shoes,
    /// The eyes' whites and irises under the lowered lids, the lashes, the nails (sheet v7).
    Eye,
    Iris,
    Lash,
    Nail,
    /// r42:arcus-upper — her small drop earrings (gold).
    Jewel,
    /// r42:arcus-upper — MakeHuman's CC0 hair, brow and lash proxies: alpha-tested cards
    /// in the Photographic bake (their textures `assets/figures/<who>_<kind>.png`), left out
    /// of the Drawn picture; their vertices carry the cards' texture coordinates.
    CardHair,
    CardBrow,
    CardLash,
    /// r42:arcus-upper — the hair as a thick shell (the Drawn picture's) and a thin scalp
    /// under the cards (the Photographic picture's).
    Shell,
    Scalp,
}

impl Stuff {
    fn of(n: u8) -> Stuff {
        match n {
            0 => Stuff::Skin,
            1 => Stuff::Lips,
            2 => Stuff::Hair,
            3 => Stuff::Velvet,
            4 => Stuff::Wool,
            5 => Stuff::Shirt,
            6 => Stuff::Tie,
            8 => Stuff::Eye,
            9 => Stuff::Iris,
            10 => Stuff::Lash,
            11 => Stuff::Nail,
            12 => Stuff::Jewel,
            13 => Stuff::CardHair,
            14 => Stuff::CardBrow,
            15 => Stuff::CardLash,
            16 => Stuff::Shell,
            17 => Stuff::Scalp,
            _ => Stuff::Shoes,
        }
    }
}

/// A bone at rest: its parent, head, tail and its frame's x (across it).
#[derive(Copy, Clone, Debug, Default)]
pub struct Bone {
    pub parent: Option<usize>,
    pub head: V3,
    pub tail: V3,
    pub x: V3,
}

/// A figure at rest (cm, y up, facing +z, its left on +x).
#[derive(Debug, Default)]
pub struct Figure {
    pub bones: Vec<Bone>,
    pub marks: Vec<(String, V3)>,
    pub pos: Vec<V3>,
    /// Four bones and their weights (0…1) a vertex.
    pub bw: Vec<[(u8, f32); 4]>,
    /// The skin's albedo detail (a luminance factor, a red shift) or the hair's strand
    /// coordinates (cm along, across).
    pub detail: Vec<[f32; 2]>,
    pub tris: Vec<[u16; 3]>,
    pub stuff: Vec<Stuff>,
    /// A bit each: 1 the body, 2 the left arm, 4 the right.
    pub part: Vec<u8>,
    /// r42:arcus-upper — each vertex's texture coordinates (the skin's: her 0…1 in u, his
    /// 2…3; a card's; a hair's strands), and the vertex it shares its place with (a copy
    /// made at a texture seam names the first: normals are summed over both, so no seam
    /// shows in the light).
    pub uv: Vec<[f32; 2]>,
    pub weld: Vec<u32>,
    /// r42:arcus-hair — a hair card vertex's degrees in from the hairline (negative: out
    /// over the skin; `build_figures.py::line_lum`): the bake feathers the cards there.
    pub line: Vec<f32>,
    /// r42:arcus-polish — a cloth vertex's distance (cm) to its cloth's edge
    /// (`figure::cloth::edges`; `EDGE_FAR` for skin and the rest).
    pub edge: Vec<f32>,
}

/// Which figure.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Who {
    Woman,
    Man,
}

static WOMAN: &[u8] = include_bytes!("../../assets/figures/woman.fig");
static MAN: &[u8] = include_bytes!("../../assets/figures/man.fig");

static WOMAN_LIDS: &[u8] = include_bytes!("../../assets/figures/woman_lids.bin");
static MAN_LIDS: &[u8] = include_bytes!("../../assets/figures/man_lids.bin");

/// r42:arcus-polish — a figure's asset: the embedded bytes, or, while the figures are being
/// built (`CANTUS_FIGURE_DIR` names a folder `tools/arcus-figures/build_figures.py` wrote),
/// that folder's file of the same name, read once — so a rebuilt figure is looked at in
/// `shoot_arcus` without compiling the app again (DEVELOPING §4). Unset in the app.
fn asset(name: &str, embedded: &'static [u8]) -> std::borrow::Cow<'static, [u8]> {
    std::env::var_os("CANTUS_FIGURE_DIR")
        .and_then(|d| std::fs::read(std::path::Path::new(&d).join(name)).ok())
        .map_or(
            std::borrow::Cow::Borrowed(embedded),
            std::borrow::Cow::Owned,
        )
}

/// r42:arcus-hand — `CLID`, a `u16` version (1), a `u32` count, then per moved vertex its
/// `u32` index and its offset at rest (three `i16` at 0.001 cm): the lids closed over the
/// eyes (`tools/arcus-figures/lids.py`). Empty when the bytes are not that.
fn parse_lids(b: &[u8]) -> Vec<(u32, V3)> {
    fn read(r: &mut Rd) -> Option<Vec<(u32, V3)>> {
        if r.take::<4>()? != *b"CLID" || r.u16()? != 1 {
            return None;
        }
        let n = r.u32()? as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let i = r.u32()?;
            let d = v(r.i16()? as f32, r.i16()? as f32, r.i16()? as f32) * 0.001;
            out.push((i, d));
        }
        Some(out)
    }
    read(&mut Rd { b, o: 0 }).unwrap_or_default()
}

/// r42:arcus-hand — the offsets at rest that close `w`'s eyes (vertex, offset), read once.
pub fn lids(w: Who) -> &'static [(u32, V3)] {
    static W: OnceLock<Vec<(u32, V3)>> = OnceLock::new();
    static M: OnceLock<Vec<(u32, V3)>> = OnceLock::new();
    match w {
        Who::Woman => W.get_or_init(|| parse_lids(&asset("woman_lids.bin", WOMAN_LIDS))),
        Who::Man => M.get_or_init(|| parse_lids(&asset("man_lids.bin", MAN_LIDS))),
    }
}

/// The figure, read once.
pub fn figure(w: Who) -> &'static Figure {
    static W: OnceLock<Figure> = OnceLock::new();
    static M: OnceLock<Figure> = OnceLock::new();
    let read = |b: &[u8], k: usize| {
        let mut f = parse(b).unwrap_or_default();
        knuckle_fix(&mut f, HAND_SCALE[k], PALM_SLIM[k]);
        // r42:arcus-polish — the thumb's web and thenar.
        if f.bones.len() >= BONES {
            for s in 0..2 {
                thumb_web(&mut f, s);
            }
        }
        smooth_tie(&mut f);
        smooth_borders(&mut f);
        lift_suit(&mut f);
        // r42:arcus-polish — where the clothes meet the skin: the cloth's edges, the
        // skin's contact shadow.
        super::figure::cloth::sink(&mut f);
        f.edge = super::figure::cloth::edges(&f);
        super::figure::cloth::contact(&mut f);
        // r42:arcus-polish2 — the shade under a collar's, a lapel's edge.
        super::figure::cloth::under_edges(&mut f);
        f
    };
    match w {
        Who::Woman => W.get_or_init(|| read(&asset("woman.fig", WOMAN), 0)),
        Who::Man => M.get_or_init(|| read(&asset("man.fig", MAN), 1)),
    }
}

/// r42:arcus-hand — each finger's base joint where a hand's knuckle is. MakeHuman's finger
/// bones begin at the webs, about a centimetre and a half past the knuckles' joints (her
/// first phalanges 2.2–3.0 cm, a real one's 3.7–4.2): bent there, a finger folded in the
/// middle of its first phalanx and the palm ran on into it — the owner's "fist", short
/// fingers on a broad palm. Each finger's base joint moves back along its metacarpal by
/// [`KNUCKLE_BACK`] of the wrist-to-base length, and the skin between is handed from the
/// hand's bone to the finger's, smoothly (a ramp round the new joint), each vertex by how
/// near it lies to that finger's line. `tools/arcus-figures/hand_rig.py::knuckle_fix` does
/// the same for the table's solve; the two must stay alike.
pub const KNUCKLE_BACK: f32 = 0.12;
const KNUCKLE_LANE: f32 = 0.8;
const KNUCKLE_RAMP: (f32, f32) = (-0.5, 0.7);

/// r42:arcus-hand — the suit's parts with clean edges: the converter told the jacket, the
/// shirt and the tie apart per triangle by the suit texture's light and blue, so single
/// triangles of one stuck into another along their edges (a white sawtooth down his tie and
/// the jacket's front, plain from the new camera). A triangle of the three with two or three
/// edge-neighbours of one other of them takes that one's stuff (twice over).
fn smooth_tie(fig: &mut Figure) {
    use std::collections::HashMap;
    let w = |i: u16| fig.weld.get(i as usize).copied().unwrap_or(i as u32);
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (k, t) in fig.tris.iter().enumerate() {
        if !matches!(fig.stuff[k], Stuff::Tie | Stuff::Shirt | Stuff::Wool) {
            continue;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (w(a), w(b));
            by_edge.entry((a.min(b), a.max(b))).or_default().push(k);
        }
    }
    for _ in 0..2 {
        let mut flip = Vec::new();
        for (k, t) in fig.tris.iter().enumerate() {
            let st = fig.stuff[k];
            if !matches!(st, Stuff::Tie | Stuff::Shirt | Stuff::Wool) {
                continue;
            }
            for other in [Stuff::Tie, Stuff::Shirt, Stuff::Wool] {
                if other == st {
                    continue;
                }
                let mut n = 0;
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let (a, b) = (w(a), w(b));
                    if let Some(ks) = by_edge.get(&(a.min(b), a.max(b))) {
                        if ks.iter().any(|&j| j != k && fig.stuff[j] == other) {
                            n += 1;
                        }
                    }
                }
                if n >= 2 {
                    flip.push((k, other));
                    break;
                }
            }
        }
        for (k, st) in flip {
            fig.stuff[k] = st;
        }
    }
}

/// r42:arcus-polish2 — the lines where the suit's parts meet (the shirt's edge against the
/// lapel, the tie's against the shirt) drawn smooth: the converter told them apart per
/// triangle, so each line ran along the triangles' edges in a zigzag — teeth down the
/// lapel and the collar in both pictures (the owner's review). Each vertex on such a line
/// with two neighbours along it is drawn toward their middle a few times (at most
/// [`BORDER_MOVE`] cm from where it was), its copies with it.
fn smooth_borders(fig: &mut Figure) {
    use std::collections::HashMap;
    let suit = |s: Stuff| matches!(s, Stuff::Tie | Stuff::Shirt | Stuff::Wool);
    let w = |i: u16| fig.weld.get(i as usize).copied().unwrap_or(i as u32) as usize;
    let mut by_edge: HashMap<(usize, usize), Vec<Stuff>> = HashMap::new();
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        if !suit(*s) {
            continue;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (w(a), w(b));
            by_edge.entry((a.min(b), a.max(b))).or_default().push(*s);
        }
    }
    let mut nb: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&(a, b), st) in &by_edge {
        if st.len() == 2 && st[0] != st[1] {
            nb.entry(a).or_default().push(b);
            nb.entry(b).or_default().push(a);
        }
    }
    let start: HashMap<usize, V3> = nb.keys().map(|&i| (i, fig.pos[i])).collect();
    let mut pos: HashMap<usize, V3> = start.clone();
    for _ in 0..BORDER_PASSES {
        let mut next = pos.clone();
        for (&i, ns) in &nb {
            if ns.len() != 2 {
                continue;
            }
            let m = (pos[&ns[0]] + pos[&ns[1]]) * 0.5;
            let p = pos[&i] * 0.5 + m * 0.5;
            let d = p - start[&i];
            next.insert(
                i,
                if d.len() > BORDER_MOVE {
                    start[&i] + d.norm() * BORDER_MOVE
                } else {
                    p
                },
            );
        }
        pos = next;
    }
    for i in 0..fig.pos.len() {
        if let Some(p) = pos.get(&(fig.weld.get(i).copied().unwrap_or(i as u32) as usize)) {
            fig.pos[i] = *p;
        }
    }
}

/// How many times the suit's lines are smoothed, and how far a vertex on one may move (cm).
const BORDER_PASSES: usize = 4;
const BORDER_MOVE: f32 = 0.7;

/// r42:arcus-hand — the suit a little further off the skin: his collar's edge showed the
/// neck's skin through it in a sawtooth (the review, from the new camera). Every vertex of
/// the jacket, the shirt and the tie rises [`SUIT_LIFT`] cm along its normal (all three
/// together, so the jacket stays over the shirt and the tie).
fn lift_suit(fig: &mut Figure) {
    let n = fig.pos.len();
    let mut suit = vec![false; n];
    let mut nrm = vec![V3::default(); n];
    let w = |i: u16, fig: &Figure| fig.weld.get(i as usize).map_or(i as usize, |&x| x as usize);
    for (k, t) in fig.tris.iter().enumerate() {
        let [a, b, c] = t.map(|i| fig.pos[i as usize]);
        let f = (b - a).cross(c - a);
        for &i in t {
            nrm[w(i, fig)] = nrm[w(i, fig)] + f;
        }
        if matches!(fig.stuff[k], Stuff::Wool | Stuff::Shirt | Stuff::Tie) {
            for &i in t {
                suit[i as usize] = true;
            }
        }
    }
    for (i, (p, &on)) in fig.pos.iter_mut().zip(suit.iter()).enumerate() {
        if on {
            let wi = fig.weld.get(i).map_or(i, |&x| x as usize);
            *p = *p + nrm[wi].norm() * SUIT_LIFT;
        }
    }
}

/// How far the suit rises off the skin (cm).
const SUIT_LIFT: f32 = 0.12;

/// r42:arcus-hand — each figure's hands scaled about the wrist (hers, his): the review
/// found his hand, wrist crease to fingertip, longer than his face (a man's is about as
/// long). `hand_rig.py::HAND_SCALE` is the same.
/// (r42:arcus-hands: hers 1.08 — wrist crease to middle fingertip 15.5 cm, 0.095 of her
/// 162.8 cm; a woman's hand is about 0.105 of her height: a small fist at the neck.)
pub const HAND_SCALE: [f32; 2] = [1.08, 0.93];

/// r42:arcus-hand — each figure's palm drawn in across toward the hand's middle line (hers,
/// his): the owner found her hand "tosca", the palm bulging out of a slim wrist (her palm
/// 1.25–1.64 times her wrist from the picture's camera; the photographs' about 1.25). The
/// fingers' bases come in with it. `hand_rig.py::PALM_SLIM` is the same.
/// (r42:arcus-hands, pass 6 — kept, though it draws her hand's breadth across the knuckles
/// to 6.4 cm, ANSUR II's 0th percentile for her stature (7.7 ± 0.34; his 8.1, the 4th):
/// 0.99 and 1.04 brought both to the middle half, but the slim factor moves the thumb's base
/// out with the fingers', and the hands' outline at the middle stood 1.8–2.1 times the wrist
/// from the camera (`arcus_hand_table`'s `PALM_WIDE`, the photographs' 1.1–1.3: the owner's
/// "tosca"). Widening the knuckles alone is open, `hand_anthro.py` measures it.)
pub const PALM_SLIM: [f32; 2] = [0.86, 1.0];

fn slim_palm(fig: &mut Figure, s: usize, k: f32) {
    let o = side(s);
    let wr = fig.bones[o + HAND].head;
    let across = (fig.bones[o + finger(1, 0)].head - fig.bones[o + finger(4, 0)].head).norm();
    let mut deltas = [V3::default(); 5];
    for (f, d) in deltas.iter_mut().enumerate() {
        let h0 = fig.bones[o + finger(f, 0)].head;
        *d = across * (-(h0 - wr).dot(across) * (1.0 - k));
        for kk in 0..3 {
            let b = &mut fig.bones[o + finger(f, kk)];
            b.head = b.head + *d;
            b.tail = b.tail + *d;
        }
    }
    for (p, w) in fig.pos.iter_mut().zip(fig.bw.iter()) {
        let mut mv = V3::default();
        for &(b, x) in w {
            let b = b as usize;
            if x <= 0.0 {
                continue;
            }
            if b == o + HAND {
                mv = mv + across * (-(*p - wr).dot(across) * (1.0 - k) * x);
            } else if (o + finger(0, 0)..=o + finger(4, 2)).contains(&b) {
                let f = (b - o - finger(0, 0)) / 3;
                mv = mv + deltas[f] * x;
            }
        }
        *p = *p + mv;
    }
}

/// r42:arcus-polish — the first web space and the thenar made a hand's, not a mitten's (the
/// owner, 2026-11-25, sheet v10's left hand: "aún sale algo tosca o gorda"; the coordinator:
/// one big convex lobe from under the neck to the wrist, "like a mitten's thumb pocket"). The
/// app's own render with each corner coloured by its bone showed the lobe was the hand
/// bone's skin on the thumb's side of the index's metacarpal: MakeHuman weights the web and
/// the thenar to the wrist and the metacarpals, so when the thumb goes under the neck that
/// flesh stays out. Each such vertex (its weight mostly the hand's, past the index's line
/// toward the thumb by `r`, between the wrist and the knuckles) is drawn in across by
/// [`WEB_IN`] of `r`, and hands [`WEB_TO_THUMB`] of its hand weight (by how far out it lies)
/// to the thumb's first bone, so the thenar goes with the thumb. The bones do not move.
/// `tools/arcus-figures/hand_rig.py::thumb_web` does the same for the table's tools.
pub const WEB_IN: f32 = 0.45;
pub const WEB_TO_THUMB: f32 = 0.6;
/// How much of its overhang past the little finger's line the palm's ulnar edge keeps
/// drawn in.
pub const EDGE_IN: f32 = 0.5;

fn thumb_web(fig: &mut Figure, s: usize) {
    let o = side(s);
    let wr = fig.bones[o + HAND].head;
    let (ix, lt) = (
        fig.bones[o + finger(1, 0)].head,
        fig.bones[o + finger(4, 0)].head,
    );
    let along_end = (fig.bones[o + finger(2, 0)].head + fig.bones[o + finger(3, 0)].head) * 0.5;
    let along = (along_end - wr).norm();
    let len = (along_end - wr).len().max(1e-3);
    let across = {
        let a = ix - lt;
        (a - along * a.dot(along)).norm()
    };
    let line = (ix - wr).dot(across);
    let ulnar = (lt - wr).dot(across);
    let hb = (o + HAND) as u8;
    let tb = (o + finger(0, 0)) as u8;
    for (p, w) in fig.pos.iter_mut().zip(fig.bw.iter_mut()) {
        let wh: f32 = w.iter().filter(|e| e.0 == hb).map(|e| e.1).sum();
        if wh < 0.4 {
            continue;
        }
        let d = *p - wr;
        let t = d.dot(along) / len;
        // The palm's other edge, past the little finger's line: the hypothenar and the
        // heel drawn in too (seen from the audience it is the hand's edge toward the
        // violin's body — the lobe the bone-coloured render showed between the fingers'
        // bases and the wrist).
        let ru = ulnar - d.dot(across);
        // (Not at the wrist's crease: drawn in there, the wrist read pinched under the palm
        // — arcus_hand_table's palm-to-wrist rule.)
        if ru > 0.0 && (0.1..1.05).contains(&t) {
            let ramp = smooth((t - 0.12) / 0.3) * (1.0 - smooth((t - 0.8) / 0.3));
            *p = *p + across * (ru * EDGE_IN * ramp * wh);
            continue;
        }
        let r = d.dot(across) - line;
        if r <= 0.0 || !(-0.1..1.05).contains(&t) {
            continue;
        }
        // Fading in from the wrist and out at the knuckles.
        let ramp = smooth((t + 0.1) / 0.3) * (1.0 - smooth((t - 0.75) / 0.3));
        *p = *p - across * (r * WEB_IN * ramp * wh);
        let give = wh * WEB_TO_THUMB * ramp * smooth(r / 1.5);
        if give < 1e-4 {
            continue;
        }
        let mut all: Vec<(u8, f32)> = w.iter().copied().filter(|e| e.1 > 0.0).collect();
        for e in all.iter_mut() {
            if e.0 == hb {
                e.1 -= give;
            }
        }
        match all.iter_mut().find(|e| e.0 == tb) {
            Some(e) => e.1 += give,
            None => all.push((tb, give)),
        }
        all.retain(|e| e.1 > 1e-6);
        all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(4);
        let tot: f32 = all.iter().map(|e| e.1).sum();
        let mut out = [(0u8, 0.0f32); 4];
        for (k, e) in all.iter().enumerate() {
            out[k] = (e.0, e.1 / tot);
        }
        *w = out;
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn knuckle_fix(fig: &mut Figure, scale: f32, slim: f32) {
    if fig.bones.len() < BONES {
        return;
    }
    for s in 0..2 {
        if slim != 1.0 {
            slim_palm(fig, s, slim);
        }
        let o = side(s);
        let wr = fig.bones[o + HAND].head;
        if scale != 1.0 {
            let hand = (o + HAND) as u8..=(o + finger(4, 2)) as u8;
            for (p, w) in fig.pos.iter_mut().zip(fig.bw.iter()) {
                let share: f32 = w
                    .iter()
                    .filter(|(b, _)| hand.contains(b))
                    .map(|x| x.1)
                    .sum::<f32>()
                    .clamp(0.0, 1.0);
                let f = 1.0 - (1.0 - scale) * share;
                *p = wr + (*p - wr) * f;
            }
            for b in o + HAND..=o + finger(4, 2) {
                let bn = &mut fig.bones[b];
                bn.head = wr + (bn.head - wr) * scale;
                bn.tail = wr + (bn.tail - wr) * scale;
            }
        }
        let mut heads = [V3::default(); 4];
        let mut axes = [V3::default(); 4];
        for f in 0..4 {
            let b = o + finger(f + 1, 0);
            let h = fig.bones[b].head;
            let ax = (h - wr).norm();
            let nh = h - ax * ((h - wr).len() * KNUCKLE_BACK);
            fig.bones[b].head = nh;
            heads[f] = nh;
            axes[f] = ax;
        }
        let hb = (o + HAND) as u8;
        for (i, w) in fig.bw.iter_mut().enumerate() {
            let Some(j) = w.iter().position(|&(b, x)| b == hb && x > 0.0) else {
                continue;
            };
            let wh = w[j].1;
            let p = fig.pos[i];
            let d = p - wr;
            let mut c = [0.0f32; 4];
            for f in 0..4 {
                let t = d.dot(axes[f]);
                let lat = (d - axes[f] * t).len();
                c[f] = (-(lat / KNUCKLE_LANE).powi(2)).exp();
            }
            let sum: f32 = c.iter().sum();
            if sum < 1e-6 {
                continue;
            }
            let mut mv = [0.0f32; 4];
            for f in 0..4 {
                let u = (p - heads[f]).dot(axes[f]);
                let r = ((u - KNUCKLE_RAMP.0) / (KNUCKLE_RAMP.1 - KNUCKLE_RAMP.0)).clamp(0.0, 1.0);
                mv[f] = wh * c[f] / sum * r * r * (3.0 - 2.0 * r);
            }
            let total: f32 = mv.iter().sum();
            if total < 1e-6 {
                continue;
            }
            // Up to nine bones, then the four heaviest kept, renormalised.
            let mut all: Vec<(u8, f32)> = w.iter().copied().filter(|&(_, x)| x > 0.0).collect();
            for e in all.iter_mut() {
                if e.0 == hb {
                    e.1 -= total;
                }
            }
            for (f, m) in mv.iter().enumerate() {
                let b = (o + finger(f + 1, 0)) as u8;
                match all.iter_mut().find(|e| e.0 == b) {
                    Some(e) => e.1 += m,
                    None => all.push((b, *m)),
                }
            }
            all.retain(|e| e.1 > 1e-6);
            all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            all.truncate(4);
            let tot: f32 = all.iter().map(|e| e.1).sum();
            let mut out = [(0u8, 0.0f32); 4];
            for (k, e) in all.iter().enumerate() {
                out[k] = (e.0, e.1 / tot);
            }
            *w = out;
        }
    }
}

struct Rd<'a> {
    b: &'a [u8],
    o: usize,
}

impl Rd<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.b.get(self.o..self.o + N)?;
        self.o += N;
        s.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take()?))
    }
    fn f16(&mut self) -> Option<f32> {
        Some(half(u16::from_le_bytes(self.take()?)))
    }
    fn v3(&mut self) -> Option<V3> {
        Some(v(self.f32()?, self.f32()?, self.f32()?))
    }
}

/// An IEEE half float.
fn half(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    match e {
        0 => s * m * (2.0f32).powi(-24),
        31 => s * f32::MAX,
        _ => s * (1.0 + m / 1024.0) * (2.0f32).powi(e - 15),
    }
}

/// A `.fig` file (`build_figures.py::write_fig`).
pub fn parse(b: &[u8]) -> Option<Figure> {
    let mut r = Rd { b, o: 0 };
    if r.take::<4>()? != *b"CFIG" || r.u16()? != 1 {
        return None;
    }
    let mut f = Figure::default();
    let nb = r.u16()? as usize;
    for _ in 0..nb {
        let p = r.i16()?;
        f.bones.push(Bone {
            parent: (p >= 0).then_some(p as usize),
            head: r.v3()?,
            tail: r.v3()?,
            x: r.v3()?,
        });
    }
    let nm = r.u16()? as usize;
    for _ in 0..nm {
        let l = r.u8()? as usize;
        let name = std::str::from_utf8(r.b.get(r.o..r.o + l)?)
            .ok()?
            .to_string();
        r.o += l;
        f.marks.push((name, r.v3()?));
    }
    let nv = r.u32()? as usize;
    let mut strand = Vec::with_capacity(nv);
    for _ in 0..nv {
        let p = v(
            r.i16()? as f32 * 0.01,
            r.i16()? as f32 * 0.01,
            r.i16()? as f32 * 0.01,
        );
        let ids = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
        let ws = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
        let lum_b = r.u8()?;
        f.line.push((lum_b as f32 - 128.0) * 0.5);
        let lum = 0.35 + lum_b as f32 / 255.0;
        let red = r.u8()? as i8 as f32 / 100.0;
        let uv = [r.f16()?, r.f16()?];
        f.pos.push(p);
        f.bw.push(std::array::from_fn(|k| (ids[k], ws[k] as f32 / 255.0)));
        f.detail.push([lum, red]);
        strand.push(uv);
    }
    let nt = r.u32()? as usize;
    for _ in 0..nt {
        let t = [r.u16()?, r.u16()?, r.u16()?];
        if t.iter().any(|&i| i as usize >= nv) {
            return None;
        }
        f.tris.push(t);
        f.stuff.push(Stuff::of(r.u8()?));
        f.part.push(r.u8()?);
    }
    // A hair vertex carries its strand coordinates instead of the skin's detail.
    for (t, s) in f.tris.iter().zip(&f.stuff) {
        if matches!(
            *s,
            Stuff::Hair
                | Stuff::CardHair
                | Stuff::CardBrow
                | Stuff::CardLash
                | Stuff::Shell
                | Stuff::Scalp
        ) {
            for &i in t {
                f.detail[i as usize] = strand[i as usize];
            }
        }
    }
    f.uv = strand;
    f.weld = weld(&f.pos);
    Some(f)
}

/// r42:arcus-upper — for each point the first point at the same place (to 0.01 cm).
fn weld(pos: &[V3]) -> Vec<u32> {
    let mut first: std::collections::HashMap<[i32; 3], u32> = Default::default();
    pos.iter()
        .enumerate()
        .map(|(i, p)| {
            let k = [p.x, p.y, p.z].map(|c| (c * 100.0).round() as i32);
            *first.entry(k).or_insert(i as u32)
        })
        .collect()
}

impl Figure {
    /// A named point at rest.
    pub fn mark(&self, name: &str) -> V3 {
        self.marks
            .iter()
            .find(|(n, _)| n == name)
            .map(|m| m.1)
            .unwrap_or_default()
    }
}

/// A figure's rig: moved to `figure::rig` (ADR 0159), named here as before.
pub use super::figure::rig::{Aim, Rot, Xf};

/// Every bone posed.
#[derive(Clone, Debug)]
pub struct Posed {
    pub xf: [Xf; BONES],
}

impl Bone {
    pub fn len(&self) -> f32 {
        (self.tail - self.head).len()
    }
    fn frame(&self) -> Rot {
        Rot::frame(self.x, self.tail - self.head)
    }
}

/// The figure posed by `aims` (a bone without one follows its parent rigidly; the root
/// without one turns by `root`).
pub fn pose(fig: &Figure, aims: &[Option<Aim>; BONES], root: &Rot, off: V3) -> Posed {
    let mut xf = [Xf::default(); BONES];
    for b in 0..fig.bones.len().min(BONES) {
        xf[b] = step(fig, b, &xf, aims[b], root, off);
    }
    Posed { xf }
}

/// Bone `b` posed by `aim` (or carried by its parent, already posed in `xf`).
fn step(fig: &Figure, b: usize, xf: &[Xf; BONES], aim: Option<Aim>, root: &Rot, off: V3) -> Xf {
    let bone = &fig.bones[b];
    let parent = bone.parent.map(|p| xf[p]);
    let carried = parent.map_or(root.apply(bone.head) + off, |p| p.apply(bone.head));
    let prot = parent.map_or(*root, |p| p.rot);
    let y0 = (bone.tail - bone.head).norm();
    match aim {
        None => Xf {
            head: bone.head,
            y0,
            k: 1.0,
            rot: prot,
            at: carried,
        },
        Some(a) => {
            let x = a.x.unwrap_or_else(|| prot.apply(bone.x));
            let fp = Rot::frame(x, a.y);
            Xf {
                head: bone.head,
                y0,
                k: a.len.map_or(1.0, |l| l / bone.len().max(1e-3)),
                rot: fp.mul(&bone.frame().t()),
                at: a.head.unwrap_or(carried),
            }
        }
    }
}

/// `base` with one arm (`s` 0 left, 1 right) posed through `a` instead (the live arms:
/// the body stays as baked). Nothing allocated.
pub fn repose_arm(fig: &Figure, base: &Posed, s: usize, a: &ArmAim) -> Posed {
    let mut aims: [Option<Aim>; BONES] = [None; BONES];
    let sh = shoulder(fig, base, s);
    arm_aims(fig, &mut aims, s, a, sh);
    let mut out = base.clone();
    let o = side(s);
    #[allow(clippy::needless_range_loop)] // each bone posed after its parent, in order
    for b in o + UPPER1..=o + finger(4, 2) {
        out.xf[b] = step(fig, b, &out.xf, aims[b], &Rot::I, V3::default());
    }
    out
}

impl Posed {
    /// A rest point carried by bone `b`.
    pub fn at(&self, b: usize, p: V3) -> V3 {
        self.xf[b].apply(p)
    }

    /// Everything moved by `d`.
    pub fn shift(&mut self, d: V3) {
        for x in &mut self.xf {
            x.at = x.at + d;
        }
    }

    /// Vertex `i` of `fig` posed.
    pub fn vertex(&self, fig: &Figure, i: usize) -> V3 {
        self.vertex_at(fig, i, fig.pos[i])
    }

    /// Vertex `i` of `fig` posed from `p` at rest (its own place, or a morph's).
    pub fn vertex_at(&self, fig: &Figure, i: usize, p: V3) -> V3 {
        let mut o = V3::default();
        for &(b, w) in &fig.bw[i] {
            if w > 0.0 {
                o = o + self.xf[(b as usize).min(BONES - 1)].apply(p) * w;
            }
        }
        o
    }
}

const UP: V3 = V3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// How far back of the stylised figure's place a realistic cellist or bassist sits (cm).
const SIT_BACK: f32 = 4.0;

/// How far a realistic cellist's head comes forward of the stylised figure's, leaning into
/// the instrument (cm), and how far the chin lifts (radians).
const LEAN_IN: f32 = 2.5;
const CHIN_UP: f32 = 0.1;

/// How far a violinist's or a violist's trunk turns to the left (radians).
const TRUNK_TURN: f32 = 0.2;

/// The figure's own turn: its left (+x at rest) to the skeleton's left, its front to the
/// skeleton's front.
pub fn facing(s: &super::bowed_figure::Skel) -> Rot {
    Rot {
        x: -s.r,
        y: UP,
        z: s.f,
    }
}

/// The joints the arms are posed through (world cm): the shoulder is the body's.
#[derive(Copy, Clone, Debug)]
pub struct ArmAim {
    pub elbow: V3,
    pub wrist: V3,
    /// The knuckles (index … little finger) and the hand's palm side (a unit vector out of
    /// the palm), or none (the hand carried by the forearm).
    pub hand: Option<HandAim>,
}

#[derive(Copy, Clone, Debug)]
pub struct HandAim {
    /// Each finger's knuckle, middle joint, last joint, tip (index … little finger).
    pub fingers: [[V3; 4]; 4],
    /// The thumb's base, knuckle, joint, tip.
    pub thumb: [V3; 4],
    /// The finger (index … little) whose tip must land exactly (the left hand's playing
    /// finger on its stop); the others are relaxed: their tips reach the pose's way at
    /// 70–86 % of their own length, so a smaller or a larger hand curls as a hand does
    /// instead of crumpling (sheet v7).
    pub exact: Option<usize>,
    /// r42:arcus-upper — the hand's and the finger bones' turns given outright (the neck
    /// hand's authored table, `figure::neck_hand`): the bones are set as they say, from the
    /// joints above, with no reach worked out.
    pub rots: Option<super::figure::neck_hand::HandRots>,
}

/// Set one arm's bones (`s` 0 left, 1 right) in `aims` through `a`, the shoulder where
/// the body carries it.
/// r42:arcus-hands — how far from its knuckle a finger that does not play may reach toward
/// its pose's tip, as shares of its length (a curled finger round a bow's stick reaches
/// about 0.6 of it; was 0.7).
pub const RELAX: (f32, f32) = (0.55, 0.86);

pub fn arm_aims(fig: &Figure, aims: &mut [Option<Aim>; BONES], s: usize, a: &ArmAim, sh: V3) {
    let o = side(s);
    let u = (a.elbow - sh).norm();
    let w = (a.wrist - a.elbow).norm();
    let mut zz = w - u * w.dot(u);
    if zz.len() < 1e-3 {
        zz = super::geo::basis(u).0;
    }
    let bend = u.cross(zz.norm()).norm();
    let up_len = (a.elbow - sh).len();
    let fo_len = (a.wrist - a.elbow).len();
    let rest_up = fig.bones[o + UPPER1].len() + fig.bones[o + UPPER2].len();
    let rest_fo = fig.bones[o + FORE1].len() + fig.bones[o + FORE2].len();
    let (ku, kf) = (up_len / rest_up.max(1e-3), fo_len / rest_fo.max(1e-3));
    aims[o + UPPER1] = Some(Aim {
        head: Some(sh),
        y: u,
        x: Some(bend),
        len: Some(fig.bones[o + UPPER1].len() * ku),
    });
    aims[o + UPPER2] = Some(Aim {
        head: Some(sh + u * (fig.bones[o + UPPER1].len() * ku)),
        y: u,
        x: Some(bend),
        len: Some(fig.bones[o + UPPER2].len() * ku),
    });
    // The hand's turn, and the forearm's twist toward it (shared out along the forearm).
    let hand_rot = a.hand.map(|h| {
        h.rots
            .map_or_else(|| hand_turn(fig, s, &h, a.wrist), |r| r.hand)
    });
    let twist = hand_rot.map_or(0.0, |r| {
        let q = r.apply(fig.bones[o + FORE2].x);
        let q = q - w * q.dot(w);
        if q.len() < 1e-4 {
            0.0
        } else {
            bend.cross(q).dot(w).atan2(bend.dot(q))
        }
    });
    let f1 = fig.bones[o + FORE1].len() * kf;
    // (r42:arcus-hand: the wrist's end of the forearm takes nearly all of the hand's turn,
    // as the radius carries the hand — at 0.7 the wrist lagged the hand by a third of its
    // turn, a pinched wrist under a hand that read swollen beside it.)
    for (k, b, head, share) in [
        (0, FORE1, a.elbow, 0.45),
        (1, FORE2, a.elbow + w * f1, 0.95),
    ] {
        let _ = k;
        aims[o + b] = Some(Aim {
            head: Some(head),
            y: w,
            x: Some(Rot::about(w, twist * share).apply(bend)),
            len: Some(fig.bones[o + b].len() * kf),
        });
    }
    if let (Some(h), Some(r)) = (a.hand, hand_rot) {
        let hb = &fig.bones[o + HAND];
        aims[o + HAND] = Some(Aim {
            head: Some(a.wrist),
            y: r.apply((hb.tail - hb.head).norm()),
            x: Some(r.apply(hb.x)),
            len: None,
        });
        // Each finger from its own knuckle where the turned hand carries it (the figure's
        // fingers keep their width and spacing) to the pose's fingertip, bent through a
        // two-bone reach toward the pose's middle joint; stretched only when the tip is out
        // of reach (the playing finger lands on its stop exactly). Each phalanx turned from
        // the hand's turn by the shortest way to its own (sheet v7: no twist — a finger's
        // roll left to the parent's turn crumpled the skin at the knuckles).
        if let Some(rots) = h.rots {
            // Every bone as the table turns it, its head where the chain carries it.
            for f in 0..5 {
                let j = if f == 0 { h.thumb } else { h.fingers[f - 1] };
                for k in 0..3 {
                    let bone = &fig.bones[o + finger(f, k)];
                    let rb = rots.bones[f * 3 + k];
                    aims[o + finger(f, k)] = Some(Aim {
                        head: Some(j[k]),
                        y: rb.apply((bone.tail - bone.head).norm()),
                        x: Some(rb.apply(bone.x)),
                        len: None,
                    });
                }
            }
            return;
        }
        let wr = hb.head;
        for f in 0..5 {
            let j = if f == 0 { h.thumb } else { h.fingers[f - 1] };
            let b0 = &fig.bones[o + finger(f, 0)];
            let (l0, l1, l2) = (
                b0.len(),
                fig.bones[o + finger(f, 1)].len(),
                fig.bones[o + finger(f, 2)].len(),
            );
            let k0 = a.wrist + r.apply(b0.head - wr);
            let tip = if f > 0 && h.exact != Some(f - 1) {
                let d = j[3] - k0;
                let full = l0 + l1 + l2;
                k0 + d.norm() * d.len().clamp(RELAX.0 * full, RELAX.1 * full)
            } else {
                j[3]
            };
            // r42:arcus-hands — a finger's three joints bend together toward its tip (the
            // last following the middle: `figure::fingers::curl`).
            if f > 0 {
                let c = super::figure::fingers::curl(k0, tip, j[1], [l0, l1, l2]);
                for k in 0..3 {
                    let (from, to) = (c[k], c[k + 1]);
                    let d = to - from;
                    let bone = &fig.bones[o + finger(f, k)];
                    let rest = r.apply((bone.tail - bone.head).norm());
                    aims[o + finger(f, k)] = Some(Aim {
                        head: Some(from),
                        y: d.norm(),
                        x: Some(Rot::between(rest, d.norm()).mul(&r).apply(bone.x)),
                        len: Some(d.len().max(0.2)),
                    });
                }
                continue;
            }
            let reach = (tip - k0).len();
            let sc = (reach / ((l0 + l1 + l2) * 0.97)).max(1.0);
            let (l0, l1, l2) = (l0 * sc, l1 * sc, l2 * sc);
            let pole = j[1] - (k0 + tip) * 0.5;
            let pip = knee(k0, tip, l0, l1 + l2, pole);
            let along = (tip - pip).norm();
            let bend = (pole - along * pole.dot(along)).norm();
            let dip = pip + along * ((tip - pip).len() * l1 / (l1 + l2)) + bend * (0.12 * sc);
            for (k, (from, to)) in [(k0, pip), (pip, dip), (dip, tip)].into_iter().enumerate() {
                let d = to - from;
                let bone = &fig.bones[o + finger(f, k)];
                let rest = r.apply((bone.tail - bone.head).norm());
                aims[o + finger(f, k)] = Some(Aim {
                    head: Some(from),
                    y: d.norm(),
                    x: Some(Rot::between(rest, d.norm()).mul(&r).apply(bone.x)),
                    len: Some(d.len().max(0.2)),
                });
            }
        }
    }
}

/// The hand's turn from rest: its wrist-to-knuckles way and the knuckles' line (index to
/// little finger) brought to the pose's.
fn hand_turn(fig: &Figure, s: usize, h: &HandAim, wrist: V3) -> Rot {
    let o = side(s);
    let kn = |f: usize| fig.bones[o + finger(f, 0)].head;
    let rest_mid = (kn(2) + kn(3)) * 0.5;
    let wr = fig.bones[o + HAND].head;
    let ry = (rest_mid - wr).norm();
    let rx = kn(1) - kn(4);
    let mid = (h.fingers[1][0] + h.fingers[2][0]) * 0.5;
    let py = (mid - wrist).norm();
    let px = h.fingers[0][0] - h.fingers[3][0];
    Rot::frame(px, py).mul(&Rot::frame(rx, ry).t())
}

/// A two-bone limb's middle joint (`elbow` with the realistic lengths).
fn knee(hip: V3, ankle: V3, a: f32, b: f32, pole: V3) -> V3 {
    super::figure::rig::two_bone(hip, ankle, a, b, pole)
}

/// Arcus's rig for the stylised skeleton `s` (ADR 0159: Arcus is the PlayRig's first
/// client): under a chin the body hangs from the jaw on the chinrest (leant 6° forward,
/// turned toward the scroll; the chair's height follows); behind a cello or a bass the
/// figure sits on the instrument's seat a little behind the stylised figure's place, the
/// head where the stylised head is, the knees and the ankles where it puts them; each arm
/// through `arms` (the live arms' pose) or resting (the left hand at the skeleton's wrist,
/// the bow hand in front of the chest to the right).
pub fn arcus_rig(
    s: &super::bowed_figure::Skel,
    held: bool,
    arms: [Option<ArmAim>; 2],
) -> super::figure::play::PlayRig {
    use super::figure::play::{Hand, Head, HeadAt, Legs, PlayRig, Seat};
    let (at, seat, legs) = if held {
        (
            HeadAt::Jaw(s.chin),
            Seat::FromHead {
                lean: 6.0f32.to_radians(),
                turn: TRUNK_TURN,
            },
            Legs::Forward,
        )
    } else {
        // Sat a little further back than the stylised figure (sheet v7: a realistic hand
        // is longer; with the cello's neck at the shoulder the elbow folded under 30°).
        let back = s.f * SIT_BACK;
        // Upright, leaning into the cello, the chin a little lifted (the owner's reference's
        // posture, 2026-11-24).
        (
            HeadAt::Eyes(s.head - back + s.f * LEAN_IN),
            Seat::Chair {
                at: s.pelvis - back,
                top: s.pelvis.y - 10.0 - back.y,
            },
            Legs::To {
                knees: s.knees,
                ankles: s.ankles,
            },
        )
    };
    let hand = |sd: usize| match arms[sd] {
        Some(a) => Hand::Aim(a),
        None => {
            let (wrist, pole) = match (sd, held) {
                (0, true) => (s.lw, s.down + s.r * 0.45 + s.f * 0.15),
                (0, false) => (s.lw, -s.r + s.down * 1.3),
                (_, true) => (s.chest + s.r * 14.0 + s.f * 24.0, s.down + s.r * 1.5),
                (_, false) => (
                    s.chest + s.r * 24.0 + s.f * 22.0 + s.down * 10.0,
                    s.r + s.down * 1.6,
                ),
            };
            Hand::Rest { wrist, pole }
        }
    };
    let (up, face) = if held {
        (s.hu, s.hf)
    } else {
        let (sn, cs) = CHIN_UP.sin_cos();
        (s.hu * cs - s.hf * sn, s.hf * cs + s.hu * sn)
    };
    PlayRig {
        facing: s.f,
        right: s.r,
        head: Head { at, up, face },
        seat,
        legs,
        hands: [hand(0), hand(1)],
        occluders: Vec::new(),
        // r42:arcus-polish3 — a violinist's shoulders (the bow hand's height, ADR 0159).
        shrug: if held { shrug() } else { [0.0; 2] },
    }
}

/// r42:arcus-polish3 — how much a realistic violinist's shoulders are raised (cm, left and
/// right): the bow shoulder carries the arm so the hand at the frog sits near the elbow's
/// height (the photographs, `tests/arcus_arm_pose.rs`).
pub const SHRUG: [f32; 2] = [0.0, 6.0];

/// [`SHRUG`], or `CANTUS_SHRUG="left,right"` for finding it (a tuning aid, read once).
pub fn shrug() -> [f32; 2] {
    use std::sync::OnceLock;
    static S: OnceLock<[f32; 2]> = OnceLock::new();
    *S.get_or_init(|| {
        std::env::var("CANTUS_SHRUG")
            .ok()
            .and_then(|s| {
                let v: Vec<f32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (v.len() == 2).then(|| [v[0], v[1]])
            })
            .unwrap_or(SHRUG)
    })
}

/// The figure posed for the skeleton `s` (the stylised figure's, posed from the
/// instrument) through Arcus's rig: every bone and the seat's height.
pub fn body(
    fig: &Figure,
    s: &super::bowed_figure::Skel,
    held: bool,
    arms: [Option<ArmAim>; 2],
) -> (Posed, f32) {
    super::figure::play::pose(fig, &arcus_rig(s, held, arms))
}

/// The shoulder (the upper arm's head) where the body carries it.
pub fn shoulder(fig: &Figure, p: &Posed, s: usize) -> V3 {
    p.at(side(s) + CLAV, fig.bones[side(s) + UPPER1].head)
}

/// The hand's length from the wrist to its knuckles' middle (side `s`), cm.
pub fn hand_reach(fig: &Figure, s: usize) -> f32 {
    let o = side(s);
    let b = &fig.bones;
    let mid = (b[o + finger(2, 0)].head + b[o + finger(3, 0)].head) * 0.5;
    (mid - b[o + HAND].head).len()
}

/// The rest lengths of an arm (upper, fore), cm.
pub fn arm_lengths(fig: &Figure, s: usize) -> (f32, f32) {
    let o = side(s);
    let b = &fig.bones;
    (
        b[o + UPPER1].len() + b[o + UPPER2].len(),
        b[o + FORE1].len() + b[o + FORE2].len(),
    )
}

/// Every vertex of `fig` posed, and smooth normals over all its triangles.
pub fn skin(fig: &Figure, p: &Posed) -> (Vec<V3>, Vec<V3>) {
    skin_with(fig, p, &[])
}

/// [`skin`] with offsets at rest added first (`morph`: vertex, offset — the closed lids).
pub fn skin_with(fig: &Figure, p: &Posed, morph: &[(u32, V3)]) -> (Vec<V3>, Vec<V3>) {
    let mut pts: Vec<V3> = (0..fig.pos.len()).map(|i| p.vertex(fig, i)).collect();
    for &(i, d) in morph {
        let i = i as usize;
        if let Some(q) = fig.pos.get(i) {
            pts[i] = p.vertex_at(fig, i, *q + d);
        }
    }
    let mut nrm = vec![V3::default(); pts.len()];
    let w = |i: usize| fig.weld.get(i).map_or(i, |&x| x as usize);
    for t in &fig.tris {
        let [a, b, c] = t.map(|i| i as usize);
        let n = (pts[b] - pts[a]).cross(pts[c] - pts[a]);
        for i in [a, b, c] {
            nrm[w(i)] = nrm[w(i)] + n;
        }
    }
    for i in 0..nrm.len() {
        nrm[i] = nrm[w(i)];
    }
    for n in &mut nrm {
        *n = n.norm();
    }
    (pts, nrm)
}

/// Which realistic figure a holder names, if one.
pub fn who(h: super::bowed_hold::Hold) -> Option<Who> {
    match h {
        super::bowed_hold::Hold::Woman => Some(Who::Woman),
        super::bowed_hold::Hold::Man => Some(Who::Man),
        _ => None,
    }
}

/// The scene's material for a figure's stuff (`tone` the skin's).
pub fn role(st: Stuff, who: Who, tone: u8) -> super::bowed_mesh::Role {
    use super::bowed_mesh::Role;
    match st {
        Stuff::Skin | Stuff::Lips => Role::Skin(tone),
        Stuff::Hair if who == Who::Woman => Role::HairFair,
        Stuff::Hair => Role::Hair,
        Stuff::Velvet => Role::Velvet,
        Stuff::Wool => Role::Wool,
        Stuff::Shirt => Role::Shirt,
        Stuff::Tie => Role::Tie,
        Stuff::Shoes => Role::Shoes,
        Stuff::Eye => Role::Eye,
        Stuff::Iris => Role::Iris,
        Stuff::Lash => Role::Lash,
        Stuff::Nail => Role::Nail(tone),
        Stuff::Jewel => Role::Brass,
        Stuff::CardHair => Role::Card(who as u8 * 3),
        Stuff::CardBrow => Role::Card(who as u8 * 3 + 1),
        Stuff::CardLash => Role::Card(who as u8 * 3 + 2),
        Stuff::Shell => Role::Shell(who as u8),
        Stuff::Scalp => Role::Scalp(who as u8),
    }
}

/// r42:arcus-hand — whether a band triangle (in the body and an arm) lies where the live
/// arm draws it opaque: every corner at least [`ARM_COVERS`] the arm's (the live arm's fade
/// is whole from 0.38).
fn covered_by_arm(fig: &Figure, t: &[u16; 3], part: u8) -> bool {
    let s = if part & 2 != 0 { 0 } else { 1 };
    let o = side(s);
    t.iter().all(|&i| {
        fig.bw[i as usize]
            .iter()
            .filter(|(b, _)| (*b as usize) >= o + UPPER1 && (*b as usize) <= o + finger(4, 2))
            .map(|x| x.1)
            .sum::<f32>()
            >= ARM_COVERS
    })
}

/// How much of a corner's weight is its arm's for the live arm to draw it opaque (its fade
/// is whole from 0.38; r42:arcus-polish2: the bake leaves out every bow-arm band triangle
/// from there — the live band is the bake's surface below it, `arcus_draw_flesh`'s `live`).
pub const ARM_COVERS: f32 = 0.38;

/// The triangles of `fig` in any of the parts `mask` names, posed as `pts`/`nrm`, added
/// to `m` a role each.
#[allow(clippy::too_many_arguments)]
pub fn emit(
    m: &mut super::bowed_mesh::Mesh,
    fig: &Figure,
    pts: &[V3],
    nrm: &[V3],
    who: Who,
    tone: u8,
    mask: u8,
    lum: &[(u32, f32)],
) {
    // r42:arcus-polish2 — per vertex, a luminance factor on the skin's detail (the lids').
    let mut shade = vec![1.0f32; if lum.is_empty() { 0 } else { fig.pos.len() }];
    for &(i, k) in lum {
        if let Some(s) = shade.get_mut(i as usize) {
            *s *= k;
        }
    }
    use super::bowed_mesh::{Role, Tris};
    let mut out: Vec<(Role, Tris, Vec<u32>)> = Vec::new();
    for (k, t) in fig.tris.iter().enumerate() {
        if fig.part[k] & mask == 0 {
            continue;
        }
        // r42:arcus-hand — the baked body leaves out the band triangles its live arm covers
        // whole (each corner the arm's): baked in the bake's arm pose, they stuck out as dark
        // shards beside the jacket where the live arm had moved on (the review, his jacket
        // under the bow arm from the new camera).
        // (The bow arm's band only: the left arm's passes behind the instrument, where the
        // picture is stamped back over the live arm — left out of the bake it left holes.)
        if mask == 1 && fig.part[k] & 4 != 0 && covered_by_arm(fig, t, fig.part[k]) {
            continue;
        }
        // (And a band triangle the bake's arm pose stretched well past its size at rest:
        // the body's cloth dragged toward an arm the bake does not draw.)
        if mask == 1 && fig.part[k] & 4 != 0 {
            let [a, b, c] = t.map(|i| i as usize);
            let stretch = [(a, b), (b, c), (c, a)]
                .iter()
                .map(|&(i, j)| (pts[i] - pts[j]).len() / (fig.pos[i] - fig.pos[j]).len().max(0.05))
                .fold(0.0f32, f32::max);
            if stretch > 1.6 {
                continue;
            }
        }
        let r = role(fig.stuff[k], who, tone);
        let e = match out.iter().position(|(x, _, _)| *x == r) {
            Some(i) => i,
            None => {
                out.push((r, Tris::default(), vec![u32::MAX; fig.pos.len()]));
                out.len() - 1
            }
        };
        let (_, tr, map) = &mut out[e];
        let idx = t.map(|i| {
            let i = i as usize;
            if map[i] == u32::MAX {
                map[i] = tr.pts.len() as u32;
                tr.pts.push(pts[i]);
                tr.nrm.push(nrm[i]);
                // r42:arcus-polish — a cloth's corner: its distance to the cloth's edge (the
                // roll of a neckline, a cuff, a lapel), its folds' coordinates as its uv.
                let cloth = matches!(r, Role::Velvet | Role::Wool);
                // (r42:arcus-polish2: a shirt's or a tie's corner: its luminance — the shade
                // under a collar's edge — and its edge's distance, `Tex::Shirt`.)
                let shirting = matches!(r, Role::Shirt | Role::Tie);
                tr.detail.push(if cloth {
                    [fig.edge.get(i).copied().unwrap_or(9.0), 0.0]
                } else if shirting {
                    [fig.detail[i][0], fig.edge.get(i).copied().unwrap_or(9.0)]
                } else {
                    let d = fig.detail[i];
                    [d[0] * shade.get(i).copied().unwrap_or(1.0), d[1]]
                });
                tr.uv.push(match r {
                    // (None carried: no fold, along the height at rest — what a fold's
                    // run is laid along, so a fold's corner and its plain neighbour blend.)
                    _ if cloth => match fig.uv.get(i) {
                        Some(&uv) if uv[0] >= 10.0 => uv,
                        _ => [10.0, fig.pos[i].y],
                    },
                    // (r42:arcus-hair: an eye's place in its texture too.)
                    Role::Skin(_) | Role::Eye | Role::Iris => {
                        fig.uv.get(i).copied().unwrap_or([-1.0, -1.0])
                    }
                    // (r42:arcus-hair: a hair card's hairline, flagged by its second.)
                    Role::Card(k) if k % 3 == 0 => [fig.line[i], 1.0],
                    _ => [-1.0, -1.0],
                });
            }
            map[i]
        });
        tr.idx.push(idx);
    }
    m.tris.extend(out.into_iter().map(|(r, t, _)| (r, t)));
}

/// The realistic figure `who` posed for `b` in `pose`, its chair (a violinist's), its
/// body baked into `m` (its arms are the live picture's) and the woman's long skirt.
pub fn build(
    m: &mut super::bowed_mesh::Mesh,
    b: super::bowed::Bowed,
    pose: &super::bowed::Pose,
    who: Who,
    tone: u8,
    eyes: super::figure::Eyes,
) {
    let s = super::bowed_figure::skeleton(b, pose);
    let fig = figure(who);
    let held = b.dims().held;
    let (posed, seat) = body(fig, &s, held, [None, None]);
    if held {
        let hips = posed.at(HIPS, fig.bones[HIPS].head);
        let at = v(hips.x, 0.0, hips.z) - s.f * 6.0;
        // r42:arcus-polish3 — a concert chair (the walnut one read as a stool).
        super::bowed_props::concert_chair(m, at, s.f.x.atan2(s.f.z), seat);
    }
    // r42:arcus-hand — the eyes closed (the default) or open.
    let morph = match eyes {
        super::figure::Eyes::Closed => lids(who),
        super::figure::Eyes::Open => &[],
    };
    let (pts, nrm) = skin_with(fig, &posed, morph);
    let lum = lid_shade(who, &pts, eyes);
    emit(m, fig, &pts, &nrm, who, tone, 1, &lum);
    if who == Who::Man {
        tie_knot(m, fig, &pts, &nrm);
    } else {
        wisps(m, fig, &posed, &pts);
    }
    // What the floor's shadow takes: the body's outline points.
    m.shadows.push(
        pts.iter()
            .step_by(97)
            .copied()
            .filter(|p| p.y < 60.0)
            .collect(),
    );
    if who == Who::Woman {
        super::bowed_human_skirt::skirt(m, fig, &posed, &s);
    }
}

/// r42:arcus-polish2 — the eyelids' shade (the owner's list: "the eyelid crease and lash
/// shadows for the closed eyes, serene and concentrated"): a closed lid in a photograph
/// shows a soft crease where it folds under the brow, a dark line where the lashes root,
/// and the lashes' soft shadow on the lid below them. From the lids' morph (what moves
/// closes the lid): its first moving band is the crease, its most moved points the margin;
/// with the eyes closed, the skin just under the margin (posed) takes the lashes' shadow.
/// (Open, the crease and the lash line stay — an open eye has them too.) Factors on the
/// skin's luminance detail, per vertex.
fn lid_shade(who: Who, pts: &[V3], eyes: super::figure::Eyes) -> Vec<(u32, f32)> {
    let morph = lids(who);
    let most = morph.iter().map(|m| m.1.len()).fold(0.0f32, f32::max);
    if most < 1e-4 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut margin = Vec::new();
    for &(i, d) in morph {
        let k = d.len() / most;
        // The crease: a soft dip where the lid starts to move (a band, not a line).
        let crease = (-((k - LID_CREASE.0) / LID_CREASE.1).powi(2)).exp();
        // The lash line: the margin's last few millimetres darker.
        let lash = smooth_k((k - 0.78) / 0.2);
        let f = (1.0 - LID_DEPTH.0 * crease) * (1.0 - LID_DEPTH.1 * lash);
        if f < 0.999 {
            out.push((i, f));
        }
        if k > 0.85 {
            margin.push(i as usize);
        }
    }
    if eyes == super::figure::Eyes::Closed && !margin.is_empty() {
        // The lashes' shadow just under the margin (the lower lid and the cheek's top).
        let moved: std::collections::HashSet<u32> = morph.iter().map(|m| m.0).collect();
        let (lo, hi) = margin.iter().fold(
            (
                V3 {
                    x: f32::MAX,
                    y: f32::MAX,
                    z: f32::MAX,
                },
                V3 {
                    x: f32::MIN,
                    y: f32::MIN,
                    z: f32::MIN,
                },
            ),
            |(a, b), &i| {
                let p = pts[i];
                (
                    V3 {
                        x: a.x.min(p.x),
                        y: a.y.min(p.y),
                        z: a.z.min(p.z),
                    },
                    V3 {
                        x: b.x.max(p.x),
                        y: b.y.max(p.y),
                        z: b.z.max(p.z),
                    },
                )
            },
        );
        let reach = LASH_SHADOW.1;
        for (i, p) in pts.iter().enumerate() {
            if moved.contains(&(i as u32))
                || p.x < lo.x - reach
                || p.x > hi.x + reach
                || p.y < lo.y - reach
                || p.y > hi.y + reach
                || p.z < lo.z - reach
                || p.z > hi.z + reach
            {
                continue;
            }
            let d = margin
                .iter()
                .filter(|&&j| pts[j].y >= p.y - 0.05)
                .map(|&j| (pts[j] - *p).len())
                .fold(f32::MAX, f32::min);
            if d < reach {
                let k = 1.0 - LASH_SHADOW.0 * (1.0 - d / reach).powf(1.5);
                out.push((i as u32, k));
            }
        }
    }
    out
}

/// Where on the lid (its share of the most it moves) the crease lies and how wide it is,
/// how deep the crease's and the lash line's shade go.
const LID_CREASE: (f32, f32) = (0.12, 0.1);
const LID_DEPTH: (f32, f32) = (0.16, 0.28);
/// How deep the lashes' shadow under a closed lid is, and how far down it reaches (cm).
const LASH_SHADOW: (f32, f32) = (0.22, 0.4);

/// r42:arcus-polish2 — her loose hairs (the owner's list: "a few flyaways and baby hairs at
/// the hairline"; every photograph of a violinist with tied hair shows them against the
/// light): short fine baby hairs springing from the front of her hairline and combed back
/// with the rest, lifting a little off it, and a few longer flyaways leaving the crown in
/// soft arcs. Rods finer than a pixel (the bake draws them as faint even lines, lit as hair).
fn wisps(m: &mut super::bowed_mesh::Mesh, fig: &Figure, posed: &Posed, pts: &[V3]) {
    use super::bowed_mesh::{Rod, Role};
    let eyes = (fig.mark("eye.L") + fig.mark("eye.R")) * 0.5;
    let at = |q: V3| posed.at(HEAD, q);
    let e = at(eyes);
    let up = (at(eyes + v(0.0, 1.0, 0.0)) - e).norm();
    let fwd = (at(eyes + v(0.0, 0.0, 1.0)) - e).norm();
    // The head's middle: behind the eyes and a little over them.
    let mid = e - fwd * 8.5 + up * 2.5;
    // The hair cards' vertices at the hairline and over the crown, posed.
    let mut line = Vec::new();
    let mut crown = Vec::new();
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        if *s != Stuff::CardHair {
            continue;
        }
        for &i in t {
            let i = i as usize;
            let l = fig.line.get(i).copied().unwrap_or(99.0);
            let p = pts[i];
            if (-0.5..1.5).contains(&l) && (p - e).dot(fwd) > -6.0 {
                line.push(i);
            } else if l > 25.0 && (p - mid).dot(up) > 4.0 {
                crown.push(i);
            }
        }
    }
    line.sort_unstable();
    line.dedup();
    crown.sort_unstable();
    crown.dedup();
    let hash = |i: usize, k: u32| {
        let x = (i as u32)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(k.wrapping_mul(40_503));
        ((x ^ (x >> 15)).wrapping_mul(2_246_822_519) >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut hair = |p0: V3, len: f32, lift: f32, sway: f32, r: f32| {
        let n = (p0 - mid).norm();
        // Back over the head and a little up, along its surface.
        let way = fwd * -1.0 + up * 0.35;
        let way = (way - n * way.dot(n)).norm();
        let side = n.cross(way).norm();
        let pt = |t: f32| {
            p0 + way * (len * t)
                + n * (lift * (std::f32::consts::PI * t * 0.8).sin() + 0.04)
                + side * (sway * t * t)
        };
        const SEGS: usize = 5;
        for k in 0..SEGS {
            let (a, b) = (k as f32 / SEGS as f32, (k + 1) as f32 / SEGS as f32);
            m.rods.push((
                Role::Wisp,
                Rod {
                    a: pt(a),
                    b: pt(b),
                    r,
                },
            ));
        }
    };
    // Baby hairs: one in every few of the hairline's points, short.
    for (k, &i) in line.iter().enumerate() {
        if hash(i, 1) > WISPS.0 || k % 2 == 1 {
            continue;
        }
        let len = 0.7 + 1.0 * hash(i, 2);
        hair(
            pts[i],
            len,
            0.12 + 0.18 * hash(i, 3),
            (hash(i, 4) - 0.5) * 0.5,
            WISP_R,
        );
    }
    // Flyaways: a few longer arcs off the crown.
    let mut n = 0;
    for &i in &crown {
        if n >= WISPS.1 || hash(i, 5) > 0.04 {
            continue;
        }
        n += 1;
        let len = 2.5 + 2.5 * hash(i, 6);
        hair(
            pts[i],
            len,
            0.5 + 0.7 * hash(i, 7),
            (hash(i, 8) - 0.5) * 1.6,
            WISP_R * 0.85,
        );
    }
}

/// How many of her hairline's points take a baby hair (a share of every other one), and how
/// many flyaways leave her crown at most; a loose hair's radius (cm: a little over a real
/// hair's 0.004, so it holds a share of a pixel at 200 %).
const WISPS: (f32, usize) = (0.35, 6);
const WISP_R: f32 = 0.012;

/// r42:arcus-polish2 — his tie's knot (the owner's list: "the tie knot with folds"): the
/// suit's tie is a painted region of its shirt, flat, and its top pinched to a point under
/// the collar. A four-in-hand knot stands off the shirt: an inverted trapezoid about 3.6 cm
/// wide under the collar and 2.4 at its foot, 3.4 tall, rounded across and fullest near its
/// top, with the diagonal fold the wrap leaves across its front and the blade coming out
/// from under it. Built over the tie's top where it is posed (`pts`, `nrm`), in the tie's
/// silk.
fn tie_knot(m: &mut super::bowed_mesh::Mesh, fig: &Figure, pts: &[V3], nrm: &[V3]) {
    use super::bowed_mesh::{Role, Tris};
    let mut tie: Vec<usize> = fig
        .tris
        .iter()
        .zip(&fig.stuff)
        .filter(|(_, s)| **s == Stuff::Tie)
        .flat_map(|(t, _)| t.iter().map(|&i| i as usize))
        .collect();
    tie.sort_unstable();
    tie.dedup();
    if tie.len() < 8 {
        return;
    }
    let top = tie.iter().map(|&i| fig.pos[i].y).fold(f32::MIN, f32::max);
    let mean = |lo: f32, hi: f32| {
        let (mut p, mut n, mut k) = (V3::default(), V3::default(), 0.0f32);
        for &i in &tie {
            let y = top - fig.pos[i].y;
            if (lo..=hi).contains(&y) {
                p = p + pts[i];
                n = n + nrm[i];
                k += 1.0;
            }
        }
        (k > 0.0).then(|| (p * (1.0 / k), n.norm()))
    };
    let (Some((head, n0)), Some((foot, _))) = (mean(0.0, 1.2), mean(8.0, 12.0)) else {
        return;
    };
    let n = n0;
    let up = {
        let u = head - foot;
        (u - n * u.dot(n)).norm()
    };
    let right = up.cross(n).norm();
    // The knot's middle a little under the tie's top (the collar's points come over its
    // upper corners).
    let c = head + up * (0.3 - KNOT_H * 0.5);
    const N: usize = 12;
    let at = |u: f32, v: f32| {
        // u across (-1…1), v up the knot (0 its foot … 1 under the collar).
        let w = KNOT_W.0 + (KNOT_W.1 - KNOT_W.0) * v;
        let round = (1.0 - u.abs().powf(2.2)).max(0.0).powf(0.55);
        let full = 0.45 + 0.55 * smooth_k((v - 0.05) / 0.4);
        // The wrap's fold: a shallow groove from upper left down to lower right.
        let d = u * 0.55 + (v - 0.55);
        let fold = (-(d / 0.11) * (d / 0.11)).exp();
        let h = KNOT_D * round * full - 0.14 * fold * round;
        c + right * (u * w * 0.5) + up * ((v - 0.5) * KNOT_H) + n * (0.12 + h)
    };
    let mut t = Tris::default();
    for j in 0..=N {
        for i in 0..=N {
            let (u, v) = (i as f32 / N as f32 * 2.0 - 1.0, j as f32 / N as f32);
            let p = at(u, v);
            let e = 0.02;
            let du = at((u + e).min(1.0), v) - at((u - e).max(-1.0), v);
            let dv = at(u, (v + e).min(1.0)) - at(u, (v - e).max(0.0));
            let mut nn = du.cross(dv).norm();
            if nn.dot(n) < 0.0 {
                nn = nn * -1.0;
            }
            t.pts.push(p);
            t.nrm.push(nn);
            // (Its luminance a little lower in the fold, and at its sides where it turns
            // under — the knot's own shade — and no edge of its own.)
            let side = 1.0 - 0.25 * smooth_k((u.abs() - 0.7) / 0.3);
            t.detail.push([side, 9.0]);
            t.uv.push([-1.0, -1.0]);
        }
    }
    let row = (N + 1) as u32;
    for j in 0..N as u32 {
        for i in 0..N as u32 {
            let (a, b, cc, d) = (
                j * row + i,
                j * row + i + 1,
                (j + 1) * row + i + 1,
                (j + 1) * row + i,
            );
            // Wound outward (toward `n`).
            let f = (t.pts[b as usize] - t.pts[a as usize])
                .cross(t.pts[cc as usize] - t.pts[a as usize]);
            if f.dot(n) >= 0.0 {
                t.idx.push([a, b, cc]);
                t.idx.push([a, cc, d]);
            } else {
                t.idx.push([a, cc, b]);
                t.idx.push([a, d, cc]);
            }
        }
    }
    m.tris.push((Role::Tie, t));
}

/// The knot's height, its width at its foot and under the collar, and how far it stands
/// off the shirt at its fullest (cm).
const KNOT_H: f32 = 3.0;
const KNOT_W: (f32, f32) = (1.9, 3.2);
const KNOT_D: f32 = 0.9;

fn smooth_k(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The figure's baked pose for `b` (its arms at rest), kept: the live arms start from its
/// shoulders.
pub fn rest_body(b: super::bowed::Bowed, who: Who) -> std::sync::Arc<(Posed, f32)> {
    use std::sync::{Arc, Mutex};
    type Kept = Vec<((super::bowed::Bowed, Who), Arc<(Posed, f32)>)>;
    static KEPT: Mutex<Kept> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, p)) = kept.iter().find(|(k, _)| *k == (b, who)) {
        return p.clone();
    }
    // (r42:arcus-polish2: the pose its holder holds it in — the realistic violinists'.)
    let hold = match who {
        Who::Woman => super::bowed_hold::Hold::Woman,
        Who::Man => super::bowed_hold::Hold::Man,
    };
    let pose = super::bowed_hold::pose(b, super::bowed_hold::Holder::of(hold));
    let s = super::bowed_figure::skeleton(b, &pose);
    let p = Arc::new(body(figure(who), &s, b.dims().held, [None, None]));
    kept.push(((b, who), p.clone()));
    p
}

/// The points the camera fits for the realistic figure holding `b`: its crown with room
/// over it, its head's sides.
pub fn fit(b: super::bowed::Bowed, who: Who) -> Vec<V3> {
    let fig = figure(who);
    let p = rest_body(b, who);
    let eyes = (fig.mark("eye.L") + fig.mark("eye.R")) * 0.5;
    let at = |q: V3| p.0.at(HEAD, q);
    vec![
        at(eyes + v(0.0, 17.0, -6.0)),
        at(eyes + v(9.5, 4.0, -6.0)),
        at(eyes + v(-9.5, 4.0, -6.0)),
    ]
}
