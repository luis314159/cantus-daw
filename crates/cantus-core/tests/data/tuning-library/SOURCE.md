These files are test data from Surge's `tuning-library` (MIT, `LICENSE.md` beside them,
"Copyright 2019-2020, Paul Walker"): <https://github.com/surge-synthesizer/tuning-library>,
`tests/data`, head `d6fcc08f` (2026-10-19), fetched 2026-11-22 from
raw.githubusercontent.com. Their SHA-256 are listed in
`research/instruments/microtuning-verified.md` §1.4; `.gitattributes` keeps their bytes (CRLF
and LF) as fetched.

They are a second, independent reader's fixtures — the outside bytes Cantus's Scala reader is
tested against (`crates/cantus-core/tests/tuning_scala.rs`). `bad/` holds the must-fail set
(outcomes from that repository's `tests/alltests.cpp`, lines 639–662). The two Scala archive
copies of that folder (`marvel12.scl`, `zeus22.scl`) are not included: the archive has no
licence (ADR 0155).
