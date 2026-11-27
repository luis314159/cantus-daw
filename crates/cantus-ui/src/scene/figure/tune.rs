//! r42:arcus-polish5 — the figures' numbers under study, overridable for a process without a
//! rebuild (as `CANTUS_CONCERT` is for the light): `CANTUS_TUNE="collar_keep=2.4,btn_g0=0.3"`
//! sets any number a figure's code reads through [`tune`] (design sheets on a machine where a
//! release build takes an hour; DEVELOPING §4). Never set by the app: unset, every number is
//! its default, the one in the code.

use std::collections::HashMap;
use std::sync::OnceLock;

/// `name`'s number: `CANTUS_TUNE`'s value for it, else `default`.
pub fn tune(name: &str, default: f32) -> f32 {
    static T: OnceLock<HashMap<String, f32>> = OnceLock::new();
    T.get_or_init(|| parse(&std::env::var("CANTUS_TUNE").unwrap_or_default()))
        .get(name)
        .copied()
        .unwrap_or(default)
}

/// `spec`'s `name=value` pairs (comma-separated; a pair that does not parse is left out).
fn parse(spec: &str) -> HashMap<String, f32> {
    spec.split(',')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((k.trim().to_string(), v.trim().parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_parse_and_the_rest_is_left_out() {
        let t = parse("a=1.5, b = -2,c=x,d");
        assert_eq!(t.get("a"), Some(&1.5));
        assert_eq!(t.get("b"), Some(&-2.0));
        assert!(!t.contains_key("c") && !t.contains_key("d"));
        assert!(parse("").is_empty());
    }
}
