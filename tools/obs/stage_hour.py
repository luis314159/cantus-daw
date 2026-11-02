"""The hour under OBS (round 35, ADR 0091 §5): Cantus's streaming window captured by OBS
Studio for an hour while the song loops, its transparency and its dancing checked every few
seconds, Cantus's CPU and memory sampled.

It never touches the owner's own OBS: it runs a **portable copy** (a copy of the installed
OBS with `portable_mode.txt` beside it, its own config folder), and Cantus with its own data
folder (`CANTUS_DATA_DIR`). Standard library only.

    python -I tools/obs/stage_hour.py --cantus target/release/cantus.exe \
        --obs F:/projects/daw-r35/downloads/obs-portable --song song.cantus \
        --out F:/projects/daw-r35/tmp/obs-hour --minutes 60

What it does:
 1. Writes OBS's websocket config (port 4466, local, no password) and marks the first run
    done, in the portable copy only; starts OBS minimised to the tray.
 2. Phase **baseline**: Cantus playing the song with the companion closed, CPU and memory
    sampled (`--baseline-minutes`).
 3. Phase **stage**: Cantus restarted with the streaming window open (transparent, 320 x 360
    at the bottom-right of the screen). OBS gets a scene with three sources capturing the
    window `Cantus Stage`: Game Capture with *Allow transparency*, Window Capture (Windows
    Graphics Capture) and Window Capture (BitBlt); each one's corners are read once — which
    keep the alpha — then only Game Capture stays on.
 4. Every `--every` seconds: a burst of four screenshots of the Game Capture source a
    sixth of a second apart — the corners must be transparent, the character's pixels
    present and moving between them — and a sample of Cantus's CPU and memory.
 5. Writes `samples.csv`, `checks.csv` and `summary.json`, and a few PNGs, to `--out`.
"""

import argparse
import atexit
import base64
import csv
import hashlib
import json
import os
import socket
import struct
import subprocess
import sys
import time
import zlib

TITLE = "Cantus Stage"
PORT = 4466


# ---- a minimal WebSocket client (RFC 6455) for obs-websocket v5 -------------------------

