//! Kits from outside (Pulsus III, ADR 0100 §7): a folder of one-shots, or an SFZ drum kit's
//! regions, made into a mapped kit on the General MIDI drum map.
//!
//! **Names.** A file's name says what it is, in English or Spanish: *kick, bd, bombo; snare,
//! sd, caja, redoblante; clap, palmas; rim, sidestick, aro; closed/open/pedal hat, hh,
//! charles, cerrado/abierto/pedal; low/mid/high/floor tom, grave/medio/agudo; crash, ride,
//! platillo; cowbell, cencerro; perc, shaker, pandereta, conga, bongó, clave, maraca…*. A
//! file whose name says nothing goes on the next free pad, in the folder's order.
//!
//! **Variants.** Files that differ only by a number (*Kick 1, Kick 2, kick_03*) are one pad's
//! round robin; files that differ by a dynamic (*soft, medium, hard, pp, mf, ff, vl1…vl4,
//! suave, fuerte*) are one pad's velocity layers, softest lowest — both at once give
//! velocity layers each cycling its own takes. At most [`MAX_LAYERS`] layers a pad and
//! [`cantus_dsp::MAX_PADS`] pads a kit: what does not fit is said, never dropped silently.

use super::{DrumPad, PadLayer, PadVoice, PulsusParams, RoundRobin, MAX_LAYERS};
use crate::model::Pitch;

/// What a drum is, by its name.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Kick,
    Rim,
    Snare,
    Clap,
    HatClosed,
    HatPedal,
    HatOpen,
    TomLow,
    TomMid,
    TomHigh,
    Crash,
    Ride,
    Cowbell,
    Perc,
}

impl Role {
    /// The General MIDI notes this role takes, first choice first.
    fn pitches(self) -> &'static [Pitch] {
        match self {
            Role::Kick => &[36, 35],
            Role::Rim => &[37],
            Role::Snare => &[38, 40],
            Role::Clap => &[39],
            Role::HatClosed => &[42],
            Role::HatPedal => &[44],
            Role::HatOpen => &[46],
            Role::TomLow => &[41, 43, 45],
            Role::TomMid => &[47, 45],
            Role::TomHigh => &[48, 50],
            Role::Crash => &[49, 57, 55],
            Role::Ride => &[51, 59, 53],
            Role::Cowbell => &[56],
            Role::Perc => &[54, 58, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70],
        }
    }

    fn is_hat(self) -> bool {
        matches!(self, Role::HatClosed | Role::HatPedal | Role::HatOpen)
    }
}

