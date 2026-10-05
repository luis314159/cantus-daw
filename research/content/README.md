# Content research: rights-free samples, where to keep them, free instruments (October 2026)

The owner's ask (2026-10-05): *maybe free, rights-free instruments preloaded in the DAW
(which also confirms compatibility at once); and, as backlog research: where to get
rights-free WAV files (kicks, snares, kits, sounds for users) and where to keep them (not
in the repo).* Research only: nothing here is a decision. Round 20, branch `r20/research`.

## Resumen (español)

- **Muestras.** Hay fuentes CC0 de buena calidad para una batería básica: las librerías
  gratuitas de **Karoryfer** (Frankensnare, Gogodze Phu I/II, Big Rusty, Swirly, Unruly),
  la percusión de **VCSL** y **Virtuosity Drums**, todas CC0 y verificadas. **No existe**
  una fuente CC0 limpia de golpes tipo 808/909: casi todo son copias de las máquinas o
  licencias "royalty-free" que prohíben redistribuir. Propuesta: **renderizar Pulsus** a
  WAV y publicarlo como CC0. Un kit básico de ~120 sonidos cabe en **35–45 MB** (FLAC).
- **No incluir nunca:** 99Sounds, Splice, Cymatics, Sonniss, Philharmonia, Maestro
  Concert Grand, Sonatina, BBC, kits de Hydrogen sin licencia o "no redistribuir", y
  volcados de cajas de ritmos de archive.org.
- **Dónde guardarlas.** No usar Git LFS (cuesta ~$40–4 070 al mes según descargas).
  Fase 1: **GitHub Releases** en un repo aparte `cantus-content`, con un manifiesto
  firmado (minisign) y SHA-256 por archivo: **$0**. Fase 2, si crece el tráfico:
  **Cloudflare R2** como espejo (sin coste de salida, ~$0–5 al mes). Gestor de contenido
  dentro de la app: descarga bajo demanda, por paquete, reanudable, instalable desde zip
  sin conexión, sin cuenta y sin telemetría, compatible con proxies como Zscaler.
  Instalador ≤ 200 MB con un kit básico ≤ 120 MB.
- **Instrumentos gratuitos.** Lo que mejor llena los huecos (piano, orquesta, guitarra,
  batería acústica) son **librerías de muestras** que Exemplar ya carga (Osiris Piano,
  VSCO 2 CE, VCSL, Virtuosity Drums, guitarras de Karoryfer: todas CC0). Para probar la
  compatibilidad: incluir 2–3 sintes libres pequeños con CLAP (**OB-Xf, Dexed, Firefly
  Synth 2**) con su código fuente al lado; el resto (Surge XT, Odin 2…) en un catálogo
  que descarga desde el sitio original. El freeware (BBC SO Discover, Vital, Ample
  Guitar, LABS) **solo se enlaza**: ninguna licencia permite redistribuirlo.
- **Decisiones del dueño:** (1) conciliar ADR 0001 ("original o CC0") con idea.md §8
  ("permisiva/CC0"); (2) ¿admitir CC-BY (Salamander Grand) con atribución?; (3) excluir
  CC-BY-SA y NC; (4) incluir plug-ins GPL en el instalador o solo descargarlos; (5) una
  excepción GPL §7 para cargar plug-ins no libres; (6) marcas (Vital → Vitalium,
  "Oberheim", "TB-303") y el logo de VST.

## The reports

| Report | Scope | The headline |
|---|---|---|
| [samples.md](samples.md) | Rights-free drums (kicks, snares, hats, claps, percussion, kits), loops, multisampled instruments, FX; per source: licence, redistribution, size, quality, formats, provenance risk; a core-kit proposal; the vetting process | CC0 drums exist (Karoryfer, VCSL, Virtuosity); clean CC0 808/909 one-shots do not, so render our own from Pulsus; CC-BY sets are good but need a ruling |
| [storage.md](storage.md) | Git LFS, GitHub Releases, a content repo, object storage and CDNs with prices; free OSS hosting; how FL Studio, Ableton, Bitwig, Reaper, MuseScore and others ship content; size budgets; an in-app content manager; a recommended design with costs | GitHub Releases on `cantus-content` + a signed manifest now ($0); Cloudflare R2 as mirror when traffic grows; LFS is wrong for users |
| [instruments.md](instruments.md) | Open-source and freeware CLAP/VST3 instruments, licences, the GPL (aggregation vs combined work, source duty), trademarks, the VST logo, bundle vs catalogue, ranked shortlists, a compatibility test set | Sample libraries in Exemplar fill the gaps; bundle 2–3 small OSS CLAP synths to prove hosting; freeware is link-only |

