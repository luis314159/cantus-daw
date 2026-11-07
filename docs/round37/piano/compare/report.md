# Chorda against the reference — measured

Rendered by `chorda_compare` through the real engine at 48 kHz; the reference is **salamander**. Single notes: A0 … C8 every minor third at pp (30), mf (75), ff (115), held 3.5 s.

**Distance** — RMS of the third-octave shape differences against salamander, every note, 63 Hz – 16 kHz (dB; lower is closer): first 50 ms / at 0.5 s

- chorda1: 21.47 / 29.61
- chorda2: 9.44 / 13.51

**Score** (lower is closer): total = shapes + decay + knock + brightness + evenness + stereo

- SCORE chorda1 64.38 = 25.54 + 11.06 + 10.06 + 10.61 + 4.16 + 2.95
- SCORE chorda2 33.08 = 11.47 + 8.75 + 1.97 + 4.85 + 4.27 + 1.77

## Single notes against salamander, by register

Each source's levels are aligned so its mean mezzo-forte level equals the reference's (offsets: chorda1 +6.2 dB, chorda2 +8.2 dB, salamander +0.0 dB). Shapes are third-octave spectra relative to each note's own total (level-free), grouped by octave; a positive number is *more* energy there than the reference. Decay ratios are the source's dB/s over the reference's (> 1: dies faster).

### chorda1

**pp** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s

| register | Δlevel | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | -0.5 | -3.9 / -3.0 | -9.9 / -10.7 | +0.6 / +1.9 | +10.3 / +8.4 | +14.9 / +11.9 | +19.7 / +16.2 | +23.9 / +19.6 | +20.5 / +7.3 | +29.1 / +25.2 |
| middle C3-B5 | +3.5 | -20.0 / -20.8 | -24.4 / -28.1 | -13.4 / -26.8 | -2.8 / -5.5 | +12.4 / +8.6 | +22.4 / +15.4 | +26.9 / +21.4 | +26.5 / +10.3 | +26.9 / +16.8 |
| treble C6-C8 | +5.3 | -17.7 / -7.8 | -26.3 / -22.1 | -21.9 / -47.8 | -20.2 / -79.9 | -9.8 / -6.9 | +3.6 / +0.5 | +5.7 / -3.2 | +18.6 / -19.3 | +23.3 / -12.5 |

attack, decay, stereo and release (source vs reference)

| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | 428 vs 164 | 391 vs 166 | -32.8 vs -28.1 | 0.00 vs 0.00 | 16 vs 21 | +4.0 vs +10.1 | +2.5 vs +3.0 | 0.84/1.01/11.04 | -5.9 vs -4.9 | 438 vs 435 | +0.4 vs -1.5 |
| middle C3-B5 | 927 vs 450 | 825 vs 469 | -42.7 vs -20.3 | 0.00 vs 0.00 | 1 vs 16 | +38.9 vs +20.7 | +5.8 vs +4.1 | 23.57/4.10/3.22 | -9.9 vs -1.2 | 532 vs 509 | -8.4 vs -0.6 |
| treble C6-C8 | 2195 vs 1482 | 938 vs 1192 | -24.5 vs -5.5 | 0.01 vs 0.00 | 0 vs 8 | +64.4 vs +24.5 | – vs +1.8 | 99.39/13.56/20.38 | -5.4 vs -0.5 | 2400 vs 968 | -0.5 vs +24.5 |

**mf** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s

| register | Δlevel | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | -2.5 | -4.8 / -2.7 | -11.2 / -11.9 | -1.2 / +0.3 | +7.6 / +6.4 | +9.2 / +8.2 | +8.4 / +5.5 | +17.8 / +15.4 | +30.7 / +18.4 | +35.5 / +31.6 |
| middle C3-B5 | -0.3 | -22.9 / -15.6 | -26.7 / -25.7 | -14.9 / -25.2 | -4.9 / -6.9 | +8.2 / +5.4 | +11.6 / +8.5 | +13.3 / +12.5 | +26.1 / +10.0 | +35.7 / +26.1 |
| treble C6-C8 | +2.9 | -24.1 / -1.8 | -34.6 / -18.1 | -25.8 / -43.1 | -21.6 / -75.7 | -11.2 / -6.4 | +0.6 / +0.4 | +4.3 / -0.5 | +19.5 / -10.4 | +28.8 / -1.6 |

attack, decay, stereo and release (source vs reference)

| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | 593 vs 215 | 538 vs 220 | -34.7 vs -28.4 | 0.01 vs 0.00 | 17 vs 22 | +5.2 vs +10.0 | +2.6 vs +2.9 | 0.87/0.98/1.67 | -5.5 vs -4.0 | 441 vs 448 | -3.2 vs -3.2 |
| middle C3-B5 | 1170 vs 520 | 998 vs 526 | -45.4 vs -22.0 | 0.00 vs 0.00 | 1 vs 16 | +38.9 vs +22.2 | +6.5 vs +5.0 | 15.95/3.54/2.38 | -9.5 vs -1.1 | 723 vs 504 | -9.3 vs -3.1 |
| treble C6-C8 | 2597 vs 1945 | 1346 vs 1719 | -31.0 vs -10.4 | 0.01 vs 0.00 | 1 vs 7 | +70.2 vs +33.9 | – vs +1.4 | 71.50/9.88/7.28 | -5.7 vs -0.8 | 2400 vs 858 | -0.1 vs -0.0 |

**ff** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s

| register | Δlevel | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | -2.5 | -5.8 / -3.1 | -12.4 / -12.7 | -2.5 / -0.9 | +6.3 / +5.5 | +8.3 / +7.9 | +7.8 / +5.9 | +12.1 / +10.6 | +25.8 / +14.6 | +39.7 / +34.6 |
| middle C3-B5 | -1.3 | -24.9 / -15.0 | -28.3 / -23.4 | -15.4 / -24.8 | -6.1 / -7.7 | +7.0 / +4.8 | +10.1 / +8.3 | +10.1 / +10.9 | +17.7 / +7.5 | +32.9 / +29.4 |
| treble C6-C8 | +1.8 | -26.1 / +0.2 | -34.2 / -16.6 | -26.3 / -41.2 | -22.8 / -77.0 | -11.6 / -7.4 | -0.8 / -1.1 | +4.2 / -0.4 | +14.4 / -15.9 | +24.1 / +2.3 |

attack, decay, stereo and release (source vs reference)

| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | 836 vs 267 | 712 vs 266 | -35.6 vs -29.4 | 0.02 vs 0.00 | 18 vs 25 | +6.3 vs +10.3 | +2.7 vs +3.0 | 0.86/1.00/1.47 | -5.4 vs -3.8 | 442 vs 461 | -3.9 vs -3.6 |
| middle C3-B5 | 1388 vs 573 | 1106 vs 546 | -46.9 vs -21.9 | 0.01 vs 0.01 | 1 vs 18 | +39.0 vs +23.1 | +6.3 vs +5.2 | 12.74/3.55/2.60 | -9.4 vs -1.0 | 875 vs 495 | -9.8 vs -3.5 |
| treble C6-C8 | 2788 vs 2167 | 1585 vs 1947 | -33.9 vs -12.6 | 0.00 vs 0.01 | 1 vs 5 | +72.5 vs +37.0 | – vs +2.9 | 60.84/8.41/5.76 | -5.8 vs -0.7 | 2400 vs 918 | +0.1 vs +1.4 |

### chorda2

**pp** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s

| register | Δlevel | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | +6.2 | -6.0 / -6.3 | -5.0 / -5.7 | +4.1 / +6.0 | +6.3 / +2.0 | -5.3 / -8.6 | -5.2 / -8.5 | -0.8 / -3.2 | -4.4 / -17.9 | -10.8 / -16.4 |
| middle C3-B5 | +2.8 | +5.2 / -28.1 | +0.5 / -14.8 | -1.2 / -4.3 | -0.4 / -0.1 | +9.1 / +4.3 | +12.5 / +3.7 | +11.2 / +7.0 | +10.2 / -3.1 | -2.9 / -25.5 |
| treble C6-C8 | +0.8 | +13.0 / -16.7 | +7.6 / -12.6 | -2.0 / -5.2 | -3.6 / -0.7 | -3.8 / +3.6 | -0.9 / +3.4 | -4.2 / +2.4 | +9.5 / -11.7 | +3.3 / -44.5 |

attack, decay, stereo and release (source vs reference)

| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | 254 vs 164 | 240 vs 166 | -28.6 vs -28.1 | 0.00 vs 0.00 | 34 vs 21 | +4.1 vs +10.1 | +3.4 vs +3.0 | 0.84/1.03/7.50 | -1.0 vs -4.9 | 463 vs 435 | +8.1 vs -1.5 |
| middle C3-B5 | 628 vs 450 | 529 vs 469 | -19.8 vs -20.3 | 0.00 vs 0.00 | 22 vs 16 | +24.2 vs +20.7 | +4.4 vs +4.1 | 11.94/1.72/5.05 | -1.7 vs -1.2 | 540 vs 509 | +3.1 vs -0.6 |
| treble C6-C8 | 1011 vs 1482 | 1365 vs 1192 | -6.8 vs -5.5 | 0.00 vs 0.00 | 9 vs 8 | +52.6 vs +24.5 | – vs +1.8 | 44.21/7.39/-0.41 | -2.3 vs -0.5 | 695 vs 968 | +2.7 vs +24.5 |

