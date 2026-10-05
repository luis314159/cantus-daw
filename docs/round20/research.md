# Round 20 — research (content): report

**Branch:** `r20/research` · **Area:** `research/content/` · research only, no code.

## Done

- `research/content/README.md`: Spanish summary at the top, the findings in one page,
  recommendations in priority order, the legal points for the owner.
- `research/content/samples.md`: rights-free samples, drums first (kicks, snares, hats,
  claps, percussion, kits), then loops, instruments, FX. For each source: licence,
  redistribution, size, quality, formats and provenance risk. Also a CC0 core-kit
  proposal (~40 MB) and the vetting process (manifest, evidence, CI check).
- `research/content/storage.md`: Git LFS vs GitHub Releases vs a content repo vs object
  storage/CDN, with 2026 prices and cost scenarios; how FL Studio, Ableton, Bitwig,
  Reaper and MuseScore ship content; size budgets; a content-manager design; the
  recommended design (GitHub Releases on `cantus-content` + signed manifest now,
  Cloudflare R2 mirror later).
- `research/content/instruments.md`: open-source and freeware CLAP/VST3 instruments,
  licences, GPL aggregation vs combined work and the source duty, trademarks (Vital vs
  Vitalium), the VST logo, ranked bundle / link-only / CI test-set lists.
- Corrected the September notes: Maestro Concert Grand, Philharmonia, Iowa MIS,
  Salamander Drumkit, LABS → Splice INSTRUMENT, sfizz archived.

## Tests

None (no code).

## Files touched outside the area

This report only.

## Left

- The owner's rulings listed in the README ("Legal points").
- Letters to Karoryfer, Prof. Fritts (Iowa MIS) and SM Drums; a primary source for
  Splendid Grand's public-domain claim.
- Items marked **not verified** in each file. The proxy blocked some sites and gnu.org
  timed out; the web-search budget also ran out.

## Notes for others

- **samples builder:** samples.md §6 (core kit), §8 (provenance manifest and rejection
  rules) and storage.md §5 (content manager, manifest, proxy behaviour) are written for
  you. The accepted SPDX ids wait on the owner; CC0-1.0 alone is safe today.
- **hosted-inst builder:** instruments.md §5.4 lists a free plug-in test set covering CLAP-native,
  clap-wrapper, JUCE, VST3-only, Rust and the plain VST3 SDK.
- **browser builder:** a "Get free instruments" catalogue (instruments.md §4.4) could
  live in the picker. Freeware entries are link-only cards.
