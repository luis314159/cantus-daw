//! A video export from start to finish, on the caller's (background) thread: frames drawn
//! on workers, encoded and written in order with the audio of each frame, the file put in
//! its place only when whole.

use crate::audio::{to_i16, Pcm};
use crate::mov::MovWriter;
use crate::mp4::{AudioParams, Mp4Writer};
use crate::pipeline::{self, PipelineError};
use crate::select::{self, Codecs, Plan, Warning};
use crate::seq::{self, SeqWriter};
use crate::timing::Timing;
use crate::yuv::{self, Nv12};
use crate::{Container, Control, Quality, VideoError};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tiny_skia::Pixmap;

/// What draws the frames: frame `n` (at `n / fps` seconds) into a pixmap of the export's
/// size, premultiplied RGBA. The pixmap comes cleared to transparent. Called from several
/// worker threads at once, for different frames.
pub trait FrameSource: Sync {
    fn draw(&self, n: u32, pixmap: &mut Pixmap);
}

impl<F: Fn(u32, &mut Pixmap) + Sync> FrameSource for F {
    fn draw(&self, n: u32, pixmap: &mut Pixmap) {
        self(n, pixmap)
    }
}

/// What to export.
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub container: Container,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
    pub quality: Quality,
    /// Keep the transparent background (MOV and PNG; MP4 has none).
    pub alpha: bool,
    /// The song, at 44 100 or 48 000 Hz; cut to the video's length, silence past its end.
    pub audio: &'a Pcm,
    /// Worker threads drawing frames (0: [`pipeline::default_workers`]).
    pub workers: usize,
    /// Replace a file already at the path (else the export refuses to).
    pub replace: bool,
    /// The MP4's encoders ([`crate::select::plan`]); `None`: the system's own.
    pub plan: Option<Plan>,
}

/// What an export did.
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    /// The file (or the sequence's folder).
    pub path: PathBuf,
    pub frames: u32,
    /// The video's length.
    pub seconds: f64,
    /// Bytes written.
    pub bytes: u64,
    /// How long the export took.
    pub wall: f64,
    /// The encoders, in words ("Media Foundation H.264 (software) + Media Foundation AAC-LC").
    pub encoder: String,
    /// What the person should know (an MP4 with MP3 audio).
    pub warnings: Vec<Warning>,
}

/// The largest picture side the export offers (H.264 level 5.1 and every platform).
pub const MAX_SIDE: u32 = 4_096;

fn check(req: &Request) -> Result<Timing, VideoError> {
    let (w, h) = (req.width, req.height);
    if w < 16 || h < 16 || w > MAX_SIDE || h > MAX_SIDE || w % 2 == 1 || h % 2 == 1 {
        return Err(VideoError::Unsupported(format!(
            "a {w} × {h} picture (each side even, 16 to {MAX_SIDE} pixels)"
        )));
    }
    if req.frames == 0 {
        return Err(VideoError::Unsupported("a video of no frames".into()));
    }
    Timing::new(req.fps, req.audio.rate, req.frames)
}

/// Export `req` drawn by `source` into `path` (a folder for a PNG sequence).
pub fn export(
    req: &Request,
    source: &dyn FrameSource,
    path: &Path,
    ctl: &Control,
) -> Result<Done, VideoError> {
    let timing = check(req)?;
    sweep_stale(path);
    let started = Instant::now();
    ctl.start(req.frames);
    if req.container == Container::PngSequence {
        return export_sequence(req, timing, source, path, ctl, started);
    }
    if path.exists() && !req.replace {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} is already there", path.display()),
        )
        .into());
    }
    // The work in progress under a name of our own, made new: nothing already there (a
    // user's `clip.mp4.part`) is ever opened, replaced or removed (review 1 #7).
    let (part, file) = made_new(path, |p| {
        std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(p)
    })?;
    let written = (|| {
        // Unbuffered: every write is a whole sample, and the index's last step reads back.
        let (file, wrote) = match req.container {
            Container::Mp4 => write_mp4(req, timing, source, file, ctl)?,
            Container::WebM => write_webm(req, timing, source, file, ctl)?,
            _ => write_mov(req, timing, source, file, ctl)?,
        };
        file.sync_all()?;
        Ok::<_, VideoError>(wrote)
    })();
    let wrote = match written {
        Ok(e) => e,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    let placed = if req.replace {
        std::fs::rename(&part, path)
    } else {
        cantus_export::job::place_new(&part, path)
    };
    if let Err(e) = placed {
        let _ = std::fs::remove_file(&part);
        return Err(e.into());
    }
    Ok(Done {
        path: path.to_path_buf(),
        frames: req.frames,
        seconds: timing.seconds(),
        bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        wall: started.elapsed().as_secs_f64(),
        encoder: wrote.0,
        warnings: wrote.1,
    })
}

