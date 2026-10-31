# Separating stems

*[Back to the manual](README.md) · [Leer en español](../es/stems.md)*

Cantus can split a recording into its parts — **drums, bass, vocals and the rest**, or with
the 6-stem model **guitar and piano** too — with a model that runs **on your computer**.
Nothing is uploaded anywhere.

## Separate a clip, a track or a selection

- Right-click an **audio clip** ▸ *Separate stems…* (with several clips of one track
  selected, they are separated together; clips on several tracks are separated one track
  at a time, since each track's parts go under a bus of its own), or
- right-click a **track's name** ▸ *Separate stems…* (every audio clip on it).

Choose **4 stems** or **6 stems**, **Fast** or **Best** (four passes averaged: a little
cleaner, four times slower) and, on a Mac, *Use the graphics processor when it can*
(on Windows and Linux the panel says it is not available with the runtime Cantus
downloads: the model runs on the processor).
Press **Separate**. The panel shows the progress; *Cancel* stops it (nothing is left
behind), *Hide* closes the panel and the separation goes on.

How good is it? On the test songs researchers use (MUSDB18-HQ), the 4-stem model scores
8.80 dB SDR on average (drums 10.05, bass 9.78, other 6.42, vocals 8.93) — the numbers its
makers published, measured again in Cantus. Busy electronic mixes are the hardest.

When it is done:

- each part is a **new audio track right under the separated track**, named *Drums*,
  *Bass*, *Other*, *Vocals*…, each on its own mixer strip;
- the strips feed **one new bus**, in a mixer group named after the track (*Mix stems*), and the
  bus goes where the original track went — so the song sounds the same, now in parts;
- every new clip sits **exactly where its original clip is**, sample for sample, with the
  same window, fades, gain, warp, crossfades and comp seams; a clip you had muted stays
  muted in every part;
- the parts play at the **original's level**: their strips start at the level that makes
  up for the pan law, so muting the original and playing the parts sounds the same (change
  the pan law later and they are trimmed with it, in the same undo step);
- the **original clips are muted**, not deleted;
- it is **one step of Undo**.

The new files go in the project's *Audio/Stems* folder. Their clips wear a small **AI**
badge: each file remembers which model made it, its version and licence, when, and from
which file (saved in the project).

A file at any sample rate works: it is converted to the model's 44.1 kHz and the parts back
to the file's own rate and length.

## The models (*Settings ▸ Models*)

The first time, Cantus shows the model's size, its **licence** and the inference runtime's,
who made it and what it was trained on. *Accept the licence and download* fetches them:

| Model | Parts | Download | Licence |
|---|---|---|---|
| HT-Demucs — 4 stems (Meta) | drums, bass, other, vocals | 84 MB + the runtime | MIT |
| HT-Demucs — 6 stems (Meta) | and guitar, piano (its piano is weak, its makers say) | 55 MB + the runtime | MIT |

The runtime is Microsoft's **ONNX Runtime** (MIT; 15 MB on Windows, 11 MB on Linux, 37 MB
on Apple silicon), downloaded once for every model, its telemetry turned off. Cantus does
not host the weights: it downloads Meta's own file, checks every byte, and rebuilds the
model from it. A download that is cut resumes where it stopped.

*Remove* deletes a model (and the runtime with the last one); it waits while a separation
runs. Before a separation starts, Cantus checks there is room on the disk for it (*Best*
needs room for four passes) and says so if not. A recording longer than one WAV file can
hold as stems (4 GB: about 45 minutes at 192 kHz) is refused with a word. Opening another
project cancels a separation under way.

## How long it takes

About a third of the song's length on a recent laptop's processor (a 3-minute song in about
65 seconds), four times that with *Best*. The model runs in a helper process of its own,
`cantus-ml`: if it fails, Cantus says so and goes on.

## For AI agents

An agent connected to Cantus can call `separate_stems` (a track or clips; 4 or 6 parts; fast
or best). It asks you first like any edit, and the model must already be installed — its
licence is yours to accept.
