//! Permissions by class, decided in Cantus (ADR 0081 §5, `research/ai/mcp-server.md` §4):
//! a client's annotations are hints; what runs is what the user's settings and grants say.
//!
//! | Class | Nothing granted | Granted |
//! |---|---|---|
//! | Read, Transport | runs | — |
//! | Edit | asks | runs: *Allow edits* in Settings, or *Allow for this session* — for that one tool, from that one agent |
//! | Destructive | asks | always asks (an answer covers that call only) |
//! | Files | refused | asks once *Allow files* is on, every time |
//!
//! Plus a tool the user denied by name is always refused, the whole AI area switched off
//! refuses everything, and an external agent (HTTP, the stdio bridge) is refused while
//! *Allow external agents* is off.

use cantus_ai::{Decision, ToolClass};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The user's choices, as the settings hold them (read each frame).
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// The AI area is on (one switch turns it all off and hides it).
    pub enabled: bool,
    /// Agents outside Cantus (HTTP, the stdio bridge) may call.
    pub external_agents: bool,
    /// Edits run without asking.
    pub allow_edits: bool,
    /// File tools may be asked for (otherwise refused).
    pub allow_files: bool,
    /// Folders file tools may use besides the project's own.
    pub folders: Vec<PathBuf>,
    /// Tools refused by name, whatever their class.
    pub denied: Vec<String>,
}

impl Default for Policy {
    /// What a new install has: on, no external agents, edits ask, files refused.
    fn default() -> Self {
        Policy {
            enabled: true,
            external_agents: false,
            allow_edits: false,
            allow_files: false,
            folders: Vec::new(),
            denied: Vec::new(),
        }
    }
}

/// What the user granted one agent while it is connected: *Allow for this session* on an
/// edit tool's card lets that tool run unasked (review 1 #2: never a whole class, never a
/// destructive or file tool).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub tools: BTreeSet<String>,
}

/// Whether *Allow for this session* can be given for a class: edits only.
pub fn grantable(class: ToolClass) -> bool {
    class == ToolClass::Edit
}

/// Why a call is refused before it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    AiOff,
    ExternalOff,
    DeniedTool,
    FilesOff,
}

impl Refusal {
    /// What the agent reads.
    pub fn words(&self, tool: &str) -> String {
        match self {
            Refusal::AiOff => "the AI area is switched off in Cantus's Settings".into(),
            Refusal::ExternalOff => {
                "external agents are not allowed: the user can allow them in Cantus's Settings ▸ AI"
                    .into()
            }
            Refusal::DeniedTool => format!("the user does not allow the tool {tool}"),
            Refusal::FilesOff => {
                "reading or writing files is off: the user can enable it in Settings ▸ AI".into()
            }
        }
    }
}

/// The decision for one call from an agent (`external`: through a door to the outside).
pub fn decide(
    class: ToolClass,
    tool: &str,
    external: bool,
    policy: &Policy,
    grants: &Grants,
) -> Result<Decision, Refusal> {
    if !policy.enabled {
        return Err(Refusal::AiOff);
    }
    if external && !policy.external_agents {
        return Err(Refusal::ExternalOff);
    }
    if policy.denied.iter().any(|d| d == tool) {
        return Err(Refusal::DeniedTool);
    }
    Ok(match class {
        ToolClass::Read | ToolClass::Transport => Decision::Allow,
        ToolClass::Edit if policy.allow_edits || grants.tools.contains(tool) => Decision::Allow,
        ToolClass::Edit | ToolClass::Destructive => Decision::AskUser,
        ToolClass::Files if !policy.allow_files => return Err(Refusal::FilesOff),
        ToolClass::Files => Decision::AskUser,
    })
}

/// Token buckets per agent and for all agents together, by kind of call (MCP: servers
/// MUST rate-limit tool calls). A new session or credential gets its own bucket, never a
/// fresh global one (review 1 #4).
#[derive(Clone, Debug)]
pub struct RateLimits {
    /// Reads, transport, the server's own tools: calls a second, and the burst allowed.
    pub reads: (f64, f64),
    /// Calls that change the project or the disk.
    pub edits: (f64, f64),
    /// Every agent together: the per-agent numbers multiplied by this.
    pub global_factor: f64,
    buckets: HashMap<(String, bool), (f64, Instant)>,
}

impl Default for RateLimits {
    fn default() -> Self {
        RateLimits {
            reads: (50.0, 200.0),
            edits: (10.0, 40.0),
            global_factor: 2.0,
            buckets: HashMap::new(),
        }
    }
}

