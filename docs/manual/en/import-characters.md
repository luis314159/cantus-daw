# Importing a character

*[Back to the manual](README.md) · [Leer en español](../es/import-characters.md)*

See also: [Saltator, the dancer](dancer.md) · [The stage](stage.md) · [Exporting a dancing video](video.md)

A dancer can wear a character of your own: one you drew, one you bought, one shared with a
free licence. Cantus reads four kinds of file:

| Format | The files | Made with |
|---|---|---|
| **Lottie** | `.json`, or a `.lottie` package | After Effects with Bodymovin, Glaxnimate (free), Synfig (free), Lottie Creator, LottieFiles |
| **Aseprite sprite sheet** | a `.json` beside its `.png` | Aseprite, LibreSprite (free) |
| **TexturePacker sprite sheet** | a `.json` (hash or array) beside its `.png` | TexturePacker, free-tex-packer (free) |
| **DragonBones** | `…_ske.json`, `…_tex.json` and `…_tex.png` together | DragonBones, LoongBones |

Rive files (`.riv`) are not read yet. 3D characters (glTF, VRM) are planned. Spine and
Live2D characters cannot be read: their licences ask every user to buy one.

## Import one

1. Open the dancer's window and its character picker, then press **Import character…** —
   or select the dancer's track and choose **File ▸ Import character…**.
2. Choose the file. For DragonBones, choose the skeleton (`…_ske.json`); its atlas is found
   beside it.
3. Cantus checks the file, then shows **the card**:
   - the character **dancing** to the song's tempo — point at a move in the list to see
     that move;
   - its **name**, **format** and **author**;
   - its **licence**: what the file or a `LICENSE` file beside it says, or, when nothing
     says, a short list to choose from (*My own work*, CC0, CC BY, CC BY-SA, the Lottie
     Simple License, *Other*, *I don't know*). What the licence asks of you is said under
     it: credit the author, share alike, not for sale;
   - **what is in the file** (animations, frames, layers or bones) and **what it costs to
     draw** a frame on a 600 × 600 stage (Cantus's budget is 1.5 ms);
   - **what was not used**, each in words — for example a Lottie file's expressions,
     which are scripts and never run (their keyframes play instead).
4. **Map the moves.** Each of Cantus's sixteen moves — *Idle, Bounce, Step left, Step
   right, Sway, Clap, Wave, Jump, Spin, Kick, Shuffle, Point, Arms up, Twist, Bow,
   Freeze* — plays one of the file's animations. Cantus guesses from the names, in English
   and Spanish (a marker or a tag called `clap`, `aplauso` or `Clap_01` is *Clap*); every
   guess says "(guessed)" and every row can be changed. A move left on *None* plays a
   related move. **Beats**: how many beats the move's animation fills at the song's tempo
   (1, 2, 4 or 8).
5. **Playback**: *Sharp pixels* for pixel art; *At its own speed* plays the file at its own
   frame rate instead of to the beat.
6. Press **Use**. The file — with its images and its licence file — is copied into the
   project's **Characters** folder and the dancer wears it. One *Undo* takes it back off.

Animations that name a **mood** (`happy`, `sleepy`, `feliz`…) play instead of *Idle*
while that mood is chosen.

## Change its moves later

In the dancer's window, **Edit moves…** opens the card again with your choices. **Save**
keeps them as a new copy of the character, and **every dancer of this project** wearing it
follows — one *Undo* takes them all back to the moves before (that copy is still there).

## Where it lives

- `Characters/<name>-<code>/` beside the project file holds the copy and `character.json`
  (its format, its licence, your mapping). The code comes from everything in it — the
  files and your choices — and a copy is never changed afterwards, so two projects saved
  in one folder share a copy only when it is exactly the same. Moving or zipping the
  project folder takes it along.
- In a project not saved yet, the copy waits in Cantus's own folder; the first save
  carries it beside the project. *Save As* into another folder carries it too.
- A character file that is missing draws Cantus's default character and the dancer's
  window says so.

## Safe by design

A character file is data, never a program: no script runs, nothing is downloaded, nothing
outside the file's own folder is read, and every size is checked before Cantus gives it
memory. A damaged or hostile file is refused with words saying why.
