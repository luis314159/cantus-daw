# ADR 0088 — Video export: the operating system's H.264 and AAC, our own containers, a PNG layer for editors

**Status:** accepted (round 35 contract; the video builder completes it with what it
measures) · **Date:** 2026-10-31 · **Extends:** ADR 0030 (content on demand), ADR 0047
(licences), ADR 0087 (the dancer: the feed and the renderer) · Measured by the round's probe:
`F:\projects\daw-r35\videoprobe\REPORT.md` (summarised here)

## Context

Round 35's dancer must export as a video with the song's audio: MP4 or WebM for social media
(square 1080×1080, vertical 1080×1920, landscape 1920×1080), a transparent layer for a video
editor, and a chroma option. Cantus had no video encoder. Every path has a licence or patent
question (H.264 and AAC are patented and licensed per unit; VP9/AV1 are royalty-free but
slow or hard to build), and Cantus is one free build for Windows, Linux and macOS (GPL
application, MPL engine).

A probe measured every candidate on a 30-second 1080×1920 30 fps clip with a flash on every
beat and a click on the same beats, decoded back and checked frame by frame:

| Path | 30 s export | Size | Sync | Notes |
|---|---|---|---|---|
| Media Foundation H.264 + AAC (software) | 3.4 s | 9 MB | 0.00 ms (B-frames off) | in-box on Windows; its sink writer has four faults (below) |
| Media Foundation, hardware encoder | 5.8 s | 31 MB | 0.00 ms | the driver's choice |
| OpenH264 (Cisco's binary) + MP3, our MP4 | 6–7 s | 9 MB | 0.00 ms | Cisco pays the royalty only for its own binary, downloaded |
| OpenH264 built from source (no nasm) | 17–29 s | 9 MB | 0.00 ms | Cantus would owe the royalty |
| rav1e AV1 + Vorbis, our WebM (no asm) | 160 s with 8 tiles; 757 s without | 15 MB | +2 ms (fixable) | royalty-free, pure Rust, slow |
| PNG MOV (RGBA) + PCM, our MOV | 1.9 s | 55 MB | 0.00 ms, bit-exact | alpha for editors, no patents |
| PNG sequence + WAV | 1.8 s | 55 MB | by construction | the universal fallback |

What platforms accept on upload (their help pages, 2026-10-31): only **H.264 + AAC-LC in
MP4** is accepted by all of YouTube, TikTok, Instagram, Facebook Reels and X (Instagram
takes AAC audio only). WebM is taken by YouTube and TikTok only. VP9 (every libvpx crate
needs a system libvpx and bindgen) and Opus (cmake) do not build here; Vorbis does.

## Decision

### 1. MP4 for social media: H.264 High + AAC-LC from the operating system, our own MP4 writer

- **Windows: Media Foundation's encoder transforms** (the H.264 and AAC MFTs, hardware
  allowed when the driver offers it), driven directly — **not** its sink writer, which (as
  measured) delays video a frame with its B-frames, writes `moov` at the end, drops AAC's
  priming (the first ~9 ms silent) and ignores the bitrate. B-frames off. The codecs are
  licensed with Windows: Cantus distributes no encoder and owes nothing. **Windows N** (no
  Media Feature Pack): detected, and the dialog says how to add the free pack — or offers
  path 1b.
- **macOS: AVFoundation / VideoToolbox and AudioToolbox**, in-box and Apple-licensed, into
  the same MP4 writer (compiled on the macOS runner; confirmed in the Mac session).
- **Linux (1b, and Windows N as a choice): Cisco's OpenH264 binary**, downloaded on demand
  through the content manager (ADR 0030: hash checked — `openh264-sys2` pins it; never in the
  installer; an off switch; the words "OpenH264 Video Codec provided by Cisco Systems, Inc."
  shown), with `UsageType::CameraVideoRealTime`. Audio: **AAC from an ffmpeg or GStreamer
  already on the system** (the person's own, never shipped); without one, **MP3 in MP4**,
  with a plain warning that Instagram may refuse it and how to get AAC.
- **Our own MP4 writer** (ISO BMFF, in Cantus): `moov` in front (fast start), `avcC`, `esds`,
  an audio edit list that trims the encoder's delay (LAME 1 105 samples; AAC's priming
  likewise), no edit list on video.

### 2. WebM: AV1 (rav1e) + Vorbis, offered as the royalty-free choice

