# The stage: where a dancer dances

*[Back to the manual](README.md) · [Leer en español](../es/stage.md)*

See also: [The dancer](dancer.md) — its moves, moods and characters · [Importing a character](import-characters.md) · [Exporting a video](video.md) — the song with its dancers (also from the companion's menu: *Export video…*).

A **dancer** is a track whose instrument makes no sound and draws a character that dances
to your song. This page is about *where* it dances; its moves, moods and characters are the
dancer's own window.

## Over the workspace, on the Stage tab, or hidden

Each dancer has a place to show (its window's *Where*):

- **Over the workspace** — the character stands right on top of your arrangement, mixer
  and editors, with nothing around it: no window, no frame, see-through. Its **X** and
  **Y** say where its feet are (0 to 1 across and down the workspace) and **Size** how tall
  it is (at 1×, about a third of the workspace's height). Automate X and it walks across
  the window.
- **On the Stage tab** — the character dances on a stage of its own, a tab of the
  workspace that opens under the inspector the first time. Drag the tab anywhere, stack it
  beside others or float it as you would any other area; your layouts keep it.
- **Hidden** — it shows nowhere and costs nothing.

## Move it, resize it, pin it

Point at a dancer and its frame appears:

- **drag its body** to move it (one undo step per drag: *Move dancer*);
- **drag the round grip** at its top-right corner to make it bigger or smaller (its feet
  stay where they are);
- in the **pill** above it: **pin** keeps it above the floating panels (a floating mixer,
  a device window); **dock** puts it on the Stage tab (on the tab, **float** puts it back
  over the workspace); **hide** hides it.

The frame fades a moment after the pointer leaves.

## What it costs

A dancer is drawn only when its pose changes: at most 60 times a second while the song
plays, 15 times a second while it is stopped (a slow breath and a blink), and not at all
when it is hidden or Cantus is minimised. Stopped, once nothing has happened for ten
seconds (no pointer over the window, no key, no Play), it holds its pose and costs nothing;
the next thing you do wakes it where it was. While the companion window is open it keeps
breathing, since that is what you stream. Each dancer is drawn into a box just big enough
for it, at your screen's real pixels, so it is crisp at every zoom.

## The companion window

*View ▸ Companion window* (**Alt+Shift+D**) opens a small window of its own, **Cantus
Stage**, that floats above everything — Cantus, your browser, a game — with no frame and a
see-through background. A character dances in it **with whatever plays**, with no
automation: the song while it plays; a beat of its own at the project's tempo when the
transport is stopped but sound comes out (a preview, your instrument through an input);
a slow breath in silence. It dances bigger when the music is louder.

Drag it by anything in it to move it; drag its bottom-right corner to resize it. Point at
it for its ⋯ button, or right-click anywhere in it, for its choices — each row steps to the
next value when clicked:

- **Character** — each of Cantus's characters, then *The project's dancers* (your dancer
  tracks, as they dance on the stage).
- **Size** — three companion sizes, a square 600 × 600, a vertical 540 × 960 and a wide
  960 × 540.
- **Background** — transparent, the theme's stage, or a chroma green, blue or magenta.
- **Visualiser** — the master's spectrum, its level, or off; **Bar and beat** and **Song
  name and tempo** on or off.
- **Always on top**, **Streaming mode**, **Copy OBS setup**, and **Close**.

The menu also shows **what it costs**: the processor time of each of its frames, its frame
rate and the share of one core they make. The graphics card's time is not measured (Cantus
does not ask the driver), and the menu says so. Minimised, the window draws nothing and
costs nothing. It stays open across sessions until you close it.

## Streaming mode

*View ▸ Streaming mode* turns the companion into a capture source for OBS Studio or
Streamlabs: the window keeps the same title, **Cantus Stage**, every session, shows nothing
but the stage (no frame or buttons when you point at it — right-click still opens its
menu), and its size and place are fixed and remembered. *Copy OBS setup* in its menu puts
the steps for its background on the clipboard.

| Background | In OBS | What you get |
|---|---|---|
| Transparent | **Window Capture**, capture method *Windows 10 (1903 and up)* | The character with nothing around it |
| Transparent | Window Capture, method *BitBlt* | A black background (use a chroma background instead) |
| Chroma green, blue or magenta | Window Capture + a **Chroma Key** filter of the same colour | The character keyed out; lower *Similarity* until its edges are clean |
| The theme's stage | Window Capture | The character on Cantus's stage |

On Windows, OBS's *Game Capture* could not capture the window on a laptop with two graphics
chips (one draws Cantus, the other runs OBS); *Window Capture* always did. On Linux, use
*Window Capture (Xcomposite)* or *Screen Capture (PipeWire)*; a desktop with a compositor
keeps the transparency, and without one choose a chroma background.

## Shortcuts

| Key | What it does |
|---|---|
| Alt+D | Show or hide the dancers on screen (*View ▸ Stage*) |
| Alt+Shift+D | Open or close the companion window |
| Ctrl+Shift+D (Cmd+Shift+D on a Mac) | Streaming mode on or off (*View ▸ Streaming mode*, or the command palette) |