/// A name's words, lowercase, with letters and digits apart (`Kick01` → `kick`, `01`).
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut digit = false;
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        let c = match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            let d = c.is_ascii_digit();
            if !cur.is_empty() && d != digit {
                out.push(std::mem::take(&mut cur));
            }
            digit = d;
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The role a file's name says, if it says one.
pub fn role_of(name: &str) -> Option<Role> {
    let w = words(name);
    let has = |xs: &[&str]| w.iter().any(|x| xs.contains(&x.as_str()));
    let hat = has(&[
        "hh", "hihat", "hat", "hats", "charles", "chh", "ohh", "phh", "hihats",
    ]) || (has(&["hi"]) && has(&["hat"]));
    if hat {
        return Some(if has(&["open", "abierto", "abierta", "ohh", "op"]) {
            Role::HatOpen
        } else if has(&["pedal", "foot", "pie", "phh", "chick"]) {
            Role::HatPedal
        } else {
            Role::HatClosed
        });
    }
    if has(&["kick", "kik", "bd", "bombo", "kickdrum", "bassdrum"])
        || (has(&["bass"]) && has(&["drum"]))
    {
        return Some(Role::Kick);
    }
    if has(&["rim", "rimshot", "sidestick", "aro", "crossstick"])
        || (has(&["side", "cross"]) && has(&["stick"]))
    {
        return Some(Role::Rim);
    }
    if has(&["snare", "sd", "snr", "caja", "redoblante", "tarola"]) {
        return Some(Role::Snare);
    }
    if has(&[
        "clap", "claps", "cp", "palmas", "palma", "aplauso", "handclap",
    ]) {
        return Some(Role::Clap);
    }
    if has(&["tom", "toms", "tomtom", "timbal"]) {
        return Some(
            if has(&["low", "lo", "floor", "ft", "grave", "bajo", "base"]) {
                Role::TomLow
            } else if has(&["high", "hi", "agudo", "alto", "rack"]) {
                Role::TomHigh
            } else {
                Role::TomMid
            },
        );
    }
    if has(&["crash", "splash", "china", "choque"]) {
        return Some(Role::Crash);
    }
    if has(&["ride"]) {
        return Some(Role::Ride);
    }
    if has(&["cowbell", "cencerro", "campana"]) {
        return Some(Role::Cowbell);
    }
    if has(&[
        "perc",
        "percussion",
        "percusion",
        "shaker",
        "tamb",
        "tambourine",
        "pandereta",
        "conga",
        "bongo",
        "bongos",
        "guiro",
        "clave",
        "claves",
        "maraca",
        "maracas",
        "triangle",
        "triangulo",
        "woodblock",
        "block",
        "agogo",
        "cabasa",
        "timbale",
        "cajon",
        "djembe",
        "snap",
        "chasquido",
    ]) {
        return Some(Role::Perc);
    }
    None
}

/// A dynamic a name says, softest 0.
fn dynamic_of(w: &str) -> Option<u8> {
    match w {
        "ppp" | "pp" | "soft" | "suave" | "ghost" | "piano" => Some(0),
        "p" | "mp" | "light" => Some(1),
        "mf" | "med" | "medium" | "medio" | "media" => Some(2),
        "f" | "loud" | "fuerte" | "accent" | "acento" => Some(3),
        "ff" | "fff" | "hard" | "max" => Some(4),
        _ => w
            .strip_prefix("vl")
            .or_else(|| w.strip_prefix("vel"))
            .and_then(|n| n.parse::<u8>().ok()),
    }
}

/// A file's pad key (its name without the variant) and its dynamic and take.
fn parse_name(stem: &str) -> (String, Option<u8>, Option<u32>) {
    let w = words(stem);
    // A number that names the drum stays in its name: a leading one (808 Kick, 909), and a
    // tom's or a cymbal's (Tom 1, Crash 2) — those are different drums, not takes.
    let named = matches!(
        role_of(stem),
        Some(Role::TomLow | Role::TomMid | Role::TomHigh | Role::Crash | Role::Ride)
    );
    let mut dynamic = None;
    let mut take = None;
    let mut key: Vec<&str> = Vec::new();
    for (i, x) in w.iter().enumerate() {
        if let Some(d) = dynamic_of(x) {
            dynamic = Some(d);
            continue;
        }
        // A number after "vl"/"rr"/"take" is that; a leading number, or a tom's or a
        // cymbal's, names the drum; any other bare number is a take.
        if x.chars().all(|c| c.is_ascii_digit()) {
            let after = |m: &[&str]| i > 0 && m.contains(&w[i - 1].as_str());
            if after(&["vl", "vel"]) {
                dynamic = x.parse().ok();
            } else if after(&["rr", "take"]) {
                take = x.parse().ok().or(take);
            } else if i == 0 || named {
                key.push(x);
            } else {
                take = x.parse().ok().or(take);
            }
            continue;
        }
        if matches!(x.as_str(), "rr" | "vl" | "vel" | "take") {
            continue;
        }
        key.push(x);
    }
    (key.join(" "), dynamic, take)
}