**mf** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s

| register | Δlevel | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | +3.4 | -6.1 / -5.4 | -5.6 / -6.3 | +3.7 / +5.7 | +5.6 / +2.1 | -4.8 / -7.2 | -13.5 / -16.7 | -7.2 / -8.5 | +6.4 / -2.6 | -0.2 / -9.6 |
| middle C3-B5 | -1.1 | +6.7 / -18.8 | +0.9 / -9.9 | -2.6 / -2.4 | -2.1 / -0.9 | +6.9 / +3.4 | +5.4 / +1.0 | +1.5 / +2.6 | +10.0 / +4.1 | +9.6 / -16.0 |
| treble C6-C8 | -1.9 | +12.2 / -11.6 | +4.3 / -10.1 | -4.6 / -6.3 | -4.6 / -2.2 | -3.6 / -0.6 | +0.9 / +3.0 | +0.3 / +3.7 | +13.0 / +2.3 | +12.0 / -28.6 |

attack, decay, stereo and release (source vs reference)

| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | 286 vs 215 | 266 vs 220 | -28.0 vs -28.4 | 0.00 vs 0.00 | 26 vs 22 | +4.2 vs +10.0 | +3.3 vs +2.9 | 0.87/0.98/1.15 | -1.1 vs -4.0 | 459 vs 448 | -0.7 vs -3.2 |
| middle C3-B5 | 797 vs 520 | 692 vs 526 | -19.2 vs -22.0 | 0.00 vs 0.00 | 20 vs 16 | +25.2 vs +22.2 | +4.3 vs +5.0 | 8.17/1.65/2.96 | -1.5 vs -1.1 | 403 vs 504 | -5.5 vs -3.1 |
| treble C6-C8 | 1743 vs 1945 | 2002 vs 1719 | -10.5 vs -10.4 | 0.01 vs 0.00 | 12 vs 7 | +51.6 vs +33.9 | – vs +1.4 | 27.21/5.22/0.31 | -2.3 vs -0.8 | 1463 vs 858 | -2.5 vs -0.0 |

**ff** — spectral shape difference, first 50 ms (dB per octave band) / at 0.5 s

| register | Δlevel | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | +2.4 | -5.4 / -4.9 | -5.8 / -6.3 | +3.7 / +5.6 | +5.8 / +2.8 | -2.3 / -3.9 | -9.5 / -11.6 | -11.2 / -11.3 | +1.9 / -2.0 | +7.9 / -6.0 |
| middle C3-B5 | -2.1 | +8.7 / -14.6 | +2.1 / -4.8 | -2.5 / -2.0 | -3.2 / -1.6 | +6.5 / +3.5 | +5.4 / +2.1 | +1.3 / +4.6 | +5.0 / +7.0 | +8.6 / -9.5 |
| treble C6-C8 | -2.4 | +13.5 / -8.5 | +8.0 / -7.1 | -4.2 / -6.8 | -7.0 / -6.8 | -4.4 / -4.0 | +0.5 / +0.8 | -1.0 / +2.4 | +11.5 / +3.0 | +9.2 / -16.5 |

attack, decay, stereo and release (source vs reference)

| register | centroid 50 ms | centroid 0.5 s | thump dB | noise | rise ms | decay early dB/s | decay late dB/s | decay ×(250/1k/4k) | side dB | release ms | release HF dB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bass A0-B2 | 318 vs 267 | 289 vs 266 | -26.3 vs -29.4 | 0.00 vs 0.00 | 27 vs 25 | +4.4 vs +10.3 | +3.3 vs +3.0 | 0.87/0.99/1.00 | -1.1 vs -3.8 | 459 vs 461 | -2.6 vs -3.6 |
| middle C3-B5 | 912 vs 573 | 805 vs 546 | -17.2 vs -21.9 | 0.00 vs 0.01 | 20 vs 18 | +25.8 vs +23.1 | +4.6 vs +5.2 | 6.42/1.53/2.66 | -1.3 vs -1.0 | 335 vs 495 | -7.1 vs -3.5 |
| treble C6-C8 | 2051 vs 2167 | 2235 vs 1947 | -11.1 vs -12.6 | 0.01 vs 0.01 | 12 vs 5 | +50.8 vs +37.0 | – vs +2.9 | 22.18/4.33/0.68 | -2.1 vs -0.7 | 2032 vs 918 | -3.0 vs +1.4 |

