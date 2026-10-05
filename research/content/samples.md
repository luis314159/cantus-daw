# Rights-free samples Cantus could bundle or offer as downloads

Research note, 2026-10-05. Research only: nothing here is a decision. The owner rules on
licences (ADR 0001: bundled samples must be original or CC0; idea.md §8: "verified
permissive/CC0"). Order: drums first, then loops, instruments, FX/foley.

Method: every licence below was read on the source's own page or repository on 2026-10-05,
unless the row says **not verified**. Several sites could not be read from this network
(corporate proxy: JavaScript challenges, a "games" category block on kenney.nl, gnu.org
timing out), and the web-search budget ran out part-way. Those rows say so. Where this
note corrects the September research (research/instruments/sampler.md §9.6), it says
**correction**.

## 0. Short answer

- **Best CC0 drum sources, all verified:** Karoryfer's free drum libraries (Frankensnare,
  Gogodze Phu I and II, Big Rusty, Swirly, Unruly), all CC0-1.0 in their GitHub repos;
  the Versilian Community Sample Library (VCSL, CC0-1.0), which has claps, hi-hat, snares,
  toms, congas, bongos, cajon, shakers, tambourines, cowbells, claves and woodblock; and
  Virtuosity Drums (Versilian x Karoryfer, CC0-1.0, jazz kit).
- **No verified CC0 source of 808/909-style electronic one-shots was found that is clean
  enough to bundle.** Most "free 808" packs are royalty-free licences that forbid
  redistribution, or are rips of the machines. Cantus should make its own: render
  Pulsus (our drum synth) to WAV and release the files CC0. This costs us nothing in
  licences and is the cleanest provenance possible.
- **Acoustic kits under CC-BY 4.0** (DrumGizmo DRSKit, MuldjordKit, CrocellKit; Naked
  Drums) are good but need the owner's ruling. DrumGizmo adds an extra request to credit
  them in album notes. This goes beyond plain CC-BY.
- **Do not bundle:** 99Sounds, Splice, Cymatics, Sonniss GDC, Philharmonia, Maestro Concert
  Grand (**correction**: "all rights reserved", not a free licence), Sonatina, BBC Sound
  Effects, Hydrogen kits marked "do not redistribute" or with no licence, and drum-machine
  sample sets from archive.org.

## 1. Legal frame for the owner

### 1.1 Does the GPL matter for samples?

Not much. Cantus's application is GPL-3.0-or-later. Its core crates are MIT OR Apache-2.0.
Samples are data the program loads; they are not part of the program. The GNU GPL FAQ
covers this in two entries. "Mere aggregation" (`#MereAggregation`) says separate works
shipped side by side on one medium do not have to share a licence. "Is there some way
that I can GPL the output people get from use of my program?" (`#GPLOutput`) says the
GPL does not cover what users make with the program. So a sample library keeps its own
licence inside a GPL installer, and the GPL does not reach users' songs.

- gnu.org timed out from this network while this section was written. The FAQ was
  later read through the Internet Archive copy for instruments.md §4.1, which quotes
  `#MereAggregation` and the plug-in entries verbatim. Re-read the live page before
  an ADR cites it.
- The GPL itself does not stop us shipping CC-BY or CC-BY-SA data. The questions that
  matter are each licence's own duties and our own policy, ADR 0001.
- The usual open-source practice (Debian, Fedora) applies: data must be redistributable
  and modifiable, and must not carry a non-commercial clause. This is why NC and
  "no redistribution" licences are out.

### 1.2 CC0 1.0 and public domain

CC0 1.0 waives copyright and related rights everywhere the law allows. Where a waiver is
not possible, it falls back to a permissive licence. It also covers database and
neighbouring rights, which matters for sound recordings.

- No attribution is needed. We should still log the source in our manifest (§9), as a
  matter of provenance, not of law.
- **CC0 does not prove provenance.** Anyone can put "CC0" on a file they ripped. The
  waiver only helps if the person giving it owned the rights. So vet the author, not
  only the label.
- **"Public domain" without CC0** (SM Drums, Splendid Grand) is weaker. Some countries
  have no legal way to dedicate a work to the public domain, and "PD" claims for samples
  are often careless. Treat these as "unclear unless the author confirms CC0 in writing".

### 1.3 CC-BY (3.0 and 4.0): the attribution duty

CC-BY lets anyone copy, modify and redistribute, including commercially, if they credit
the author. Both versions are compatible with shipping inside a GPL app (§1.1).

**How a DAW meets the duty:**

1. Ship `CREDITS` / `LICENSES-SAMPLES.txt` in the installer, with one entry per library:
   title, author, licence name and URL, source URL, and "modified: …" if we changed it.
2. Add an in-app credits view (Help → About → Sample credits) that lists the same data.
   For CC-BY 4.0, §3(a)(2) accepts attribution "in any reasonable manner based on the
   medium". A credits screen plus a file is the normal way.
3. Keep a licence file next to the samples on disk: `LICENSE.txt` in the library folder.

**What 4.0 changes from 3.0:**

- 4.0 explicitly lets you give attribution by linking to a page that holds it.
- 4.0 is international. 3.0 Unported is close, but has ported national variants.
- 4.0 gives a 30-day window to fix an attribution mistake before the licence ends
  (§6(b)). 3.0 ends at once.
- 4.0 licenses sui generis database rights. 3.0 is vaguer about them.

**Modified samples.** Trimming, normalising, fades, loop points, resampling and
re-encoding to FLAC are all allowed under BY.

- **CC-BY 4.0 §3(a)(1)(B):** "indicate if You modified the Licensed Material and retain
  an indication of any previous modifications". Text verified on the legal code page,
  2026-10-05.
- **CC-BY 3.0** asks for credit in adaptations, such as "French translation of X". For
  a mere format change it is unclear whether that counts as an adaptation.