/// The note a numbered tom or cymbal takes: Tom 1 the highest, down to the floor; Crash 1
/// and 2, Ride 1 and 2 on their General MIDI notes. `None` for any other name.
fn numbered_pitch(key: &str, role: Option<Role>) -> Option<Pitch> {
    let n: usize = key.split(' ').find_map(|w| w.parse().ok())?;
    let list: &[Pitch] = match role? {
        Role::TomLow | Role::TomMid | Role::TomHigh => &[50, 48, 47, 45, 43, 41],
        Role::Crash => &[49, 57, 55],
        Role::Ride => &[51, 59, 53],
        _ => return None,
    };
    list.get(n.checked_sub(1)?).copied()
}

/// A name for a pad from its key: words capitalised.
fn title(key: &str) -> String {
    key.split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A file's stem (its name without folder and extension).
fn stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').map_or(name, |(s, _)| s)
}

/// One pad's files on their way: its key, its role, and each file's dynamic, take and path.
type Group = (String, Option<Role>, Vec<(Option<u8>, Option<u32>, String)>);

/// A mapped kit from audio files (a folder's, in its order), and what could not be kept.
pub fn kit_from_files(files: &[String]) -> (PulsusParams, Vec<String>) {
    let mut log = Vec::new();
    // Group the files by pad: (key, role) in first-seen order.
    let mut groups: Vec<Group> = Vec::new();
    for f in files {
        let (key, dynamic, take) = parse_name(stem(f));
        let role = role_of(stem(f));
        let key = if key.is_empty() {
            stem(f).to_string()
        } else {
            key
        };
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.2.push((dynamic, take, f.clone())),
            None => groups.push((key, role, vec![(dynamic, take, f.clone())])),
        }
    }
    let mut taken: Vec<Pitch> = Vec::new();
    let mut pads: Vec<DrumPad> = Vec::new();
    let mut free = (60u8..=127).chain((24u8..36).rev());
    // Named drums first, in role order, so a kit's kick gets 36 whatever the folder's order.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by_key(|i| (groups[*i].1.is_none(), groups[*i].1, *i));
    for i in order {
        let (key, role, files) = &groups[i];
        if pads.len() >= cantus_dsp::MAX_PADS {
            log.push(format!("{key}: more than {} pads", cantus_dsp::MAX_PADS));
            continue;
        }
        let pitch = numbered_pitch(key, *role)
            .filter(|p| !taken.contains(p))
            .or_else(|| role.and_then(|r| r.pitches().iter().copied().find(|p| !taken.contains(p))))
            .or_else(|| free.by_ref().find(|p| !taken.contains(p)));
        let Some(pitch) = pitch else {
            log.push(format!("{key}: no note left"));
            continue;
        };
        taken.push(pitch);
        let mut pad = DrumPad::new(pitch, &title(key), PadVoice::Sample);
        if role.is_some_and(Role::is_hat) {
            pad.choke = 1;
        }
        let mut files = files.clone();
        files.sort_by_key(|(d, t, _)| (d.unwrap_or(0), t.unwrap_or(0)));
        let dynamics: Vec<u8> = {
            let mut d: Vec<u8> = files.iter().filter_map(|f| f.0).collect();
            d.sort_unstable();
            d.dedup();
            d
        };
        for (dynamic, _, path) in &files {
            if pad.layers.len() >= MAX_LAYERS {
                log.push(format!(
                    "{key}: more than {MAX_LAYERS} layers ({})",
                    file_name(path)
                ));
                continue;
            }
            let mut layer = PadLayer::new(path.clone());
            if dynamics.len() > 1 {
                let at = dynamics
                    .iter()
                    .position(|d| Some(*d) == *dynamic)
                    .unwrap_or(0);
                let (lo, hi) = band(at, dynamics.len());
                layer.vel_lo = lo;
                layer.vel_hi = hi;
            }
            pad.layers.push(layer);
        }
        // More takes than dynamics: the takes of a zone cycle.
        if pad.layers.len() > dynamics.len().max(1) {
            pad.round_robin = RoundRobin::Cycle;
        }
        pads.push(pad);
    }
    (PulsusParams::of(pads), log)
}