## Phrases

Loudness (LUFS, integrated), peak to loudness (PLR, dB), spectral centroid, stereo correlation and side under mid; then the long-term spectrum's difference from salamander per octave (dB, level-free).

| phrase | source | LUFS | PLR | centroid Hz | corr | side dB | 63 | 125 | 250 | 500 | 1k | 2k | 4k | 8k | 16k |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| middle_c | salamander | -34.6 | 14.9 | 462 | 0.52 | -4.2 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| middle_c | chorda1 | -41.7 | 16.7 | 996 | 0.80 | -9.5 | -21.3 | -18.4 | -10.9 | -3.5 | +10.8 | +11.0 | +6.4 | +15.8 | +25.5 |
| middle_c | chorda2 | -43.6 | 13.5 | 603 | 0.63 | -6.4 | -3.3 | -2.1 | +0.9 | -12.6 | +9.8 | +2.4 | -11.7 | -2.7 | -5.3 |
| soft_chords | salamander | -37.9 | 17.4 | 345 | 0.37 | -3.3 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| soft_chords | chorda1 | -40.4 | 17.5 | 856 | 0.81 | -9.4 | -21.6 | -15.0 | -10.7 | -0.4 | +12.4 | +9.5 | +15.7 | +20.4 | +21.2 |
| soft_chords | chorda2 | -42.5 | 16.1 | 435 | 0.21 | -1.7 | -12.8 | -8.1 | +1.1 | +0.7 | +4.9 | -0.7 | +0.4 | +2.3 | -12.1 |
| fast_run | salamander | -29.1 | 12.7 | 609 | 0.17 | -1.3 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| fast_run | chorda1 | -32.3 | 10.2 | 1242 | 0.78 | -9.0 | -17.6 | -23.6 | -15.9 | -7.7 | +5.0 | +10.1 | +9.6 | +23.5 | +32.9 |
| fast_run | chorda2 | -37.1 | 11.8 | 827 | 0.36 | -3.2 | +10.9 | +2.4 | -0.9 | -3.1 | +3.8 | +4.9 | -0.0 | +8.7 | +8.8 |
| ballad | salamander | -33.5 | 12.3 | 289 | 0.30 | -2.7 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| ballad | chorda1 | -40.4 | 15.0 | 748 | 0.71 | -6.9 | -6.8 | -9.4 | -2.3 | +1.5 | +11.3 | +10.4 | +11.2 | +26.4 | +27.8 |
| ballad | chorda2 | -37.2 | 14.4 | 330 | 0.18 | -1.0 | -7.5 | -6.9 | +5.5 | -1.5 | +1.8 | -3.1 | -5.0 | +5.9 | -9.0 |
| ff_chord | salamander | -21.8 | 19.4 | 392 | 0.24 | -2.1 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| ff_chord | chorda1 | -27.9 | 22.3 | 998 | 0.74 | -7.3 | -5.5 | -10.6 | -7.3 | -0.8 | +8.2 | +7.4 | +7.2 | +14.6 | +31.2 |
| ff_chord | chorda2 | -26.6 | 21.0 | 447 | 0.17 | -1.1 | +0.8 | -6.4 | +3.1 | -0.0 | +2.8 | -1.1 | -4.4 | -1.7 | +3.6 |
| bass | salamander | -32.0 | 14.5 | 235 | 0.50 | -4.4 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| bass | chorda1 | -39.4 | 19.7 | 586 | 0.67 | -4.7 | -5.2 | -7.8 | +0.6 | +4.7 | +8.2 | +7.2 | +9.2 | +22.4 | +32.3 |
| bass | chorda2 | -35.5 | 16.1 | 270 | 0.22 | -1.0 | -9.6 | -4.9 | +5.8 | +1.8 | -5.4 | -13.4 | -14.0 | -0.7 | -7.9 |
| top | salamander | -39.3 | 14.5 | 2121 | 0.21 | -1.8 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 | +0.0 |
| top | chorda1 | -38.1 | 15.9 | 2333 | 0.89 | -7.7 | -20.3 | -28.5 | -22.1 | -16.0 | -11.7 | +0.7 | -2.3 | +16.0 | +29.8 |
| top | chorda2 | -45.8 | 15.7 | 2037 | 0.16 | -1.1 | +9.7 | -0.5 | -4.7 | -5.6 | -6.7 | +0.4 | -5.9 | +6.2 | +12.9 |

