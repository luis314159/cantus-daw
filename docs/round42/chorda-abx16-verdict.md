# Chorda candidate 16 against the Salamander: the owner's blind verdict (2026-11-25)

On 2026-11-25 the owner and a second listener took the offline kit `abx5-dc967aa5a1`. It was rebuilt that day with `chorda_abx` from `tools/chorda-voicing/candidates/c16` and the Salamander Grand in `$CANTUS_REFS/piano`. The key is in `F:\projects\cantus-work\listen\chorda-abx16-key`.

The results were read with `tools/listening/abx_read.py <results> <key.json>`. The owner keeps the result files in the main tree at `docs/prueba_chorda_last/` (untracked; his).

The kit compares three voices:
- **chorda3**: candidate 16, the shipped default.
- **chorda2**: beta.14's voicing.
- **salamander**: the Salamander Grand.

## The owner (Luis)

The listener used headphones. He has 3 years of audio as a hobby and some piano.

**ABX, Chorda against the Salamander: 16/16** (p < 0.0001; 12/16 is critical at α 0.05). He tells them apart every time. In his words, he knows Chorda's timbre: *"se que es chorda no por que suene mas o menos real si no porque creo que ya conozco el timbre"*.

**Preferences, candidate 16 against the Salamander: the Salamander, 5 phrases to 2.**

| phrase | preferred | his words about Chorda |
|---|---|---|
| soft_chords | Chorda | "suena más real" |
| ff_chord | Chorda | the Salamander's clip "tiene un medio que no me encanta" |
| middle_c | Salamander | "tiene un medio que se me hace algo feo, aunque creo suena más real" |
| fast_run | Salamander | "sin medios, muy metálico"; the Salamander's "medios interesantes" |
| top | Salamander | the Salamander has "un sonido de mecanismo de martillos, suena muy real" and Chorda "le falta eso" |
| bass | Salamander | "me suena más real, pero la diferencia no se me hace mucha" |
| ballad | Salamander | he preferred it believing it was Chorda |

**Candidate 16 against beta.14's voicing: candidate 16, 7 of 7 phrases.** Among his comments:
- "Clip 1 mil veces, clip 2 está feo";
- for a beta.14 ff chord, "creo que clipea".

So the round-40 voicing is a clear step forward.

**Catch questions** (one clip against itself): ff_chord passed ("se me hacen iguales"). On the ballad he heard a difference between identical clips after 3 plays ("clip 2 suena mucho mejor"). Weigh his ballad preferences accordingly. The ABX is unaffected.

**His request:** Chorda should be able to reach both colours, the Salamander's mids and its own, "cambiando de piano, ajustando los micros o con EQ".

## The second listener

ABX 8/16 (p 0.60, chance). Three ABX trials were answered with no plays at all, and both catch questions failed. **The result is excluded**: it is unreliable, as the owner expected.

## What the next Chorda pass aims at (round 43, "Chorda's sound")

1. **The mids around middle C.** A colouration he calls ugly, nasal in round 40's c7 verdict and still there. Find the band (the C4 phrase's spectrum against the Salamander's) and its cause (the soundboard modes, the strike point, the mic image) before any EQ.
2. **Fast runs sound metallic, with no mids.** Check the upper partials' decay and inharmonic ring against the Salamander on short notes, and the mid-band energy in the attack.
3. **The top octave lacks the hammer and action noise.** The Salamander carries the key, hammer and damper mechanics. Model the action noise as a layer: the key's thump, the hammer's knock and the damper's release, measured from recordings.
4. **The bass:** close, but the Salamander is still preferred.
5. **The timbre's signature.** 16/16 means a constant cue. Find which one with the sound lab (`tools/soundlab`) on all seven phrases, Chorda against the Salamander, per band and per feature.
6. **Reachable colours.** Expose the mid-band character (the piano model's "voicing" and the mic images) so a user can move toward the Salamander's mids; that is his request.

Then build a new kit, and the owner takes it alone. His ear is the gate.
