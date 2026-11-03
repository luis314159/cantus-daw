# Editing a clip with an ARA plug-in

*[Back to the manual](README.md) · [Leer en español](../es/ara.md)*

**ARA 2** (Audio Random Access, by Celemony) lets a plug-in see a whole recording and the
clips that play it, instead of only the sound passing through it. Pitch and timing editors,
spectral editors and alignment tools are made this way. Cantus hosts ARA 2 plug-ins in the
**VST3** format.

## Bind a clip

1. Install the plug-in and let the plug-in scan find it (*Settings ▸ Plug-ins ▸ Scan for plug-ins*;
   Cantus scans again by itself, once, the VST3 plug-ins it scanned before it knew ARA).
2. Right-click an **audio clip** ▸ in the **ARA plug-in** section, *Bind to* the plug-in.

The clip now plays the plug-in's render of it in place of its file. Clip gain, fades, the
track and the mix apply as to any clip; an export, a bounce and the stems hear the render
too. *Undo* takes the binding back in one step.

The section always says it: **ARA plug-ins run inside Cantus, not isolated.** Other
plug-ins can run in a helper process, so one that crashes cannot take Cantus with it; an
ARA plug-in shares the recording with Cantus directly, so it runs in Cantus's own process.
Save often while you try a new one.

## What the plug-in keeps

- **Its edits are saved with the project.** When you save, Cantus asks the plug-in for its
  state for each bound clip and stores it in the project file; opening the project gives
  it back.
- **Moving, trimming or splitting a clip** tells the plug-in, which renders the clip again.
- **Deleting a clip, or letting it go,** keeps the plug-in's work for *Undo*: the clip comes
  back with everything the plug-in had done to it.
- **A copy of a bound clip** starts with what the plug-in had done to the original.

## Let a clip go

Right-click the clip ▸ *Let go of* the plug-in. The clip plays its file again. Binding it
again starts clean; *Bind to … with its previous edits* brings back what the plug-in had
done to it.

## When the plug-in is not there

A project opened on a computer without the plug-in keeps the binding and the plug-in's saved
state; the clip plays **its own file**, and its menu says why (*not installed*, or what the
plug-in answered). Install the plug-in and the clip is rendered again.

## Open the plug-in's editor

Right-click the bound clip ▸ *Open* the plug-in. Its own window opens on the clip. Every
edit you make there is **one undo step** in Cantus (*Edit ▸ Undo* takes it back in the
plug-in too), and the clip renders again in the background: you keep hearing the last render
until the new one is ready, and the clip's body shows how far it is. Only the part the
plug-in says changed is rendered again.

## If a plug-in crashes or stops answering

If Cantus closes while an ARA plug-in is working, the next start opens your project with
that plug-in's clips playing their sources, and the clip's menu says why: choose *Retry* to
run it again. A render that takes far too long stops the plug-in for the session; Cantus
never waits for it. *Play sources only (ARA plug-ins off)* in the same menu turns every ARA
plug-in off: each bound clip plays its own file.

## What is not there yet

- A bound clip cannot also be reversed, warped or have Cantus's own pitch edits: the
  plug-in does pitch and time. Remove those before binding.
- The plug-in runs inside Cantus's process; running it isolated comes in a later version.

Cantus is tested with an ARA test plug-in built with Cantus (and, where it has been built,
with Celemony's reference plug-in); tell us how your ARA plug-ins behave ([report a bug](README.md#15-report-a-bug)).
