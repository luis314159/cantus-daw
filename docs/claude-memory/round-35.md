---
name: round-35
description: "Round 35 (2026-10-31 to 11-02) — the dancing character (BACKLOG §6i), done: squashed to master, published, v0.1.0-beta.10 tagged (Windows+Linux); Win 5 972 + Linux 5 934; next = docs/next-session.md (round 36, the professional studio)"
metadata:
  node_type: memory
  type: project
  originSessionId: f40cc6ed-5cdd-4c32-bcba-0c89b1c34150
  modified: 2026-10-31T20:39:50.577Z
---

Round 35 = docs/next-session.md of round 34 ([[decision-round35-dancer]]). Item 0 had nothing
(macOS/CI/Release green on public 782dc960/0514b44f, no FLAKY, 0 issues, no Mac session →
beta.10 = Windows + Linux).

- Contract **b6f03bfe** on `r35/integration` (F:\projects\daw-r35\integration; base in
  daw-r35/base.txt): `cantus-core/src/inst/dancer.rs` (keys forever, Move×16, Mood×8,
  BASE_KEY 48), `cantus-engine/src/dance.rs` (DanceFrame/DanceFeed, seqlock, offline
  timeline), new GPL crate `cantus-dance` (Character trait, pure `pose_at`, tiny-skia
  renderer, stand-in character, import seam, `examples/preview.rs`),
  `cantus-ui/src/dance_texture.rs`. Briefs: docs/round35/BUILDING.md.
- Design: the dancer is a silent **native instrument** (InstrumentKind::Dancer, schema 49) —
  automation/modulators/MIDI learn/catalogue for free; React-to = a follower modulator on
  `level`; engine publishes frames; pose is a pure function of the frame; one tiny-skia
  renderer for screen, companion and video.
- ADRs: 0087 dancer, 0088 video, 0089 import, 0090 cast, 0091 stage. Keymap 15 = stage.
- Builders (background agents, worktrees daw-r35/<name>): dancer, cast, import, stage —
  each stops after a first delivery to be merged; resume with SendMessage. Video builder
  starts after ADR 0088 from the probe's report (F:\projects\daw-r35\videoprobe\REPORT.md).
- slot.sh = F:\projects\daw-r35\slot.sh (3 build + 1 WSL). Owner preview GIF of the
  stand-in: F:\projects\daw-r35\preview\dance.gif. Owner wants it super professional and
  fun ([[feedback-dancer-quality]]).
- Owner gave permission to push, publish and tag when all green (beta.10).

See [[cantus-parallel-builders]], [[feedback-review-cost]], [[feedback-no-leftovers]].

ADR 0088 (committed d4bede88): MP4 = H.264+AAC via Media Foundation MFTs (not the sink
writer: B-frame offset, moov at end, lost AAC priming) and AVFoundation on macOS into our own
MP4 writer; Linux = Cisco OpenH264 downloaded + system ffmpeg/GStreamer AAC else MP3-in-MP4;
WebM = rav1e+Vorbis (slow); editors = PNG MOV + PNG sequence; no ffmpeg shipped. Owner-only:
import the PNG MOV into their editors; upload tests to platforms.

2026-10-31/11-01: first deliveries of all five merged into r35/integration (video 3484aac7,
stage 4c47fd16, cast Trino `builtin:songbird` 3942b1ed, dancer "Saltator" schema 49 f0a3d752).
Import confirmed: Lottie (own player), Aseprite/TexturePacker, DragonBones in; Rive out (BACKLOG),
no schema 50. All five agents were cut by a weekly API limit and resumed with SendMessage on
2026-11-01 (the owner said "Reanuda todo"). Open: choreo_song test assumed the stand-in's Jump
length (dancer fixing).

2026-11-01: owner saw the six characters (Trino songbird default, Miso cat, Lana lamb, Tic
robot, Corchea note — renamed from Quaver for a trademark clash — Sol child) and the family
hero video (docs/round35/cast/family-hero.{gif,mp4}): "Se ve bien, continúa". Cast, import
and video builders done (targets deleted for cast and video); dancer and stage on review fixes
(stage: true per-pixel alpha via UpdateLayeredWindow, companion ≤ 5 % of a core). Integration
at b1db1c8c.

2026-11-02: dancer (Trino in the house demo, f707f37b) and stage (fourth pass 6ab8c0d1: corner
resize, native menu window while minimised, physical-pixel placement, Ctrl+Alt+D, preview's
visualiser from its own sound, shots with Trino) merged. Integrator fixes: chroma-key contact
shadow (Stage::shadows), Spanish tú register + 'video', shoot_video waits for the preview,
the landing page's dancer section (web/src/dance.rs, wide hero 1440×318 from the hero example
`png 1440 400 30 0.98 0.95` composited + ffmpeg from videoprobe/tools). Docs written: STATUS,
round35/integration.md, README tour, releases/v0.1.0-beta.10.md, beta-checklist, next-session
(round 36 = the professional studio, BACKLOG §0.4 item 6 — integrator's proposal). Gotchas:
`cargo fmt --all` hits os error 206 on Windows (run per package); Git Bash mangles /mnt/f paths
for wsl (use MSYS_NO_PATHCONV=1). Owner test MOV: F:\projects\daw-r35\videoprobe\out\alpha_png.mov.

2026-11-02 (end): final review's six fixes merged (stage b4c1ac79), version bump 720627d1. Windows on
720627d1: workspace 5 972 passed / 85 ignored, engine fuzz 1 011, engine on the pool 169, clippy 0.
Linux (WSL, CANTUS_CI_DIR ~/ci-r35-integration) found one race: video_export's "a MOV asks for no
encoders" read the process-wide PLANS count while another test's worker still ran `ffmpeg
-encoders` — fixed by a per-window `VideoUi::asked` (2bc74573); Linux then 5 934 / 85, clippy 0.
Then: squash-merged to master in one commit ("Round 35: the dancing character …"), pushed to
origin, publish.sh to cantus-daw, lightweight tag v0.1.0-beta.10 from the publish clone. Next
round = docs/next-session.md: round 36 the professional studio (recording mode §4c, VCA +
snapshots, external gear + latency ping, Mackie/HUI, Edison-class editor, ARA 2 measured first)
— the integrator's proposal; the owner may change it.