/// Velocity band `i` of `n`, splitting 1…127 evenly.
fn band(i: usize, n: usize) -> (u8, u8) {
    let n = n.max(1) as f32;
    let lo = 1.0 + (i as f32 * 127.0 / n).floor();
    let hi = ((i as f32 + 1.0) * 127.0 / n).floor();
    (lo.clamp(1.0, 127.0) as u8, hi.clamp(1.0, 127.0) as u8)
}

/// A mapped kit from an SFZ drum kit's regions (Exemplar's zones as `sampler_sfz` reads
/// them): one pad per key a region plays on alone, its regions as layers — velocity range,
/// round robin (a sequence cycles, a random range picks), tune, level, pan — named after a
/// region's name, else the General MIDI drum the key is. What does not fit is said.
///
/// **Chokes.** SFZ's exclusive groups are `group=` (`zf::EXCL`) and `off_by=` (`zf::OFFBY`),
/// and they are one-way: a key whose region is in group *g* cuts a key whose region is
/// `off_by=`*g*. A Pulsus choke group is shared, so two keys that cut each other either way
/// share one. The General MIDI hat keys (42, 44, 46) choke together whenever the kit cuts
/// among them: a kit like Virtuosity Drums makes the closed hat cut the open one through a
/// silent region, which Exemplar's reader does not keep. A group number Exemplar holds only
/// up to 63 (the reader stores 64 for anything above) cannot be told apart: it is left out,
/// and said.
pub fn kit_from_zones(zones: &[crate::inst::sampler::Zone]) -> (PulsusParams, Vec<String>) {
    use crate::inst::sampler::zf;
    let mut log = Vec::new();
    let mut keys: Vec<Pitch> = Vec::new();
    let mut spans = 0usize;
    for z in zones {
        let (lo, hi) = (z.get(zf::LO) as u8, z.get(zf::HI) as u8);
        if hi > lo.saturating_add(3) {
            spans += 1;
        }
        for k in lo..=hi.min(lo.saturating_add(3)) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    if spans > 0 {
        log.push(format!(
            "{spans} regions span more than four keys: their first four kept"
        ));
    }
    keys.sort_unstable();
    let chokes = choke_groups(zones, &keys, &mut log);
    let mut pads = Vec::new();
    for key in keys {
        if pads.len() >= cantus_dsp::MAX_PADS {
            log.push(format!(
                "key {key}: more than {} pads",
                cantus_dsp::MAX_PADS
            ));
            continue;
        }
        let on: Vec<&crate::inst::sampler::Zone> = zones
            .iter()
            .filter(|z| {
                let (lo, hi) = (z.get(zf::LO) as u8, z.get(zf::HI) as u8);
                lo <= key && key <= hi && z.sample.is_some()
            })
            .collect();
        if on.is_empty() {
            continue;
        }
        let name = on
            .iter()
            .find_map(|z| z.name.clone())
            .unwrap_or_else(|| match gm_name(key) {
                "Key" => format!("Key {key}"),
                n => n.to_string(),
            });
        let mut pad = DrumPad::new(key, &name, PadVoice::Sample);
        pad.choke = chokes
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0, |(_, c)| *c);
        let on = fit_layers(key, on, &mut log);
        for z in &on {
            let mut l = PadLayer::new(z.sample.clone().unwrap_or_default());
            l.vel_lo = (z.get(zf::VLO) as u8).max(1);
            l.vel_hi = (z.get(zf::VHI) as u8).max(l.vel_lo);
            l.tune = z.get(zf::TUNE) / 100.0;
            l.level_db = z.get(zf::GAIN);
            l.pan = (z.get(zf::PAN) / 100.0).clamp(-1.0, 1.0);
            pad.layers.push(l);
        }
        if on.iter().any(|z| z.get(zf::SEQLEN) > 1.0) {
            pad.round_robin = RoundRobin::Cycle;
        } else if on
            .iter()
            .any(|z| z.get(zf::RLO) > 0.0 || z.get(zf::RHI) < 1.0)
        {
            pad.round_robin = RoundRobin::Random;
        }
        pads.push(pad);
    }
    (PulsusParams::of(pads), log)
}

