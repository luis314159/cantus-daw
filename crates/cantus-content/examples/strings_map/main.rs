//! r42:strings-rec — measure the recorded strings' samples and write their packs' metadata
//! (ADR 0158). Run by `tools/content/strings/map.sh`; reads the sources
//! `tools/content/strings/fetch_sources.sh` fetched, outside the repository:
//!
//!   cargo run --release -p cantus-content --example strings_map -- <sources> <out.json> [pack]
//!
//! For every sample: its pitch (YIN over its steady part, searched an octave and a half
//! either side of what its name says), so its **root** is the key it sounds (the octave its
//! name gets wrong, fixed) and its **tune** the cents that bring it to that key; for a sustain
//! or a tremolo, its **loop** (`strings_analysis::find_loop`), kept only when Exemplar plays it
//! **click-free** (the seam no louder above 5 kHz than the loop's own playing, +3 dB). Then
//! each instrument's zones: keys split halfway between roots, the dynamics crossfaded on the
//! mod wheel (sustains) or split by velocity (short notes), round robins, keyswitches. The
//! JSON holds, per pack, the `strings` build and its numbers; `tools/content/strings/merge.py`
//! puts them into the manifest.

mod libs;

use cantus_content::strings_analysis as an;
use cantus_content::stringsbuild::{StringsBuild, StringsInstrument, StringsZone};
use cantus_content::wav;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A library: its pack, where its files are (a zip, with the prefix its entries start with,
/// or a folder), how its names read, and its instruments' names.
struct Lib {
    pack: &'static str,
    zip: Option<(&'static str, &'static str)>,
    folder: Option<&'static str>,
    parse: fn(&str) -> Option<libs::Meta>,
    instruments: &'static [(&'static str, &'static str, bool)],
}

/// (instrument id, its name, the scripted legato on)
const LIBS: &[Lib] = &[
    Lib {
        pack: "vsco2-strings",
        zip: None,
        folder: Some("vsco"),
        parse: libs::vsco,
        instruments: &[
            ("violins", "Violins (section)", true),
            ("violas", "Violas (section)", true),
            ("celli", "Celli (section)", true),
            ("violin", "Solo violin", true),
            ("bass", "Solo double bass", true),
            ("bass-nv", "Solo double bass, senza vibrato", true),
        ],
    },
    Lib {
        pack: "karoryfer-bigcat-cello",
        zip: Some(("bigcat.zip", "Karoryfer_Bigcat_cello/")),
        folder: None,
        parse: libs::bigcat,
        instruments: &[("cello", "Bigcat cello", true)],
    },
    Lib {
        pack: "dsmolken-double-bass",
        zip: Some(("smolken.zip", "dsmolken_double_bass/")),
        folder: None,
        parse: libs::smolken,
        instruments: &[("bass", "Smolken double bass", true)],
    },
    Lib {
        pack: "karoryfer-meatbass",
        zip: Some(("meatbass.zip", "Meatbass/")),
        folder: None,
        parse: libs::meatbass,
        instruments: &[("meatbass", "Meatbass", true)],
    },
    Lib {
        pack: "karoryfer-sneakybass",
        zip: Some(("sneakybass.zip", "Sneakybass/")),
        folder: None,
        parse: libs::sneakybass,
        instruments: &[("sneakybass", "Sneakybass (pizzicato)", false)],
    },
];

/// One sample measured.
#[derive(Clone, Debug)]
struct Measured {
    rel: String,
    meta: libs::Meta,
    root: u8,
    tune: f32,
    octave_fix: i32,
    looped: Option<[i64; 2]>,
    xf_ms: f32,
    loop_err_db: f32,
    seam_excess_db: f32,
    rms_db: f32,
    frames: usize,
    /// The pitch measured, cents above key 0 (`None`: none found).
    cents: Option<f64>,
    /// The partials' fit near the root, over what the zone plays held (its loop, or its
    /// steady part) or a short note's tone: the zone's mean pitch, what its tune comes from;
    /// and why there is none.
    fit: Option<f64>,
    fit_none: Option<&'static str>,
}

/// Each zone's tune from the partials' fit near its root (after the octave fix): the zone's
/// mean pitch made its key's, its vibrato kept.
fn fit_tunes(ms: &mut [Measured], fs: &[(String, Vec<u8>)]) {
    let bytes: BTreeMap<&str, &Vec<u8>> = fs.iter().map(|(r, b)| (r.as_str(), b)).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<&mut Measured>> =
        ms.iter_mut().map(std::sync::Mutex::new).collect();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(slot) = slots.get(i) else { break };
                let mut m = slot.lock().unwrap();
                let Some(b) = bytes.get(m.rel.as_str()) else {
                    continue;
                };
                let Ok(a) = wav::parse(b) else { continue };
                let rate = a.rate as f32;
                let x = an::mono(&a.samples, a.channels);
                // What a held zone plays (Exemplar through its loop, crossfaded), a short note's
                // tone: the pitch is read where the check reads it.
                let held = sustained(m.meta.art);
                let xf = (m.xf_ms / 1000.0 * rate) as i64;
                let (x, from, to) = if held {
                    an::held_play(&x, rate, m.looped, xf)
                } else {
                    let (a, b) = an::tone_span(&x, rate);
                    (x, a, b)
                };
                let to = to.min(x.len());
                // A tremolo's bow changes break the partials apart frame by frame (the probe:
                // `strings_tune_probe`): its tune comes from YIN near the root instead.
                let root = an::hz_of_key(m.root as f64);
                let yin = || {
                    an::yin_cents(&x, rate, from, to, root)
                        .filter(|c| (c - m.root as f64 * 100.0).abs() < an::NEAR_CENTS)
                };
                // A sustain: the partials' fit and YIN, two measures of a different kind; where
                // they agree within two cents their mean (each one's noise halved), where they do
                // not the fit has caught another peak of an ensemble's spread: YIN's. A short
                // note: the fit over its tone.
                let fit = if m.meta.art == "tremolo" {
                    yin().or_else(|| an::harmonic_cents(&x, rate, from, to, root))
                } else if held {
                    match (an::harmonic_cents(&x, rate, from, to, root), yin()) {
                        (Some(f), Some(y)) if (f - y).abs() <= 2.0 => Some(0.5 * (f + y)),
                        (Some(_), Some(y)) => Some(y),
                        (f, y) => f.or(y),
                    }
                } else {
                    an::harmonic_cents(&x, rate, from, to, root)
                };
                match fit {
                    Some(c) => {
                        m.fit = Some(c);
                        m.tune = (m.root as f64 * 100.0 - c) as f32;
                    }
                    None => {
                        m.fit_none = Some(if to <= from + 4_096 {
                            "too short to measure"
                        } else {
                            "no voiced frame"
                        });
                        eprintln!("{}: {}", m.rel, m.fit_none.unwrap());
                    }
                }
            });
        }
    });
}

