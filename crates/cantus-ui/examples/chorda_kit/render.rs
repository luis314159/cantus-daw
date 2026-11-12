//! Rendering the phrases through the real engine (`cantus_engine::render`, the export's
//! path): Chorda at a model and settings, and the Salamander Grand as the picker installs it
//! (the pack's SFZ in Exemplar, all 16 velocity layers, streamed from disk).

use super::phrases::Phrase;
use cantus_core::automation::AutoPoint;
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot, Ticks};
use cantus_engine::{render, RenderOptions, SampleCache};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const SR: u32 = 48_000;
const BPM: f64 = 120.0;
pub const SALAMANDER: &str = "salamander-grand-piano";

/// A stereo render.
#[derive(Clone, Debug, Default)]
pub struct Stereo {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl Stereo {
    pub fn len(&self) -> usize {
        self.l.len()
    }
    pub fn gain(&mut self, g: f32) {
        for x in self.l.iter_mut().chain(self.r.iter_mut()) {
            *x *= g;
        }
    }
    pub fn slice(&self, a: usize, b: usize) -> Stereo {
        let b = b.min(self.len());
        let a = a.min(b);
        Stereo {
            l: self.l[a..b].to_vec(),
            r: self.r[a..b].to_vec(),
        }
    }
    pub fn mono(&self) -> Vec<f32> {
        self.l
            .iter()
            .zip(&self.r)
            .map(|(a, b)| 0.5 * (a + b))
            .collect()
    }
    pub fn peak(&self) -> f32 {
        self.l
            .iter()
            .chain(self.r.iter())
            .fold(0.0f32, |m, x| m.max(x.abs()))
    }
}

/// What plays a phrase.
#[derive(Clone, Debug)]
pub enum Source {
    Chorda {
        model: f32,
        values: Vec<(String, f32)>,
    },
    Salamander,
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Chorda { model, .. } => format!("chorda{}", *model as i32),
            Source::Salamander => "salamander".into(),
        }
    }
}

/// The content folder the comparison installs packs into (never the app's own).
pub fn content_root(downloads: &Path) -> PathBuf {
    downloads.join("content")
}

/// Install the Salamander Grand through Cantus's own pack code (download, SHA-256, unpack —
/// what *Sound packs…* does), unless it is there.
pub fn install(root: &Path) -> Result<(), String> {
    let mut m = cantus_content::Manager::new(root, cantus_content::Manifest::bundled());
    if m.is_installed(SALAMANDER) {
        return Ok(());
    }
    if !m.install(SALAMANDER) {
        return Err("the pack could not start installing".into());
    }
    eprintln!("installing {SALAMANDER} into {} …", root.display());
    for f in m.wait_idle(std::time::Duration::from_secs(3600)) {
        if let Err(e) = f.result {
            return Err(format!("{e}"));
        }
    }
    if m.is_installed(SALAMANDER) {
        Ok(())
    } else {
        Err("the install did not finish".into())
    }
}

/// The Salamander as the instrument picker gives it (`grand-full`: all 16 layers).
pub fn salamander_spec(root: &Path) -> Result<NativeSpec, String> {
    let m = cantus_content::Manager::new(root, cantus_content::Manifest::bundled());
    let inst = m
        .instruments()
        .into_iter()
        .find(|i| i.pack == SALAMANDER && i.instrument.id == "grand-full")
        .ok_or("the Salamander Grand is not installed (run `install` first)")?;
    let choice = cantus_ui::views::samples_content::sounds::SoundChoice(inst);
    let imp = cantus_ui::views::samples_content::sounds::import_of(&choice)?;
    Ok(cantus_ui::views::samples_content::sounds::spec_of(&imp))
}