/// The pool every agent of one kind shares: in-process callers (the assistant panel, tests)
/// and external ones (HTTP, the bridge) never starve each other (review 2 #11).
fn pool(external: bool) -> &'static str {
    if external {
        "\u{0}all-external"
    } else {
        "\u{0}all-in-process"
    }
}

fn is_pool(k: &str) -> bool {
    k.starts_with('\u{0}')
}

impl RateLimits {
    fn rate_burst(&self, mutating: bool, factor: f64) -> (f64, f64) {
        let (rate, burst) = if mutating { self.edits } else { self.reads };
        (rate * factor, burst * factor)
    }

    /// The tokens `key`'s bucket holds at `now` (refilled), without spending any.
    fn level(&mut self, key: &str, mutating: bool, factor: f64, now: Instant) -> f64 {
        let (rate, burst) = self.rate_burst(mutating, factor);
        let entry = self
            .buckets
            .entry((key.to_string(), mutating))
            .or_insert((burst, now));
        let elapsed = now.saturating_duration_since(entry.1).as_secs_f64();
        entry.0 = (entry.0 + elapsed * rate).min(burst);
        entry.1 = now;
        entry.0
    }

    /// Take one call from `key`'s bucket and its pool's at `now`: `Err(wait)` when either is
    /// empty — and then neither is spent (review 2 #14).
    pub fn take(
        &mut self,
        key: &str,
        mutating: bool,
        external: bool,
        now: Instant,
    ) -> Result<(), Duration> {
        let global = self.global_factor;
        let pool = pool(external);
        let mine = self.level(key, mutating, 1.0, now);
        let all = self.level(pool, mutating, global, now);
        if mine >= 1.0 && all >= 1.0 {
            for (k, _) in [(key, 1.0), (pool, global)] {
                if let Some(e) = self.buckets.get_mut(&(k.to_string(), mutating)) {
                    e.0 -= 1.0;
                }
            }
            return Ok(());
        }
        let wait = |level: f64, factor: f64| {
            let (rate, _) = self.rate_burst(mutating, factor);
            ((1.0 - level).max(0.0) / rate).max(0.0)
        };
        let w = wait(mine, 1.0).max(wait(all, global));
        Err(Duration::from_secs_f64(w.max(0.001)))
    }

    /// How long a bucket takes to fill from empty: after that, idle, it is full and may go.
    pub fn refill_time(&self) -> Duration {
        let (r1, b1) = self.reads;
        let (r2, b2) = self.edits;
        Duration::from_secs_f64((b1 / r1).max(b2 / r2).max(0.0))
    }

    /// Agents' buckets idle long enough to be full again go (review 2 #14: the idle time is
    /// the time to refill, not a fixed one).
    pub fn prune(&mut self, now: Instant) {
        let idle = self.refill_time();
        self.buckets
            .retain(|(k, _), (_, at)| is_pool(k) || now.saturating_duration_since(*at) < idle);
    }

    /// Forget an agent (it disconnected).
    pub fn forget(&mut self, key: &str) {
        self.buckets.retain(|(k, _), _| k != key);
    }

