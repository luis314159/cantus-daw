//! The libraries the recorded strings come from and how their files are named: which
//! instrument, articulation, dynamic, bow and round robin each file is, and the note its
//! name says (an octave off in VSCO 2 CE and in the Bigcat cello — the measurement decides).

/// What a file's name says.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Meta {
    /// The instrument's id within its pack.
    pub inst: &'static str,
    pub art: &'static str,
    pub bow: Option<&'static str>,
    /// The dynamic's rank, softest 0.
    pub dyn_rank: u8,
    pub rr: u8,
    /// The key the name says (scientific pitch, C4 = 60), before the octave fix.
    pub named: u8,
}

/// A note name (`C`, `Eb`, `A#`, lower or upper case) and octave as a key, C4 = 60.
pub fn key_of(name: &str, octave: i32) -> Option<u8> {
    let mut c = name.chars();
    let step = match c.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let acc = match c.as_str() {
        "" => 0,
        "#" | "s" => 1,
        "b" => -1,
        _ => return None,
    };
    let k = 12 * (octave + 1) + step + acc;
    (0..=127).contains(&k).then_some(k as u8)
}

/// `A#0`, `Eb1`, `c1`, `a2` → a key.
pub fn note_token(t: &str) -> Option<u8> {
    let i = t.find(|c: char| c.is_ascii_digit() || c == '-')?;
    key_of(&t[..i], t[i..].parse().ok()?)
}

fn rank(table: &[&str], d: &str) -> Option<u8> {
    table.iter().position(|x| *x == d).map(|i| i as u8)
}

fn rr_of(t: &str) -> u8 {
    let d: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    d.parse().unwrap_or(1).max(1)
}

/// VSCO 2 CE's strings (paths under `Strings/`).
pub fn vsco(path: &str) -> Option<Meta> {
    let parts: Vec<&str> = path.split('/').collect();
    let (folder, sub, file) = (*parts.get(1)?, *parts.get(2)?, *parts.get(3)?);
    let stem = file.strip_suffix(".wav")?;
    let t: Vec<&str> = stem.split('_').collect();
    let inst = match folder {
        "Violin Section" => "violins",
        "Viola Section" => "violas",
        "Cello Section" => "celli",
        "Solo Violin" => "violin",
        "Solo Contrabass" if sub == "SusNV" => "bass-nv",
        "Solo Contrabass" => "bass",
        _ => return None,
    };
    let art = match sub.to_ascii_lowercase().as_str() {
        "susvib" | "arco vib" => "sustain",
        "susnv" => "sustain",
        "spic" => "spiccato",
        "pizz" | "pizzt" => "pizzicato",
        "trem" => "tremolo",
        _ => return None,
    };
    // The note is the token holding a digit after a letter; the dynamic `v1`/`v2`/`v3` or
    // `p`/`f`; the round robin `RR1`/`rr1`/`1`.
    let named = t.iter().find_map(|x| {
        (x.len() <= 3 && x.chars().next()?.is_ascii_alphabetic() && !x.starts_with('v'))
            .then(|| note_token(x))
            .flatten()
    })?;
    let dyn_rank = t.iter().find_map(|x| match *x {
        "v1" | "p" => Some(0),
        "v2" | "v3" | "f" => Some(1),
        _ => None,
    })?;
    let rr = t
        .iter()
        .rev()
        .find(|x| x.to_ascii_lowercase().starts_with("rr") || x.parse::<u8>().is_ok())
        .map_or(1, |x| rr_of(x));
    Some(Meta {
        inst,
        art,
        bow: None,
        dyn_rank,
        rr,
        named,
    })
}

/// The Karoryfer Bigcat cello (paths under `Samples/`).
pub fn bigcat(path: &str) -> Option<Meta> {
    let parts: Vec<&str> = path.split('/').collect();
    let (sub, file) = (*parts.get(1)?, *parts.get(2)?);
    let t: Vec<&str> = file.strip_suffix(".wav")?.split('_').collect();
    let dyns = ["p", "mp", "mf", "f"];
    let named = note_token(t.first()?)?;
    let (art, bow, dyn_rank, rr) = match sub {
        "sus" => (
            "sustain",
            Some(if *t.get(2)? == "d" { "down" } else { "up" }),
            rank(&dyns, t.get(1)?)?,
            1,
        ),
        "staccato" => ("staccato", None, rank(&dyns, t.get(1)?)?, rr_of(t.get(2)?)),
        "pizzcato" => ("pizzicato", None, 0, rr_of(t.get(1)?)),
        _ => return None,
    };
    Some(Meta {
        inst: "cello",
        art,
        bow,
        dyn_rank,
        rr,
        named,
    })
}

