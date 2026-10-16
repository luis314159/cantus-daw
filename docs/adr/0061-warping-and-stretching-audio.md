# ADR 0061 — Warping and stretching audio: our own stretcher, warp markers, analysis at import

**Status:** Accepted (round 29, *warp*; part A — part B, playback under the tempo map, follows
the tempo builder's merge) · **Date:** 2026-10-16 · **Round:** 29 (warp) · **Builds on:** ADR
0053 (the pitch base: the tracker, the epochs, TD-PSOLA), ADR 0054 (the pitch render path: a
worker, a disk cache, renders attached to the snapshot) · **Extends:** ADR 0004 (commands),
ADR 0023 (analysis stored), ADR 0031 (offline = real time), ADR 0047 (licences) ·
**Replaces:** ADR 0025 §1's *"warping is a later decision"* for warped clips · **Serves:**
BACKLOG §3 *Time* and *Editing*

> Measurements and tests: `docs/round29/warp.md`. The manual: §5, *Warping and stretching
> audio*.

## 1. Licences (reviewed before any code was written)

The stretcher, the shifter, the onset detector and the tempo/beat tracker are **our own
implementation, in `cantus-dsp` (MPL-2.0)**, from papers. No third-party code is copied,
ported or linked; no new crate is added; `THIRD-PARTY-NOTICES.md` does not change.

| Reference | Licence | How it was used |
|---|---|---|
| Rubber Band (R2/R3) | GPL-2.0-or-later or commercial | **Not read for code, not ported.** A listening reference only. `research/engine/dsp-quality.md` §2 and `README.md` say it is "usable since Cantus is GPL" — that predates ADR 0047: `cantus-dsp` and `cantus-engine` are MPL-2.0, so GPL code can never enter them, and a GPL crate (the UI) could not feed the engine's render path. This ADR corrects those notes. |
| SoundTouch | LGPL-2.1 | Not used, not ported (WSOLA is textbook; we do not use it). |
| zplane élastique | commercial | A listening reference only (in Ableton Live, Cubase, FL Studio). |
| Signalsmith Stretch | MIT | Studied as a *design* (G. Luff, ADC 2022, "Four ways to write a pitch-shifter"): pitch-shifting by **mapping STFT bins** with the spectral envelope corrected, inside the time-stretcher, instead of stretch-then-resample. The idea is used; no code was copied (if it ever is, it is carried with its MIT notice). |
| Bungee | MPL-2.0 | Studied as a *design*: a fixed synthesis hop with analysis frames placed anywhere by a time map, and the instantaneous frequency taken from **two analysis frames one synthesis hop apart** (so the analysis hop may vary freely). The idea is textbook (Flanagan and Golden 1966; Dolson 1986) and is ours; no code was copied. |
| aubio, Loris | GPL | Not read. |
| Papers | — | J. Laroche, M. Dolson, *Improved phase vocoder time-scale modification of audio*, IEEE TSAP 7(3), 1999 (identity phase locking around spectral peaks); J. Driedger, M. Müller, *A review of time-scale modification of music signals*, Appl. Sci. 6(2):57, 2016, and Driedger, Müller, Ewert, *Improving TSM using harmonic-percussive separation*, IEEE SPL 21(1), 2014; Z. Průša, N. Holighaus, *Phase vocoder done right*, EUSIPCO 2017 (PGHI); A. Röbel, *A new approach to transient processing in the phase vocoder*, DAFx 2003 (phase reset at onsets); J. Bonada, *Automatic technique in frequency domain for near-lossless time-scale modification of audio*, ICMC 2000; A. Röbel, X. Rodet, *Efficient spectral envelope estimation and its application to pitch shifting and envelope preservation*, DAFx 2005 (true envelope); D. Ellis, *Beat tracking by dynamic programming*, JNMR 36(1), 2007; S. Böck, G. Widmer, *Maximum filter vibrato suppression for onset detection* (SuperFlux), DAFx 2013; S. Dixon, *Onset detection revisited*, DAFx 2006. |

The licence rule stands for later work: a GPL/LGPL stretcher may be *heard* as a reference;
MIT/BSD/Apache/MPL code may be used with its notice, recorded here first.