/// The octave fix is the library's naming, the same for every file of an instrument's
/// articulation: the fix most of its files measure is given to all, and a file whose own
/// measurement is more than 50 cents off its key so fixed (a short note's transient read
/// as another partial) plays at its key untuned — said in the report.
fn agree_octaves(ms: &mut [Measured]) {
    let mut groups: BTreeMap<(&'static str, &'static str), BTreeMap<i32, usize>> = BTreeMap::new();
    for m in ms.iter() {
        if m.cents.is_some() && m.tune.abs() <= 50.0 {
            *groups
                .entry((m.meta.inst, m.meta.art))
                .or_default()
                .entry(m.octave_fix)
                .or_default() += 1;
        }
    }
    for m in ms.iter_mut() {
        let fix = groups
            .get(&(m.meta.inst, m.meta.art))
            .and_then(|g| g.iter().max_by_key(|(_, n)| **n).map(|(f, _)| *f))
            .unwrap_or(m.octave_fix);
        m.octave_fix = fix;
        m.root = (m.meta.named as i32 + 12 * fix).clamp(0, 127) as u8;
        m.tune = match m.cents {
            Some(c) if (m.root as f64 * 100.0 - c).abs() <= 50.0 => {
                (m.root as f64 * 100.0 - c) as f32
            }
            _ => {
                eprintln!(
                    "{}: its pitch is not its key's; played at key {} untuned",
                    m.rel, m.root
                );
                0.0
            }
        };
    }
}

fn sustained(art: &str) -> bool {
    matches!(art, "sustain" | "tremolo")
}

