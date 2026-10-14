# Third-party notices

**Cantus** — a free, open-source digital audio workstation. Created by Luis Ángel
Almazán — [lalmazan.com](https://lalmazan.com). Source: <https://github.com/luis314159/cantus>.

Cantus is built from the work of many people. This file names every part of a Cantus
release that someone else made, the licence it is under and the notices those licences
ask us to carry. Nothing here limits the rights each licence gives you.

- [Cantus itself](#cantus-itself)
- [Patched crates: cpal and asio-sys](#patched-crates-cpal-and-asio-sys)
- [The Steinberg ASIO SDK](#the-steinberg-asio-sdk)
- [Trademarks](#trademarks)
- [Maths: libm and musl](#maths-libm-and-musl)
- [Fonts](#fonts)
- [Bible translations (the verse at start)](#bible-translations-the-verse-at-start)
- [Drivers Cantus links to but does not include](#drivers-cantus-links-to-but-does-not-include)
- [The installer](#the-installer)
- [The Rust toolchain](#the-rust-toolchain)
- [Content](#content) — the sound packs Cantus downloads when you ask, and their
  credits, generated from the content manifest
- [Rust crates](#rust-crates) — every crate compiled into the program, generated from
  `Cargo.lock`
- [Licence texts](#licence-texts) and [standard texts](#standard-texts)

## Cantus itself

Copyright © 2026 Luis Ángel Almazán and the Cantus contributors.

| Part | Licence |
|---|---|
| The application — `cantus` (the program), `cantus-ui`, `cantus-ai`, `cantus-host`, `cantus-content`, `cantus-update`, `cantus-plugins` (the CLAP and VST3 plug-ins), `xtask` | GPL-3.0-or-later (`LICENSE-GPL`) |
| The engine crates — `cantus-core`, `cantus-dsp`, `cantus-engine`, `cantus-project`, `cantus-rt` | MPL-2.0 (`LICENSE-MPL`; each crate's `LICENSE` carries the licence's Exhibit A notice — ADR 0047) |

The program as distributed is a combined work under the **GNU General Public License,
version 3 or later** (ADR 0001). Its complete corresponding source is the Git tag of the
release at <https://github.com/luis314159/cantus>, plus the two inputs that are not
committed there: the Steinberg ASIO SDK (below: where it comes from and its pinned
SHA-256) and the crates from crates.io that `Cargo.lock` names (each listed below). The
logos, icons, the interface's drawings, the factory presets and the demo songs are
original work of the project, under the same licence as the crate that holds them.

## Patched crates: cpal and asio-sys

`third_party/cpal` (cpal 0.18.2) and `third_party/asio-sys` (asio-sys 0.4.0) are the
crates.io releases of the RustAudio project, <https://github.com/RustAudio/cpal>, with
Cantus's ASIO fixes marked `CANTUS PATCH` (`third_party/README.md` lists every change).

- Licence: **Apache License 2.0** — `third_party/cpal/LICENSE`. asio-sys's crate ships
  without the file, so the same project's text is copied to `third_party/asio-sys/LICENSE`.
- Copyright © the cpal and asio-sys authors (RustAudio). Modified by the Cantus
  contributors in 2026, as Apache-2.0 §4(b) asks us to say: the changes are in
  `src/host/asio/` and `src/host/mod.rs` of cpal and in `build.rs`, `src/lib.rs` and
  `src/bindings/mod.rs` of asio-sys.

## The Steinberg ASIO SDK

Cantus's ASIO support is built with the **Steinberg ASIO SDK 2.3.4** (October 2025), which
Steinberg licenses under the proprietary Steinberg ASIO License **or** the GNU General
Public License version 3, at the developer's choice. **Cantus uses it under the GPL-3.0
option.** The SDK's host files (`common/asio.cpp`, `host/asiodrivers.cpp`,
`host/pc/asiolist.cpp` and their headers) are compiled into `cantus.exe` by `asio-sys`.

- Copyright © 2025 Steinberg Media Technologies GmbH. All Rights Reserved.
- The SDK is not committed to Cantus's repository. `cargo run -p xtask -- asio-sdk`
  downloads it from Steinberg,
  <https://download.steinberg.net/sdk_downloads/ASIO-SDK_2.3.4_2025-10-15.zip>, and checks
  its SHA-256, `d5ebf0c20dd2c5f43771fd0c1418f4b361bf52434ee670097cfa6b3a335e2eca`.
- The SDK's `LICENSE.txt` says, in part: *"This Software Development Kit is licensed under
  the terms of the Steinberg ASIO License, or alternatively under the terms of the General
  Public License (GPL) Version 3. You may use the Software Development Kit according to
  either of these licenses as it is most appropriate for your project on a case-by-case
  basis (commercial or not)."* The SDK is provided by Steinberg "as is", without warranty.

## Trademarks

- **ASIO** is a registered trademark of Steinberg Media Technologies GmbH. (ASIO® — the
  *Steinberg ASIO Usage Guidelines* ask for this attribution in product credits and
  documentation.) Cantus uses the word only to say which audio drivers it works with, and
  does not use the ASIO logo.
- **VST** is a registered trademark of Steinberg Media Technologies GmbH. Cantus hosts
  VST3 plug-ins and exports its own effects as VST3 plug-ins through the MIT-licensed
  `vst3` crate (coupler-rs); it does not use the VST logo.
- **CLAP** is the CLever Audio Plug-in API by Bitwig and u-he (MIT licence).
- **World English Bible** is a trademark of eBible.org (see below).
- Windows is a trademark of the Microsoft group of companies. GitHub is a trademark of
  GitHub, Inc. Other names are the trademarks of their owners and are used only to
  describe compatibility.
- "Cantus" is the project's name; a trademark search is pending (ADR 0001).

## Maths: libm and musl

Every transcendental function in Cantus's audio code goes through `libm` 0.2.16 (pinned,
`cantus_dsp::math`), a Rust port of musl's maths library. Its `LICENSE.txt` is printed in
full under [Licence texts](#licence-texts) (look for `libm 0.2.16`): MIT for the crate
(Copyright © 2018 Jorge Aparicio), musl libc's notice (Copyright © 2005–2020 Rich
Felker, et al.), and the notices musl carries from FreeBSD and Sun Microsystems
(Copyright © 1993, 2004 Sun Microsystems).

## Fonts

| Font | Where | Copyright | Licence |
|---|---|---|---|
| IBM Plex Sans, Sans Condensed and Mono (Regular, Medium, SemiBold, Condensed Italic) | Cantus's interface (`crates/cantus-ui/assets/fonts/`) | © 2017 IBM Corp., with Reserved Font Name "Plex" | SIL Open Font License 1.1 — `crates/cantus-ui/assets/fonts/LICENSE-OFL.txt`, printed below |
| emoji-icon-font | the symbol fallback after every Plex face (`crates/cantus-ui/assets/fonts/`, as egui distributed it) | © 2014 John Slegers | MIT — `crates/cantus-ui/assets/fonts/LICENSE-emoji-icon-font.txt`, printed below |

Fonts are bundled with the program; they are not sold by themselves. egui's other default
fonts (Ubuntu Light, Hack, Noto Emoji) are not built into Cantus (round 26). For names in
scripts these fonts lack (Japanese, Korean, Hindi, Arabic…), Cantus reads your operating
system's own fonts from its font folder when they are first needed; they are neither
bundled nor distributed with Cantus.

<!-- font-licences -->

The IBM Plex licence, as `LICENSE-OFL.txt` begins: *"Copyright © 2017 IBM Corp. with
Reserved Font Name "Plex". This Font Software is licensed under the SIL Open Font
License, Version 1.1."*

## Bible translations (the verse at start)

Cantus shows a Bible verse when it starts (on by default; *Settings ▸ General ▸ Show a
verse at start* turns it off). The texts are in `crates/cantus-ui/locales/verses/`; how
they were taken is in that folder's `README.md`. They are data under their own terms, not
under the GPL.

- **Reina-Valera 1960** (`es.txt`) — **not in this build**: Cantus neither carries nor
  shows it until its publisher's written permission is granted, and the file is kept
  privately until then. With the permission, its notice, word for word, will be:

  > Reina-Valera 1960 © Sociedades Bíblicas en América Latina, 1960. Renovado © Sociedades
  > Bíblicas Unidas, 1988. Utilizado con permiso.
- **Reina-Valera 1909** (`es-1909.txt`, the Spanish text Cantus shows) — public domain;
  text from eBible.org.
- **World English Bible** (`en.txt`, the *Updated* edition) — public domain; text from
  eBible.org. "World English Bible" is a trademark of eBible.org; the text is unmodified.

## Drivers Cantus links to but does not include

- **FlexASIO** by Etienne Dechamps (MIT licence), <https://github.com/dechamps/FlexASIO>.
  Not bundled. The installer's optional, unticked *FlexASIO driver (downloaded now)*
  component downloads its latest release from GitHub and runs its own installer; the app's
  *Get a low-latency driver* window only opens its release page.
- **ASIO4ALL** by Michael Tippach. Not bundled and never downloaded by Cantus (its licence
  does not allow redistribution): the app opens <https://asio4all.org> in your browser.
- The interface makers' own drivers: links to their pages only.

## The installer

`Cantus-<version>-windows-x64-setup.exe` is made with **NSIS** 3.11
(<https://nsis.sourceforge.io>): NSIS is under the zlib/libpng licence (Copyright ©
1999–2025 NSIS Contributors); its LZMA decompressor, which the installer contains, is
under the Common Public License 1.0, and NSIS's source code is available from that site.

## The Rust toolchain

`cantus.exe` and the plug-ins contain the Rust standard library (`std`, `core`, `alloc`),
Copyright © The Rust Project Developers, under MIT OR Apache-2.0
(<https://github.com/rust-lang/rust>), and link to the Microsoft C runtime that Windows
provides.
