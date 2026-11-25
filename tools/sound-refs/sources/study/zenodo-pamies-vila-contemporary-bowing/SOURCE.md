# Pàmies-Vilà 2025: motion capture and audio of bowing techniques by six cellists (two taken)

- **URL:** https://doi.org/10.5281/zenodo.17207713 (https://zenodo.org/records/17207713)
- **Access date:** 2026-11-23
- **Licence:** the Zenodo licence field is `cc-by-4.0`.
- **What was taken:**
  - `ReadMe_CelloContemporaryBowingRecordings.txt` (its MD5 matches Zenodo), the score PDF, and
    `BeginningsDurationsR3.csv`.
  - From `Participant1.zip` and `Participant3.zip`: the WAV and the motion-capture CSV only,
    range-extracted with CRC-32 checked. The duplicate `.mat` copies were left out.
  - Participants 2, 4, 5 and 6 (about 600 MB more) were not taken, to stay within the disk budget.

## Content, from the readme (verbatim excerpts)

> About 10-minute recordings of six cellists playing contemporary bowing techniques using four open strings (A3,D3,G2,C2).
> Motion capture data (given in meters with respect to the ground coordinate frame, Fs = 240 fps)
> Sound WAV-file (Fs = 50000 Hz): The sound was recorded using a microphone (ROGA Instruments RG50) at 45 cm in front of the instrument (channel 2) and an AKG microphone around 2 meters away from the instrument (channel 1).
> The approximate beginn and duration of every exercise for every player is given in the document "BeginningsDurationsR3.csv"

- **Tasks** (from `BeginningsDurationsR3.csv`, semicolon-separated with decimal commas):
  - molto sul tasto, ordinario sul tasto, **ordinario legato**, **ordinario détaché**,
    ordinario sul ponticello, extreme sul ponticello;
  - playing on the bridge, below the pegs, on the tailpiece and on the body;
  - col legno (ordinario and wood, f and p).

  Each task lasts about 14 to 30 s.
- **Mocap:** OptiTrack Motive CSV exports (header rows, then frames) with the bow and cello
  markers as in the readme (`MTip`, `MFrog`, `MBow_A`, `MBow_B`, `MBow_C`, `MNut`, `MBridge`).
- **Players:** P1 and P3, two different cellists. They are independent of the instruments
  already in hand. The instrument is not named and may be the lab's cello for every player.
- **Format:** WAV, 2 channels, 16-bit, 50 kHz (P1 is 500 s).
- **Files:** 4 data files (2 WAV, 2 CSV) plus 3 documents, 349 MB.

## Verdict

**CC BY 4.0: it may ship with attribution.** In practice it is measurement data for bow
gestures and ordinario versus tasto/ponticello timbre. **Analysis is allowed.**