About 1 300 lines, written by three research agents on 2026-10-05 and reviewed here.
Every fact has a dated source at the end of its file; what could not be checked is
marked **not verified** (the corporate proxy blocked some sites and timed out on
gnu.org). This round's review corrected and aligned the three files: where they
disagree on a sample library's licence, samples.md wins (it read each licence file).

**Corrections to the September research** (`research/instruments/sampler.md` §9.6):

- **Maestro Concert Grand** is "All rights reserved … may not modify and spread": not free.
- **Philharmonia** samples "must not be … made available 'as is' (i.e. as samples or as a
  sampler instrument)": not a free licence.
- **University of Iowa MIS** has no formal licence, only "used for any projects, without
  restrictions": ask Prof. Fritts for an explicit CC0 statement before bundling.
- **Salamander Drumkit** is CC-BY-SA-3.0, not CC-BY. Salamander Grand v3 is confirmed
  CC-BY-3.0.
- **Spitfire LABS** now redirects to **Splice INSTRUMENT** (freemium, Splice account).
- **sfizz** is archived (library repo 2026-06-21; last release 1.2.3, January 2024).
  Exemplar already covers SFZ.

## The findings in one page

### Samples (samples.md)

- **CC0 drums, verified:** Karoryfer Frankensnare (~12 snares, claps, tambourine), Gogodze
  Phu I and II (a full multi-mic kit with hi-hat openness steps), Big Rusty, Swirly,
  Unruly; VCSL percussion (claps, hi-hat, snares, toms, congas, bongos, cajon, shakers,
  tambourines, cowbells, claves, woodblock, cymbals); Virtuosity Drums (jazz kit, 1.1 GB).
- **CC0 instruments:** VSCO 2 CE (chamber orchestra, ~2.3 GB), VCSL (~4 GB, ~150
  instruments), Osiris Piano (460 MB, the only verified CC0 concert-style piano), the
  Karoryfer basses, electric guitars, winds and world instruments.
- **CC-BY, good, needs a ruling:** Salamander Grand v3 (the standard free grand), Naked
  Drums, the DrumGizmo kits (which add an album-credit request), Headroom Piano.
- **Electronic one-shots:** no clean CC0 source. The *sound* of an 808 is free; a
  recording of a unit or its ROM is not. **Render Pulsus presets to WAV and release them
  CC0**; the presets in the repo are their source. About 3–6 engineering days.
- **Loops:** no bundle-safe source. Make them in Cantus from the CC0 kit and release them
  CC0 with their project files.
- **Core kit:** ~120 one-shots + ~10 cymbals, 48 kHz/24-bit FLAC, **35–45 MB**, from
  Karoryfer, VCSL and Pulsus renders, mapped to GM notes; with a lean Osiris Piano the
  whole core stays under 150 MB.
- **Process:** a provenance manifest per file (source URL, SPDX licence, author, date,
  SHA-256, modifications), a saved copy of every licence, rejection rules, and a CI check
  that every bundled file is in the manifest; the credits view is generated from it.

### Storage (storage.md)

| Option | Verdict | Cost at 1k / 10k / 100k downloads of 500 MB a month |
|---|---|---|
| Git LFS on GitHub | **Reject**: 10 GiB/month free, forks count against us, users would need git | ~$40 / $407 / $4 070 |
| **GitHub Releases** on a `cantus-content` repo | **Phase 1**: 2 GiB per file, "no limit on … bandwidth usage" (AUP §9 may throttle extreme use) | $0 / $0 / $0 (risk grows) |
| **Cloudflare R2** + custom domain | **Phase 2 mirror**: $0.015/GB-month, egress free | ~$0 / ~$0–1 / ~$0–5 |
| Bunny CDN, B2 behind a free-egress CDN | Plan B if R2 changes | ~$3 / $26 / $251 (Bunny Volume) |
| SourceForge, archive.org | Free mirrors (SourceForge) and archival copies (archive.org) | $0 |