/// Each key's choke group (1…16) from the regions' exclusive groups (see
/// [`kit_from_zones`]); keys in none are left out.
fn choke_groups(
    zones: &[crate::inst::sampler::Zone],
    keys: &[Pitch],
    log: &mut Vec<String>,
) -> Vec<(Pitch, u8)> {
    use crate::inst::sampler::zf;
    // A group Exemplar's reader could not hold reads 64: unknowable.
    const CLAMPED: u32 = 64;
    let mut clamped = 0usize;
    let mut groups: Vec<(Pitch, Vec<u32>, Vec<u32>)> =
        keys.iter().map(|k| (*k, Vec::new(), Vec::new())).collect();
    for z in zones {
        let (lo, hi) = (z.get(zf::LO) as u8, z.get(zf::HI) as u8);
        let (g, off) = (z.get(zf::EXCL) as u32, z.get(zf::OFFBY) as u32);
        if g >= CLAMPED || off >= CLAMPED {
            clamped += 1;
        }
        for (k, gs, offs) in groups.iter_mut() {
            if lo <= *k && *k <= hi {
                if g > 0 && g < CLAMPED && !gs.contains(&g) {
                    gs.push(g);
                }
                if off > 0 && off < CLAMPED && !offs.contains(&off) {
                    offs.push(off);
                }
            }
        }
    }
    if clamped > 0 {
        log.push(format!(
            "{clamped} regions name an exclusive group above 63: not read as a choke"
        ));
    }
    // Union keys that cut each other, either way.
    let n = groups.len();
    let mut root: Vec<usize> = (0..n).collect();
    fn find(root: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    let cuts = |a: usize, b: usize, g: &[(Pitch, Vec<u32>, Vec<u32>)]| {
        g[a].1.iter().any(|x| g[b].2.contains(x))
    };
    let mut linked = vec![false; n];
    for a in 0..n {
        for b in 0..n {
            if a != b && cuts(a, b, &groups) {
                let (ra, rb) = (find(&mut root, a), find(&mut root, b));
                root[ra] = rb;
                linked[a] = true;
                linked[b] = true;
            }
        }
    }
    // The General MIDI hats choke together when the kit cuts among them at all.
    let hats: Vec<usize> = (0..n)
        .filter(|i| [42, 44, 46].contains(&groups[*i].0))
        .collect();
    if hats.iter().any(|i| linked[*i]) {
        for w in hats.windows(2) {
            let (ra, rb) = (find(&mut root, w[0]), find(&mut root, w[1]));
            root[ra] = rb;
        }
        for i in &hats {
            linked[*i] = true;
        }
    }
    let mut ids: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for i in 0..n {
        if !linked[i] {
            continue;
        }
        let r = find(&mut root, i);
        let id = match ids.iter().position(|x| *x == r) {
            Some(p) => p,
            None => {
                ids.push(r);
                ids.len() - 1
            }
        };
        if id < 16 {
            out.push((groups[i].0, id as u8 + 1));
        }
    }
    if ids.len() > 16 {
        log.push(format!("{} choke groups: the first 16 kept", ids.len()));
    }
    out
}

/// A key's regions cut to [`MAX_LAYERS`]: a kit recorded with several microphones plays one
/// region per microphone at each hit — one position is kept (the folder holding the most
/// of the key's regions); then, still too many, each velocity zone keeps the same number of
/// its takes. What was left out is said.
fn fit_layers<'a>(
    key: Pitch,
    on: Vec<&'a crate::inst::sampler::Zone>,
    log: &mut Vec<String>,
) -> Vec<&'a crate::inst::sampler::Zone> {
    use crate::inst::sampler::zf;
    if on.len() <= MAX_LAYERS {
        return on;
    }
    let total = on.len();
    let folder = |z: &crate::inst::sampler::Zone| -> String {
        let s = z.sample.clone().unwrap_or_default().replace('\\', "/");
        s.rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default()
    };
    // The microphone position (folder) with the most regions, the first seen on a tie.
    let mut folders: Vec<(String, usize)> = Vec::new();
    for z in &on {
        let f = folder(z);
        match folders.iter_mut().find(|(g, _)| *g == f) {
            Some((_, n)) => *n += 1,
            None => folders.push((f, 1)),
        }
    }
    let mut kept = on;
    if folders.len() > 1 {
        let best = folders
            .iter()
            .enumerate()
            .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
            .map(|(_, (f, _))| f.clone())
            .unwrap_or_default();
        kept.retain(|z| folder(z) == best);
        log.push(format!(
            "key {key}: {} microphone positions, one kept ({})",
            folders.len(),
            file_name(&best)
        ));
    }
    if kept.len() > MAX_LAYERS {
        // Each velocity zone keeps the same number of its takes.
        let mut zones: Vec<(u8, u8)> = kept
            .iter()
            .map(|z| (z.get(zf::VLO) as u8, z.get(zf::VHI) as u8))
            .collect();
        zones.sort_unstable();
        zones.dedup();
        let each = (MAX_LAYERS / zones.len().max(1)).max(1);
        let mut seen: Vec<((u8, u8), usize)> = Vec::new();
        kept.retain(|z| {
            let v = (z.get(zf::VLO) as u8, z.get(zf::VHI) as u8);
            let n = match seen.iter_mut().find(|(k, _)| *k == v) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    seen.push((v, 1));
                    1
                }
            };
            n <= each
        });
        kept.truncate(MAX_LAYERS);
    }
    if kept.len() < total {
        log.push(format!(
            "key {key}: {} of its {total} regions kept (at most {MAX_LAYERS} layers)",
            kept.len()
        ));
    }
    kept
}

