//! The 32 DX7 algorithms as topologies — facts about the instrument, written from the
//! published algorithm chart, not from any program. Operators are numbered as on the DX7
//! (1…6, 1 at the bottom); Modulus maps DX operator *n* to its operator letter *n* (1 → A,
//! 6 → F), so G and H are free for the user.
//!
//! An algorithm is a starting point: applying one writes matrix cells, outputs and the
//! feedback, and every cell stays editable afterwards (research §8.2 P0 1).

/// One DX7 algorithm.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DxAlgorithm {
    /// Modulation edges `(source, target)`, DX operator numbers 1…6.
    pub edges: &'static [(u8, u8)],
    /// The operators heard (DX numbers).
    pub carriers: &'static [u8],
    /// Where the feedback goes: `(from, into)`. Equal numbers are self-feedback; algorithms
    /// 4 and 6 feed an operator's output back into the top of its stack instead.
    pub feedback: (u8, u8),
}

const fn alg(
    edges: &'static [(u8, u8)],
    carriers: &'static [u8],
    feedback: (u8, u8),
) -> DxAlgorithm {
    DxAlgorithm {
        edges,
        carriers,
        feedback,
    }
}

/// `DX_ALGORITHMS[n - 1]` is algorithm *n*.
pub const DX_ALGORITHMS: [DxAlgorithm; 32] = [
    // 1–2: a pair and a stack of four.
    alg(&[(2, 1), (6, 5), (5, 4), (4, 3)], &[1, 3], (6, 6)),
    alg(&[(2, 1), (6, 5), (5, 4), (4, 3)], &[1, 3], (2, 2)),
    // 3–4: two stacks of three; 4 feeds operator 4 back into 6.
    alg(&[(3, 2), (2, 1), (6, 5), (5, 4)], &[1, 4], (6, 6)),
    alg(&[(3, 2), (2, 1), (6, 5), (5, 4)], &[1, 4], (4, 6)),
    // 5–6: three pairs; 6 feeds operator 5 back into 6.
    alg(&[(2, 1), (4, 3), (6, 5)], &[1, 3, 5], (6, 6)),
    alg(&[(2, 1), (4, 3), (6, 5)], &[1, 3, 5], (5, 6)),
    // 7–9: a pair, and 4 and 6→5 into 3.
    alg(&[(2, 1), (4, 3), (5, 3), (6, 5)], &[1, 3], (6, 6)),
    alg(&[(2, 1), (4, 3), (5, 3), (6, 5)], &[1, 3], (4, 4)),
    alg(&[(2, 1), (4, 3), (5, 3), (6, 5)], &[1, 3], (2, 2)),
    // 10–11: a stack of three, and 5 and 6 into 4.
    alg(&[(3, 2), (2, 1), (5, 4), (6, 4)], &[1, 4], (3, 3)),
    alg(&[(3, 2), (2, 1), (5, 4), (6, 4)], &[1, 4], (6, 6)),
    // 12–13: a pair, and 4, 5 and 6 into 3.
    alg(&[(2, 1), (4, 3), (5, 3), (6, 3)], &[1, 3], (2, 2)),
    alg(&[(2, 1), (4, 3), (5, 3), (6, 3)], &[1, 3], (6, 6)),
    // 14–15: a pair, and 5 and 6 into 4 into 3.
    alg(&[(2, 1), (4, 3), (5, 4), (6, 4)], &[1, 3], (6, 6)),
    alg(&[(2, 1), (4, 3), (5, 4), (6, 4)], &[1, 3], (2, 2)),
    // 16–17: one carrier under 2, 4→3 and 6→5.
    alg(&[(2, 1), (3, 1), (4, 3), (5, 1), (6, 5)], &[1], (6, 6)),
    alg(&[(2, 1), (3, 1), (4, 3), (5, 1), (6, 5)], &[1], (2, 2)),
    // 18: one carrier under 2, 3 and 6→5→4.
    alg(&[(2, 1), (3, 1), (4, 1), (5, 4), (6, 5)], &[1], (3, 3)),
    // 19: a stack of three, and 6 into both 4 and 5.
    alg(&[(3, 2), (2, 1), (6, 4), (6, 5)], &[1, 4, 5], (6, 6)),
    // 20: 3 into 1 and 2; 5 and 6 into 4.
    alg(&[(3, 1), (3, 2), (5, 4), (6, 4)], &[1, 2, 4], (3, 3)),
    // 21: 3 into 1 and 2; 6 into 4 and 5.
    alg(&[(3, 1), (3, 2), (6, 4), (6, 5)], &[1, 2, 4, 5], (3, 3)),
    // 22: a pair; 6 into 3, 4 and 5.
    alg(&[(2, 1), (6, 3), (6, 4), (6, 5)], &[1, 3, 4, 5], (6, 6)),
    // 23: 1 alone, 3→2, 6 into 4 and 5.
    alg(&[(3, 2), (6, 4), (6, 5)], &[1, 2, 4, 5], (6, 6)),
    // 24: 1 and 2 alone, 6 into 3, 4 and 5.
    alg(&[(6, 3), (6, 4), (6, 5)], &[1, 2, 3, 4, 5], (6, 6)),
    // 25: 1, 2 and 3 alone, 6 into 4 and 5.
    alg(&[(6, 4), (6, 5)], &[1, 2, 3, 4, 5], (6, 6)),
    // 26–27: 1 alone, 3→2, 5 and 6 into 4.
    alg(&[(3, 2), (5, 4), (6, 4)], &[1, 2, 4], (6, 6)),
    alg(&[(3, 2), (5, 4), (6, 4)], &[1, 2, 4], (3, 3)),
    // 28: a pair, 5→4→3, 6 alone.
    alg(&[(2, 1), (4, 3), (5, 4)], &[1, 3, 6], (5, 5)),
    // 29: 1 and 2 alone, 4→3, 6→5.
    alg(&[(4, 3), (6, 5)], &[1, 2, 3, 5], (6, 6)),
    // 30: 1 and 2 alone, 5→4→3, 6 alone.
    alg(&[(4, 3), (5, 4)], &[1, 2, 3, 6], (5, 5)),
    // 31: four alone and 6→5.
    alg(&[(6, 5)], &[1, 2, 3, 4, 5], (6, 6)),
    // 32: six carriers, feedback on 6.
    alg(&[], &[1, 2, 3, 4, 5, 6], (6, 6)),
];

/// The algorithm whose edges, carriers and feedback match, if any (1-based) — what the
/// editor prints when a patch still has a DX topology.
pub fn dx_algorithm_of(
    edges: &[(u8, u8)],
    carriers: &[u8],
    feedback: Option<(u8, u8)>,
) -> Option<usize> {
    let same =
        |a: &[(u8, u8)], b: &[(u8, u8)]| a.len() == b.len() && a.iter().all(|e| b.contains(e));
    let same_c = |a: &[u8], b: &[u8]| a.len() == b.len() && a.iter().all(|e| b.contains(e));
    DX_ALGORITHMS
        .iter()
        .position(|a| {
            same(a.edges, edges)
                && same_c(a.carriers, carriers)
                && feedback.is_none_or(|f| f == a.feedback)
        })
        .map(|i| i + 1)
}