- **Competitors:** REAPER 17 MB, no content; MuseScore 129 MB with a 51 MB SoundFont,
  Muse Sounds through the separate Muse Hub; Surge XT 183 MB with factory content; FL
  Studio 1.03 GB plus in-browser Content Library and FL Cloud Sounds; Ableton ~6 GB
  install, content as Packs (Suite 71+ GB) installed from the browser or as `.alp` files
  for offline machines; Bitwig's in-app Package Manager (1.5–3.4 GB packages).
- **Content manager:** first-run "download the factory library" (skippable); per-pack
  install, update and remove with a licence sheet before install; a manifest signed with
  minisign (Ed25519) with a rising `sequence` against rollback; SHA-256 per archive and
  file; resumable Range downloads into a staging folder and an atomic install; mirrors in
  order; *Install from file* for offline machines; the library on any drive; projects
  reference `content://<pack>/<path>` + version + hash, with a missing-content dialog;
  pack updates never change the audio behind an existing path.
- **Behind Zscaler:** OS certificate store (rustls-native-certs or native-tls), honour
  `HTTPS_PROXY`, detect HTML interstitials, never disable TLS checks.
- **Privacy:** no account, no telemetry; one manifest check a day at most, switchable off.

### Free instruments (instruments.md)

- **Gap by gap** (Cantus has Firmus, Pulsus II, Unda, Calor, Modulus, Exemplar, Gravis,
  Organum, Lamina):

  | Gap | Rights-free, through Exemplar | Link-only freeware |
  |---|---|---|
  | Acoustic piano | Osiris Piano (CC0); Salamander Grand v3 (CC-BY, if ruled in) | BBC SO Discover piano, Splice INSTRUMENT (ex-LABS) Soft Piano |
  | Orchestral | VSCO 2 CE, VCSL (CC0) | **BBC SO Discover** (the best free orchestra; Spitfire account) |
  | Guitar | Electric: Karoryfer Shinyguitar, Emilyguitar, Black And Green Guitars (CC0). Acoustic: **none verified** | Ample Guitar M Lite v4, Ample Bass P Lite |
  | Acoustic drums | Virtuosity Drums, Karoryfer kits (CC0); Naked Drums, DrumGizmo kits (CC-BY) | MT Power Drum Kit 2 (not verified) |
  | World | VCSL, Karoryfer (Gogodze Phu, lyre, zither, erhu, tagelharpa), RipplerX (GPL-3, modelled) | Pianobook libraries |
  | Vocal / choir | **Gap remains**: nothing verified | Pianobook (not individually verified) |

- **OSS plug-ins worth bundling** (Windows binaries, verified licences): OB-Xf 1.0.3
  (GPL-3, CLAP), Dexed 1.0.1 (GPL-3, CLAP), Firefly Synth 2 2.1.6 (MIT, CLAP + VST3),
  Surge XT 1.3.4 (GPL-3; needs its ~250 MB content), Six Sines 1.2.1 (MIT), Odin 2 2.4.1
  (GPL-3), JC303 0.13.0 (GPL-3), RipplerX 1.5.19 (GPL-3, VST3 only). They overlap
  Cantus's own synths: their value is **proving the host**, not filling a gap.
- **Not usable:** Synth1, Helm, Tunefish and DrumGizmo's plug-in are VST2 only on Windows;
  Vitalium is a 2021 fork shipped as LV2 only on Windows; Zyn-Fusion's Windows binaries
  are sold.
- **Freeware is link-only:** no EULA read grants a third party the right to redistribute;
  most sit behind an account, an app or an e-mail (Vital, BBC SO Discover, Splice
  INSTRUMENT, Komplete Start, Decent Sampler). The catalogue opens the vendor's page.
- **Compatibility test set for CI** (< 150 MB, all redistributable): free-audio
  clap-plugins, Six Sines, Firefly Synth 2, OB-Xf, RipplerX, one nih-plug effect, sfizz.
  It covers CLAP-native, clap-wrapper, JUCE CLAP/VST3, VST3-only JUCE, Rust and the plain
  VST3 SDK. Manual smoke tests only: TyrellN6 and sforzando (freeware CLAP), BBC SO
  Discover and Decent Sampler (big VST3 samplers). Useful to the hosted-inst builder now.