- **Safe practice for both:** always mark changes, e.g. "Trimmed, normalised and
  re-encoded to FLAC by the Cantus project".

**Users' songs.**

- Under plain CC-BY, a user who publishes music made with CC-BY samples is in principle
  distributing an adaptation, and so has an attribution duty. This bothers people. Most
  DAW users will not credit "Salamander Grand Piano" in a pop single.
- That burden falls on Cantus users, not on the Cantus project. It is the main practical
  reason ADR 0001 prefers CC0.
- DrumGizmo kits add a note on their wiki pages. CrocellKit, DRSKit and Aasimonster say
  that "any context in which other credits (e.g. to performers) are presented must
  include an attribution". MuldjordKit (since January 2021) asks for "Drum samples
  provided by DrumGizmo.org" in album credits "where available". (Verified 2026-10-05.)

### 1.4 CC-BY-SA (share-alike)

Share-alike applies to **adaptations of the samples**. If we edit a BY-SA sample, our
edited version must also be BY-SA. Shipping the files unchanged, or trimmed and marked
as BY-SA, is fine.

- **Does it reach user songs?** CC's own position is that using a sample in a new song
  can make the song an adaptation of the sample. A song that contains a BY-SA sample may
  therefore have to be released under BY-SA.
  - This is the "infection" risk musicians worry about. Many libraries (jRhodes3d, for
    example) add a separate statement that freely allows music made with them, because
    BY-SA alone does not say so.
  - **Flag:** never bundle BY-SA samples as a default without the owner's ruling. If they
    are offered at all, use an opt-in download that shows the licence on screen.
  - Not legal advice. The CC FAQ treats this as case-by-case, and the exact passage was
    not re-read this session (**not verified**).
- **BY-SA libraries found:** Salamander Drumkit (CC-BY-SA-3.0 per sfzinstruments;
  **correction** of any belief that it is CC-BY), Sam's Sonor (BY-SA-4.0), LivingRoom
  Upright Micro (BY-SA-4.0), BJDNielPercussions (BY-SA-4.0), and Hydrogen's AVLDrumkits
  (BY-SA).

### 1.5 CC-BY-NC and Sampling Plus: exclude

- **NC (non-commercial)** clashes with the freedom to sell that the GPL and the Open
  Source Definition protect. It also traps users who sell their music. Exclude NC
  material: jRhodes3c (BY-NC-SA), jRhodes3d samples (BY-NC), and Freesound's BY-NC
  sounds.
- **Sampling Plus 1.0** was retired by Creative Commons in 2011. Freesound tells users to
  treat legacy Sampling+ sounds as BY-NC. Exclude Sonatina Symphonic Orchestra (Sampling
  Plus, with unclear sources) and any Freesound Sampling+ files.

### 1.6 "Royalty-free" licences that forbid redistribution: do not bundle

These let you use the sounds *inside your music*, but not pass the files on. Bundling
them in an installer, or serving them as an in-app download, is exactly what they forbid.

| Vendor | Clause (verified 2026-10-05 unless noted) |
|---|---|
| 99Sounds | "You do not have the right to sell or redistribute (even for free) the audio files … on their own". Also bans use in noise or sleep apps. |
| Splice | §3.1.1.3: you may not "sublicense the Sounds in isolation as sound effects, loops, or as source material for any other form of sample", nor use them "in a manner competitive" to Splice. |
| Cymatics | Store terms: no copying or reselling without written permission. Its free packs carry their own end-user licences, which this session did not read (**not verified**, but treat as no). |
| Sonniss GDC bundle | FAQ: "Not as standalone files or in sound effect libraries … The restriction only applies to raw redistribution." |
| Philharmonia | "must not be sold or made available 'as is' (i.e. as samples or as a sampler instrument)". **Correction:** this is not a free licence. |
| Looperman, BPB packs, LANDR, Black Octopus, Echo Sound Works, Goldbaby, Reverb Machine | Not verified this session (pages blocked or not reached). All are usually "use in music, no redistribution". Assume **no** until a written CC0 grant exists. |
| SampleSwap | Licence page not found (the FAQ URL returns 404). The site mixes per-track licences. **Not verified**: do not bundle. |

## 2. Drums (first priority)

Legend for "Bundle?":

- **yes**: CC0 or equivalent; can ship.
- **attr**: yes with attribution (CC-BY); needs the owner's ruling.
- **SA**: share-alike; needs the owner's ruling.
- **no**: do not ship.
- **unclear**: the licence or its source cannot be confirmed.

Sizes are download or extracted sizes as published. GitHub repo sizes are marked
"repo"; they are FLAC and may include git history.

### 2.1 Acoustic kits and one-shots

