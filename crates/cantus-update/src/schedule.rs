//! When to ask, and what was learnt last time — `update.json` beside `settings.json`.
//!
//! A check that reached GitHub is not repeated for a day. A check that did not (offline,
//! a proxy's page, GitHub's rate limit) is tried again on a later start, but not within the
//! hour. The answer's `ETag` is kept: asking again with `If-None-Match` costs nothing
//! against GitHub's limit of 60 anonymous requests an hour per address, which a whole
//! office behind one proxy shares.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DAY: u64 = 24 * 60 * 60;
pub const RETRY: u64 = 60 * 60;

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// When a check last reached GitHub and got an answer (Unix seconds).
    pub last_success: Option<u64>,
    /// When a check was last tried, whatever came of it.
    pub last_attempt: Option<u64>,
    /// The last answer's `ETag`, and the answer itself, to reuse on `304 Not Modified`.
    pub etag: Option<String>,
    pub release_json: Option<String>,
    /// A version the user chose to skip: not offered again (a newer one is).
    pub skipped: Option<String>,
}

/// `t` is in the future of `now` by more than a minute: the clock went back (or the file
/// came from another machine). Such a stamp must not stop checks for years.
fn from_the_future(t: u64, now: u64) -> bool {
    t > now + 60
}

impl State {
    /// Whether an automatic check is due at `now`.
    pub fn due(&self, now: u64) -> bool {
        let fresh = |t: Option<u64>, period: u64| {
            t.is_some_and(|t| !from_the_future(t, now) && now < t + period)
        };
        !fresh(self.last_success, DAY) && !fresh(self.last_attempt, RETRY)
    }

    pub fn load(path: &Path) -> State {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        // Write beside, then rename: a crash mid-write never leaves half a file.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

/// `update.json` in the folder that holds the user's settings.
pub fn state_path(config_dir: &Path) -> PathBuf {
    config_dir.join("update.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_is_due_at_most_daily_and_a_failure_retries_after_an_hour() {
        let t0 = 1_800_000_000;
        let mut s = State::default();
        assert!(s.due(t0), "never checked");
        s.last_attempt = Some(t0);
        s.last_success = Some(t0);
        assert!(!s.due(t0 + 1));
        assert!(!s.due(t0 + DAY - 1));
        assert!(s.due(t0 + DAY));
        // A failed attempt the next day: an hour before the next try.
        s.last_attempt = Some(t0 + DAY);
        assert!(!s.due(t0 + DAY + RETRY - 1));
        assert!(s.due(t0 + DAY + RETRY));
        // A clock that went back does not freeze the schedule.
        assert!(s.due(t0 - 10 * DAY));
    }

    #[test]
    fn the_state_survives_a_round_trip_and_a_broken_file_is_a_fresh_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = state_path(dir.path());
        assert_eq!(State::load(&path), State::default());
        let s = State {
            last_success: Some(5),
            last_attempt: Some(6),
            etag: Some("W/\"abc\"".into()),
            release_json: Some("{}".into()),
            skipped: Some("0.3.0".into()),
        };
        s.save(&path).unwrap();
        assert_eq!(State::load(&path), s);
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(State::load(&path), State::default());
    }
}
