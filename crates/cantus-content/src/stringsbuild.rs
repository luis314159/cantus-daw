//! r42:strings-rec — a recorded strings instrument written as an SFZ at install (ADR 0158),
//! from what the manifest says of its samples: each zone's file, keys and **measured** root
//! and tuning (the octave its name gets wrong fixed, the cents it sits off its key corrected:
//! `tools/content/strings/`), its articulation and bow, the controller crossfade that is its
//! dynamic layer, its round robin and the **loop found offline** in it. Nothing is computed
//! here but text: the measuring was done once, by the tool, and stored as the pack's metadata.
//!
//! The file is plain SFZ with three of Cantus's own opcodes (`cantus_articulation`,
//! `cantus_bow`, `cantus_legato*`), which other players ignore; any SFZ player plays its
//! sustains looped, its layers crossfaded by the mod wheel, and its articulations on
//! keyswitches (`sw_last`), and Exemplar plays them by the note's articulation too.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::Path;

/// The instruments a pack writes at install.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StringsBuild {
    pub instruments: Vec<StringsInstrument>,
}

/// One instrument: an SFZ at `sfz` (relative to the pack's folder).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StringsInstrument {
    pub sfz: String,
    /// Its name, written in the file's header.
    pub name: String,
    /// The first keyswitch: articulation *k* of [`ARTICULATIONS`] the instrument has is on
    /// `switch + k` (sustain first, the default); none when it has one articulation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch: Option<u8>,
    /// The articulation in force before any keyswitch is pressed (`None`: the sustain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_art: Option<String>,
    /// The scripted legato's crossfade, glide and start past the attack, ms (`None`: off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legato: Option<[f32; 3]>,
    /// The controller the dynamic layers crossfade on (1: the mod wheel).
    #[serde(default = "mod_wheel")]
    pub dynamics_cc: u8,
    /// The instrument's level, dB, so it plays where the native ones sit (ADR 0039).
    #[serde(default)]
    pub gain_db: f32,
    /// Release time per articulation, ms (`[sustain-like, short]`).
    #[serde(default = "releases")]
    pub release_ms: [f32; 2],
    pub zones: Vec<StringsZone>,
}

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

fn mod_wheel() -> u8 {
    1
}

fn releases() -> [f32; 2] {
    [450.0, 250.0]
}

/// A zone: one recording of one articulation, dynamic and bow.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StringsZone {
    /// The sample, relative to the pack's folder.
    pub file: String,
    /// `sustain`, `staccato`, `spiccato`, `pizzicato`, `tremolo` (`legato` plays sustains).
    pub art: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bow: Option<String>,
    pub lo: u8,
    pub hi: u8,
    /// The measured key (the octave fix), and the cents to add so it plays in tune.
    pub root: u8,
    #[serde(default)]
    pub tune: f32,
    /// The controller crossfade: in from `xf[0]` to `xf[1]`, out from `xf[2]` to `xf[3]`.
    pub xf: [u8; 4],
    /// A short note's velocity layer (`lovel`, `hivel`); a sustain's dynamics crossfade.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vel: Option<[u8; 2]>,
    /// Round robin: position and length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<[u8; 2]>,
    /// The loop: start and end (inclusive), in frames, and its crossfade in ms.
    #[serde(default, rename = "loop", skip_serializing_if = "Option::is_none")]
    pub looped: Option<[i64; 2]>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub xf_ms: f32,
    #[serde(default)]
    pub gain_db: f32,
}

/// The articulations an instrument's keyswitches cover, in order (sustain first).
pub const ARTICULATIONS: [&str; 5] = ["sustain", "tremolo", "spiccato", "staccato", "pizzicato"];

/// Whether an articulation is short (its release the short one, never looped).
fn short(art: &str) -> bool {
    matches!(art, "staccato" | "spiccato" | "pizzicato")
}