    /// How many agents' buckets are kept (for tests).
    pub fn kept(&self) -> usize {
        self.buckets.keys().filter(|(k, _)| !is_pool(k)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASSES: [ToolClass; 5] = [
        ToolClass::Read,
        ToolClass::Edit,
        ToolClass::Destructive,
        ToolClass::Files,
        ToolClass::Transport,
    ];

    fn on() -> Policy {
        Policy {
            external_agents: true,
            ..Policy::default()
        }
    }

    #[test]
    fn with_nothing_changed_the_decision_is_the_contracts() {
        for class in CLASSES {
            let d = decide(class, "t", false, &Policy::default(), &Grants::default());
            let expected = cantus_ai::tool::default_decision(class);
            match d {
                Ok(d) => assert_eq!(d, expected, "{class:?}"),
                Err(Refusal::FilesOff) => assert_eq!(expected, Decision::Deny, "{class:?}"),
                Err(r) => panic!("{class:?}: {r:?}"),
            }
        }
    }

    #[test]
    fn grants_and_settings_open_only_their_tool_and_never_destruction_or_files() {
        let mut p = on();
        p.allow_edits = true;
        let g = Grants::default();
        assert_eq!(
            decide(ToolClass::Edit, "t", true, &p, &g),
            Ok(Decision::Allow)
        );
        assert_eq!(
            decide(ToolClass::Destructive, "t", true, &p, &g),
            Ok(Decision::AskUser),
            "allowing edits never allows destruction"
        );
        assert_eq!(
            decide(ToolClass::Files, "t", true, &p, &g),
            Err(Refusal::FilesOff)
        );
        p.allow_files = true;
        assert_eq!(
            decide(ToolClass::Files, "t", true, &p, &g),
            Ok(Decision::AskUser)
        );
        p.allow_edits = false;
        let g = Grants {
            tools: ["t".to_string()].into(),
        };
        assert_eq!(
            decide(ToolClass::Edit, "t", true, &p, &g),
            Ok(Decision::Allow)
        );
        assert_eq!(
            decide(ToolClass::Edit, "u", true, &p, &g),
            Ok(Decision::AskUser),
            "one tool, not the class"
        );
        assert_eq!(
            decide(ToolClass::Destructive, "t", true, &p, &g),
            Ok(Decision::AskUser)
        );
        assert_eq!(
            decide(ToolClass::Files, "t", true, &p, &g),
            Ok(Decision::AskUser)
        );
        assert!(grantable(ToolClass::Edit));
        assert!(!grantable(ToolClass::Destructive) && !grantable(ToolClass::Files));
    }

    #[test]
    fn a_denied_tool_external_agents_and_the_off_switch_refuse() {
        let p = Policy {
            denied: vec!["get_x".into()],
            ..on()
        };
        let g = Grants::default();
        assert_eq!(
            decide(ToolClass::Read, "get_x", false, &p, &g),
            Err(Refusal::DeniedTool)
        );
        assert_eq!(
            decide(ToolClass::Read, "get_y", false, &p, &g),
            Ok(Decision::Allow)
        );
        let closed = Policy::default();
        assert_eq!(
            decide(ToolClass::Read, "t", true, &closed, &g),
            Err(Refusal::ExternalOff)
        );
        assert_eq!(
            decide(ToolClass::Read, "t", false, &closed, &g),
            Ok(Decision::Allow)
        );
        let off = Policy {
            enabled: false,
            ..on()
        };
        for class in CLASSES {
            assert_eq!(decide(class, "t", false, &off, &g), Err(Refusal::AiOff));
        }
    }

    #[test]
    fn the_buckets_allow_a_burst_then_the_rate_and_a_new_name_shares_its_pool() {
        let mut r = RateLimits {
            edits: (10.0, 3.0),
            global_factor: 2.0,
            ..RateLimits::default()
        };
        let t0 = Instant::now();
        for _ in 0..3 {
            assert!(r.take("a", true, true, t0).is_ok());
        }
        let wait = r.take("a", true, true, t0).unwrap_err();
        assert!(wait <= Duration::from_millis(100) && wait > Duration::ZERO);
        assert!(
            r.take("b", true, true, t0).is_ok(),
            "each agent has its own bucket"
        );
        assert!(
            r.take("a", false, true, t0).is_ok(),
            "reads have their own bucket"
        );
        // Renaming does not escape: the pool (6) has 2 left after a's 3 and b's 1.
        let fresh = (0..10)
            .filter(|i| r.take(&format!("n{i}"), true, true, t0).is_ok())
            .count();
        assert_eq!(fresh, 2, "the pool bounds every external agent together");
        // The in-process pool is its own (review 2 #11).
        assert!(r.take("panel", true, false, t0).is_ok());
        r.prune(t0 + Duration::from_secs(3600));
        assert_eq!(r.kept(), 0, "idle agents' buckets go");
        r.forget("a");
    }

    /// Review 2 #14: a refusal by the pool spends nothing of the agent's own bucket.
    #[test]
    fn a_refused_call_spends_nothing() {
        // Each agent: 1 token, back at 0.1 a second; the pool: 10, back at 1 a second.
        let mut r = RateLimits {
            edits: (0.1, 1.0),
            global_factor: 10.0,
            ..RateLimits::default()
        };
        let t0 = Instant::now();
        for i in 0..10 {
            assert!(r.take(&format!("n{i}"), true, true, t0).is_ok());
        }
        assert!(r.take("b", true, true, t0).is_err(), "the pool is spent");
        // A second on, the pool has one again; b's own token was never spent.
        assert!(r.take("b", true, true, t0 + Duration::from_secs(1)).is_ok());
        assert_eq!(r.refill_time(), Duration::from_secs(10));
    }
}
