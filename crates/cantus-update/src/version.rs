//! Versions as Cantus tags them: `v0.2.0`, `v1.0.0-beta.2` — Semantic Versioning's
//! precedence, without the build metadata (`+…` is ignored, as SemVer says).

use std::cmp::Ordering;
use std::fmt;

/// `major.minor.patch[-pre]`. Ordered by SemVer precedence: numbers first, and a
/// pre-release sorts before its release (`1.0.0-rc.1 < 1.0.0`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// The dot-separated identifiers after `-`; empty for a release.
    pub pre: Vec<String>,
}

impl Version {
    /// Read `1.2.3`, `v1.2.3`, `V1.2.3-rc.1`, `1.2.3+build`. A missing minor or patch is
    /// not accepted: a tag the release workflow made always has all three.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim();
        let text = text
            .strip_prefix('v')
            .or_else(|| text.strip_prefix('V'))
            .unwrap_or(text);
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (text, None),
        };
        let mut nums = core.split('.');
        let num = |s: Option<&str>| -> Option<u64> {
            let s = s?;
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            // SemVer forbids leading zeros ("01"); a tag with one is not ours.
            if s.len() > 1 && s.starts_with('0') {
                return None;
            }
            s.parse().ok()
        };
        let major = num(nums.next())?;
        let minor = num(nums.next())?;
        let patch = num(nums.next())?;
        if nums.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(p) => {
                let ids: Vec<String> = p.split('.').map(str::to_owned).collect();
                let valid = |id: &String| {
                    !id.is_empty()
                        && id
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                };
                if !ids.iter().all(valid) {
                    return None;
                }
                ids
            }
        };
        Some(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

/// SemVer §11: numeric identifiers compare as numbers and sort before alphanumeric ones;
/// a longer list of equal identifiers sorts after a shorter one.
fn cmp_pre(a: &[String], b: &[String]) -> Ordering {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    for (x, y) in a.iter().zip(b) {
        let o = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(m), Ok(n)) => m.cmp(&n),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => x.cmp(y),
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| cmp_pre(&self.pre, &other.pre))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} parses"))
    }

    #[test]
    fn tags_parse_with_or_without_the_v() {
        assert_eq!(v("v0.2.0"), v("0.2.0"));
        assert_eq!(v("V1.10.3").minor, 10);
        assert_eq!(v("1.0.0+ci.7"), v("1.0.0"));
        assert_eq!(v("1.0.0-rc.1").pre, vec!["rc", "1"]);
        assert_eq!(v(" v2.0.1 ").to_string(), "2.0.1");
        assert_eq!(v("1.0.0-beta.2").to_string(), "1.0.0-beta.2");
    }

    #[test]
    fn what_is_not_a_version_is_refused() {
        for bad in [
            "", "v", "1", "1.2", "1.2.3.4", "01.2.3", "1.x.3", "1.2.3-", "1.2.3-a..b", "latest",
            "1.2.-3", "-1.2.3",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad:?} must not parse");
        }
    }

    #[test]
    fn precedence_is_semver_s() {
        // SemVer 2.0.0 §11's own example chain.
        let chain = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "1.10.0",
            "2.0.0",
        ];
        for pair in chain.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(v("0.9.0").cmp(&v("v0.9.0")), Ordering::Equal);
    }
}
