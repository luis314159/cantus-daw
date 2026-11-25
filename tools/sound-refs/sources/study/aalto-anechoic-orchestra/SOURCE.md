# Aalto University anechoic recordings of symphonic music: cello and double-bass tracks

- **URLs:**
  - Page: https://research.cs.aalto.fi/acoustics/virtual-acoustics/research/acoustic-measurement-and-analysis/85-anechoic-recordings.html
  - Older page: https://users.aalto.fi/~ktlokki/Sinfrec/sinfrec.html
  - Zips: `https://mediatech.aalto.fi/images/research/virtualacoustics/recordings/{mozart,beethoven,bruckner,mahler}_mp3.zip`
- **How they were taken:** only the `*_vc*_6.mp3` (cello) and `*_kb*_6.mp3` (Kontrabass)
  entries, plus `mp3info.txt`, by HTTP range requests on each zip (CRC-32 checked).
- **Access date:** 2026-11-23
- **Paper:** Pätynen, J., Pulkki, V., Lokki, T., "Anechoic recording system for symphony
  orchestra," Acta Acustica united with Acustica 94(6), 856-865, 2008.

## Terms, verbatim

> The samples can be used free for academic research.
>
> Please, refer to this article if you use these recording in your research.

No other licence is stated. The page says to contact the authors (Jukka Pätynen, Tapio Lokki)
for the PCM versions.

## Content

- **Instruments and players:** orchestral players (one part per instrument, or two divisi parts
  in Bruckner) recorded one at a time in an anechoic chamber. Players and instruments are not
  named. These are independent of the instruments already in hand.
- **Microphone:** 6 is the frontal main microphone (azimuth 0°, elevation 11°).
- **Format:** MP3, 192 kbit/s, mono ("LAME encoded with options '-b 192 -m m'").
- **Files:** 10 audio files plus 2 `mp3info.txt` (12 files, 34 MB). Only the Mozart and Mahler
  zips contain an `mp3info.txt`.
  - Mozart, Don Giovanni aria, ~3:47: `mozart_vc_6`, `mozart_kb_6`
  - Beethoven, Symphony 7/I bars 1-53, ~3:11: `beethoven_vc_6`, `beethoven_kb_6`
  - Bruckner, Symphony 8/II bars 1-61, ~1:27: `bruckner_vca_6`, `bruckner_vcb_6`, `bruckner_kba_6`, `bruckner_kbb_6`
  - Mahler, Symphony 1/IV bars 1-85, ~2:12: `mahler_vc_6`, `mahler_kb_6`
- **Articulations and dynamics:** real orchestral passages, with legato, détaché, pizzicato
  passages and a wide dynamic range as the scores ask. The gain was fixed for the whole session,
  so quiet parts have a low SNR.

## Verdict

**Measurement only; never ship.** There is no CC licence. Use is granted "free for academic
research". Analysis is allowed under that wording, but it is unclear whether research for an
open-source product counts as "academic". Cite the paper. The denoised WAV versions of the same
performances are in `../phenicx-anechoic/`.

## Addendum, 2026-11-24: violin and viola tracks

The same zips were opened the same way, by range requests with CRC-32 checked. This added 29
violin and viola tracks, all with mic 6, MP3 at 192 kbit/s mono:

| Piece | Tracks |
|---|---|
| Mozart | `vl1`, `vl2`, `vla` |
| Beethoven | `vl1a`, `vl1b`, `vl2a`, `vl2b`, `vlaa`, `vlab` |
| Bruckner | `vl1a`-`vl1h`, `vl2a`-`vl2d`, `vlaa`-`vlac` |
| Mahler | `vl1a`, `vl1b`, `vl2a`, `vl2b`, `vla` |

Each letter is a separately recorded player or desk part. The folder now holds 39 audio files,
121 MB. The terms are unchanged: "free for academic research", **measurement only**.