/// The last part of a path, `/` or `\` apart: what a log line names, never the folders
/// above (an agent is told file names only).
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
}

/// The General MIDI drum a key is (key 35…81), else "Key n".
pub fn gm_name(key: Pitch) -> &'static str {
    const GM: [&str; 47] = [
        "Acoustic Bass Drum",
        "Bass Drum",
        "Side Stick",
        "Acoustic Snare",
        "Hand Clap",
        "Electric Snare",
        "Low Floor Tom",
        "Closed Hi-Hat",
        "High Floor Tom",
        "Pedal Hi-Hat",
        "Low Tom",
        "Open Hi-Hat",
        "Low-Mid Tom",
        "Hi-Mid Tom",
        "Crash Cymbal 1",
        "High Tom",
        "Ride Cymbal 1",
        "Chinese Cymbal",
        "Ride Bell",
        "Tambourine",
        "Splash Cymbal",
        "Cowbell",
        "Crash Cymbal 2",
        "Vibraslap",
        "Ride Cymbal 2",
        "Hi Bongo",
        "Low Bongo",
        "Mute Hi Conga",
        "Open Hi Conga",
        "Low Conga",
        "High Timbale",
        "Low Timbale",
        "High Agogo",
        "Low Agogo",
        "Cabasa",
        "Maracas",
        "Short Whistle",
        "Long Whistle",
        "Short Guiro",
        "Long Guiro",
        "Claves",
        "Hi Wood Block",
        "Low Wood Block",
        "Mute Cuica",
        "Open Cuica",
        "Mute Triangle",
        "Open Triangle",
    ];
    match key {
        35..=81 => GM[(key - 35) as usize],
        _ => "Key",
    }
}

#[cfg(test)]
#[path = "pads_import_tests.rs"]
mod tests;
