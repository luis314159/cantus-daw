//! Stems in every format: the engine's one-pass stems render (ADR 0021 §3) into a hidden
//! folder of 32-bit float WAV files beside the destination, then each converted and
//! encoded beside its place. Stems are never loudness-normalised — one by one they would
//! no longer add up to the mix. Every stem is written whole (`name.ext.part`) before any
//! is put in its place, so a stopped or failed export leaves the folder as it found it —
//! the stems of an earlier export into it included.

use crate::job::{
    clipped, commit_all, encode_one, peak_db, stem_output_name, Control, Done, Phase, StemsJob,
};
use crate::process::Audio;
use crate::settings::{ExportSettings, Tags};
use crate::ExportError;
use std::path::{Path, PathBuf};

/// A folder removed with everything in it when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn read_wav(path: &Path) -> Result<Audio, ExportError> {
    let r = hound::WavReader::open(path).map_err(|e| ExportError::Render(e.to_string()))?;
    let spec = r.spec();
    let samples: Result<Vec<f32>, _> = r.into_samples::<f32>().collect();
    Ok(Audio {
        rate: spec.sample_rate,
        channels: spec.channels,
        samples: samples.map_err(|e| ExportError::Render(e.to_string()))?,
    })
}

/// The stem's name as the desk shows it: `03 Bass.wav` → `Bass`.
fn display_name(file: &str) -> &str {
    let base = file.strip_suffix(".wav").unwrap_or(file);
    match base.split_once(' ') {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => base,
    }
}

/// The share of the progress bar the render takes; encoding the stems has the rest.
const RENDER_SHARE: f32 = 0.6;

/// Render the stems and write each in the chosen format into `folder`.
pub fn export_stems(
    job: StemsJob,
    s: &ExportSettings,
    tags: &Tags,
    folder: &Path,
    ctl: &Control,
) -> Result<Done, ExportError> {
    std::fs::create_dir_all(folder)?;
    let scratch = Scratch(folder.join(format!(".cantus-stems-{}", std::process::id())));
    ctl.set(Phase::Render, 0.0);
    let rendered = cantus_engine::render_stems_to_folder_with(
        job.snapshot,
        &job.options,
        &job.specs,
        &scratch.0,
        job.units,
        job.instruments,
        &mut ctl.span(Phase::Render, 0.0, RENDER_SHARE),
    )
    .map_err(|e| match e {
        cantus_engine::StemsError::Cancelled => ExportError::Cancelled,
        cantus_engine::StemsError::Empty => ExportError::Empty,
        e => ExportError::Render(e.to_string()),
    })?;
    let mut done = Done {
        silent: rendered
            .silent
            .iter()
            .map(|n| stem_output_name(n, s))
            .collect(),
        unaligned: rendered.unaligned,
        cut_notes: rendered.cut_notes,
        seconds: rendered.seconds,
        peak_db: f32::NEG_INFINITY,
        ..Default::default()
    };
    let n = rendered.written.len().max(1) as f32;
    let mut parts = Vec::with_capacity(rendered.written.len());
    let result = (|| -> Result<(), ExportError> {
        for (i, wav) in rendered.written.iter().enumerate() {
            let name = wav
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let audio = read_wav(wav)?;
            // Disk space: the float file goes as soon as it is read.
            let _ = std::fs::remove_file(wav);
            let mut t = tags.clone();
            let stem = display_name(&name);
            t.title = if tags.title.trim().is_empty() {
                stem.to_string()
            } else {
                format!("{} — {stem}", tags.title.trim())
            };
            let from = RENDER_SHARE + (1.0 - RENDER_SHARE) * i as f32 / n;
            let to = RENDER_SHARE + (1.0 - RENDER_SHARE) * (i + 1) as f32 / n;
            let path = folder.join(stem_output_name(&name, s));
            let (part, a, _) = encode_one(audio, s, &t, &path, ctl, (from, to), false)?;
            parts.push(part);
            done.clipped += clipped(&a, s);
            done.rate = a.rate;
            done.peak_db = done.peak_db.max(peak_db(&a));
        }
        Ok(())
    })();
    drop(scratch);
    // Stopped or failed: the parts go when dropped, and nothing in the folder was touched;
    // then all of them go in, or none.
    result?;
    done.files = commit_all(parts)?;
    Ok(done)
}

/// The names the stems would take in `s`'s format (for the dialog and the tests).
pub fn output_names(specs: &[cantus_engine::StemSpec], s: &ExportSettings) -> Vec<String> {
    specs
        .iter()
        .map(|spec| stem_output_name(&spec.file_name, s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::display_name;

    #[test]
    fn the_display_name_drops_the_number_and_the_extension() {
        assert_eq!(display_name("03 Bass.wav"), "Bass");
        assert_eq!(display_name("00 Master.wav"), "Master");
        assert_eq!(display_name("Lead vox.wav"), "Lead vox");
    }
}