/// Our own work in progress beside `target` that an ended process left (a quit or a crash
/// during an export): `.<name>.cantus-part-<pid>-<n>` whose process is gone — removed,
/// file or folder. Anything else, and the parts of a Cantus still running, stay
/// (review 2 #2).
pub fn sweep_stale(target: &Path) {
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return;
    };
    let prefix = format!(".{}.cantus-part-", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    }) else {
        return;
    };
    for e in entries.flatten() {
        let file = e.file_name().to_string_lossy().into_owned();
        let Some(rest) = file.strip_prefix(&prefix) else {
            continue;
        };
        let Some((pid, n)) = rest.split_once('-') else {
            continue;
        };
        let (Ok(pid), true) = (pid.parse::<u32>(), n.bytes().all(|b| b.is_ascii_digit())) else {
            continue;
        };
        if pid == std::process::id() || process_alive(pid) {
            continue;
        }
        let p = e.path();
        let _ = match std::fs::symlink_metadata(&p) {
            Ok(m) if m.is_dir() => std::fs::remove_dir_all(&p),
            Ok(_) => std::fs::remove_file(&p),
            Err(_) => Ok(()),
        };
    }
}

/// Whether a process with this id is running (when in doubt, yes: nothing is removed).
pub fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        crate::mf::process_alive(pid)
    }
    #[cfg(not(windows))]
    {
        if Path::new("/proc").is_dir() {
            return Path::new(&format!("/proc/{pid}")).exists();
        }
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_or(true, |s| s.success())
    }
}

/// A file or folder made new beside `target` for the work in progress, under a name of
/// ours (`.<name>.cantus-part-<process>-<n>`): `make` must fail with `AlreadyExists` when
/// something is there, and then the next number is tried. What is returned is ours alone
/// to fill, rename or remove.
pub fn made_new<T>(
    target: &Path,
    make: impl Fn(&Path) -> std::io::Result<T>,
) -> std::io::Result<(PathBuf, T)> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = target.with_file_name(format!(".{name}.cantus-part-{}-{n}", std::process::id()));
        match make(&p) {
            Ok(v) => return Ok((p, v)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

/// The pipeline's errors as the export's.
fn lift(e: PipelineError<VideoError>, ctl: &Control) -> VideoError {
    match e {
        PipelineError::Consumer(e) => e,
        PipelineError::WorkerLost(_) if ctl.cancelled() => VideoError::Cancelled,
        PipelineError::WorkerLost(n) => {
            VideoError::encoder("frame", format!("drawing frame {n} failed"))
        }
    }
}

fn workers(req: &Request) -> usize {
    if req.workers == 0 {
        pipeline::default_workers()
    } else {
        req.workers
    }
}

/// A worker's pixmap, cleared, with frame `n` drawn on it.
fn draw<'p>(source: &dyn FrameSource, pm: &'p mut Pixmap, n: u32) -> &'p Pixmap {
    pm.fill(tiny_skia::Color::TRANSPARENT);
    source.draw(n, pm);
    pm
}

fn new_pixmap(w: u32, h: u32) -> Pixmap {
    Pixmap::new(w, h).expect("a checked, non-zero size")
}

/// The audio of frame `n` as interleaved stereo 16-bit.
fn frame_audio(audio: &Pcm, t: &Timing, n: u32, floats: &mut Vec<f32>, out: &mut Vec<i16>) {
    audio.stereo(t.audio_of_frame(n), floats);
    to_i16(floats, out);
}