/// The files of a library: (path inside the pack, bytes).
fn files(src: &Path, lib: &Lib) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut out = Vec::new();
    if let Some((zip, strip)) = lib.zip {
        let f = std::fs::File::open(src.join(zip)).expect("the zip");
        let mut z = zip::ZipArchive::new(f).expect("a zip");
        for i in 0..z.len() {
            let mut e = z.by_index(i).expect("an entry");
            let name = e.name().map(|n| n.to_string()).unwrap_or_default();
            let Some(rel) = name.strip_prefix(strip) else {
                continue;
            };
            if !rel.to_ascii_lowercase().ends_with(".wav") || (lib.parse)(rel).is_none() {
                continue;
            }
            let mut b = Vec::new();
            e.read_to_end(&mut b).expect("read");
            out.push((rel.to_string(), b));
        }
    }
    if let Some(folder) = lib.folder {
        let root = src.join(folder);
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("a folder").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Ok(rel) = p.strip_prefix(&root) {
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    if rel.ends_with(".wav") && (lib.parse)(&rel).is_some() {
                        out.push((rel, std::fs::read(&p).expect("read")));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Exemplar playing `data`'s loop at its own rate for three turns past the loop end: the
/// worst seam's excess above 5 kHz ([`an::seam_excess_db`]).
fn click_check(data: &[f32], ch: usize, rate: f32, ls: i64, le: i64, xf: i64) -> f32 {
    use cantus_dsp::exemplar::{resample, Exemplar, Globals, Region, Source, ZoneParams};
    let mut levels: [Option<std::sync::Arc<[f32]>>; resample::MAX_LEVELS] = Default::default();
    levels[0] = Some(std::sync::Arc::from(data.to_vec()));
    let frames = data.len() / ch;
    let src = Source {
        levels,
        channels: ch,
        rate,
        frames,
        tune_ct: 0.0,
        stream: None,
    };
    let mut e = Exemplar::new(rate);
    e.set_globals(&Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 50.0],
        vel_amp: 0.0,
        ..Globals::default()
    });
    let z = ZoneParams {
        root: 60,
        region: Region {
            end: frames as i64,
            looping: true,
            ls,
            le,
            xf,
            ..Region::default()
        },
        ..ZoneParams::default()
    };
    e.put_zone(0, &z, &src);
    e.note_on(60, 1.0, 1);
    let len = (le - ls + 1) as usize;
    let total = le as usize + 1 + 3 * len + 512;
    let mut out = Vec::with_capacity(total);
    let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    while out.len() < total {
        l.fill(0.0);
        r.fill(0.0);
        e.process(&mut l, &mut r);
        out.extend(l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)));
    }
    let seams: Vec<usize> = (0..3).map(|k| le as usize + 1 + k * len).collect();
    let from = ls as usize;
    an::seam_excess_db(
        &out[from..],
        rate,
        &seams.iter().map(|s| s - from).collect::<Vec<_>>(),
    )
}

fn measure(rel: &str, bytes: &[u8], meta: libs::Meta) -> Option<Measured> {
    let a = wav::parse(bytes)
        .map_err(|e| eprintln!("{rel}: {e}"))
        .ok()?;
    let rate = a.rate as f32;
    let m = an::mono(&a.samples, a.channels);
    let hint = an::hz_of_key(meta.named as f64);
    let (from, to) = an::steady_span(&m, rate)
        .filter(|_| sustained(meta.art))
        .unwrap_or(((rate * 0.03) as usize, m.len().min((rate * 0.6) as usize)));
    let cents = an::yin_cents(&m, rate, from, to, hint)
        .or_else(|| an::yin_cents(&m, rate, 0, m.len(), hint));
    if cents.is_none() {
        eprintln!("{rel}: no pitch found; its library's octave fix and no tune");
    }
    let heard = cents.map_or(meta.named as i32, |c| (c / 100.0).round() as i32);
    let octave_fix = ((heard - meta.named as i32) as f64 / 12.0).round() as i32;
    let root = (meta.named as i32 + 12 * octave_fix).clamp(0, 127) as u8;
    let tune = cents.map_or(0.0, |c| (root as f64 * 100.0 - c) as f32);
    let rms = an::envelope(&m[from..to.min(m.len())], to.saturating_sub(from).max(1))
        .first()
        .copied()
        .unwrap_or(0.0);
    let mut out = Measured {
        rel: rel.to_string(),
        meta,
        root,
        tune,
        octave_fix,
        looped: None,
        xf_ms: 0.0,
        loop_err_db: 0.0,
        seam_excess_db: 0.0,
        rms_db: 20.0 * cantus_dsp::math::log10(rms.max(1e-9)),
        frames: a.frames(),
        cents,
        fit: None,
        fit_none: None,
    };
    if sustained(meta.art) {
        let hz = cents.map_or(hint, |c| an::hz_of_key(c / 100.0));
        // The first loop that plays click-free: the usual place, then others, shorter.
        let mut worst = None;
        for (start, len) in [
            (0.3, 0.6),
            (0.2, 0.5),
            (0.45, 0.45),
            (0.25, 0.35),
            (0.5, 0.3),
        ] {
            let Some(lp) = an::find_loop_at(&a.samples, a.channels, rate, hz, start, len) else {
                continue;
            };
            let ex = click_check(&a.samples, a.channels, rate, lp.ls, lp.le, lp.xf);
            if ex <= 3.0 {
                out.loop_err_db = lp.err_db;
                out.seam_excess_db = ex;
                out.looped = Some([lp.ls, lp.le]);
                out.xf_ms = lp.xf as f32 / rate * 1000.0;
                break;
            }
            worst = Some(ex);
        }
        if out.looped.is_none() {
            if let Some(ex) = worst {
                out.seam_excess_db = ex;
                eprintln!("{rel}: every loop tried clicks ({ex:.1} dB): left unlooped");
            }
        }
    }
    Some(out)
}