class Ws:
    def __init__(self, host, port):
        self.s = socket.create_connection((host, port), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode()
        req = (
            f"GET / HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\n"
            f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Protocol: obswebsocket.json\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )
        self.s.sendall(req.encode())
        head = b""
        while b"\r\n\r\n" not in head:
            chunk = self.s.recv(1)
            if not chunk:
                raise ConnectionError("closed during the handshake")
            head += chunk
        if b" 101 " not in head.split(b"\r\n", 1)[0]:
            raise ConnectionError(head.decode(errors="replace"))
        self.next_id = 0

    def _recv_exact(self, n):
        buf = bytearray()
        while len(buf) < n:
            chunk = self.s.recv(n - len(buf))
            if not chunk:
                raise ConnectionError("closed")
            buf += chunk
        return bytes(buf)

    def send(self, obj):
        data = json.dumps(obj).encode()
        head = bytearray([0x81])
        n = len(data)
        if n < 126:
            head.append(0x80 | n)
        elif n < 65536:
            head.append(0x80 | 126)
            head += struct.pack(">H", n)
        else:
            head.append(0x80 | 127)
            head += struct.pack(">Q", n)
        mask = os.urandom(4)
        head += mask
        self.s.sendall(bytes(head) + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def recv(self):
        message = bytearray()
        while True:
            b0, b1 = self._recv_exact(2)
            n = b1 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._recv_exact(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._recv_exact(8))[0]
            payload = self._recv_exact(n)
            op = b0 & 0x0F
            if op == 0x9:  # ping: answer
                continue
            if op == 0x8:
                raise ConnectionError("closed by OBS")
            message += payload
            if b0 & 0x80:
                return json.loads(message.decode())

    def identify(self):
        hello = self.recv()
        if hello.get("op") != 0:
            raise ConnectionError(f"no Hello: {hello}")
        if hello["d"].get("authentication"):
            raise ConnectionError("the portable OBS asks for a password: its config was not used")
        self.send({"op": 1, "d": {"rpcVersion": 1, "eventSubscriptions": 0}})
        ok = self.recv()
        if ok.get("op") != 2:
            raise ConnectionError(f"not identified: {ok}")

    def call(self, kind, data=None):
        self.next_id += 1
        rid = str(self.next_id)
        self.send({"op": 6, "d": {"requestType": kind, "requestId": rid, "requestData": data or {}}})
        while True:
            m = self.recv()
            if m.get("op") == 7 and m["d"].get("requestId") == rid:
                st = m["d"]["requestStatus"]
                if not st.get("result"):
                    raise RuntimeError(f"{kind}: {st}")
                return m["d"].get("responseData") or {}


# ---- PNG (8-bit RGBA or RGB, not interlaced): what OBS's screenshots are ----------------

def png_rgba(data):
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos, idat, w, h, ctype = 8, b"", 0, 0, 6
    while pos < len(data):
        n = struct.unpack(">I", data[pos:pos + 4])[0]
        kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, inter = struct.unpack(">IIBBBBB", body)
            assert depth == 8 and inter == 0 and ctype in (2, 6), (depth, ctype, inter)
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    raw = zlib.decompress(idat)
    bpp = 4 if ctype == 6 else 3
    stride = w * bpp
    out = bytearray(h * stride)
    prev = bytearray(stride)
    i = 0
    for y in range(h):
        f = raw[i]
        line = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        out[y * stride:(y + 1) * stride] = line
        prev = line
    if bpp == 3:
        rgba = bytearray()
        for k in range(0, len(out), 3):
            rgba += out[k:k + 3] + b"\xff"
        out = rgba
    return w, h, bytes(out)


def alpha_at(img, x, y):
    w, h, px = img
    return px[(y * w + x) * 4 + 3]


def opaque_count(img):
    w, h, px = img
    return sum(1 for k in range(3, len(px), 16) if px[k] > 128) * 4


def corners_clear(img):
    w, h, _ = img
    pts = [(2, 2), (w - 3, 2), (2, h - 3), (w - 3, h - 3)]
    return all(alpha_at(img, x, y) == 0 for x, y in pts)


def soft_count(img):
    """Pixels (every 4th) of partial alpha: soft shadows, glass, anti-aliased edges."""
    w, h, px = img
    return sum(1 for k in range(3, len(px), 16) if 8 <= px[k] <= 247) * 4


def changed_share(a, b):
    """The share of pixels (sampled every 4th) whose alpha changed by more than 32."""
    if a[0] != b[0] or a[1] != b[1]:
        return 1.0
    pa, pb = a[2], b[2]
    n = d = 0
    for k in range(3, len(pa), 16):
        n += 1
        if abs(pa[k] - pb[k]) > 32:
            d += 1
    return d / max(n, 1)


# ---- the processes ---------------------------------------------------------------------

def process_sample(pid):
    """CPU seconds so far, working set and private bytes of a process (PowerShell)."""
    cmd = (
        f"$p = Get-Process -Id {pid} -ErrorAction Stop; "
        "\"$($p.TotalProcessorTime.TotalSeconds) $($p.WorkingSet64) $($p.PrivateMemorySize64)\""
    )
    out = subprocess.run(
        ["powershell.exe", "-NoProfile", "-Command", cmd],
        capture_output=True, text=True, timeout=30,
    ).stdout.split()
    return float(out[0]), int(out[1]), int(out[2])


MINIMISE_PS = r"""
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class M {
  public delegate bool Each(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Each f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
}
'@
$target = [uint32]$args[0]
[M]::EnumWindows({ param($h, $l)
  $p = 0; [M]::GetWindowThreadProcessId($h, [ref]$p) | Out-Null
  if ($p -eq $target -and [M]::IsWindowVisible($h)) {
    $sb = New-Object System.Text.StringBuilder 256
    [M]::GetWindowText($h, $sb, 256) | Out-Null
    $t = $sb.ToString()
    if ($t -like '*Cantus' -and $t -ne 'Cantus Stage') { [M]::ShowWindow($h, 6) | Out-Null; "minimised: $t" }
  }
  $true }, [IntPtr]::Zero) | Out-Null
"""


def minimise_main_window(pid):
    """Minimise Cantus's main window (not its stage): the companion then dances from the
    application's logic alone, as when a streamer keeps Cantus minimised."""
    script = os.path.join(os.environ.get("TEMP", "."), "cantus-minimise.ps1")
    with open(script, "w", encoding="utf-8") as f:
        f.write(MINIMISE_PS)
    out = subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass",
                          "-File", script, str(pid)], capture_output=True, text=True,
                         timeout=60)
    print(out.stdout.strip() or out.stderr.strip())


