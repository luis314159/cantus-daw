---
name: reference-elevenlabs
description: "Owner bought ElevenLabs Starter (2026-11-21) for the launch video's voice; API key goes in Windows Credential Manager target cantus-elevenlabs; never for sound packs"
metadata:
  node_type: memory
  type: reference
  originSessionId: c330c055-9d87-4a1d-9df8-77cfd8e6440e
  modified: 2026-11-21T17:18:45.080Z
---

- Plan: ElevenLabs **Starter** ($6/mo, ~30k credits ≈ characters/month, commercial licence),
  bought by the owner 2026-11-21 for the launch video's narration (EN + Latin American ES).
- The API key: Windows Credential Manager, generic credential **target `cantus-elevenlabs`**
  (user `api`), stored by the owner himself. Read it at the moment of use, like
  `~/.cantus-tools/gh-api.ps1` does for GitHub; never print it, never write it into the repo,
  memory, logs or a command line.
- Sending the script to ElevenLabs is publishing it to an outside service: only the video's
  narration lines, which the owner approved for this.
- **Not for Cantus's sound packs:** ElevenLabs' prohibited-use policy (as summarised in Nov
  2026) forbids distributing raw generated effects as standalone samples or sound libraries,
  and Cantus's packs are standalone CC0/CC BY files (ADR 0042, [[decision-cc-by-content]],
  [[decision-packs-always-free]]). Packs stay our own renders.

See [[decision-video-onda-lapis]], [[reference-name-pronunciation]].
- 2026-11-21: **ElevenLabs won on the owner's ear** — the first take of Jon, Victoria and Alejandro; Gabriela disliked. The video's narration is ElevenLabs.