/// Positions of `n` dynamic layers on the controller, and each one's crossfade.
fn layer_xf(rank: usize, n: usize) -> [u8; 4] {
    if n <= 1 {
        return [0, 0, 127, 127];
    }
    let at = |i: usize| (127 * i / (n - 1)) as u8;
    let xin = if rank == 0 {
        [0, 0]
    } else {
        [at(rank - 1), at(rank)]
    };
    let xout = if rank + 1 == n {
        [127, 127]
    } else {
        [at(rank), at(rank + 1)]
    };
    [xin[0], xin[1], xout[0], xout[1]]
}

fn instrument(id: &str, name: &str, legato: bool, ms: &[Measured]) -> StringsInstrument {
    let mine: Vec<&Measured> = ms.iter().filter(|m| m.meta.inst == id).collect();
    let mut zones = Vec::new();
    let arts: Vec<&str> = {
        let mut a: Vec<&str> = mine.iter().map(|m| m.meta.art).collect();
        a.sort();
        a.dedup();
        a
    };
    for art in &arts {
        let of: Vec<&&Measured> = mine.iter().filter(|m| m.meta.art == *art).collect();
        let mut roots: Vec<u8> = of.iter().map(|m| m.root).collect();
        roots.sort();
        roots.dedup();
        let n_dyn = of
            .iter()
            .map(|m| m.meta.dyn_rank)
            .max()
            .map_or(1, |d| d as usize + 1);
        for m in &of {
            let i = roots.iter().position(|r| *r == m.root).unwrap_or(0);
            let lo = if i == 0 {
                m.root.saturating_sub(5)
            } else {
                (roots[i - 1] + m.root) / 2 + 1
            };
            let hi = if i + 1 == roots.len() {
                m.root.saturating_add(5).min(127)
            } else {
                (m.root + roots[i + 1]) / 2
            };
            // Its round robin among the takes of the same note, dynamic and bow.
            let mut takes: Vec<u8> = of
                .iter()
                .filter(|o| {
                    o.root == m.root
                        && o.meta.dyn_rank == m.meta.dyn_rank
                        && o.meta.bow == m.meta.bow
                })
                .map(|o| o.meta.rr)
                .collect();
            takes.sort();
            let pos = takes.iter().position(|r| *r == m.meta.rr).unwrap_or(0) + 1;
            let ranks: Vec<u8> = {
                let mut r: Vec<u8> = of
                    .iter()
                    .filter(|o| o.root == m.root)
                    .map(|o| o.meta.dyn_rank)
                    .collect();
                r.sort();
                r.dedup();
                r
            };
            let rank = ranks
                .iter()
                .position(|r| *r == m.meta.dyn_rank)
                .unwrap_or(0);
            let short = !sustained(art);
            let (xf, vel) = if short {
                let n = ranks.len().max(1);
                let lo_v = (1 + 127 * rank / n).max(1) as u8;
                let hi_v = (127 * (rank + 1) / n) as u8;
                ([0, 0, 127, 127], Some([lo_v, hi_v]))
            } else {
                (layer_xf(rank, ranks.len()), None)
            };
            let _ = n_dyn;
            zones.push(StringsZone {
                file: m.rel.clone(),
                art: art.to_string(),
                bow: m.meta.bow.map(str::to_string),
                lo,
                hi,
                root: m.root,
                tune: (m.tune * 10.0).round() / 10.0,
                xf,
                vel,
                seq: (takes.len() > 1).then_some([pos as u8, takes.len() as u8]),
                looped: m.looped,
                xf_ms: (m.xf_ms * 10.0).round() / 10.0,
                gain_db: 0.0,
            });
        }
    }
    // The level: the sustains' loudest layer around the middle of the range at −16 dBFS.
    let loud: Vec<f32> = {
        let top = mine.iter().map(|m| m.meta.dyn_rank).max().unwrap_or(0);
        mine.iter()
            .filter(|m| m.meta.dyn_rank == top)
            .map(|m| m.rms_db)
            .collect()
    };
    let median = {
        let mut l = loud.clone();
        l.sort_by(|a, b| a.total_cmp(b));
        l.get(l.len() / 2).copied().unwrap_or(-16.0)
    };
    // The keyswitches an octave under the lowest zone, on a C — or, for an instrument too low
    // for an 88-key keyboard to reach under it (a bass), an octave over its highest.
    let lowest = zones.iter().map(|z| z.lo).min().unwrap_or(36);
    let highest = zones.iter().map(|z| z.hi).max().unwrap_or(84);
    let switch = if lowest >= 33 {
        (lowest - 12) / 12 * 12
    } else {
        ((highest as u16 + 12).div_ceil(12) * 12).min(120) as u8
    };
    StringsInstrument {
        sfz: format!("Cantus/{id}.sfz"),
        name: name.to_string(),
        switch: (arts.len() > 1).then_some(switch),
        like: None,
        default_art: None,
        legato: legato.then_some([90.0, 60.0, 90.0]),
        dynamics_cc: 1,
        gain_db: ((-16.0 - median) * 10.0).round() / 10.0,
        release_ms: [450.0, 250.0],
        zones,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let src = PathBuf::from(
        args.get(1)
            .expect("usage: strings_map <sources> <out.json> [pack]"),
    );
    let out = PathBuf::from(args.get(2).expect("the output"));
    let only = args.get(3).cloned();
    let mut packs = serde_json::Map::new();
    for lib in LIBS
        .iter()
        .filter(|l| only.as_deref().is_none_or(|o| o == l.pack))
    {
        let fs = files(&src, lib);
        eprintln!("{}: {} files", lib.pack, fs.len());
        let ms: Vec<Measured> = {
            use std::sync::Mutex;
            let done = Mutex::new(Vec::new());
            let next = std::sync::atomic::AtomicUsize::new(0);
            std::thread::scope(|s| {
                for _ in 0..4 {
                    s.spawn(|| loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((rel, b)) = fs.get(i) else { break };
                        let meta = (lib.parse)(rel).expect("parsed");
                        if let Some(m) = measure(rel, b, meta) {
                            done.lock().unwrap().push(m);
                        }
                    });
                }
            });
            let mut v = done.into_inner().unwrap();
            v.sort_by(|a, b| a.rel.cmp(&b.rel));
            agree_octaves(&mut v);
            fit_tunes(&mut v, &fs);
            v
        };
        let build = StringsBuild {
            instruments: lib
                .instruments
                .iter()
                .map(|(id, name, legato)| instrument(id, name, *legato, &ms))
                .filter(|i| !i.zones.is_empty())
                .collect(),
        };
        let report: Vec<serde_json::Value> = ms
            .iter()
            .map(|m| {
                serde_json::json!({
                    "file": m.rel, "named": m.meta.named, "root": m.root, "octave_fix": m.octave_fix,
                    "tune": m.tune, "cents": m.cents, "fit": m.fit, "fit_none": m.fit_none, "loop": m.looped, "loop_err_db": m.loop_err_db,
                    "seam_excess_db": m.seam_excess_db, "rms_db": m.rms_db, "frames": m.frames,
                })
            })
            .collect();
        let fixes: BTreeMap<i32, usize> = ms.iter().fold(BTreeMap::new(), |mut a, m| {
            *a.entry(m.octave_fix).or_default() += 1;
            a
        });
        eprintln!("{}: octave fixes {fixes:?}", lib.pack);
        packs.insert(
            lib.pack.to_string(),
            serde_json::json!({ "strings": build, "report": report }),
        );
    }
    std::fs::write(&out, serde_json::to_string_pretty(&packs).unwrap()).expect("write");
    eprintln!("wrote {}", out.display());
}