| Source | URL | Licence (verified 2026-10-05) | Bundle? | Size | Quality | Formats | Provenance risk |
|---|---|---|---|---|---|---|---|
| Karoryfer Frankensnare v2.1 | https://shop.karoryfer.com/pages/free-frankensnare | CC0-1.0 (repo LICENSE; the shop page says all free libraries are CC0) | **yes** | ~330 MB repo; >3500 samples | Excellent. About 12 snares from 10" to 20", brushes and sticks, rimshots, "sympathy snares", **several kinds of clap**, guitar chiks, tambourine | SFZ + FLAC/WAV | Low (their own recordings, named instruments) |
| Karoryfer Gogodze Phu Vol II | https://shop.karoryfer.com/pages/free-gogodze-phu-vol-ii | CC0-1.0 | **yes** | ~135 MB repo, >400 MB extracted; 1701 samples | Very good, lo-fi/vintage character. Kick; snare (centre, edge, sidestick); 3 deadened toms; hi-hat with 5 openness steps plus foot. 7 mics, two premixes | SFZ + WAV | Low |
| Karoryfer Gogodze Phu Vol I | https://shop.karoryfer.com/pages/free-gogodze-phu-vol-i | CC0-1.0 | **yes** | 90 MB; 544 WAV | Good. 5 Ghanaian bobobo drums + cajon | SFZ + WAV | Low |
| Karoryfer Big Rusty Drums | https://shop.karoryfer.com/pages/free-big-rusty-drums | CC0-1.0 | **yes** | 2.3 GB; >4400 samples | Very detailed 1980s Polish kit. Sticks, brushes, mallets, stirs, a cymbal stack | SFZ + WAV | Low |
| Karoryfer Swirly Drums | https://shop.karoryfer.com/pages/free-swirly-drums | CC0-1.0 | **yes** | 1.6 GB | Brushes only. Unusual percussion (brushed cowbell, darbouka, djembe, bongos, cajon as kick) | SFZ + WAV | Low |
| Karoryfer Unruly Drums | https://shop.karoryfer.com/pages/free-unruly-drums | CC0-1.0 | **yes** | 2 GB; >5600 samples | Every drum is a snare; hi-hat with 6 openness steps; quirky | SFZ + WAV | Low |
| Karoryfer The Hat with the Phat | https://shop.karoryfer.com/pages/free-the-hat-with-the-phat | **Conflict:** the repo says CC0-1.0, the shop page says "GPL 3" | unclear (ask Karoryfer) | >600 MB | A giant hi-hat made from two rides. 6 openness steps, 4–5 velocities, 4 round robins | SFZ + WAV | Low, but the licence texts disagree |
| Virtuosity Drums (Versilian x Karoryfer) | https://versilian-studios.com/virtuosity-drums/ | CC0-1.0 (sfzinstruments list) | **yes** | 1.1 GB | Deeply sampled jazz kit; won the KVR Developer Challenge 2021 soundware category | SFZ + WAV | Low |
| VCSL percussion folders | https://github.com/sgossner/VCSL | CC0-1.0 (repo LICENSE + README) | **yes** | Whole repo ~4 GB; each instrument ~20–75 MB | Clean mid-close stereo, 44.1/48 kHz, 16/24-bit. Percussion has more velocities and round robins than the melodic instruments. Includes **claps, hi-hat, snares (Modern 1–3, rope tension, legacy), toms, concert bass drum, bongos, conga, darbuka, frame drum, cajon, cowbells, claves, woodblock, shakers, tambourines, agogo, cabasa, guiro, triangles, vibraslap, gongs, cymbals** | WAV + SFZ (generator script included, CC0) | Low (Versilian's own recordings) |
| DrumGizmo DRSKit 2.1 | https://drumgizmo.org/wiki/doku.php?id=kits:drskit | CC-BY-4.0 + extra credit request | **attr** | ~750 MB (SFZ port 754 MB) | Very good rock/jazz kit, **13 mics**, many velocities | DrumGizmo XML + WAV; SFZ port | Low |
| DrumGizmo MuldjordKit 3 | https://drumgizmo.org/wiki/doku.php?id=kits:muldjordkit | CC-BY-4.0 + "Drum samples provided by DrumGizmo.org" | **attr** | ~350 MB (SFZ port 347 MB) | Tama Superstar, **16 channels** | DG XML + WAV; SFZ port | Low |
| DrumGizmo CrocellKit 1.1 | https://drumgizmo.org/wiki/doku.php?id=kits:crocellkit | CC-BY-4.0 + credit request | **attr** | 5.5 GB | Studio-sampled metal kit, **15 channels**; a stereo downmix by H. Järvinen exists | DG XML + WAV | Low |
| DrumGizmo The Aasimonster 2.1 | https://drumgizmo.org/wiki/doku.php?id=kits:the_aasimonster | CC-BY-4.0 + credit request | **attr, with caution** | 2.3 GB | 14 mics; a known double-hit timing issue between the ambience and kick channels | DG XML + WAV | **Medium:** the kick channel includes the output of an **Alesis DM5 module ("Speed Metal" sample)**, which is commercial ROM content |
| Naked Drums (Wilkinson Audio) | https://github.com/sfzinstruments/WilkinsonAudio.NakedDrums | CC-BY-4.0 (repo) | **attr** | 1.3 GB | Excellent. 10 round robins, up to 5 velocities, mic layers (direct, OH, room, far, M/S, wide OH), 44.1 kHz | SFZ + FLAC | Low |
| Salamander Drumkit | https://archive.org/details/SalamanderDrumkit | CC-BY-SA-3.0 per the sfzinstruments listing. The archive.org page loaded but its licence field was not parsed: **partly verified** | **SA** | ~370 MB | Decent, older; several velocities | SFZ + WAV; Hydrogen | Low |
| Sam's Sonor | https://sfzinstruments.github.io/drums/sams_sonor/ | CC-BY-SA-4.0 | **SA** | not stated | Sonor Force 3001 kit | SFZ + WAV | Low; the original site is gone (archive.org link only) |
| SM Drums v1.2 | https://smmdrums.wordpress.com/ | "Public Domain" per sfzinstruments; **the SM Drums site itself states no licence** | **unclear** | 2.2 GB+ (cymbals 1.8 GB + multi-mic 1.4 GB) | Deeply sampled multi-mic collector kit | WAV, SFZ, Kontakt, Reaper | Low for the recording itself; **licence unclear**; hosted on Google Drive |
| Dim Cabasa | https://sfzinstruments.github.io/percussion/dim_cabasa/ | CC-BY-4.0 | attr | 12 MB | One cabasa | SFZ | Low |
| The Clap, Breezy Day (Malaclypse the Younger) | https://sfzinstruments.github.io/percussion/ | MIT | yes (keep the notice) | small | Hand claps imitating a kit; a wind chime | SFZ | Low; not inspected |
| BJDNielPercussions | https://sfzinstruments.github.io/percussion/ | CC-BY-SA-4.0 | SA | 4 MB | Novelty percussion | SFZ | Low |
| Freesound CC0 drum one-shots | https://freesound.org (licence filter = CC0) | per file; a CC0 subset exists | **yes, per file** | any | Ranges from studio-grade to phone recordings | WAV/FLAC/AIFF/MP3 | **Medium–high:** uploaders sometimes post machine rips or excerpts of commercial packs as "CC0". Vet each file. |

