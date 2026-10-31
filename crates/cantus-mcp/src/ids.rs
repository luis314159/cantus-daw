//! Handles and tokens that cannot be guessed (MCP's security best practices: "random,
//! user-bound handles"): sessions, jobs, changes, bearer tokens — from the operating
//! system's random source (`getrandom`, review 3 #15), never a hash of the clock.

/// `n` random bytes, hex.
pub fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    // The system's source does not fail on the systems Cantus runs on; if it ever did,
    // nothing guessable is handed out — the process stops.
    getrandom::fill(&mut bytes).expect("the operating system's random source");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A fresh handle: `prefix` and 32 hex digits (`job-3f…`).
pub fn fresh(prefix: &str) -> String {
    format!("{prefix}-{}", random_hex(16))
}

#[cfg(test)]
mod tests {
    #[test]
    fn handles_never_repeat_and_look_random() {
        let a: std::collections::BTreeSet<String> =
            (0..1000).map(|_| super::fresh("job")).collect();
        assert_eq!(a.len(), 1000);
        assert!(a.iter().all(|h| h.starts_with("job-") && h.len() == 4 + 32));
        assert_eq!(super::random_hex(32).len(), 64);
    }
}