def screen_size():
    cmd = (
        "Add-Type -AssemblyName System.Windows.Forms; "
        "$b = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea; "
        "\"$($b.Width) $($b.Height)\""
    )
    out = subprocess.run(["powershell.exe", "-NoProfile", "-Command", cmd],
                         capture_output=True, text=True, timeout=30).stdout.split()
    return int(out[0]), int(out[1])


def prepare_obs(obs):
    cfg = os.path.join(obs, "config", "obs-studio")
    ws = os.path.join(cfg, "plugin_config", "obs-websocket")
    os.makedirs(ws, exist_ok=True)
    with open(os.path.join(ws, "config.json"), "w") as f:
        json.dump({"alerts_enabled": False, "auth_required": False, "first_load": False,
                   "server_enabled": True, "server_password": "", "server_port": PORT}, f)
    for name in ("global.ini", "user.ini"):
        path = os.path.join(cfg, name)
        if not os.path.exists(path):
            with open(path, "w") as f:
                f.write("[General]\nFirstRun=false\nEnableAutoUpdates=false\n")
    assert os.path.exists(os.path.join(obs, "portable_mode.txt")), "not a portable copy"


def ended_at_exit(p):
    """Whatever happens, the processes this started end with it."""
    def stop():
        try:
            if p.poll() is None:
                p.terminate()
        except OSError:
            pass
    atexit.register(stop)
    return p


def start_obs(obs):
    exe_dir = os.path.join(obs, "bin", "64bit")
    return ended_at_exit(subprocess.Popen(
        [os.path.join(exe_dir, "obs64.exe"), "--portable", "--multi", "--minimize-to-tray",
         "--disable-shutdown-check", "--disable-updater", "--disable-missing-files-check"],
        cwd=exe_dir,
    ))


def connect(deadline=90):
    t = time.time()
    while True:
        try:
            ws = Ws("127.0.0.1", PORT)
            ws.identify()
            return ws
        except OSError:
            if time.time() - t > deadline:
                raise
            time.sleep(2)


def cantus_data(out, phase, companion, pos):
    data = os.path.join(out, f"cantus-{phase}")
    os.makedirs(data, exist_ok=True)
    settings = {"audio_setup": {"setup_done": True}, "stage": {
        "companion_open": companion, "streaming": True, "backdrop": "transparent",
        "on_top": True, "size": [320.0, 360.0], "pos": pos, "vis": "spectrum",
        "bar_beat": True, "song": True,
    }}
    with open(os.path.join(data, "settings.json"), "w") as f:
        json.dump(settings, f)
    return data


def start_cantus(cantus, song, data):
    env = dict(os.environ, CANTUS_DATA_DIR=data)
    return ended_at_exit(
        subprocess.Popen([cantus, "--play", "--window", "1280", "800", song], env=env))