/// The SFZ of `inst`, its samples named relative to the folder the file is in (`up`: how
/// many folders below the pack's folder it is).
pub fn sfz(inst: &StringsInstrument, up: usize) -> String {
    let prefix = "../".repeat(up);
    let mut s = String::new();
    let _ = writeln!(
        s,
        "// {} — written by Cantus at install (ADR 0158).",
        inst.name
    );
    let _ = writeln!(
        s,
        "// Loops found offline; roots and tuning measured; sustains crossfade on CC{}.",
        inst.dynamics_cc
    );
    let _ = writeln!(s, "<control>\ndefault_path={prefix}");
    let arts: Vec<&str> = ARTICULATIONS
        .iter()
        .copied()
        .filter(|a| inst.zones.iter().any(|z| z.art == *a))
        .collect();
    let _ = writeln!(s, "<global>\nvolume={:.2}\namp_veltrack=30", inst.gain_db);
    if let Some(k) = inst.switch.filter(|_| arts.len() > 1) {
        let first = inst
            .default_art
            .as_deref()
            .and_then(|d| arts.iter().position(|a| *a == d))
            .unwrap_or(0);
        let _ = writeln!(
            s,
            "sw_lokey={k}\nsw_hikey={}\nsw_default={}",
            k as usize + arts.len() - 1,
            k as usize + first
        );
    }
    if let Some([xf, glide, skip]) = inst.legato {
        let _ = writeln!(
            s,
            "cantus_legato=on\ncantus_legato_crossfade={:.3}\ncantus_legato_glide={:.3}\ncantus_legato_skip={:.3}",
            xf / 1000.0,
            glide / 1000.0,
            skip / 1000.0
        );
    }
    for (k, art) in arts.iter().enumerate() {
        let rel = inst.release_ms[short(art) as usize] / 1000.0;
        let _ = writeln!(s, "<group>\ngroup_label={art}\ncantus_articulation={art}\nampeg_attack=0.004\nampeg_release={rel:.3}");
        if let Some(sw) = inst.switch.filter(|_| arts.len() > 1) {
            let _ = writeln!(s, "sw_last={}", sw as usize + k);
        }
        for z in inst.zones.iter().filter(|z| z.art == *art) {
            let _ = write!(
                s,
                "<region> sample={} lokey={} hikey={} pitch_keycenter={}",
                z.file, z.lo, z.hi, z.root
            );
            if z.tune != 0.0 {
                let _ = write!(s, " tune={:.1}", z.tune);
            }
            if z.gain_db != 0.0 {
                let _ = write!(s, " volume={:.2}", z.gain_db);
            }
            let cc = inst.dynamics_cc;
            if z.xf != [0, 0, 127, 127] {
                let _ = write!(
                    s,
                    " xfin_locc{cc}={} xfin_hicc{cc}={} xfout_locc{cc}={} xfout_hicc{cc}={}",
                    z.xf[0], z.xf[1], z.xf[2], z.xf[3]
                );
            }
            if let Some([lo, hi]) = z.vel {
                let _ = write!(s, " lovel={lo} hivel={hi}");
            }
            if let Some(b) = &z.bow {
                let _ = write!(s, " cantus_bow={b}");
            }
            if let Some([pos, len]) = z.seq {
                let _ = write!(s, " seq_length={len} seq_position={pos}");
            }
            match z.looped {
                Some([ls, le]) if !short(art) => {
                    let _ = write!(
                        s,
                        " loop_mode=loop_continuous loop_start={ls} loop_end={le} loop_crossfade={:.4}",
                        z.xf_ms / 1000.0
                    );
                }
                _ if short(art) => {
                    let _ = write!(s, " loop_mode=one_shot");
                }
                _ => {}
            }
            s.push('\n');
        }
    }
    s
}

/// Write every instrument of `build` into the pack's folder `root`: how many files.
pub fn build(build: &StringsBuild, root: &Path) -> std::io::Result<usize> {
    for inst in &build.instruments {
        let path = root.join(&inst.sfz);
        let up = Path::new(&inst.sfz)
            .parent()
            .map_or(0, |p| p.components().count());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, sfz(inst, up))?;
    }
    Ok(build.instruments.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instrument_is_written_as_plain_sfz_with_its_loops_switches_and_layers() {
        let inst = StringsInstrument {
            sfz: "Cantus/cello.sfz".into(),
            name: "Cello".into(),
            switch: Some(24),
            default_art: None,
            legato: Some([80.0, 50.0, 100.0]),
            dynamics_cc: 1,
            gain_db: 3.0,
            release_ms: [450.0, 250.0],
            zones: vec![
                StringsZone {
                    file: "Samples/sus/C1_p_d.wav".into(),
                    art: "sustain".into(),
                    bow: Some("down".into()),
                    lo: 36,
                    hi: 38,
                    root: 36,
                    tune: -3.2,
                    xf: [0, 0, 0, 127],
                    looped: Some([44_100, 132_300]),
                    xf_ms: 333.0,
                    ..StringsZone::default()
                },
                StringsZone {
                    file: "Samples/pizz/C1_1.wav".into(),
                    art: "pizzicato".into(),
                    lo: 36,
                    hi: 38,
                    root: 36,
                    xf: [0, 0, 127, 127],
                    seq: Some([1, 4]),
                    ..StringsZone::default()
                },
            ],
        };
        let text = sfz(&inst, 1);
        for want in [
            "default_path=../",
            "sw_lokey=24\nsw_hikey=25\nsw_default=24",
            "cantus_legato=on",
            "cantus_legato_skip=0.100",
            "cantus_articulation=sustain",
            "sw_last=24",
            "tune=-3.2",
            "xfin_locc1=0 xfin_hicc1=0 xfout_locc1=0 xfout_hicc1=127",
            "cantus_bow=down",
            "loop_mode=loop_continuous loop_start=44100 loop_end=132300 loop_crossfade=0.3330",
            "cantus_articulation=pizzicato",
            "sw_last=25",
            "seq_length=4 seq_position=1 loop_mode=one_shot",
        ] {
            assert!(text.contains(want), "{want} in:\n{text}");
        }
        let dir = tempfile::tempdir().unwrap();
        let b = StringsBuild {
            instruments: vec![inst],
        };
        assert_eq!(build(&b, dir.path()).unwrap(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Cantus/cello.sfz")).unwrap(),
            text
        );
    }
}
