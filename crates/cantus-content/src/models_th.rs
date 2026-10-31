//! A model's weights rebuilt from its maker's own PyTorch file at install (ADR 0085), so
//! Cantus never hosts weights: the `.th` downloaded from Meta (its hash checked), each
//! tensor the map names copied to its place in `<name>.weights`, and the result's hash
//! checked against the map's — a file identical, byte for byte, to the one
//! `crates/cantus-ml/tools/export_htdemucs.py` writes beside the graph it was checked with.
//!
//! A `.th` is a zip: `<root>/data.pkl` (a pickle naming each tensor's storage, offset,
//! shape and strides — `models_pickle`) and `<root>/data/<n>` (each storage's raw bytes,
//! little-endian).

use crate::models_pickle::{self, Val};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Where each weight goes in `<name>.weights` (`tools/export_htdemucs.py` writes it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightMap {
    pub format: u32,
    /// The `.th` it is made from.
    pub source_sha256: String,
    /// The graph that reads it.
    pub graph_sha256: String,
    pub weights_size: u64,
    pub weights_sha256: String,
    pub tensors: Vec<MapEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapEntry {
    /// The tensor's name in the model's state (`encoder.0.conv.weight`).
    pub key: String,
    /// Bytes from the file's start.
    pub offset: u64,
    /// `f16` (the only one the models so far store).
    pub dtype: String,
    pub shape: Vec<u64>,
}

/// A tensor as the pickle names it.
#[derive(Clone, Debug, PartialEq)]
pub struct TensorRef {
    pub storage: String,
    /// Bytes per element of the storage's type.
    pub element: usize,
    /// Elements into the storage.
    pub offset: u64,
    pub shape: Vec<u64>,
    pub stride: Vec<u64>,
}

fn storage_element(class: &Val) -> Option<usize> {
    match class {
        Val::Global(m, n) if m == "torch" => match n.as_str() {
            "HalfStorage" | "BFloat16Storage" => Some(2),
            "FloatStorage" => Some(4),
            "DoubleStorage" | "LongStorage" => Some(8),
            _ => None,
        },
        _ => None,
    }
}

/// One `torch._utils._rebuild_tensor_v2(storage, offset, size, stride, …)`.
fn tensor_of(v: &Val) -> Option<TensorRef> {
    let Val::Call(func, args) = v else {
        return None;
    };
    if !matches!(&**func, Val::Global(m, n) if m == "torch._utils" && n == "_rebuild_tensor_v2") {
        return None;
    }
    let args = args.items()?;
    let Val::Persistent(pid) = args.first()? else {
        return None;
    };
    // ('storage', torch.HalfStorage, '<key>', 'cpu', <numel>)
    let pid = pid.items()?;
    if pid.first()?.as_str()? != "storage" {
        return None;
    }
    let element = storage_element(pid.get(1)?)?;
    let storage = pid.get(2)?.as_str()?.to_string();
    let ints = |v: &Val| -> Option<Vec<u64>> {
        v.items()?
            .iter()
            .map(|x| x.as_int().filter(|&i| i >= 0).map(|i| i as u64))
            .collect()
    };
    Some(TensorRef {
        storage,
        element,
        offset: args.get(1)?.as_int().filter(|&i| i >= 0)? as u64,
        shape: ints(args.get(2)?)?,
        stride: ints(args.get(3)?)?,
    })
}

/// The tensors of a demucs package's `state` (or of a plain state dict).
pub fn state_of(root: &Val) -> Result<HashMap<String, TensorRef>, String> {
    let state = root.get("state").unwrap_or_else(|| root.clone());
    let entries = state.entries().ok_or("the file holds no state dict")?;
    let mut out = HashMap::new();
    for (k, v) in entries {
        let key = k
            .as_str()
            .ok_or("a state key that is not a name")?
            .to_string();
        let t = tensor_of(&v).ok_or_else(|| format!("{key} is not a tensor this reader knows"))?;
        out.insert(key, t);
    }
    Ok(out)
}

fn contiguous(t: &TensorRef) -> bool {
    let mut want = 1u64;
    for (d, s) in t.shape.iter().zip(&t.stride).rev() {
        if *d != 1 && *s != want {
            return false;
        }
        want *= d;
    }
    t.shape.len() == t.stride.len()
}

/// Rebuild `out` from the `.th` at `th` as `map` says, and check its hash. Written beside
/// `out` first and renamed into place, so `out` is whole or absent.
pub fn build_weights(
    th: &Path,
    map: &WeightMap,
    out: &Path,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let file = std::fs::File::open(th).map_err(|e| format!("{}: {e}", th.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("the model file is not a PyTorch archive: {e}"))?;
    let pkl_name = (0..zip.len())
        .filter_map(|i| {
            zip.name_for_index(i)
                .and_then(|r| r.ok())
                .map(|n| n.to_string())
        })
        .find(|n| n.ends_with("/data.pkl") || n == "data.pkl")
        .ok_or("the archive has no data.pkl")?;
    let root_dir = pkl_name.trim_end_matches("data.pkl").to_string();
    let mut pkl = Vec::new();
    {
        let mut e = zip.by_name(&pkl_name).map_err(|e| e.to_string())?;
        if e.size() > 64 << 20 {
            return Err("data.pkl is too large".into());
        }
        e.read_to_end(&mut pkl).map_err(|e| e.to_string())?;
    }
    let state = state_of(&models_pickle::load(&pkl)?)?;
    if state.len() != map.tensors.len() {
        return Err(format!(
            "the file holds {} tensors; the map names {}",
            state.len(),
            map.tensors.len()
        ));
    }
    let tmp = out.with_extension("weights.part");
    let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(|e| e.to_string())?);
    let mut hash = Sha256::new();
    let mut written = 0u64;
    let result = (|| -> Result<(), String> {
        let mut entries: Vec<&MapEntry> = map.tensors.iter().collect();
        entries.sort_by_key(|e| e.offset);
        let mut storages: HashMap<String, Vec<u8>> = HashMap::new();
        for e in entries {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let t = state
                .get(&e.key)
                .ok_or_else(|| format!("{} is not in the file", e.key))?;
            if e.dtype != "f16" || t.element != 2 {
                return Err(format!(
                    "{}: the map and the file disagree on its type",
                    e.key
                ));
            }
            if t.shape != e.shape || !contiguous(t) {
                return Err(format!(
                    "{}: shape {:?}, the map says {:?}",
                    e.key, t.shape, e.shape
                ));
            }
            if e.offset < written {
                return Err(format!("{}: the map's tensors overlap", e.key));
            }
            if !storages.contains_key(&t.storage) {
                let name = format!("{root_dir}data/{}", t.storage);
                let mut s = zip
                    .by_name(&name)
                    .map_err(|_| format!("{name} is not in the archive"))?;
                if s.size() > 1 << 31 {
                    return Err(format!("{name} is too large"));
                }
                let mut bytes = Vec::with_capacity(s.size() as usize);
                s.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                storages.insert(t.storage.clone(), bytes);
            }
            let bytes = &storages[&t.storage];
            let n: u64 = e.shape.iter().product::<u64>() * 2;
            let from = t.offset * 2;
            let slice = bytes
                .get(from as usize..(from + n) as usize)
                .ok_or_else(|| format!("{}: past its storage's end", e.key))?;
            let pad = vec![0u8; (e.offset - written) as usize];
            for chunk in [&pad[..], slice] {
                w.write_all(chunk).map_err(|e| e.to_string())?;
                hash.update(chunk);
            }
            written = e.offset + n;
        }
        if written != map.weights_size {
            return Err(format!(
                "{written} bytes made; the map says {}",
                map.weights_size
            ));
        }
        w.flush().map_err(|e| e.to_string())?;
        let got = crate::fetch::hex(&hash.clone().finalize());
        if got != map.weights_sha256 {
            return Err(format!(
                "the rebuilt weights hash to {got}, not {}",
                map.weights_sha256
            ));
        }
        Ok(())
    })();
    drop(w);
    match result {
        Ok(()) => std::fs::rename(&tmp, out).map_err(|e| e.to_string()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}
