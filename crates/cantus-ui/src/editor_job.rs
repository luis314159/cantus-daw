//! The audio editor's work off the interface thread (ADR 0096): reading a take, making an
//! edit and writing it as a content-addressed file, learning a noise profile, drawing a
//! spectrogram (whole, or the visible span in detail). Each function is plain data in, plain
//! data out; the module's thread runs one at a time and answers through a channel the
//! interface polls once a frame. The agents' tools call the same functions on their jobs'
//! threads.

use crate::waveform::SourcePeaks;
use cantus_core::audio_edit::{self, wav, AudioBuf, Edit, Edited, Joins, Sel};
use cantus_dsp::denoise::{stft::Spectrogram, NoiseProfile};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// What the thread is asked.
pub enum Job {
    /// Read the file at `path` whole, at its own rate.
    Load { path: PathBuf },
    /// Make `edit` of `sel` of `buf`, write the result into `dir` (a name from its content).
    Apply {
        buf: Arc<AudioBuf>,
        sel: Sel,
        edit: Box<Edit>,
        joins: Joins,
        dir: PathBuf,
    },
    /// Read `path`, conform it to the take, and paste it at `sel`.
    PasteFile {
        buf: Arc<AudioBuf>,
        sel: Sel,
        path: PathBuf,
        joins: Joins,
        dir: PathBuf,
    },
    /// Learn what noise sounds like from `sel`.
    Learn { buf: Arc<AudioBuf>, sel: Sel },
    /// The picture of `buf` for the spectral view.
    Picture { buf: Arc<AudioBuf> },
    /// The picture of `start..end` of `buf` at `hop` (the view zoomed in).
    Detail {
        buf: Arc<AudioBuf>,
        start: usize,
        end: usize,
        hop: usize,
    },
    /// Write `part` (a selection) as a file of its own into `dir`.
    Export {
        part: AudioBuf,
        dir: PathBuf,
        from: usize,
    },
}

/// The audio a view needs of a buffer: its peaks.
pub struct Loaded {
    pub buf: Arc<AudioBuf>,
    pub peaks: Arc<SourcePeaks>,
}

/// What came back.
pub enum Done {
    Loaded(Result<Loaded, String>),
    Applied(Result<Applied, String>),
    Learned(Result<NoiseProfile, String>),
    Picture(Arc<Spectrogram>),
    Detail(usize, Arc<Spectrogram>),
    /// The file a selection was written as (and whether this wrote it), and where the
    /// selection began in the take.
    Exported(Result<(PathBuf, bool), String>, usize),
}

/// An edit made and written.
pub struct Applied {
    /// The edit's name (`Edit::name`): its undo label and what the status says.
    pub name: &'static str,
    /// Whether the edit leaves the take's timing where it was (`Edit::keeps_timing`).
    pub keeps_timing: bool,
    pub edited: Edited,
    pub peaks: Arc<SourcePeaks>,
    pub buf: Arc<AudioBuf>,
    /// Where the file is, and its SHA-256 (hex) — the pool's `sha256`.
    pub file: PathBuf,
    pub sha256: String,
    /// This edit wrote the file (it was not there): if the edit is never made, it goes.
    pub fresh: bool,
}

/// Why an edit changed nothing (said as *unchanged*, never as an error).
pub const UNCHANGED: &str = "nothing changed";

fn peaks_of(buf: &AudioBuf) -> Arc<SourcePeaks> {
    Arc::new(SourcePeaks::compute(buf.rate, buf.channels, &buf.data))
}

/// Read `path` as the editor edits it: at the file's own rate and channels.
pub fn read(path: &Path) -> Result<AudioBuf, String> {
    let data = cantus_engine::SampleCache::new().load(path)?;
    let buf = AudioBuf::new(data.rate, data.channels.max(1), data.frames.to_vec());
    if buf.frames() == 0 {
        return Err("the file has no audio".into());
    }
    Ok(buf)
}

pub fn load(path: &Path) -> Result<Loaded, String> {
    let buf = Arc::new(read(path)?);
    let peaks = peaks_of(&buf);
    Ok(Loaded { buf, peaks })
}

/// `clip` at the take's rate (the engine's windowed-sinc resampler, the one every file
/// the engine plays goes through) and channel count (mono doubled, stereo averaged).
pub fn conform(clip: &AudioBuf, rate: u32, channels: u8) -> AudioBuf {
    let from = clip.channels.max(1) as usize;
    let data = cantus_engine::samples::resample::resample(&clip.data, from, clip.rate, rate);
    let frames = data.len() / from;
    let to = channels.max(1) as usize;
    let mut out = vec![0.0f32; frames * to];
    for f in 0..frames {
        let src = &data[f * from..f * from + from];
        for (c, o) in out[f * to..f * to + to].iter_mut().enumerate() {
            *o = if from == to {
                src[c]
            } else if from == 1 {
                src[0]
            } else if to == 1 {
                src.iter().sum::<f32>() / from as f32
            } else {
                src[c.min(from - 1)]
            };
        }
    }
    AudioBuf::new(rate, channels.max(1), out)
}