Pure Rust (BSD-2 with AOM's patent licence), no download; tiles and threads on; honestly
labelled slower (about five times real time at 1080×1920 without assembly). Its first
packet's offset fixed so sync is 0 ms. Whether rav1e's assembly (nasm in every build and CI
runner) is worth it is the video builder's measurement — decided in this ADR's amendment.

### 3. A transparent layer for editors: QuickTime MOV with PNG frames, and a PNG sequence

MOV with `png ` frames (RGBA, depth 32, straight alpha) and 16-bit PCM audio — the `png`
crate and our own MOV writer, the same on every system, bit-exact, no patents; and a PNG
sequence + WAV as the universal fallback. (ProRes 4444 is in-box only on macOS: there, an
option.) Which editors import it is said in the manual from what is verified.

### 4. Chroma

Any output with a solid key colour (`render::CHROMA_*`). The dialog says that MP4's 4:2:0
colour blurs the character's edges and offers the PNG MOV for keying-quality work.

### 5. No ffmpeg is shipped

Distributing H.264/AAC encoders would expose Cantus to per-unit fees (Via LA: AVC free to
100 000 units a year, then $0.20; AAC $0.98 a unit from the first). An ffmpeg already on the
person's system may be used where §1 says; never downloaded by Cantus.

### 6. Sync and frames

- Video timestamps are the frame index on a rational timescale (30 000 a second; 1 000 a
  frame at 30 fps, 500 at 60); audio is the export's own render (the same as *Export audio*),
  cut per frame and timed by its sample index; both start at 0.
- The dancers' poses come from the render's own feed timeline (ADR 0087: offline, every
  block's `DanceFrame`), sampled at each frame's time — the screen and the video agree.
- Frames render on worker threads (tiny-skia at 1080×1920 is 11–14 ms a frame on one
  thread) in order, with bounded memory.
- A test decodes every output (FFmpeg in the test tools, not shipped) and checks a flash on
  each beat against the click within half a frame, at 30 and 60 fps.

## Consequences

- Windows and macOS export the platforms' universal format in seconds, with nothing to
  download and no fee; Linux downloads 1 MB from Cisco once for H.264 and uses the system's
  AAC when there is one.
- Cantus carries its own MP4, MOV and WebM writers (small, tested by decoding back).
- The video code lives in a new GPL crate (`cantus-video`) beside `cantus-export`, with the
  OS bindings it needs (the `windows` crate's Media Foundation; objc2 on macOS); nothing in
  the MPL crates.
- Owner-only: importing the PNG MOV into the editors the owner uses (Resolve, Premiere, CapCut);
  uploading a test video to each platform; the legal review gains Cisco's OpenH264 terms.

## Amendment 1 — the first delivery, measured (video builder, 2026-10-31)

`crates/cantus-video` (GPL) holds the MP4 and MOV writers, the PNG sequence + WAV, the frame
pipeline and Media Foundation's encoders driven directly (`src/mf/`, the only unsafe code).
What building it measured, and what it decided:

- **Media Foundation's AAC encoder adds no delay of its own and silences its first packet**
  (decoded with FFmpeg ignoring the edit list: samples 0–1 023 are zero, the second beat's
  click lands on sample 24 000 exactly). That is the probe's "click at t=0 lost". Cantus
  feeds it **1 024 samples of silence first** (and 2 048 after the song, so the last packet
  is overlapped and whole); the audio edit list starts at 1 024 and lasts exactly the
  video. Decoded back by FFmpeg at 30 and 60 fps: **every click on its flash at 0.000 ms**,
  the click at time 0 kept and whole (every click's peak within 10 % of the others), the
  audio exactly the video's length (`tests/sync.rs`).
- **Windows' own player stack (Media Foundation's MPEG-4 source) ignores audio edit
  lists**: there every frame is at its time and the audio is **one AAC packet (21.333 ms)
  late** — measured to be exactly as late as an MP4 FFmpeg itself makes of the same clip
  (`tests/mf.rs` plays both). No AAC MP4 can do better there: a sample at decoded time 0
  is always silent with this encoder. FFmpeg — what the platforms ingest with — honours
  the list, so what is uploaded is exact.
- **The software H.264 transform** (synchronous; the hardware one was slower here and
  three times the bytes at the probe's CBR): High profile, no B-frames, a key frame every
  two seconds, BT.709 limited range in the stream's VUI and the MP4's `colr` (FFprobe
  reads `tv, bt709`), unconstrained VBR around the chosen quality's mean (`Quality`:
  0.07 / 0.13 / 0.25 bits a pixel a frame). The encoder spends less on simple pictures:
  the 30-second clip came out at 1.9 Mbit/s with a **PSNR of 44.5 dB** (minimum 42.2 dB)
  against the lossless render. Its worker threads are **capped at 8**: its default (one a
  processor thread) peaked at 646 MB for the same speed as 8 threads' 449 MB.
- **Media Foundation is looked up, never imported.** The `windows` crate imports
  `mfplat.dll` into every binary that calls `MFStartup` — and a program importing a DLL the
  system lacks does not start at all (Windows N without the Media Feature Pack; Windows
  Server without its Media Foundation feature). The few functions are found in `System32`
  when a video is first exported (`mf/sys.rs`); their absence is "no H.264 encoder", said in
  the dialog. A test reads its own executable's import table and fails if any Media
  Foundation DLL is in it. The tests that need the encoders skip (said) on a system
  without them, unless `CANTUS_REQUIRE_MF=1`.
- **The index in front without copying the file**: the writer reserves room for the index
  from the expected sample counts, writes the samples after it, then the index and a
  `free` box; past 4 GB the `mdat` takes the reserved `wide` box for its 64-bit size, and
  offsets go to `co64`. An index that outgrows its room moves the samples up in place.
- **The pipeline**: frames drawn on up to 8 workers (processor threads less one), each into
  its own pixmap, converted there (NV12, or PNG with the fast deflate), handed over in order
  through two-place channels: at most 3 × workers frames exist at once (a test holds it).

The 30-second 1080×1920 export, the stand-in character through `cantus_dance::render` and a
120 BPM song (`cargo run -p cantus-video --release --example measure`; this machine, other
builds running, ±30 %):

| Output | Wall | Size | Peak memory |
|---|---|---|---|
| MP4 1080×1920 30 fps (H.264 + AAC 192 kbit/s) | 3.8 s (8× real time) | 7.1 MB | 449 MB |
| MP4 1080×1920 60 fps | 7.5 s | 11.8 MB | 449 MB |
| MP4 1080×1080 30 fps | 2.2 s | 4.4 MB | 296 MB |
| MP4 1920×1080 30 fps | 3.6 s | 4.4 MB | 449 MB |
| MOV PNG RGBA + PCM 30 fps | 2.2–3.0 s | 79 MB | 149 MB |
| MOV PNG RGBA + PCM 60 fps | 4.9 s | 153 MB | 149 MB |
| PNG sequence + WAV 30 fps | 2.4 s | 79 MB | 149 MB |

The stand-in draws in 1.2 ms a frame at 1080×1920 on one thread; the cast's characters will
cost more, which the workers absorb.

## Amendment 2 — the rest of the paths, measured (video builder, 2026-11-01)

- **Linux (and Windows N): Cisco's OpenH264 through the content manager.** The `.bz2`
  comes from `https://ciscobinary.openh264.org/` by the content manager's checked fetch
  (the system's certificates, a proxy's page refused); its SHA-256 is pinned in
  `cantus-video` for linux x86-64, linux arm64 and windows x86-64 (measured 2026-10-31), it
  is unpacked with a 16 MB bound by a pure-Rust bzip2 decoder, and the library's SHA-256 is
  checked against `openh264-sys2`'s list of Cisco's builds before it loads. The dialog
  switches it on and off, shows Cisco's words and links its licence. Decoded back: 0.000 ms
  with MP3 and with FFmpeg's AAC (`tests/alt_audio.rs`). Cisco's binary warns that
  bitrate cannot be held without frame skipping; skipping stays off (one access unit a
  frame), so the bitrate is a target.
- **AAC without the system's encoder**: an FFmpeg or GStreamer (gst-libav's `avenc_aac`)
  already on the system, driven through pipes (ADTS in, headers taken off; libavcodec's
  1 024-sample priming in the edit list). Without either, **MP3 in the MP4** (LAME, 1 105
  samples of delay) with the warning that Instagram takes AAC only. GStreamer's path was
  not available on any machine of this round to decode back; it uses the same libavcodec
  encoder as FFmpeg's path.
- **macOS: VideoToolbox and AudioToolbox through self-contained FFI** (`src/apple/vt.rs`,
  `at.rs`: only `std`, so they compile for `aarch64-apple-darwin` and
  `x86_64-apple-darwin` on any machine — done here with `-D warnings`). AVCC samples become
  Annex B with the parameter sets before key frames; AudioToolbox's own priming
  (`kAudioConverterPrimeInfo`) is the edit list's start. What only a Mac confirms is row
  18 of `docs/mac-session.md`.
- **WebM: rav1e at a constant quantizer.** rav1e 0.8's rate control (a target bitrate)
  produced streams that neither libaom nor dav1d could decode past some frame (a 60 fps
  clip with a flash on every beat: frames 57… lost); the same clip at a constant quantizer
  decodes whole. Quality maps to quantizer 130 / 100 / 70. Vorbis packets are each timed
  from the setup header's modes (block sizes per packet, as FFmpeg's parser reads them), so
  the first packet's offset is 0: **0.000 ms** at 30 and 60 fps decoded back. Our WebM
  writer streams clusters, cues each key frame and patches the sizes at the end.
- **rav1e's assembly: on in releases.** Measured at 1080×1920, speed 10, 16 tiles, 16
  threads (WSL, the same machine): **2.1–2.3 frames a second without, 6.0–6.6 with nasm —
  2.8× faster, the bitstream byte-identical**. A 30-second vertical WebM goes from about
  7 minutes to about 2½. `cantus-video`'s feature `av1-asm` (through `cantus-ui/video-asm`
  and `cantus/video-asm`) is turned on by `xtask package`, which refuses to build without
  nasm, and by the Linux and macOS release jobs (nasm from apt and Homebrew; Windows from
  Chocolatey). Developer builds without nasm still export WebM, slower, the same bytes.
- **The song's video**: the export renders the song with the export's own render and the
  dancers' units keeping their timelines (`RenderProgress::dance`); each frame samples
  them with the engine's own rule (`cantus_engine::dance::sample`) at the range's start
  plus `n / fps`. Tested: an automation clip of `move` over bars 3–4 claps on frames
  120…239 exactly at 30 fps, and the frames written in the file are, bit for bit, the
  renderer's picture of the screen's pose at the same moment (`tests/song.rs`). A project
  without dancers shows the default character dancing to the beat.
