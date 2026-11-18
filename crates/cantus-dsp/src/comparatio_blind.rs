//! Blind listening: which one is playing is hidden, the order is a hash of a seed (the same
//! seed gives the same test, on any machine), and the score is an exact binomial.
//!
//! Two tests:
//!
//! - **ABX** — A is the mix, B the reference, X one of them, hidden. The listener hears A,
//!   B and X as often as they like and says which X is; after the trials, the score and
//!   the chance of doing as well by guessing ([`p_value`]).
//! - **Preference** — two buttons, *One* and *Two*, are the mix and the reference in an
//!   order hidden for each trial; the listener picks the one they prefer, and learns at the
//!   end how often that was the reference.
//!
//! Each trial's hidden choice is one bit of a SplitMix64 hash of `(seed, trial)` — a fair
//! coin per trial, independent of the others (a balanced order would let a listener who
//! counts guess the last ones). Loudness matching should be on: a level step gives the
//! answer away; and switching between two buttons that hide the same source changes
//! nothing at all (no fade), as switching between the same source should.

/// SplitMix64's finaliser (Steele, Lea and Flood, 2014): a well-mixed 64-bit hash.
pub fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The hidden coin of trial `trial` of test `seed`.
fn coin(seed: u64, trial: u32, salt: u64) -> bool {
    mix64(seed ^ mix64(trial as u64 ^ salt)) >> 63 == 1
}

const ABX_SALT: u64 = 0x4142_5800_0000_0001;
const PREF_SALT: u64 = 0x5052_4546_0000_0002;

/// What a button plays.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Mix,
    Reference,
}

/// In ABX trial `trial`, X is the reference (else the mix).
pub fn x_is_reference(seed: u64, trial: u32) -> bool {
    coin(seed, trial, ABX_SALT)
}

/// In preference trial `trial`, button One plays the reference (and Two the mix).
pub fn one_is_reference(seed: u64, trial: u32) -> bool {
    coin(seed, trial, PREF_SALT)
}

/// The chance of `correct` or more right out of `trials` by guessing (one-sided, p = ½),
/// exact: `Σ_{k ≥ correct} C(trials, k) / 2^trials`.
pub fn p_value(correct: u32, trials: u32) -> f64 {
    if correct == 0 {
        return 1.0;
    }
    if correct > trials {
        return 0.0;
    }
    // C(n, k) / 2^n built term by term in f64 (exact enough up to hundreds of trials).
    let n = trials as f64;
    let mut term = 1.0f64; // C(n, 0)
    for _ in 0..trials {
        term *= 0.5;
    }
    let mut tail = 0.0;
    for k in 0..=trials {
        if k >= correct {
            tail += term;
        }
        term *= (n - k as f64) / (k as f64 + 1.0);
    }
    tail.min(1.0)
}

/// Most trials a test keeps.
pub const MAX_TRIALS: usize = 64;

/// An ABX session: the seed, the trial under way, and the answers, kept but not shown
/// until the end. Plain data (the panel's; the audio thread needs only the seed and the
/// trial).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Abx {
    pub seed: u64,
    pub trials: u32,
    answered: u32,
    correct: u32,
    /// Bit `i`: the answer to trial `i` was "X is the reference".
    answers: u64,
}

impl Abx {
    pub fn new(seed: u64, trials: u32) -> Self {
        Abx {
            seed,
            trials: trials.clamp(1, MAX_TRIALS as u32),
            answered: 0,
            correct: 0,
            answers: 0,
        }
    }

    /// The trial under way (equal to [`Self::trials`] when the test is over).
    pub fn trial(&self) -> u32 {
        self.answered
    }

    pub fn finished(&self) -> bool {
        self.answered >= self.trials
    }

    /// What X plays in the trial under way.
    pub fn x(&self) -> Source {
        if x_is_reference(self.seed, self.answered) {
            Source::Reference
        } else {
            Source::Mix
        }
    }

    /// The listener says X is `guess`; the next trial begins. Ignored once finished.
    pub fn answer(&mut self, guess: Source) {
        if self.finished() {
            return;
        }
        if guess == Source::Reference {
            self.answers |= 1 << self.answered;
        }
        if guess == self.x() {
            self.correct += 1;
        }
        self.answered += 1;
    }

    /// `(correct, answered)` — for the end of the test.
    pub fn score(&self) -> (u32, u32) {
        (self.correct, self.answered)
    }

    pub fn p_value(&self) -> f64 {
        p_value(self.correct, self.answered)
    }
}

/// A preference session: how often the reference was preferred.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Preference {
    pub seed: u64,
    pub trials: u32,
    answered: u32,
    reference_preferred: u32,
}

impl Preference {
    pub fn new(seed: u64, trials: u32) -> Self {
        Preference {
            seed,
            trials: trials.clamp(1, MAX_TRIALS as u32),
            answered: 0,
            reference_preferred: 0,
        }
    }

    pub fn trial(&self) -> u32 {
        self.answered
    }

    pub fn finished(&self) -> bool {
        self.answered >= self.trials
    }

    /// What button One (`one = true`) or Two plays in the trial under way.
    pub fn button(&self, one: bool) -> Source {
        if one_is_reference(self.seed, self.answered) == one {
            Source::Reference
        } else {
            Source::Mix
        }
    }

    /// The listener prefers button One (`true`) or Two.
    pub fn choose(&mut self, one: bool) {
        if self.finished() {
            return;
        }
        if self.button(one) == Source::Reference {
            self.reference_preferred += 1;
        }
        self.answered += 1;
    }

    /// `(times the reference was preferred, trials answered)`.
    pub fn score(&self) -> (u32, u32) {
        (self.reference_preferred, self.answered)
    }
}
