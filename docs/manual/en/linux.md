# Cantus on Linux — installing and setting up audio

[Español](../es/linux.md) · [Back to the manual](README.md)

## What you need

- A 64-bit PC (x86-64) with a Linux from 2022 or later: Ubuntu 22.04, Linux Mint 21,
  Debian 12, Fedora 36, openSUSE Leap 15.5, Arch — or anything newer (glibc 2.35 or
  later).
- A desktop with X11 or Wayland (on Wayland Cantus runs through XWayland, which every
  Wayland desktop has; see [Plug-in windows](#plug-in-windows)).
- About 300 MB of disk.

## Install: the AppImage

1. Download `Cantus-<version>-linux-x86_64.AppImage` from the
   [releases page](https://github.com/luis314159/cantus-daw/releases) — or from
   [cantusdaw.com](https://cantusdaw.com).
2. Make it runnable: in your file manager, *Properties ▸ Permissions ▸ Allow executing
   file as program*; or in a terminal: `chmod +x Cantus-*.AppImage`.
3. Double-click it (or `./Cantus-*.AppImage`).

The AppImage is the whole program in one file: nothing is installed system-wide and no
administrator password is asked for. On its first start it puts **Cantus in your
applications menu** — with its icon, and *Open with Cantus* for `.cantus` projects —
pointing at the file where it is; move the file and the next start updates the entry. To
keep it out of the menu, start it with `CANTUS_NO_DESKTOP_ENTRY=1`.

A good place to keep it: `~/Applications`. **Updates** replace that same file.

**Uninstall**: delete the AppImage; remove the menu entry with
`rm ~/.local/share/applications/com.cantusdaw.Cantus.desktop`. Your settings are in
`~/.config/cantus` and your presets, recordings and autosaves in `~/.local/share/cantus`
(the `XDG_CONFIG_HOME` and `XDG_DATA_HOME` folders, when you set those): delete them only
if you want them gone. Your projects are never touched.

**Flatpak**: a Flatpak (`com.cantusdaw.Cantus`) is prepared for Flathub; until it is
published there, use the AppImage.

## Audio: the driver

Open *Settings ▸ Audio* (or the audio chip in the top bar). The **Driver** list offers
what your system has, the best first:

| Driver | When | Latency |
|---|---|---|
| **PipeWire** | Ubuntu 22.10 and later, Fedora 34 and later, Debian 12, Mint 22, Arch with PipeWire — most desktops today. Cantus talks to PipeWire directly. | Low: the buffer you choose is what PipeWire runs Cantus at (64–256 frames is comfortable on most machines). |
| **JACK** | When a JACK server is running (JACK2, or PipeWire's JACK). | Low: the server's period. |
| **ALSA** | Always. Through the system's default device (which on a PipeWire or PulseAudio system goes through them), or straight to a sound card (`hw:` devices) when nothing else uses it. | Higher through the default; low straight to the card. |

Pick the driver, the **Output** and **Input** devices, the **Sample rate** and the
**Buffer size** (frames), and *Apply*. Cantus remembers the choice for each interface.
Nothing to install: if PipeWire's or JACK's library is missing, those drivers are simply
not offered.

**Lower latency with PipeWire.** PipeWire runs every program at one quantum (its buffer).
Cantus asks for the buffer you chose; if another program asks for a bigger one, PipeWire
may run the graph at that. To pin it: `pw-metadata -n settings 0 clock.force-quantum 128`
(back to normal: `… 0`).

**JACK through PipeWire.** On a PipeWire system without a JACK server, install your
distribution's PipeWire JACK package (`pipewire-jack`) to use the JACK driver there too;
PipeWire's own driver is just as fast.

## Real-time priority

Under *Settings ▸ Audio*, the line **Real-time priority** says what the system gave
Cantus's audio threads: for example `SCHED_FIFO priority 50` (direct), `SCHED_RR
priority 20 via rtkit`, or `refused: …` with the reason. The audio thread of PipeWire and
JACK is made real-time by them (`driver-owned thread: SCHED_FIFO`); Cantus raises its
own threads.

Most desktops grant it through **rtkit** with nothing to do. If the line says *refused*,
either install and start `rtkit` (`sudo apt install rtkit`), or give your user the right
to ask directly — the classic audio group:

```text
sudo usermod -aG audio $USER
echo '@audio - rtprio 95' | sudo tee /etc/security/limits.d/95-audio.conf
echo '@audio - memlock unlimited' | sudo tee -a /etc/security/limits.d/95-audio.conf
```

then log out and in again. (Ubuntu Studio and Fedora Jam set this up already.)

## Plug-ins

Cantus finds CLAP and VST3 plug-ins in the standard Linux folders:

| Format | Folders |
|---|---|
| CLAP | `~/.clap`, `/usr/lib/clap`, `/usr/local/lib/clap`, and the folders in `CLAP_PATH` |
| VST3 | `~/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3`, and the folders in `VST3_PATH` |

Install a plug-in the way its maker says (usually: copy the `.clap` file or the `.vst3`
folder into `~/.clap` or `~/.vst3`), then *Settings ▸ Plug-ins ▸ Scan for plug-ins*. Each file is read
in a separate scanner process, so a plug-in that crashes while being read only shows as
*blocked*. A plug-in can also run in a process of its own (*When a plug-in crashes* in
the manual): a crash there is caught, Cantus carries on, and you can reload it.

Cantus's own effects are also in the AppImage as CLAP and VST3 plug-ins, for other
programs: extract them with `./Cantus-*.AppImage --appimage-extract` and copy
`squashfs-root/usr/share/cantus/plugins/clap/*` into `~/.clap` and
`…/plugins/vst3/*` into `~/.vst3`.

### Plug-in windows

A plug-in's own editor opens in a window of its own, kept above Cantus's. On Linux that
window is an **X11** window, because plug-ins draw into X11 windows only; on a Wayland
desktop it goes through XWayland, and so does Cantus's own window, so the two stay
together. (Starting Cantus with `CANTUS_WAYLAND=1` gives it a native Wayland window
instead; plug-in editors then open as ordinary windows that are not kept above it.)

## Files, dialogs and fonts

- Open and save dialogs are your desktop's own (through the XDG desktop portal; with
  `zenity` where there is no portal). On a bare window manager without either, install
  `xdg-desktop-portal-gtk` or `zenity`.
- Text in Chinese, Japanese, Korean, Hindi or Thai uses the fonts your system has for it
  (asked of fontconfig): install `fonts-noto-cjk` (Debian/Ubuntu) or
  `google-noto-sans-cjk-fonts` (Fedora) if those characters show as boxes.

## Troubleshooting

| What you see | What to do |
|---|---|
| No sound, the driver list has only ALSA | PipeWire or JACK is not running for your user. Try ALSA with the *default* device, or start your desktop's sound service. |
| Clicks and dropouts | Raise the buffer (*Settings ▸ Audio ▸ Buffer size*); check the **Real-time priority** line (see above); close programs that use the CPU heavily. |
| A plug-in's window stays empty or does not open | Cantus is running with `CANTUS_WAYLAND=1`, or the plug-in needs a library your system lacks: start Cantus from a terminal and read what the plug-in prints. |
| The AppImage does not start (*FUSE*) | Very old AppImages needed `libfuse2`; Cantus's does not. If your system blocks AppImages, run `./Cantus-*.AppImage --appimage-extract` and start `squashfs-root/AppRun`. |