def screenshot(ws, source, width=320):
    r = ws.call("GetSourceScreenshot", {"sourceName": source, "imageFormat": "png",
                                         "imageWidth": width})
    data = base64.b64decode(r["imageData"].split(",", 1)[1])
    return data, png_rgba(data)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cantus", required=True)
    ap.add_argument("--obs", required=True)
    ap.add_argument("--song", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--minutes", type=float, default=60.0)
    ap.add_argument("--baseline-minutes", type=float, default=5.0)
    ap.add_argument("--every", type=float, default=10.0)
    ap.add_argument("--pos", type=float, nargs=2, default=None,
                    help="the window's place in Cantus's points (default: half-way across, a third down)")
    ap.add_argument("--baselines", nargs="*", default=[],
                    help="label=song.cantus: the main window playing each, companion closed")
    ap.add_argument("--baseline-minimise", action="store_true",
                    help="minimise the main window during the baselines too")
    ap.add_argument("--minimise", action="store_true",
                    help="minimise Cantus's main window during the stage phase")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    sw, sh = screen_size()
    # In the application's points (the system's scale and Cantus's zoom both apply): half
    # way across and a third down lands on the screen at any scale up to about 2.
    pos = list(a.pos) if a.pos else [round(sw * 0.5), round(sh * 0.3)]

    samples = open(os.path.join(a.out, "samples.csv"), "w", newline="")
    sw_csv = csv.writer(samples)
    sw_csv.writerow(["phase", "t_s", "cpu_s", "cpu_pct_core", "working_set_mb", "private_mb"])

    def sample_loop(phase, pid, minutes, each=None):
        t0 = time.time()
        last = None
        while time.time() - t0 < minutes * 60:
            cpu, wset, priv = process_sample(pid)
            now = time.time()
            pct = ""
            if last:
                pct = round(100.0 * (cpu - last[1]) / max(now - last[0], 1e-3), 2)
            sw_csv.writerow([phase, round(now - t0, 1), round(cpu, 2), pct,
                             round(wset / 2**20, 1), round(priv / 2**20, 1)])
            samples.flush()
            last = (now, cpu)
            if each:
                each(now - t0)
            time.sleep(max(0.0, a.every - (time.time() - now)))

    # Baselines: a song playing in the main window, no companion — the plain song by
    # default, or each `label=song` given (with a dancer, with it hidden…).
    runs = [b.split("=", 1) for b in a.baselines] or [["baseline", a.song]]
    if a.baseline_minutes > 0:
        for label, song in runs:
            p = start_cantus(a.cantus, song, cantus_data(a.out, label, False, pos))
            time.sleep(15)
            if a.baseline_minimise:
                minimise_main_window(p.pid)
                time.sleep(3)
            sample_loop(label, p.pid, a.baseline_minutes)
            p.terminate()
            p.wait(30)
    if a.minutes <= 0:
        samples.close()
        summarise(a, {}, {"checks": 0}, [r[0] for r in runs])
        return
    prepare_obs(a.obs)
    obs = start_obs(a.obs)
    ws = connect()
    cantus = start_cantus(a.cantus, a.song, cantus_data(a.out, "stage", True, pos))
    time.sleep(15)
    if a.minimise:
        minimise_main_window(cantus.pid)
        time.sleep(3)

    scene = "Cantus stage test"
    try:
        ws.call("CreateScene", {"sceneName": scene})
    except RuntimeError:
        pass
    ws.call("SetCurrentProgramScene", {"sceneName": scene})
    window = f"{TITLE}:Window Class:cantus.exe"
    sources = {
        # Multi-adapter compatibility (shared memory): on a machine where one GPU draws
        # Cantus and another runs OBS, a shared texture cannot be opened across them.
        "Game Capture (alpha)": ("game_capture", {"capture_mode": "window", "window": window,
                                                  "allow_transparency": True, "priority": 2,
                                                  "force_shmem": True}),
        "Window Capture WGC": ("window_capture", {"window": window, "method": 2,
                                                   "priority": 2}),
        "Window Capture BitBlt": ("window_capture", {"window": window, "method": 1,
                                                      "priority": 2}),
    }
    for name, (kind, settings) in sources.items():
        try:
            ws.call("CreateInput", {"sceneName": scene, "inputName": name, "inputKind": kind,
                                    "inputSettings": settings, "sceneItemEnabled": True})
        except RuntimeError as e:
            print("source", name, e, file=sys.stderr)
    time.sleep(8)

    methods = {}
    for name in sources:
        try:
            raw, img = screenshot(ws, name)
            with open(os.path.join(a.out, f"first-{name.replace(' ', '_')}.png"), "wb") as f:
                f.write(raw)
            methods[name] = {"size": img[:2], "corners_transparent": corners_clear(img),
                             "opaque_pixels": opaque_count(img), "soft_pixels": soft_count(img)}
        except Exception as e:  # a method that captures nothing says so
            methods[name] = {"error": str(e)}
    print(json.dumps(methods, indent=1))
    # The capture that keeps the alpha of Cantus's keyed window is Windows Graphics
    # Capture (measured: `methods` above); the others are turned off for the hour.
    main_source = "Window Capture WGC"
    for name in [n for n in sources if n != main_source]:
        try:
            items = ws.call("GetSceneItemId", {"sceneName": scene, "sourceName": name})
            ws.call("SetSceneItemEnabled", {"sceneName": scene,
                                            "sceneItemId": items["sceneItemId"],
                                            "sceneItemEnabled": False})
        except RuntimeError:
            pass

    checks = open(os.path.join(a.out, "checks.csv"), "w", newline="")
    ck = csv.writer(checks)
    ck.writerow(["t_s", "corners_transparent", "opaque_pixels", "soft_pixels",
                 "moved_share_max", "frames_distinct"])
    totals = {"checks": 0, "clear": 0, "moving": 0, "drawn": 0, "soft": 0}
    kept = [0]

    def check(t):
        burst, digests = [], set()
        for _ in range(4):
            try:
                raw, img = screenshot(ws, main_source)
            except RuntimeError as e:
                print(f"{t:.0f} s: no screenshot ({e})", file=sys.stderr)
                totals["checks"] += 1
                ck.writerow([round(t, 1), "", 0, 0, 0, 0])
                return
            burst.append(img)
            digests.add(hashlib.sha1(img[2]).hexdigest())
            if kept[0] < 6 and len(burst) == 1 and int(t) % 600 < a.every:
                with open(os.path.join(a.out, f"t{int(t):05d}.png"), "wb") as f:
                    f.write(raw)
                kept[0] += 1
            time.sleep(1 / 6)
        clear = all(corners_clear(i) for i in burst)
        opaque = opaque_count(burst[0])
        soft = soft_count(burst[0])
        moved = max(changed_share(burst[i], burst[i + 1]) for i in range(len(burst) - 1))
        ck.writerow([round(t, 1), clear, opaque, soft, round(moved, 4), len(digests)])
        totals["soft"] += int(soft > 0)
        checks.flush()
        totals["checks"] += 1
        totals["clear"] += int(clear)
        totals["drawn"] += int(opaque > 500)
        totals["moving"] += int(len(digests) > 1 and moved > 0.002)

    try:
        sample_loop("stage", cantus.pid, a.minutes, check)
    finally:
        cantus.terminate()
        obs.terminate()
    samples.close()
    checks.close()

    summarise(a, methods, totals, [r[0] for r in runs])


def summarise(a, methods, totals, labels):
    rows = list(csv.DictReader(open(os.path.join(a.out, "samples.csv"))))

    def stats(phase):
        pct = [float(r["cpu_pct_core"]) for r in rows if r["phase"] == phase and r["cpu_pct_core"]]
        ws_mb = [float(r["working_set_mb"]) for r in rows if r["phase"] == phase]
        pv = [float(r["private_mb"]) for r in rows if r["phase"] == phase]
        if not pct:
            return None
        return {"cpu_pct_core_mean": round(sum(pct) / len(pct), 2), "cpu_pct_core_max": max(pct),
                "working_set_mb_first": ws_mb[0], "working_set_mb_last": ws_mb[-1],
                "working_set_mb_max": max(ws_mb), "private_mb_first": pv[0],
                "private_mb_last": pv[-1], "samples": len(pct)}

    summary = {"capture_methods": methods, "stage": stats("stage"), "checks": totals,
               "minutes": a.minutes}
    for label in labels:
        summary[label] = stats(label)
    with open(os.path.join(a.out, "summary.json"), "w") as f:
        json.dump(summary, f, indent=1)
    print(json.dumps(summary, indent=1))
    n = totals.get("checks", 0)
    ok = n == 0 or (totals["clear"] == n and totals["drawn"] == n
                    and totals["moving"] >= 0.9 * n)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
