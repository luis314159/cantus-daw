# Round 40 — `hyb-refit` (the hybrid's packs, version 2)

**Status: refitted, measured and tested; to upload (integrator, after the owner's OK).**
Branch `r40/hyb-refit` (from `r40/integration` 06e88bb7), worktree
`F:\projects\daw-r40\hyb-refit`.

The drum-sound work merged into `r40/integration` regenerated `pulsus_measured.bin`, so
`model_hash()` moved from 2161706810 (`0x80D9073A`) to **3977877751 (`0xED1998F7`)** and both
published packs (`tympanum-hybrid-{jazz,studio}-1.zip`) became stale: they still play, the
engine logs them stale, and the ignored `the_published_hybrid_pack_downloads_matches_its_hash_and_fits_this_model`
fails on the hash. Both kits were fitted again with `refit_hybrid.sh` on this build's model
(content root `F:\projects\daw-r40\tmp\hyb\v2\content`: junctions to tymp-engine's
Virtuosity Drums and DRSKit, so the shared content folder's residuals were not touched).

## What changed

- `tools/tympanum-fit/residual_zip.py`: `VERSION = 2`, `DATE` 2026-11-14 (the zip's entry
  times and the manifest's `checked`); the new name never overwrites a published file, and a
  user holding version 1 is offered the update (`Status::Update`).
- `crates/cantus-content/manifest/content.json`: both entries at version 2 — URL
  `…/content-r40/tympanum-hybrid-{kit}-2.zip`, size, SHA-256, `installed_size` (written by
  `residual_manifest.py`). The model hash is not a manifest field: it is in each residual's
  `CRS1` header (byte 12), checked = 3977877751 in both.
- `tools/tympanum-fit/README.md`: bump `VERSION` before a refit whose zips are published.

| Pack (`F:\projects\daw-r40\tmp\hyb\v2\`) | bytes | SHA-256 | installed |
|---|---|---|---|
| `tympanum-hybrid-jazz-2.zip` | 5 167 317 | `fa356fce46be5dc452f190dd98fb73644a4642770fe484050ebdfb4045935ddc` | 5 770 564 |
| `tympanum-hybrid-studio-2.zip` | 5 263 453 | `00a8e5e8c25854f406e43f4c90055f14408a0fd15f7aaad2ca4cb667a1798080` | 5 769 473 |

Both start with `PK`; each holds `residual.crs` (byte-identical to the fit's), `LICENSE.txt`,
`README.txt` (the fit's report).

## Measured (`hybrid_measure.py`, as drums-hyb; 13 articulations × 4 velocities)

Distance from the recording, mean |dB| over third octaves 60 Hz–16 kHz (levels aligned):

| | 0–50 ms gen → hyb | 50–200 ms gen → hyb | groove 6–16 kHz flatness gen · hyb (rec) | components/kHz gen · hyb (rec) |
|---|---|---|---|---|
| jazz v1 | 4.78 → 3.96 | 4.39 → 3.56 | 0.233 · 0.262 (0.280) | 22.5 · 27.0 (28.0) |
| **jazz v2** | **4.08 → 3.53** | **4.01 → 3.22** | 0.325 · 0.330 (0.280) | 39.3 · 36.8 (28.0) |
| studio v1 | 4.78 → 3.79 | 4.16 → 3.23 | 0.250 · 0.280 (0.363) | 15.0 · 20.2 (30.9) |
| **studio v2** | **4.29 → 3.46** | **4.17 → 3.13** | 0.354 · 0.377 (0.363) | 44.7 · 37.7 (30.9) |

The generated model came closer on its own (−0.7 / −0.5 dB in 0–50 ms), and the hybrid on top
of it is the closest yet in both windows for both kits (jazz −0.43 / −0.34 dB vs v1, studio
−0.33 / −0.10). The hybrid's gain over the model is smaller in jazz's attack (0.55 dB, was
0.82) — the model already holds more of it. The groove's highs now *exceed* the recording's
density (the new model's, 39–45 components/kHz against 28–31); the hybrid pulls that back
toward the recording rather than adding to it. Hits where the hybrid is farther than the
model by > 0.5 dB: jazz tom-mid at 40/80/110 and crash/open hat at 40 (v1: tom-mid 40,
tom-floor 110, ride 40); studio none (v1: tom-mid 110, closed hat 40). Fit reports' share of
the first 200 ms the model explains, v1 → v2: jazz snare 76 → 86 %, rimshot 64 → 73 %, key 46
nearly closed 36 → 52 %, tom-floor 81 → 73 %; studio kick 79 → 85 %, tom-high 74 → 84 %,
half hat 72 → 43 %, sidestick 76 → 60 %, crash 73 → 66 %. Detail:
`F:\projects\daw-r40\tmp\hyb\v2\measure-{jazz,studio}.txt`, `v2\{jazz,studio}\report-*.txt`;
the listen `v2\listen-{jazz,studio}\` (generated · hybrid · recording).

## Tests

Green (Windows): `cantus-dsp` `tympanum_residual` (10), `cantus-engine` `tympanum_hybrid`,
`rt_memory` `tympanum_hybrid_never_allocates`, all of `cantus-content`, `cantus-ui`
`tympanum_hybrid_e2e`; clippy `-D warnings` on `cantus-content` and `cantus-ui`, all
targets (no Rust changed). The ignored, networked
`the_published_hybrid_pack_downloads_matches_its_hash_and_fits_this_model` **fails with
HTTP 404** until the two zips are uploaded (it now fetches `…-2.zip`); the bytes it will
check were verified here (size, SHA-256, `PK`, the residual's model hash). The Linux check
in WSL was not run (nothing platform-specific changed).

## For the integrator

1. With the owner's OK, upload `tympanum-hybrid-jazz-2.zip` and
   `tympanum-hybrid-studio-2.zip` to the public release `content-r40` of
   `luis314159/cantus-daw` (keep the `-1` assets: older builds point at them).
2. Run `cargo test -p cantus-ui --test tympanum_hybrid_e2e -- --ignored the_published`.
3. Merge `r40/hyb-refit`.