/// The D. Smolken double bass (paths `arco/…`, `pizz/…`).
pub fn smolken(path: &str) -> Option<Meta> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let t: Vec<&str> = parts[1].strip_suffix(".wav")?.split('_').collect();
    let named = note_token(t.get(1)?)?;
    match parts[0] {
        "arco" => Some(Meta {
            inst: "bass",
            art: "sustain",
            bow: Some(if *t.get(3)? == "down" { "down" } else { "up" }),
            dyn_rank: rank(&["pp", "p", "mf", "f", "ff"], t.get(2)?)?,
            rr: 1,
            named,
        }),
        "pizz" => {
            // `fa` … `pd`: the dynamic's letter (p, m, f), then the take.
            let d = t.get(2)?;
            let (dl, take) = d.split_at(1);
            Some(Meta {
                inst: "bass",
                art: "pizzicato",
                bow: None,
                dyn_rank: rank(&["p", "m", "f"], dl)?,
                rr: take.bytes().next()?.checked_sub(b'a')? + 1,
                named,
            })
        }
        _ => None,
    }
}

/// Karoryfer's Meatbass (paths `Samples/arco_looped/…`, `Samples/pizz/…`).
pub fn meatbass(path: &str) -> Option<Meta> {
    let parts: Vec<&str> = path.split('/').collect();
    let (sub, file) = (*parts.get(1)?, *parts.get(2)?);
    let t: Vec<&str> = file.strip_suffix(".wav")?.split('_').collect();
    let named = note_token(t.first()?)?;
    let layer = t
        .get(1)?
        .strip_prefix("vl")?
        .parse::<u8>()
        .ok()?
        .checked_sub(1)?;
    match sub {
        "arco" => Some(Meta {
            inst: "meatbass",
            art: "sustain",
            bow: Some(if *t.get(2)? == "down" { "down" } else { "up" }),
            dyn_rank: layer,
            rr: 1,
            named,
        }),
        "pizz" => Some(Meta {
            inst: "meatbass",
            art: "pizzicato",
            bow: None,
            dyn_rank: layer,
            rr: rr_of(t.get(2)?),
            named,
        }),
        _ => None,
    }
}

/// Karoryfer's Sneakybass (paths `Samples/pluck/…`): its plucked notes.
pub fn sneakybass(path: &str) -> Option<Meta> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.get(1) != Some(&"pluck") {
        return None;
    }
    let t: Vec<&str> = parts.get(2)?.strip_suffix(".wav")?.split('_').collect();
    Some(Meta {
        inst: "sneakybass",
        art: "pizzicato",
        bow: None,
        dyn_rank: 0,
        rr: rr_of(t.get(3)?),
        named: note_token(t.get(1)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse() {
        assert_eq!(note_token("A#0"), Some(22));
        assert_eq!(note_token("Eb1"), Some(27));
        assert_eq!(note_token("c1"), Some(24));
        let m = vsco("Strings/Violin Section/susVib/VlnEns_susVib_A3_v1.wav").unwrap();
        assert_eq!(
            (m.inst, m.art, m.named, m.dyn_rank),
            ("violins", "sustain", 57, 0)
        );
        let m = vsco("Strings/Solo Contrabass/Pizz/BKCtbss_Pizz_A#0_v3_rr2.wav").unwrap();
        assert_eq!(
            (m.inst, m.art, m.named, m.dyn_rank, m.rr),
            ("bass", "pizzicato", 22, 1, 2)
        );
        let m = bigcat("Samples/sus/Eb1_mf_g.wav").unwrap();
        assert_eq!(
            (m.art, m.bow, m.dyn_rank, m.named),
            ("sustain", Some("up"), 2, 27)
        );
        let m = smolken("pizz/pizz_a2_fc.wav").unwrap();
        assert_eq!((m.art, m.dyn_rank, m.rr, m.named), ("pizzicato", 2, 3, 45));
    }
}