fn write_mp4<W: std::io::Read + std::io::Write + std::io::Seek>(
    req: &Request,
    t: Timing,
    source: &dyn FrameSource,
    out: W,
    ctl: &Control,
) -> Result<(W, (String, Vec<Warning>)), VideoError> {
    // Media Foundation started for this thread where the system has it (not Windows N).
    #[cfg(windows)]
    let _session = crate::mf::Session::start().ok();
    let plan = req
        .plan
        .clone()
        .unwrap_or_else(|| select::plan(&Codecs::default()));
    let bitrate = req.quality.bitrate(req.width, req.height, req.fps);
    let picked = select::make(
        &plan,
        req.width,
        req.height,
        req.fps,
        bitrate,
        req.audio.rate,
    )?;
    let (mut venc, mut aenc, warnings) = (picked.video, picked.audio, picked.warnings);
    let params = AudioParams {
        codec: aenc.codec(),
        rate: aenc.rate(),
        channels: aenc.channels(),
        frame_len: aenc.frame_len(),
        delay: aenc.delay(),
    };
    let mut mp4 = Mp4Writer::new(out, req.width, req.height, t, params)?;
    let (w, h) = (req.width, req.height);
    let (mut aus, mut packets) = (Vec::new(), Vec::new());
    let (mut floats, mut pcm) = (Vec::new(), Vec::new());
    pipeline::run(
        t.frames,
        workers(req),
        &ctl.cancel,
        || new_pixmap(w, h),
        |pm, n| {
            let mut f = Nv12::default();
            yuv::rgba_to_nv12(draw(source, pm, n).data(), w, h, &mut f);
            f
        },
        |n, frame| {
            if ctl.cancelled() {
                return Err(VideoError::Cancelled);
            }
            venc.encode(&frame, &mut aus)?;
            for au in aus.drain(..) {
                mp4.video(&au)?;
            }
            frame_audio(req.audio, &t, n, &mut floats, &mut pcm);
            aenc.encode(&pcm, &mut packets)?;
            for p in packets.drain(..) {
                mp4.audio(&p)?;
            }
            ctl.frame_done();
            Ok(())
        },
    )
    .map_err(|e| lift(e, ctl))?;
    venc.finish(&mut aus)?;
    for au in aus.drain(..) {
        mp4.video(&au)?;
    }
    aenc.finish(&mut packets)?;
    for p in packets.drain(..) {
        mp4.audio(&p)?;
    }
    let name = format!("{} + {}", venc.name(), aenc.name());
    Ok((mp4.finish()?, (name, warnings)))
}

fn write_webm<W: std::io::Write + std::io::Seek>(
    req: &Request,
    t: Timing,
    source: &dyn FrameSource,
    out: W,
    ctl: &Control,
) -> Result<(W, (String, Vec<Warning>)), VideoError> {
    use crate::webm::{Tracks, WebmWriter, AUDIO, VIDEO};
    let q = crate::av1::quantizer(req.quality);
    let mut av1 = crate::av1::Av1::new(req.width, req.height, req.fps, q)?;
    let mut vorbis = crate::vorbis::Vorbis::new(t.rate, 2)?;
    let mut apk = Vec::new();
    vorbis.encode(&[], &mut apk)?;
    let audio_private = vorbis
        .private()
        .ok_or_else(|| VideoError::encoder("Vorbis", "no headers"))?;
    let tracks = Tracks {
        width: req.width,
        height: req.height,
        fps: req.fps,
        video_private: av1.private(),
        rate: t.rate,
        channels: 2,
        audio_private,
        duration_ms: t.seconds() * 1000.0,
    };
    let mut webm = WebmWriter::new(out, &tracks)?;
    let video_ms = |frame: u64| (frame * 1000 * 2 + t.fps as u64) / (2 * t.fps as u64);
    let audio_ms = |s: u64| (s * 1000 * 2 + t.rate as u64) / (2 * t.rate as u64);
    let (w, h) = (req.width, req.height);
    let (mut vpk, mut floats) = (Vec::new(), Vec::new());
    let put = |webm: &mut WebmWriter<W>,
               vpk: &mut Vec<crate::av1::Av1Packet>,
               apk: &mut Vec<crate::vorbis::Packet>|
     -> Result<(), VideoError> {
        for p in vpk.drain(..) {
            webm.block(VIDEO, video_ms(p.frame), p.key, p.data)?;
        }
        for p in apk.drain(..) {
            webm.block(AUDIO, audio_ms(p.start), true, p.data)?;
        }
        Ok(())
    };
    pipeline::run(
        t.frames,
        workers(req),
        &ctl.cancel,
        || new_pixmap(w, h),
        |pm, n| {
            let mut f = Nv12::default();
            yuv::rgba_to_nv12(draw(source, pm, n).data(), w, h, &mut f);
            f
        },
        |n, frame| {
            if ctl.cancelled() {
                return Err(VideoError::Cancelled);
            }
            av1.encode(&frame, &mut vpk)?;
            req.audio.stereo(t.audio_of_frame(n), &mut floats);
            vorbis.encode(&floats, &mut apk)?;
            put(&mut webm, &mut vpk, &mut apk)?;
            ctl.frame_done();
            Ok(())
        },
    )
    .map_err(|e| lift(e, ctl))?;
    av1.finish(&mut vpk)?;
    vorbis.finish(&mut apk)?;
    put(&mut webm, &mut vpk, &mut apk)?;
    let name = "AV1 (rav1e) + Vorbis".to_string();
    Ok((webm.finish()?, (name, Vec::new())))
}