/// Write `buf` into `dir` as the file its bytes name; a file that is there already with
/// these bytes is the file (the same edit twice is one file). Returns its path, its hash
/// and whether this call wrote it.
pub fn write_addressed(buf: &AudioBuf, dir: &Path) -> Result<(PathBuf, String, bool), String> {
    let bytes = wav::encode(buf);
    let hash = wav::sha256_hex(&bytes);
    let path = dir.join(wav::file_name(&hash));
    let same = std::fs::read(&path).is_ok_and(|b| wav::sha256_hex(&b) == hash);
    if same {
        return Ok((path, hash, false));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Beside it first, then in place: a cut write never leaves half a file under the name
    // that says it is whole (a leftover `.part` is tidied, `editor::commit::tidy`).
    let tmp = dir.join(format!("{}.part", wav::file_name(&hash)));
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok((path, hash, true))
}

/// Make an edit (a paste conformed to the take first), and write it unless it changed
/// nothing.
pub fn apply(
    buf: Arc<AudioBuf>,
    sel: Sel,
    edit: &Edit,
    joins: &Joins,
    dir: &Path,
) -> Result<Applied, String> {
    let conformed;
    let edit = match edit {
        Edit::Paste { clip } if clip.rate != buf.rate || clip.channels != buf.channels => {
            conformed = Edit::Paste {
                clip: conform(clip, buf.rate, buf.channels),
            };
            &conformed
        }
        e => e,
    };
    let edited = audio_edit::apply_with(&buf, sel, edit, joins).map_err(|e| e.to_string())?;
    if !edited.changed {
        return Err(UNCHANGED.into());
    }
    let (file, sha256, fresh) = write_addressed(&edited.buf, dir)?;
    let out = Arc::new(edited.buf.clone());
    let peaks = peaks_of(&out);
    Ok(Applied {
        name: edit.name(),
        keeps_timing: edit.keeps_timing(),
        edited,
        peaks,
        buf: out,
        file,
        sha256,
        fresh,
    })
}

/// A noise profile from `sel`.
pub fn learn(buf: &AudioBuf, sel: Sel) -> Result<NoiseProfile, String> {
    let part = buf.slice(sel);
    NoiseProfile::learn(&part.planar(), buf.rate).map_err(|e| e.to_string())
}

/// The spectrogram of the mono mix, read sparser for a long take so the picture stays
/// within what a texture holds; [`detail`] draws a zoomed-in span at its own hop.
pub fn picture(buf: &AudioBuf) -> Spectrogram {
    let hop = (buf.frames() / 6000).max(512);
    Spectrogram::of(&buf.mono(), buf.rate, hop)
}

/// The spectrogram of `start..end` at `hop` (its first column at `start`).
pub fn detail(buf: &AudioBuf, start: usize, end: usize, hop: usize) -> Spectrogram {
    let end = end.min(buf.frames()).max(start + 1);
    let mono = buf.slice(Sel::new(start, end)).mono();
    Spectrogram::of(&mono, buf.rate, hop)
}

fn run(job: Job) -> Done {
    match job {
        Job::Load { path } => Done::Loaded(load(&path)),
        Job::Apply {
            buf,
            sel,
            edit,
            joins,
            dir,
        } => Done::Applied(apply(buf, sel, &edit, &joins, &dir)),
        Job::PasteFile {
            buf,
            sel,
            path,
            joins,
            dir,
        } => Done::Applied(
            read(&path).and_then(|clip| apply(buf, sel, &Edit::Paste { clip }, &joins, &dir)),
        ),
        Job::Learn { buf, sel } => Done::Learned(learn(&buf, sel)),
        Job::Picture { buf } => Done::Picture(Arc::new(picture(&buf))),
        Job::Detail {
            buf,
            start,
            end,
            hop,
        } => Done::Detail(start, Arc::new(detail(&buf, start, end, hop))),
        Job::Export { part, dir, from } => Done::Exported(
            write_addressed(&part, &dir).map(|(p, _, fresh)| (p, fresh)),
            from,
        ),
    }
}

/// The editor's thread: a queue in, a channel out.
pub struct Worker {
    tx: Sender<(u64, Job)>,
    rx: Receiver<(u64, Done)>,
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

impl Worker {
    pub fn new() -> Self {
        let (tx, jobs) = channel::<(u64, Job)>();
        let (done_tx, rx) = channel::<(u64, Done)>();
        let _ = std::thread::Builder::new()
            .name("cantus-editor".into())
            .spawn(move || {
                while let Ok((gen, job)) = jobs.recv() {
                    if done_tx.send((gen, run(job))).is_err() {
                        break;
                    }
                }
            });
        Worker { tx, rx }
    }

    pub fn send(&self, gen: u64, job: Job) -> bool {
        self.tx.send((gen, job)).is_ok()
    }

    pub fn poll(&self) -> Option<(u64, Done)> {
        self.rx.try_recv().ok()
    }
}
