//! Ids are forever: a host stores them in every project and automation lane. This test
//! holds every one the plug-ins have ever published — the plug-ins' CLAP ids and VST3
//! class ids, and every parameter's id — against `tests/golden/ids.txt`.
//!
//! A changed or missing id fails. A new parameter (a new key in a table) fails too until
//! it is written down: run the test with `CANTUS_BLESS_IDS=1` to append it, and commit the
//! file. Never edit a line that is already there.

use cantus_plugins::desc;
use cantus_plugins::params::{self, param_id};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/ids.txt")
}

/// Every id as `(what, id)` lines, in a stable order.
fn current() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in &desc::ALL {
        out.push((format!("{} clap", d.slug), d.clap_id.to_string()));
        out.push((format!("{} vst3", d.slug), d.vst3_cid_hex()));
        for p in &params::table(d.kind).params {
            out.push((format!("{} {}", d.slug, p.key), p.id.to_string()));
        }
    }
    out
}

#[test]
fn ids_never_change() {
    let path = golden_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let golden: BTreeMap<String, String> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (what, id) = l.rsplit_once(' ')?;
            Some((what.to_string(), id.to_string()))
        })
        .collect();
    let now = current();
    let now_map: BTreeMap<&str, &str> = now.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut problems = Vec::new();
    for (what, id) in &golden {
        match now_map.get(what.as_str()) {
            Some(now) if now == id => {}
            Some(now) => problems.push(format!("{what}: was {id}, is {now}")),
            None => problems.push(format!("{what}: gone (was {id})")),
        }
    }
    assert!(problems.is_empty(), "ids changed:\n{}", problems.join("\n"));
    let new: Vec<&(String, String)> = now
        .iter()
        .filter(|(w, _)| !golden.contains_key(w))
        .collect();
    if !new.is_empty() {
        if std::env::var("CANTUS_BLESS_IDS").is_ok() {
            let mut text = if text.is_empty() {
                "# Every id the exported plug-ins publish (ADR 0020). Lines are only ever added.\n"
                    .to_string()
            } else {
                text.clone()
            };
            for (what, id) in &new {
                text.push_str(&format!("{what} {id}\n"));
            }
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
        } else {
            panic!(
                "{} new ids are not in {} yet (first: {} {}): run with CANTUS_BLESS_IDS=1",
                new.len(),
                path.display(),
                new[0].0,
                new[0].1
            );
        }
    }
}

#[test]
fn no_two_parameters_share_an_id() {
    for d in &desc::ALL {
        let mut seen = BTreeSet::new();
        for p in &params::table(d.kind).params {
            assert!(seen.insert(p.id), "{}: {} collides", d.slug, p.key);
            assert!(p.id < 0x8000_0000, "VST3 leaves the upper half to hosts");
        }
    }
}

#[test]
fn the_id_rule_is_fnv1a_of_the_key() {
    // Written out, so the rule itself cannot drift: FNV-1a 32 of "thr", top bit cleared.
    let mut h: u32 = 0x811C_9DC5;
    for b in b"thr" {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    assert_eq!(param_id("thr"), h & 0x7FFF_FFFF);
}

#[test]
fn the_vst3_class_ids_follow_their_rule() {
    for d in &desc::ALL {
        let hash = Sha256::digest(format!("cantus/vst3/{}", d.slug).as_bytes());
        let words: Vec<u32> = hash[..16]
            .chunks(4)
            .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(words, d.vst3_cid, "{}", d.slug);
    }
}