## Recommendations, in priority order

1. **Rule on the content licence policy first** (an ADR, see "Legal points" 1–3). Every
   later step depends on it, and the samples builder's content manager needs the list of
   accepted SPDX ids.
2. **Build the provenance pipeline before the first sample lands:** the manifest
   (samples.md §8 fields + storage.md §5.2 fields in one schema), the evidence folder with
   saved licence texts, the CI check, and the generated credits view and
   `CREDITS-SAMPLES.txt`.
3. **Ship a CC0 core kit in the installer** (~40 MB of drums from Karoryfer and VCSL) and
   keep the demo songs on it alone, so every demo opens offline.
4. **Render Pulsus to a CC0 electronic kit** (808/909-style kicks, snares, claps, hats) with
   a deterministic `xtask`; do not look for third-party 808 samples.
5. **Create `cantus-content`** with CI that builds pack zips (FLAC stored, ≤ 2 GiB each),
   SHA-256s and a minisign-signed manifest, and publishes to GitHub Releases. Keep the
   signing key offline; embed two public keys for rotation.
6. **The in-app content manager** (storage.md §5): first-run prompt, per-pack install,
   *Install from file*, proxy-safe networking, `content://` references (a schema bump with
   fixture and migration), missing-content dialog.
7. **Optional CC0 packs as downloads:** Osiris Piano (the default piano under today's
   ADR), VSCO 2 CE and VCSL split by section, Virtuosity Drums, the full Karoryfer kits,
   guitars and basses. Point the manifest at upstream URLs first and our copy second.
8. **Prove hosting with real plug-ins:** the CI test set above; bundle OB-Xf, Dexed and
   Firefly Synth 2 as an optional installer component **with their exact source mirrored
   as assets of the same release** and a `THIRD-PARTY-PLUGINS` notice.
9. **A "Get free instruments" catalogue** in the device picker: OSS entries download from
   upstream with pinned version and SHA-256 (Surge XT, Odin 2, JC303, RipplerX, Six
   Sines); freeware entries are cards that open the vendor page and say what the vendor
   requires (account, app, e-mail).
10. **Phase 2 hosting** when downloads pass ~5k a month or GitHub complains: Cloudflare R2
    on a custom domain as mirror #1, SourceForge as a free mirror; apply to Cloudflare
    Project Alexandria and Fastly Fast Forward.
