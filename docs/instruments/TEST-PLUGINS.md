# Test plug-ins: free instruments Cantus is proved with

`crates/cantus-host/tests/real_instruments.rs` loads real, free, open-source CLAP and VST3
instruments (ADR 0028). The plug-ins are **not** in the repository (binaries, hundreds of
megabytes, their own licences). To run that test on a new machine, download them into one
folder **outside the clone** and point `CANTUS_PLUGINS` at it:

```text
set CANTUS_PLUGINS=D:\cantus-plugins
cargo test -p cantus-host --test real_instruments -- --ignored --nocapture --test-threads 1
```

Only official binaries from each project's own GitHub releases, gathered on 2026-10-05.
Check every archive before unpacking it: a zip starts with `PK`, an installer with `MZ`, and
the SHA-256 below must match (a corporate proxy may answer with an HTML page while it
scans a download — wait and retry; never bypass it). Nothing needs to be installed.

| Product | Version | Licence | Formats | Archive and SHA-256 |
|---|---|---|---|---|
| Surge XT | 1.3.4 | GPL-3.0 | CLAP + VST3 | https://github.com/surge-synthesizer/releases-xt/releases/download/1.3.4/surge-xt-win64-1.3.4-pluginsonly.zip `564e162c560af07ad4ed47fe1bfcd827cf97a575de30d06c48249aad2e7c35e6` |
| Surge XT factory data | 1.3.4 | GPL-3.0 | data | https://github.com/surge-synthesizer/releases-xt/releases/download/1.3.4/surge-xt-win64-1.3.4-portable-install.zip `a5f2376702ad218c0382640ed0c7b9ebb222ba2410a210879899c0f726859d8e` |
| Dexed | 1.0.1 | GPL-3.0 | CLAP + VST3 | https://github.com/asb2m10/dexed/releases/download/v1.0.1/Dexed-1.0.1-win.zip `1f118445f2c5c411636bb41ddf8b456db09af74cd38f9ede8a93a01020505f05` |
| OB-Xf | 1.0.3 | GPL-3.0 | CLAP + VST3 | https://github.com/surge-synthesizer/OB-Xf/releases/download/v1.0.3/ob-xf-Windows-v1.0.3.zip `149fda0649daf8a3aa0851801f92515f0dbf6203acf33c0edc7d3c445e80b1e7` |
| Six Sines | 1.2.1 | MIT | CLAP + VST3 | https://github.com/baconpaul/six-sines/releases/download/v1.2.1/six-sines-Windows-2026-07-20-18ecb36.zip `9f7c7fa5cb1c5e11b53c0a9306ee443149cc7bc0a109380dbbe1578f3a7b6e5d` |
| sfizz | 1.2.3 | BSD-2-Clause | VST3 | https://github.com/sfztools/sfizz-ui/releases/download/1.2.3/sfizz-1.2.3-win64.exe `56f57dfe443018e947b82b8e2bf4c9d824a9c961ff2b8eb6c5e929030b71b937` (an Inno Setup installer: unpack it with innoextract 1.9, do not run it) |
| Firefly Synth 2 | 2.1.6 | MIT | CLAP + VST3 | https://github.com/sjoerdvankreel/firefly-synth-2/releases/download/v2.1.6/FireflySynth2-windows-clap-Release.zip `c71f5afb001f0c402d53422ca8ee3f5e28588b2a7b87846706d41c06adf9c262` and …/FireflySynth2-windows-vst3-Release.zip `40a5d4f1d94e3f644bc59985b22432f8de4c71491af6ba47dec2401bcae7d9a3` |
| JC303 | 0.13.0 | GPL-3.0 | CLAP + VST3 | https://github.com/midilab/jc303/releases/download/v0.13.0/jc303-0.13.0-windows_x64-plugins.zip `ab0f9fb5515fd41ee3e967efeee8503bbaa19bfb8a71d3e95ac91a8751c00f99` |
| RipplerX | 1.5.19 | GPL-3.0 | VST3 | https://github.com/tiagolr/ripplerx/releases/download/v1.5.19/ripplerx-win-v1.5.19.zip `517c0e99f3ff37cf7b3941ed9afbece1423a0da69d70a9dd8076a473d630a50d` |

## Layout the test expects

One folder per product under `CANTUS_PLUGINS`; the scanner walks it recursively, so the
zips' own layouts work as they unpack:

```text
<CANTUS_PLUGINS>\SurgeXT\Surge XT.clap, Surge XT.vst3\, Surge XT Effects.*, SurgeXTData\
<CANTUS_PLUGINS>\Dexed\Dexed.clap, Dexed.vst3\
<CANTUS_PLUGINS>\OB-Xf\OB-Xf.clap, OB-Xf.vst3\
<CANTUS_PLUGINS>\SixSines\Six Sines.clap, Six Sines.vst3      (a single-file VST3)
<CANTUS_PLUGINS>\sfizz\sfizz.vst3\
<CANTUS_PLUGINS>\FireflySynth2\FireflySynth2.clap\, FireflySynth2.vst3\   (the CLAP is a bundle folder)
<CANTUS_PLUGINS>\JC303\CLAP\JC303.clap, VST3\JC303.vst3\
<CANTUS_PLUGINS>\RipplerX\VST3\RipplerX.vst3\
```

## Factory content

- **Surge XT** reads `SurgeXTData\` next to the plug-in (its official portable layout):
  copy that folder out of the portable zip. Without it Surge loads with the init patch only.
- **OB-Xf**'s patches are a separate archive (`ob-xf-assets-v1.0.3.zip`); not needed.
- **Dexed**, **Six Sines**, **JC303**, **RipplerX** carry what they need; **Firefly Synth 2**
  keeps its presets inside each bundle (keep the bundle whole); **sfizz** loads `.sfz` files
  and has no factory content.

## Not used, and why

Odin 2 and Geonkick ship only as Inno Setup installers innoextract 1.9 cannot unpack;
Vitalium has no Windows binary on GitHub; OB-Xd 2's licence is unclear (OB-Xf replaces it).