### 2.2 Electronic and drum-machine one-shots

This is the weak spot. What was checked:

| Source | URL | Licence | Bundle? | Notes / provenance |
|---|---|---|---|---|
| Hydrogen TR808EmulationKit (ships with Hydrogen) | https://github.com/hydrogen-music/hydrogen/tree/main/data/drumkits | drumkit.xml says "GPL" (ArtemioLabs) | unclear | Called an *emulation*, but the samples carry "GPL" with no stated source for the sounds. Ask the author, or skip. |
| Hydrogen GMRockKit | same | "GPL" (Glen MacArthur / Sebastian Moors) | unclear | GPL is an odd licence for audio (the WAV is its own "source"). Usable only if the owner accepts GPL data in a GPL app. |
| Hydrogen online kits (feed) | http://hydrogen-music.org/feeds/drumkit_list.php | **mixed**, per kit | per kit | See §2.3. Many legacy "(sf)" kits have **no licence** and carry the names of commercial machines (Boss DR-110, TR808909, TD-7, Classic 626). **Do not bundle.** |
| Freesound CC0 machine hits | https://freesound.org | CC0 per file | per file | Many "808" uploads are rips of sample CDs. Accept only hits the uploader recorded from hardware they own (and says so), or synthesized hits. |
| archive.org drum-machine archives | https://archive.org | none or mixed | **no** | Sampled from commercial machines and sample CDs; the uploader is rarely the rights holder. |
| Nicolas Rosenthal, Reverb Machine, Goldbaby, Legowelt, Free Wave Samples, Black Octopus free, Echo Sound Works | various | **not verified** this session | **no, until verified** | The usual terms are "free for music, do not redistribute". Legowelt and Goldbaby sample real hardware. They may license recordings of their own machines, but their free-pack terms usually forbid redistribution. |
| Wave Alchemy MFB Tanzbar (free), BPB Game Boy kit | https://sfzinstruments.github.io/drum_machines/ | no licence shown | **no** | Royalty-free promotional packs. |

The *sound* of an 808 is not protected; a recording of a specific unit, or its ROM
samples, is. So a clean CC0 electronic set has to be **synthesized or recorded by us**
(§7).

### 2.3 Hydrogen drumkits: licence by kit (from the official feed)