11. **Write to the authors** of the unclear cases: Karoryfer (The Hat with the Phat: CC0 or
    GPL-3?), Prof. Lawrence Fritts (an explicit CC0 statement for the Iowa MIS), SM Drums
    (a licence statement), Splendid Grand (a primary source for Akai's dedication).
12. **Longer term: original content.** A Cantus-recorded piano and acoustic kit, or a
    commission (e.g. from Karoryfer or Versilian, who already publish CC0) under a
    contract with assignment, a CC0 release and a no-third-party-samples warranty. The
    vocal/choir and acoustic-guitar gaps have no rights-free answer today.

## Legal points the owner must decide

This is not legal advice. It sets out what the sources say; a lawyer should read the final ADR.

1. **Reconcile ADR 0001 with idea.md §8.** ADR 0001 says bundled samples must be
   *original or CC0*; idea.md §8 says *verified permissive/CC0*. A proposal:
   - **In the installer:** original (released CC0 by the project) or CC0-1.0 only. The
     Unlicense and MIT-licensed data (a few SFZ files) count as CC0-equivalent if the
     owner agrees; they need only their notice.
   - **Optional downloads:** the same, plus CC-BY-3.0/4.0 *if* the owner admits it (point 2).
   - **Never:** NC, ND, Sampling Plus, "royalty-free, no redistribution", "public domain"
     with no primary source, and anything whose author cannot be identified.
   - The GPL does not decide this: samples are separate works shipped alongside (GPL FAQ
     `#MereAggregation`), and the GPL does not reach what users make (`#GPLOutput`).
2. **CC-BY with attribution?** It would admit Salamander Grand v3 (the best free grand),
   Naked Drums and the DrumGizmo kits. Cantus can meet the duty with a credits file and
   an in-app credits view generated from the manifest. CC-BY 4.0 §3(a)(1)(B) also
   requires marking modifications (trimming, normalising, FLAC re-encoding). **The catch is downstream:** a
   user who releases a song made with CC-BY samples arguably owes attribution too, and
   DrumGizmo explicitly asks for album credits. That is the main reason to keep CC-BY out
   of the installer and, if admitted, offer it only as opt-in downloads that show the
   licence before install.
3. **CC-BY-SA: exclude by default.** Share-alike may reach users' songs that contain the
   samples (Salamander Drumkit, the AVL Hydrogen kits). GPL-labelled audio (some Hydrogen
   kits) is odd for samples and also best left out.
4. **Bundling GPL plug-ins vs downloading them from upstream.** Both are lawful.
   - The FSF reads a dynamically loaded plug-in that shares data structures as one
     combined program with the host (`#GPLPlugins`). Cantus is GPL-3.0-or-later, so
     GPL-3, GPL-2-or-later, AGPL-3 (JUCE 8 builds, GPLv3 §13) and permissive plug-ins are
     all fine to ship. **GPL-2-only plug-ins: keep them out of the bundle.**
   - Shipping a GPL binary obliges Cantus to provide the exact corresponding source for
     as long as it ships the binary (`#AnonFTPAndSendSources`): mirror each upstream
     source tarball beside the release, do not rely on upstream links, and do not patch
     the plug-ins.
   - The permissive core crates are unaffected: they link no plug-in.
   - A catalogue that downloads from upstream redistributes nothing and leaves upstream
     the credit and the updates. But it needs the network and proves less about compatibility.
5. **A GPL-3 §7 additional permission** allowing Cantus to load non-free CLAP/VST3
   plug-ins. Users running proprietary plug-ins privately is not distribution, so there
   is no practical problem today. The permission removes doubt for redistributors such
   as Linux distributions. Only copyright holders can grant it. Cantus has no CLA, so
   every contributor to the GPL crates holds part of the copyright; best decided while
   their number is small.
6. **Trademarks.** The GPL grants none.
   - **Vital:** its README forbids naming a binary distribution "Vital", using Vital's
     online services from other builds, and redistributing its factory presets; hence
     DISTRHO's **Vitalium**. Cantus may name Vital only when linking to vital.audio.
   - **Cardinal** removed all VCV branding for the same reason.
   - Never use "Oberheim", "Roland TB-303", "Yamaha DX7" or "808/909" as names or
     categories; "303-style" and the plug-ins' own names (OB-Xf, JC303, Dexed) are safe.
   - Sample packs named after commercial machines or famous drummers are a provenance
     red flag, not just a trademark one.
   - ADR 0001's open TODO stands: a trademark search on "Cantus" in class 9 before a
     public release.
7. **The VST logo.** The VST 3 SDK has been MIT since 3.8 (2025-10-29), so hosting VST3
   raises no copyright question. The name and the "VST Compatible" logo remain
   Steinberg's trademarks, under the *Steinberg VST Usage Guidelines* shipped in the SDK:
   logos unaltered and only on compatible products, never "VST" inside a product name.
   Recommended: use the word "VST3" descriptively in the picker and add "VST is a
   trademark of Steinberg Media Technologies GmbH, registered in Europe and other
   countries" to the About box and docs; use the logo only after reading the guidelines.
8. **Commissioned content** needs a written contract: assignment (or work made for hire),
   a CC0 release, and the author's warranty that no third-party samples were used.

## Not verified (summary)

Each file lists its own. The ones that matter for a decision:

- The live GNU GPL FAQ page (read through the Internet Archive copy).
- Freesound's own pages and API terms.
- The free-pack terms of Cymatics, LANDR, Black Octopus, Goldbaby, Reverb Machine,
  Looperman and SampleSwap: assume "no redistribution".
- GeneralUser GS, FluidR3_GM and MuseScore_General (believed MIT).
- Backblaze prices (the site is blocked by the proxy); AWS S3 list prices; whether
  CloudFront's flat-rate plans allow bulk downloads.
- Whether GitHub's release CDN honours HTTP Range requests: test it before relying on
  resumable downloads.
- The freeware EULAs of Spitfire/Splice, Komplete Start, Ample Sound, u-he and Plogue;
  Keyzone Classic, Piano One, MT Power Drum Kit 2, SSD 5.5's free tier.
- All cost figures for commissioning sound design (rough market knowledge, not quotes).
