# Saltus, the pitch unit — first sounds (fx side round, wave 3)

Guitar: Carbonthief, CC0 (Strummed Guitar Chords, the Renoise forum's library).
Am – F – C – G at 90 BPM, struck on beats 1 and 3, eight bars; `guitar/00-guitar-di.wav`
is the dry DI. The bass line and the sung line are synthesized by Cantus's own test
material (plucked strings; a glottal source through vowel formants — not a singer): each
set has its dry twin (`00-…`). Every processed file is level-matched to its dry twin (RMS),
its peak kept under −1 dBFS. 48 kHz, 24-bit WAV. Rendered by Saltus's DSP alone
(`cargo run -p cantus-dsp --example saltus_sounds`), the engine not yet wired.

