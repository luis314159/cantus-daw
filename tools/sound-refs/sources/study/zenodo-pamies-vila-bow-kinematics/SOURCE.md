# Pàmies-Vilà et al. 2024: cello bow kinematics (human player and robot), motion capture and audio

- **URL:** https://doi.org/10.5281/zenodo.10696680 (https://zenodo.org/records/10696680)
- **Paper:** M. Pàmies-Vilà, A. Mayer, E. Matusiak, V. Chatziioannou (2024), "A method for the
  reproduction of cello bow kinematics using a robotic arm and motion capture".
- **Access date:** 2026-11-23
- **Licence:** the Zenodo licence field is `cc-by-4.0`.

## Content, from `data/ReadMe_CelloSpringRecordings.txt` (verbatim excerpts)

> Recording of an about 1-minute musical piece on the cello, using three open strings (A3,D3,G2)
> Synchronised recordings comparing one human-player and two robot performances.
> Place of recording: Department of Music Acoustics, mdw - University of Music and Performing Arts Vienna, February 2023
> Motion capture data (given in meters with respect to the robot coordinate frame, Fs = 240 fps)
> The sound was recorded using a microphone (ROGA Instruments RG50) at 45 cm in front of the instrument.

- **Mocap CSV columns:** `Time_s`, then x/y/z in metres for each of `MTip`, `MFrog`, `MBow_A`,
  `MBow_B`, `MBow_C` (bow markers), `MNut` and `MBridge`. From these you can derive bow
  position, bow velocity, bow-bridge distance and tilt. There is no force channel.
- **Audio:** `sound_player.wav`, `sound_caseA.wav` and `sound_caseB.wav`, 44.1 kHz, about 68 s
  each.
- **Instrument and player:** one professional cellist on the lab's cello (not named). They are
  independent of the instruments already in hand. The playing is détaché/legato open strings in
  Vivaldi's "Spring".

## Files

8 files, 37 MB: 3 CSV, 3 WAV, the score PDF and the readme. Every MD5 matches Zenodo.

## Verdict

**CC BY 4.0: it may ship with attribution**, but it is a short open-string study recording, so
it is mainly measurement data. Analysis is allowed.