fn write_mov<W: std::io::Read + std::io::Write + std::io::Seek>(
    req: &Request,
    t: Timing,
    source: &dyn FrameSource,
    out: W,
    ctl: &Control,
) -> Result<(W, (String, Vec<Warning>)), VideoError> {
    let mut mov = MovWriter::new(out, req.width, req.height, t, req.alpha)?;
    let (w, h, alpha) = (req.width, req.height, req.alpha);
    let (mut floats, mut pcm) = (Vec::new(), Vec::new());
    pipeline::run(
        t.frames,
        workers(req),
        &ctl.cancel,
        || (new_pixmap(w, h), Vec::new()),
        |(pm, scratch), n| seq::encode_png(draw(source, pm, n).data(), w, h, alpha, scratch),
        |n, png| {
            if ctl.cancelled() {
                return Err(VideoError::Cancelled);
            }
            frame_audio(req.audio, &t, n, &mut floats, &mut pcm);
            mov.frame(&png, &pcm)?;
            ctl.frame_done();
            Ok(())
        },
    )
    .map_err(|e| lift(e, ctl))?;
    let kind = if alpha { "PNG with alpha" } else { "PNG" };
    Ok((mov.finish()?, (format!("{kind} + 16-bit PCM"), Vec::new())))
}

fn export_sequence(
    req: &Request,
    t: Timing,
    source: &dyn FrameSource,
    dir: &Path,
    ctl: &Control,
    started: Instant,
) -> Result<Done, VideoError> {
    if dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} is already there", dir.display()),
        )
        .into());
    }
    let stem = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".into());
    let (part, mut out) = made_new(dir, |p| SeqWriter::new(p, &stem, t.frames))?;
    let (w, h, alpha) = (req.width, req.height, req.alpha);
    let written = (|| {
        pipeline::run(
            t.frames,
            workers(req),
            &ctl.cancel,
            || (new_pixmap(w, h), Vec::new()),
            |(pm, scratch), n| seq::encode_png(draw(source, pm, n).data(), w, h, alpha, scratch),
            |_, png| {
                if ctl.cancelled() {
                    return Err(VideoError::Cancelled);
                }
                out.frame(&png)?;
                ctl.frame_done();
                Ok(())
            },
        )
        .map_err(|e| lift(e, ctl))?;
        let bytes = out.bytes();
        let (mut floats, mut pcm) = (Vec::new(), Vec::new());
        req.audio.stereo(0..t.audio_samples(), &mut floats);
        to_i16(&floats, &mut pcm);
        out.finish(&pcm, t.rate)?;
        Ok::<_, VideoError>(bytes + 44 + pcm.len() as u64 * 2)
    })();
    let bytes = match written {
        Ok(b) => b,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&part);
            return Err(e);
        }
    };
    if let Err(e) = std::fs::rename(&part, dir) {
        let _ = std::fs::remove_dir_all(&part);
        return Err(e.into());
    }
    Ok(Done {
        path: dir.to_path_buf(),
        frames: req.frames,
        seconds: t.seconds(),
        bytes,
        wall: started.elapsed().as_secs_f64(),
        warnings: Vec::new(),
        encoder: if alpha {
            "PNG with alpha + WAV"
        } else {
            "PNG + WAV"
        }
        .into(),
    })
}