| Licence in the feed | Kits |
|---|---|
| CC0 | Audiophob (its info says "sounds from freesound.org", so it inherits Freesound's per-file risk) |
| CC-BY 3.0 | belofilms AC-Guitar-Strums; rumpf_kit_z01_gm |
| CC-BY ("CC-Attribution") | DeathMetal (sf) |
| CC-BY, named after famous drummers | Dave Grohl, Ian Paice, John Bonham, Travis Barker, Tre Cool. **High provenance risk** (they may be sampled from records). Do not bundle. |
| CC-BY-SA | BJA_Pacific (3.0 US); circAfrique v4 (3.0); Denon CRB-90 (a machine name: risk); the AVLDrumkits family (BlackPearl-2023, BlondeBop, BlondeBop-HotRod, BuskmansHoliday, RedZeppelin-2023) |
| GPL | BeatBuddy Kit (a commercial product name: risk); Forzee Stereo (GPL-2); Gimme A Hand 1.0; The Black Pearl 1.0 |
| WTFPL | SF3007-2011-Set-03 |
| "Do not redistribute" | GSCW Kit 1 and 2; the Roland_MC-307 kits CR78, TR-606, TR-808, TR-909 and Techno1 (**ripped Roland ROM**) |
| "All Rights Reserved" | JazzFunkKit (Orange Tree Samples) |
| none or vague ("Free!!!", "creative commons") | about 20 legacy "(sf)" kits; K-27 Trash Kit; Lightning1024; Drumkit excepcional |

The AVLDrumkits are the best-recorded Hydrogen kits. They are BY-SA, so they need the
owner's ruling (§1.4). The feed was read on 2026-10-05; the AVL kits are presumably
Glen MacArthur's AV Linux kits (**not verified**).

## 3. Loops

No verified source of good, bundle-safe CC0 loops was found. The loop sites people know
are all of the "use in music, do not redistribute" kind.

| Source | URL | Licence | Bundle? | Notes |
|---|---|---|---|---|
| Freesound loops, CC0 filter | https://freesound.org/search/?f=license:"Creative Commons 0" | CC0 per file | yes, per file | Tempo and key tags are unreliable. Many "loops" are excerpts of commercial packs or songs: vet each one by ear and check the uploader's history. Freesound's licence set is CC0, CC-BY and CC-BY-NC; legacy Sampling+ counts as NC (confirmed via search results; freesound.org itself could not be fetched here). |
| Looperman | https://www.looperman.com | Free for use in music; redistributing the loops is not allowed (from general knowledge; the terms page loaded but the clause was not found: **not verified**) | **no** | User uploads; provenance is often poor. |
| SampleSwap | https://sampleswap.org | per item; licence page 404 | **no** | Some content is fine, but there is no single clear grant. |
| ccMixter | https://ccmixter.org | mostly CC-BY / BY-NC (**not verified**) | not by default | Song stems and a cappellas, not drum loops. |
| Splice / Cymatics / LANDR / BPB free loop packs | see §1.6 | no redistribution | **no** | |

**Recommendation.** Make Cantus's loops ourselves: patterns in Cantus built from the CC0
one-shots and Pulsus, rendered to audio and released as CC0 together with their
`.cantus` project files. The demo songs from commit 7305d6e show this workflow already
exists. Loops made this way inherit the CC0 status of their parts, and they double as
tutorial material.

## 4. Instruments (multisampled)

| Source | URL | Licence | Bundle? | Size | Quality | Formats | Provenance risk |
|---|---|---|---|---|---|---|---|
| VSCO 2 CE | https://github.com/sgossner/VSCO-2-CE | CC0-1.0 (repo, verified) | **yes** | ~2.3 GB repo (prior note: ~3 GB) | Good chamber orchestra (strings, winds, brass, percussion, keys); few velocities and round robins | SFZ + WAV | Low |
| VCSL (melodic parts) | https://github.com/sgossner/VCSL | CC0-1.0 (verified) | **yes** | ~4 GB repo (updated Jan 2026) | Consistent, mostly whole-tone sampled, 2–3 velocities, 1 round robin, 20–75 MB per instrument. Also includes a **TX81Z** folder (recordings of a Yamaha FM synth) | WAV + SFZ | Low. The TX81Z recordings may contain factory presets; recordings of a hardware synth are usually fine, but flag them |
| VCSL Keys | https://versilian-studios.com/vcsl-keys/ | CC0-1.0 (sfzinstruments list) | **yes** | 680 MB | Pianos and harpsichords from VCSL | SFZ | Low |
| Osiris Piano v0.925 (Karoryfer x Versilian) | https://shop.karoryfer.com/pages/free-osiris-piano | CC0 (page and repo, verified) | **yes** | 460 MB FLAC; >1200 samples | Quiet, low-noise piano, several mic positions | SFZ + FLAC (needs sforzando-level opcodes) | Low |
| Scarypiano | https://shop.karoryfer.com/pages/free-scarypiano | CC0-1.0 | yes | not stated | Processed piano for effects, not a main piano | SFZ | Low |
| Karoryfer basses, guitars, winds (Meatbass, Growlybass, Pastabass, Sneakybass, Swagbass, Fashionbass, Big Little Bass, Black and Blue Basses, Black and Green Guitars, Emilyguitar, Shinyguitar, Bear Sax, Weresax, War Tuba, Squidpipes, Cowsynth, Horse Pulse, Bigcat Cello, String Cyborgs, Caveman Cosmonaut…) | https://shop.karoryfer.com/pages/free-samples | "All our free sample libraries are under a Creative Commons Zero license", except Marie Ork | **yes, per library** | 13 MB (Cowsynth) to GB scale | Generally high, characterful | SFZ + WAV/FLAC | Low. Check each repo's LICENSE: older downloads may carry older licences, and The Hat with the Phat's pages disagree |
| Salamander Grand Piano v3 | https://archive.org/details/SalamanderGrandPianoV3 | CC-BY-3.0 (LICENSE in sfzinstruments repo, verified) | **attr** | 1.18 GB WAV 48k/24; 394 MB WAV 44.1k/16; ~730 MB FLAC repo | Yamaha C5, 2× AKG C414 AB, **16 velocity layers**, sampled in minor thirds, hammer and resonance releases | SFZ, WAV/FLAC | Low |
| Accurate-Salamander v6 | https://www.ir.isas.jaxa.jp/~cyamauch/AccurateSalamander/ | CC-BY (derived from Salamander) | attr | 1.6 GB | Retuned, with more consistent dynamics | SFZ | Low |
| Headroom Piano | https://sfzinstruments.github.io/pianos/headroom_piano/ | CC-BY-4.0 (listing) | attr | 156 MB | Yamaha C3 | SFZ | Low |
| Greg Sullivan E-Pianos | https://sfzinstruments.github.io/pianos/greg_sullivan_e-pianos/ | CC-BY-3.0 (listing) | attr | 21.5 MB | 3 small electric pianos | SFZ | Low |
| Splendid Grand Piano | https://github.com/sfzinstruments/SplendidGrandPiano | README: "released as public domain in early 2000 by Akai company"; repo has no licence file | **unclear** | 77 MB | Steinway, 4 velocities | SFZ + FLAC | **Medium:** no primary source for Akai's dedication was found |
| Maestro Concert Grand | https://github.com/sfzinstruments/MatsHelgesson.MaestroConcertGrandPiano | **"All rights reserved … You may not modify and spread this soundfont without the author's written permission"** (mcgreadme.txt, verified) | **no** (**correction**) | 268 MB | Yamaha CF-3, 5 velocities, X/Y KM84 | SFZ + FLAC | The sfzinstruments port has the author's permission by e-mail; that permission does not extend to us |
| Piano in 162 (Ivy Audio) | http://ivyaudio.com/Piano-in-162 | Ivy Audio's own licence (the listing shows none; **not verified**) | **no, until verified** | 4.9 GB | Steinway B, 5 dynamics, 2 round robins, pedal up/down, 2 mic positions | SFZ / Kontakt | Low; licence unknown |
| LivingRoom Upright Micro | https://sfzinstruments.github.io/pianos/ | CC-BY-SA-4.0 | SA | 107 MB | Character upright | SFZ | Low |
| jRhodes3c / jRhodes3d | https://sfzinstruments.github.io/pianos/ | BY-NC-SA-4.0 / samples BY-NC-4.0 | **no** (NC) | 18 / 92 MB | Rhodes Mark I | SFZ | |
| Estate Grand LE, Church Steinway (Pianobook), TKDrums "Basic" | https://sfzinstruments.github.io/pianos/ | vendor freeware licences, not shown (**not verified**) | **no** | | | | |
| Upright Piano KW | https://freepats.zenvoid.org | **not verified** this session (believed CC0 via FreePats) | check | | | SFZ/SF2 | |
| University of Iowa MIS | https://theremin.music.uiowa.edu/MIS.html | No licence. The page says the files "may be downloaded and used for any projects, without restrictions" (verified) | **unclear**: ask Prof. Fritts for a written CC0 or equivalent statement | GB scale | Anechoic, single notes; pre-2011: 16/44.1 mono KM84, pp/mf/ff; since 2011: 24/96 stereo Decca-tree | AIFF/WAV | Low; this is a licence-form problem, not a provenance one. **Correction:** the MIS is generous but not formally CC0. |
| Philharmonia samples | https://philharmonia.co.uk/resources/sound-samples/ | Use in music allowed; "must not be sold or made available 'as is' (i.e. as samples or as a sampler instrument)" | **no** (**correction**) | | Short MP3 notes | MP3 | |
| Sonatina Symphonic Orchestra | (various mirrors) | Sampling Plus 1.0; sources unclear | **no** | ~500 MB | | SFZ | High |
| GeneralUser GS | https://schristiancollins.com/generaluser.php | Custom licence (page needs JavaScript: **not verified this session**) | no / unclear | ~30 MB | Good GM set | SF2 | The author has said some samples have uncertain origins (prior note) |
| FluidR3_GM / MuseScore_General | https://github.com/musescore/MuseScore/tree/master/share/sound | Believed MIT (**not verified this session**). The MuseScore repo states its placeholder is "an exact copy of FluidR3Mono_GM.sf3" | attr-like (MIT notice) | ~140 MB SF2 / ~40 MB SF3 | Decent GM set | SF2/SF3 | **Medium:** FluidR3 was assembled from many older sources |
| MTG Solo Sax | (prior note) | CC-BY-4.0 per the September note (**not re-verified**) | attr | | | | |
| Musical Artifacts | https://musical-artifacts.com | Per-item licences (the site sat behind a JavaScript challenge: **not verified**) | per item | | Directory of SF2/SFZ files | | Mixed |
| FreePats | https://freepats.zenvoid.org | Per bank; many GPL or CC0 (**not verified**) | per bank | | | SF2/SFZ | Mixed |

## 5. FX and foley

| Source | URL | Licence | Bundle? | Notes |
|---|---|---|---|---|
| Freesound CC0 | https://freesound.org | CC0 per file | yes, per file | Best general FX pool; vet each file. |
| Kenney audio packs | https://kenney.nl/assets/category:Audio | Believed CC0 (**not verified**: the site is blocked by the corporate proxy as a "games" category) | yes if confirmed | UI clicks and game SFX. Useful for Cantus's own UI sounds, less so for music. |
| OpenGameArt | https://opengameart.org | Per asset: CC0, CC-BY, BY-SA, GPL, OGA-BY (FAQ read) | per asset, CC0 only | Use the CC0 filter; many items are re-uploads, so check the "original source" field. |
| Sonniss GDC bundle | https://sonniss.com/gameaudiogdc | Royalty-free, "not as standalone files or in sound effect libraries" (verified) | **no** | |
| BBC Sound Effects (RemArc) | https://sound-effects.bbcrewind.co.uk/licensing | Personal/educational/research use; commercial use needs a paid licence (page needs JavaScript: **not verified this session**) | **no** | |
| VCSL "Other" folders | https://github.com/sgossner/VCSL | CC0 | yes | Ratchet, slapstick, anvil, brake drum, flexatone, wind-machine-like friction idiophones. Musical FX. |

## 6. What a core kit could be (CC0 only, about 60–120 MB)

The goal is an FL-style "Packs" folder that works offline from first launch, made only of
CC0 material and Cantus-original renders. Each item becomes a single premixed stereo
one-shot, trimmed, with a short fade and peaks normalised to about −1 dBFS, stored as
48 kHz/24-bit FLAC. Multi-mic sources are mixed down once by us; that mixdown is a new
CC0 work (we should waive our rights in it too).

| Category | Count | Source (all CC0) | Notes |
|---|---|---|---|
| Kicks (acoustic) | 6–8 | Gogodze Phu II kick (2–3 velocities × "47" and "13" mixes); Big Rusty kick; Swirly "cajon kick"; VCSL BassDrum1 (concert boom) | Covers vintage, rock and cinematic. |
| Kicks (electronic) | 12–20 | **Pulsus renders** (808-style long and short, 909-style, sub-boom, clicky) | Synthesized by Cantus: no third-party provenance at all. |
| Snares | 10–14 | Frankensnare (14" maple, 14x8 aluminium rimshot, 10" ash, 20" poplar, sympathy snare); VCSL Snare Modern 1–3; Gogodze II snare and sidestick | Pick 1–2 velocities per drum. |
| Snares (electronic) | 6–10 | Pulsus renders | |
| Claps | 6–8 | Frankensnare claps; VCSL Claps; Pulsus 808/909-style claps | |
| Hi-hats | 10–14 | Gogodze II hi-hat (closed, 2–3 open steps, foot); VCSL Hi-HatCymbal; Pulsus closed/open metallic hats | Keep closed/open pairs so they can share a choke group. |
| Toms | 6 | Gogodze II deadened toms; VCSL Tom1/Tom2 | |
| Cymbals | 4–6 | VCSL SuspendedCymbal1/2; Big Rusty crash/ride (trim long tails to ~4 s) | Cymbals dominate the size budget. |
| Percussion | 20–30 | VCSL conga, bongos, darbuka, frame drum, cowbells, claves, woodblock, shakers (S/L), tambourines, agogo, cabasa, guiro, triangle, vibraslap; Gogodze I cajon and bobobo | |
| Kits | 3–4 SFZ-like mappings | "Acoustic (Gogodze)", "Brush (Swirly subset)", "Electronic (Pulsus renders)", "Percussion" | Map to General MIDI note numbers for MIDI import compatibility. |

**Size estimate.** About 120 one-shots × an average of 1.5 s stereo at 48 kHz/24-bit is
roughly 50 MB as WAV. Add about 10 cymbals at 4 s (~12 MB). FLAC brings the total to
about 35–45 MB. Adding a lean melodic set leaves room inside 150 MB: Osiris Piano
reduced to one mic and fewer velocities (~100 MB), or VCSL instruments at 20–75 MB each.

**Larger optional downloads** (in-app "Get more sounds", CC0): the full Big Rusty,
Unruly, Swirly, Virtuosity Drums, VCSL, VSCO 2 CE and Osiris Piano. The CC-BY items
(Salamander Grand, Naked Drums, the DrumGizmo kits) stay behind the owner's ruling. If
approved, offer them only as opt-in downloads that show the licence before download, and
list them in the credits view.

## 7. Cantus-original one-shots

**Pulsus renders (recommended, nearly free).** Cantus's Pulsus already synthesizes drums.

- Write a deterministic offline render job (an `xtask`) over a preset list. It produces
  the electronic half of the kit, bit-reproducible from presets checked into the repo.
- Release the renders under CC0 with the presets as their "source". Provenance is
  perfect: the output can be rebuilt from code in our repo.
- Effort: about 2–4 engineering days for the render tool, plus 1–2 days of sound-design
  passes on presets.

**Recording original acoustic one-shots.** Options, with rough costs. All figures are
**not verified**: the web-search budget ran out, so they are general market knowledge,
not quotes.

| Option | Rough cost | Notes |
|---|---|---|
| Recording in-house with one decent mic and room | gear on hand: 0; a few days of time | Fine for percussion, claps, foley, shakers. A drum kit needs a good room. |
| Hiring a drummer, kit and small studio for one day | roughly USD 300–1,500 per day | Enough for one well-sampled kit (5 velocities × 3 round robins of the core pieces). |
| Commissioning a freelance sound designer for about 150 one-shots, with a full CC0 assignment in the contract | roughly USD 1,000–5,000 | Contract must say "work made for hire / assignment + CC0 release" and include the designer's warranty that no third-party samples were used. |
| Asking Karoryfer or Versilian, who already publish CC0 and run commissions | quote needed | Strong fit: an established CC0 track record and clean provenance. |

## 8. Process: vetting and logging each bundled file

1. **Provenance manifest** (`assets/samples/MANIFEST.toml`, or JSON, one record per file
   or per library). Fields:
   - `path`, `sha256`, `source_url`, `source_version` or `commit`;
   - `licence` (SPDX id, e.g. `CC0-1.0`), `licence_url`;
   - `author`, `date_retrieved`;
   - `modifications` (e.g. "trimmed, normalised −1 dBFS, FLAC 48k/24");
   - `reviewer`, `notes`.
2. **Licence evidence.**
   - Save a copy (text or PDF) of the licence page or LICENSE file at retrieval time
     under `assets/samples/evidence/`. Pages change; the Hat-with-the-Phat conflict is
     an example.
   - Prefer the licence inside the downloaded archive over the web page. When the two
     disagree, ask the author and log the reply.
3. **Rejection rules.** Reject when:
   - the licence is missing or vague ("free", "royalty-free", "creative commons" with no
     version);
   - it is NC, ND, Sampling+, or forbids redistribution;
   - the author cannot be identified;
   - the file is named after a commercial machine or library and the uploader does not
     state they recorded their own unit;
   - the "PD" claim has no primary source.
4. **Anti-rip checks.**
   - Search the file's name and pack name on the web.
   - Compare audio fingerprints against known commercial sets where possible (e.g.
     Chromaprint/AcoustID; this catches only identical files).
   - Be wary of 808/909/LinnDrum/DMX/SP-1200 names, "sample CD" names, and uploaders who
     post hundreds of "CC0" packs in a day.
   - Prefer libraries with photos or videos of the recording session and named
     instruments, as Karoryfer, Versilian and DrumGizmo provide.
5. **CI check.**
   - A test that every file under `assets/samples/` appears in the manifest with a
     matching checksum and an allowed SPDX id (`CC0-1.0`, plus whatever the owner
     approves).
   - The credits view and `CREDITS-SAMPLES.txt` are generated from the manifest, so
     attribution cannot drift.
6. **ADR.** One ADR per accepted licence class: e.g. "CC-BY-4.0 allowed for opt-in
   downloads". This fits the house habit of one ADR per decision.

## 9. What could not be verified (2026-10-05)

- **GNU GPL FAQ wording**: gnu.org timed out; `#MereAggregation` was confirmed from the
  archive.org copy (instruments.md §4.1); `#GPLOutput` is paraphrased.
- **Freesound's own pages and API** (not fetchable from this network). Licence set
  confirmed only via search results. API details are not verified: APIv2 needs a token;
  original-file download needs OAuth2.
- Kenney (blocked by policy), BBC RemArc terms, GeneralUser GS, Musical Artifacts,
  Bedroom Producers Blog (all behind JavaScript or bot challenges).
- Looperman's redistribution clause; SampleSwap's licence page (404).
- Cymatics free-pack EULA; LANDR, Black Octopus, Echo Sound Works, Goldbaby, Reverb
  Machine, Legowelt, Free Wave Samples, Nicolas Rosenthal packs.
- Upright Piano KW, FreePats, Piano in 162 licence, FluidR3_GM and MuseScore_General
  (believed MIT), MTG Solo Sax.
- Primary evidence for Akai's public-domain release of Splendid Grand.
- The exact current licence field on archive.org for the Salamander Drumkit. The
  sfzinstruments listing says CC-BY-SA-3.0.
- Hydrogen's main website (timed out). Kit licences were read from its drumkit feed and
  its GitHub repo instead.
- All cost figures in §7.

## 10. Recommendations, in priority order

1. **Adopt the CC0-only core kit (§6)** from Karoryfer (Frankensnare, Gogodze Phu I/II,
   Big Rusty/Swirly subsets) and VCSL percussion. Mix it down to ~40–60 MB of FLAC
   one-shots, with a manifest.
2. **Build the Pulsus render job (§7)** for all electronic drums (808/909-style kicks,
   snares, claps, hats) and release the renders CC0. Do not look for third-party 808
   samples.
3. **Set up the manifest, evidence folder and CI check (§8)** before the first sample
   lands.
4. **Offer large CC0 libraries as in-app downloads:** Virtuosity Drums, the full
   Karoryfer kits, VCSL, VSCO 2 CE, Osiris Piano. Osiris is the best CC0 piano candidate
   for a default piano.
5. **Owner ruling needed on CC-BY** (Salamander Grand v3, Naked Drums, DrumGizmo kits).
   If allowed, make them opt-in downloads only, shown with the licence, and listed in the
   credits view. Tell users about the downstream attribution expectation; for DrumGizmo,
   their extra album-credit request.
6. **Owner ruling on BY-SA and GPL-labelled audio** (AVLDrumkits, Salamander Drumkit,
   Hydrogen GPL kits). The default suggestion is to exclude them, because of the
   share-alike risk to users' songs.
7. **Write to the authors to clear up unclear cases:**
   - Karoryfer: the Hat with the Phat, CC0 or GPL-3?
   - Prof. Lawrence Fritts: an explicit CC0 statement for Iowa MIS.
   - SM Drums: a licence statement.
8. **Exclude for good:**
   - NC, Sampling+ and no-redistribution material (§1.5–1.6).
   - Philharmonia, Maestro Concert Grand, Sonatina, BBC, Sonniss.
   - Hydrogen kits that are unlicensed, "do not redistribute" or named after machines or
     drummers.
   - archive.org drum-machine dumps.
   - The Aasimonster kick channel (Alesis DM5 ROM).
9. **Loops:** produce our own from the core kit and Cantus patterns, CC0, with project
   files. Use Freesound CC0 loops only one by one, after vetting.

## Sources (all accessed 2026-10-05)

- SFZ Instruments lists: https://sfzinstruments.github.io/drums/,
  https://sfzinstruments.github.io/drum_machines/,
  https://sfzinstruments.github.io/percussion/, https://sfzinstruments.github.io/pianos/
- SFZ detail pages: https://sfzinstruments.github.io/pianos/salamander/,
  https://sfzinstruments.github.io/pianos/splendid_grand_piano/,
  https://sfzinstruments.github.io/pianos/maestro_concert_grand_piano/,
  https://sfzinstruments.github.io/pianos/in_162/,
  https://sfzinstruments.github.io/drums/sm_drums/,
  https://sfzinstruments.github.io/drums/salamander/,
  https://sfzinstruments.github.io/drums/naked_drums/
- GitHub repos (README/LICENSE/API licence field):
  https://github.com/sfzinstruments/SplendidGrandPiano,
  https://github.com/sfzinstruments/MatsHelgesson.MaestroConcertGrandPiano (incl.
  mcgreadme.txt), https://github.com/sfzinstruments/SalamanderGrandPiano,
  https://github.com/sfzinstruments/WilkinsonAudio.NakedDrums,
  https://github.com/sfzinstruments/karoryfer.frankensnare (and gogodze-phu-vol-i,
  gogodze-phu-vol-ii, big-rusty-drums, swirly-drums, unruly-drums,
  Karoryfer.TheHatWithThePhat, Osiris_Piano, Karoryfer.HorsePulse),
  https://github.com/sgossner/VCSL, https://github.com/sgossner/VSCO-2-CE,
  https://github.com/hydrogen-music/hydrogen (data/drumkits),
  https://github.com/musescore/MuseScore/tree/master/share/sound
