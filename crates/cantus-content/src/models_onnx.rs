//! Preparing a model whose maker ships an ONNX graph (round 34, builder transcribe, ADR
//! 0086): Basic Pitch's `nmp.onnx` is the graph, weights inside. Nothing is rebuilt: its
//! hash, checked by the content manager when it arrived, is checked again (the file may
//! have been touched since), and the marker is written last, so a model is *ready* only
//! once it is whole. A model may name a same-length byte edit ([`OnnxPatch`]) made to a
//! copy — Onsets and Frames Drums' output gather pointed at the hi-hat's trained pitch —
//! the copy's hash checked against the manifest's.

use super::{Marker, Model, MARKER};
use crate::models_th::WeightMap;
use std::path::Path;

/// The weight map of a model with none to rebuild.
pub(super) fn no_map() -> WeightMap {
    WeightMap {
        format: 0,
        source_sha256: String::new(),
        graph_sha256: String::new(),
        weights_size: 0,
        weights_sha256: String::new(),
        tensors: Vec::new(),
    }
}

/// A same-length byte edit of a maker's graph: `find` (hex) occurs exactly once and becomes
/// `replace` (hex); the edited file hashes to `sha256`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnnxPatch {
    pub find: String,
    pub replace: String,
    pub sha256: String,
}

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{s} is not hex"));
    }
    Ok((0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0))
        .collect())
}

/// What validation asks of a patch: hex of the same non-zero length, a hash.
pub(super) fn check_patch(p: &OnnxPatch) -> Result<(), String> {
    let (a, b) = (unhex(&p.find)?, unhex(&p.replace)?);
    if a.is_empty() || a.len() != b.len() {
        return Err("a patch replaces bytes with as many".into());
    }
    if p.sha256.len() != 64 || !p.sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("a patch names its result's hash".into());
    }
    Ok(())
}

/// `bytes` with the patch made, or why not (not found, or found more than once).
pub fn apply(bytes: &[u8], p: &OnnxPatch) -> Result<Vec<u8>, String> {
    check_patch(p)?;
    let (find, replace) = (unhex(&p.find)?, unhex(&p.replace)?);
    let hits: Vec<usize> = bytes
        .windows(find.len())
        .enumerate()
        .filter(|(_, w)| *w == &find[..])
        .map(|(i, _)| i)
        .take(2)
        .collect();
    if hits.len() != 1 {
        return Err(format!(
            "the patch's bytes occur {} times, not once",
            hits.len()
        ));
    }
    let mut out = bytes.to_vec();
    out[hits[0]..hits[0] + find.len()].copy_from_slice(&replace);
    Ok(out)
}

pub(super) fn prepare(
    m: &Model,
    dir: &Path,
    file: &str,
    patch: Option<&OnnxPatch>,
) -> Result<(), String> {
    let entry = m
        .files
        .iter()
        .find(|f| f.to == file)
        .ok_or_else(|| format!("{file} is not among the model's files"))?;
    let path = dir.join(file);
    let got = crate::fetch::sha256_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if got != entry.sha256 {
        return Err(format!("{file} hashes to {got}, not {}", entry.sha256));
    }
    let stem = file.trim_end_matches(".onnx");
    let (graph, sha) = match patch {
        None => (stem.to_string(), got),
        Some(p) => {
            // The edit goes to a copy: the maker's file stays as it arrived.
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let edited = apply(&bytes, p)?;
            let h = crate::fetch::hex(&<sha2::Sha256 as sha2::Digest>::digest(&edited));
            if h != p.sha256 {
                return Err(format!("{file} edited hashes to {h}, not {}", p.sha256));
            }
            let name = format!("{stem}.cantus");
            let out = dir.join(format!("{name}.onnx"));
            let tmp = dir.join(format!("{name}.onnx.part"));
            std::fs::write(&tmp, &edited).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
            (name, h)
        }
    };
    let size = std::fs::metadata(dir.join(format!("{graph}.onnx")))
        .map_err(|e| e.to_string())?
        .len();
    let k = Marker {
        id: m.id.clone(),
        version: m.version,
        graph,
        graph_sha256: sha,
        // The graph's own size: what a later start checks it is whole by.
        weights_size: size,
        weights_sha256: String::new(),
    };
    let json = serde_json::to_vec_pretty(&k).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(MARKER), json).map_err(|e| e.to_string())
}

/// What a prepared ONNX model's graph must be, from the manifest this build carries (never
/// from the marker on disk): its name, its hash (the maker's file's, or the patched copy's)
/// and its size as `weights_size` — a same-length edit keeps the maker's size.
pub(super) fn map_of(
    m: &Model,
    file: &str,
    patch: Option<&OnnxPatch>,
) -> Option<(String, WeightMap)> {
    let entry = m.files.iter().find(|f| f.to == file)?;
    let stem = file.trim_end_matches(".onnx");
    let (graph, sha) = match patch {
        None => (stem.to_string(), entry.sha256.clone()),
        Some(p) => (format!("{stem}.cantus"), p.sha256.clone()),
    };
    Some((
        graph,
        WeightMap {
            graph_sha256: sha,
            weights_size: entry.size,
            ..no_map()
        },
    ))
}
