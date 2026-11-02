# Exporting a dancing video

*[Back to the manual](README.md) · [Leer en español](../es/video.md)* · See also: [the
dancer](dancer.md) · [the stage](stage.md) · [importing characters](import-characters.md)

*File ▸ Export video…* makes a video of your dancers with the song: a vertical clip for
TikTok, Reels or Shorts, a square one, a wide one for YouTube, or a transparent layer for a
video editor. What you see in the preview is what the file will hold, frame for frame —
the dancers move exactly as they do on the screen, their automation included.

## The window

- **Preview** (left): the frame at any moment of the song. Drag the bar under it to look
  around. The background and the title are drawn as they will be in the file; a
  transparent background shows as a checkerboard. To show the dancers exactly as the file
  will, the preview renders a few seconds of the song around that moment — automation,
  notes and modulators included (plug-ins from other makers are left out of it; they are in
  the file).
- **Which dancers**: every dancer of the project except one set to show nowhere (hidden).
  An imported character still being read is waited for ("Loading the characters…"); one
  that cannot be read stops the export and says which.
- **Format**:
  - **MP4** — H.264 and AAC, the one format every platform takes (YouTube, TikTok,
    Instagram, Facebook, X). Made in seconds.
  - **WebM** — AV1 and Vorbis, royalty-free; YouTube and TikTok take it. It takes longer to
    make: the estimate under the choices says how long.
  - **MOV + alpha** — lossless PNG frames with a transparent background, for Final Cut,
    Kdenlive, Shotcut and other editors.
  - **PNG frames** — a folder of numbered PNG files and the song as a WAV: any editor
    imports it.
- **Shape**: vertical 1080 × 1920, square 1080 × 1080, wide 1920 × 1080, or another size
  (720 × 1280 up to 4K, and Instagram's 1080 × 1350).
- **Frame rate**: 30 or 60 frames a second. **Quality**: Good, High (what platforms
  recommend) or Best.
- **Background**: the theme's stage colour, transparent (MOV and PNG only), a chroma key
  (green, blue or magenta) or black. A key in an MP4 or WebM blurs a little at the
  character's edges (those formats store colour at half resolution): for clean keying use
  MOV + alpha.
- **Title**: the song's name and tempo along the foot of the picture.
- **Range**: the whole song, the time selection or the loop.
- **Encoders**: which encoder makes the picture and the sound on this computer, said before
  you start.

The footer says how long the export should take and how big the file will be; during the
export, how far it is and how long is left. *Stop* stops it and leaves nothing behind.

## In sync, to the sample

Each frame is drawn at its exact time and the sound starts on the same sample: a beat's
step lands on its beat in every player and on every platform. One exception you might
notice when comparing very closely: **Windows' own player (Media Player, Films & TV) plays
the sound of any AAC MP4 about 21 ms late** — files from other programs too — because it
ignores the instruction that trims the encoder's start. What you upload is exact.

## Where the encoders come from

Cantus ships no video encoder of its own (H.264 and AAC are licensed per copy):

- **Windows** uses Windows' own H.264 and AAC encoders. *Windows N* editions lack them:
  add the free *Media Feature Pack* (Settings ▸ Apps ▸ Optional features), or use Cisco's
  OpenH264 (below).
- **macOS** uses macOS's own encoders (VideoToolbox and AudioToolbox).
- **Linux** uses **Cisco's OpenH264**: switch it on in the window and press *Download from
  Cisco* (once, about 0.6 MB). It is Cisco's own build, checked byte for byte, and Cisco's
  licence applies (*OpenH264 Video Codec provided by Cisco Systems, Inc.*); switch it off any
  time. The sound is AAC when FFmpeg or GStreamer (with gst-libav) is installed — the
  window says which — and otherwise MP3, which plays everywhere but which Instagram
  refuses: install FFmpeg (`sudo apt install ffmpeg`) for AAC.
- **WebM, MOV and PNG** need nothing: they are made by Cantus itself on every system.

## For agents

An assistant connected through MCP can export a video with `export_video` (format, shape,
frame rate, quality, background, title, range, file) and follow it with
`get_video_export`. Like every tool that writes files, it is refused until you allow files
in *Settings ▸ AI*, and you are always asked.