- Karoryfer: https://shop.karoryfer.com/pages/free-samples and the pages
  free-frankensnare, free-gogodze-phu-vol-i, free-gogodze-phu-vol-ii,
  free-big-rusty-drums, free-swirly-drums, free-unruly-drums, free-the-hat-with-the-phat,
  free-horse-pulse, free-cowsynth, free-osiris-piano
- DrumGizmo: https://drumgizmo.org/wiki/doku.php?id=kits, …?id=kits:drskit,
  …?id=kits:muldjordkit, …?id=kits:crocellkit, …?id=kits:the_aasimonster
- Hydrogen drumkit feed: http://hydrogen-music.org/feeds/drumkit_list.php
- SM Drums: https://smmdrums.wordpress.com/, https://smmdrums.wordpress.com/about/
- University of Iowa MIS: https://theremin.music.uiowa.edu/MIS.html
- Philharmonia: https://philharmonia.co.uk/resources/sound-samples/
- 99Sounds: https://99sounds.org/license/
- Splice terms: https://splice.com/terms
- Cymatics terms: https://cymatics.fm/pages/terms-of-service
- Sonniss: https://sonniss.com/gameaudiogdc
- OpenGameArt FAQ: https://opengameart.org/content/faq
- Creative Commons: https://creativecommons.org/licenses/by/4.0/legalcode.en,
  https://creativecommons.org/publicdomain/zero/1.0/legalcode.en,
  https://creativecommons.org/faq/,
  https://creativecommons.org/2011/09/12/celebrating-freesound-2-0-retiring-sampling-licenses/
- Freesound (via search results only): https://en.wikipedia.org/wiki/Freesound
- Attempted but not readable from this network: https://www.gnu.org/licenses/gpl-faq.html,
  https://freesound.org/help/faq/, https://kenney.nl/assets/category:Audio,
  https://sound-effects.bbcrewind.co.uk/licensing,
  https://schristiancollins.com/generaluser.php, https://musical-artifacts.com/,
  https://bedroomproducersblog.com/free-samples/, https://sampleswap.org/faq.php,
  https://www.looperman.com/help/terms (loaded, clause not found), https://hydrogen-music.org/