pub fn chorda_spec(model: f32, values: &[(String, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Chorda);
    s.set("model", model);
    for (k, v) in values {
        s.set(k, *v);
    }
    s
}

fn ticks(s: f64) -> Ticks {
    Ticks::from_seconds(s, BPM)
}

/// One track playing `phrase` (its notes and its sustain pedal as a CC 64 lane).
pub fn project(spec: NativeSpec, phrase: &Phrase) -> Project {
    let mut p = Project::new(phrase.id);
    p.tempo_bpm = BPM;
    let mut t = p.make_track("Piano");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Play", Ticks::ZERO, ticks(phrase.len));
    for (start, len, key, vel) in &phrase.notes {
        let n = p.make_note(ticks(*start), ticks(*len), *key, *vel);
        clip.notes.push(n);
    }
    if !phrase.pedal.is_empty() {
        // A step at each point: the value before it held to its tick, then the new one.
        let mut pts = vec![AutoPoint::new(Ticks::ZERO, 0.0)];
        let mut last = 0.0;
        for (t, v) in &phrase.pedal {
            pts.push(AutoPoint::new(ticks(*t), last));
            pts.push(AutoPoint::new(ticks(*t), *v));
            last = *v;
        }
        clip.lanes.push(ClipLane::new(LaneKind::Cc(64), pts));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// Streamed samples need a cache folder of their own (the stream's rule: a tool sets one).
pub fn set_cache(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    cantus_engine::inst::stream::hub().set_cache_dir(dir.to_path_buf());
}

/// Render `phrase` through `spec` to its length (no tail: the phrases leave their own).
pub fn render_spec(spec: NativeSpec, phrase: &Phrase) -> Result<Stereo, String> {
    let p = project(spec, phrase);
    let mut snap = Snapshot::compile(&p, 1);
    let errors = SampleCache::new().attach(&mut snap);
    if let Some((path, why)) = errors.first() {
        return Err(format!("{}: {why}", path.display()));
    }
    let r = render(
        Arc::new(snap),
        &RenderOptions {
            sample_rate: SR,
            end: Some(ticks(phrase.len)),
            tail_seconds: 0.0,
            ..Default::default()
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    if r.cut_notes > 0 {
        return Err(format!("{} notes were cut short by streaming", r.cut_notes));
    }
    let mut s = Stereo::default();
    for f in r.samples.chunks(2) {
        s.l.push(f[0]);
        s.r.push(f[1]);
    }
    Ok(s)
}

pub fn render_source(src: &Source, root: &Path, phrase: &Phrase) -> Result<Stereo, String> {
    let spec = match src {
        Source::Chorda { model, values } => chorda_spec(*model, values),
        Source::Salamander => salamander_spec(root)?,
    };
    render_spec(spec, phrase)
}

/// 32-bit float WAV (the analysis's), or 24-bit PCM (the listening kit's: what every
/// browser plays).
pub fn write_wav(path: &Path, s: &Stereo, float: bool) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: if float { 32 } else { 24 },
        sample_format: if float {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        },
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for (a, b) in s.l.iter().zip(&s.r) {
        if float {
            w.write_sample(*a).map_err(|e| e.to_string())?;
            w.write_sample(*b).map_err(|e| e.to_string())?;
        } else {
            let q = |x: f32| (x.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
            w.write_sample(q(*a)).map_err(|e| e.to_string())?;
            w.write_sample(q(*b)).map_err(|e| e.to_string())?;
        }
    }
    w.finalize().map_err(|e| e.to_string())
}

/// 24-bit FLAC (`cantus-export`'s encoder, the export dialog's): lossless, about half a
/// 24-bit WAV, and played by every current browser — the kit as a web page's (`--flac`).
pub fn write_flac(path: &Path, s: &Stereo) -> Result<(), String> {
    use cantus_export::settings::{Depth, Dither, Tags};
    let mut samples = Vec::with_capacity(s.l.len() * 2);
    for (a, b) in s.l.iter().zip(&s.r) {
        samples.push(*a);
        samples.push(*b);
    }
    let audio = cantus_export::Audio {
        rate: SR,
        channels: 2,
        samples,
    };
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    let mut progress = |_: f32| true;
    cantus_export::encode::flac(
        &mut out,
        &audio,
        Depth::Int24,
        Dither::Off,
        &Tags::default(),
        "",
        &mut progress,
    )
    .map_err(|e| format!("{e:?}"))?;
    use std::io::Write as _;
    out.flush().map_err(|e| e.to_string())
}

pub fn read_wav(path: &Path) -> Result<Stereo, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = r.spec();
    let mut s = Stereo::default();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|x| x.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|x| x.unwrap_or(0) as f32 * scale)
                .collect()
        }
    };
    for f in samples.chunks(spec.channels as usize) {
        s.l.push(f[0]);
        s.r.push(*f.get(1).unwrap_or(&f[0]));
    }
    Ok(s)
}
