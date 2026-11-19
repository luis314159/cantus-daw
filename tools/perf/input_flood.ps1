# Input under a real mouse's rate (third_party/README.md, eframe patch 1; 2026-11-18).
#
#   pwsh tools/perf/input_flood.ps1 -Exe <cantus.exe> [-Song <file.cantus>] [-Seconds 3]
#
# A real mouse sends ~1 000 moves a second. This starts Cantus with a fresh data folder and
# the repaint log (CANTUS_REPAINT_LOG), brings its window to the front, sends relative moves
# at ~1 kHz with the system's own input path (mouse_event), and reads from the log how many
# input events each second's frames handled and how long input kept arriving after the
# moves stopped. A healthy build handles ~1 000+ a second and stops within a frame or two;
# beta.15's first build handled 48 a second and kept going ~9 s (each event waited a vsync).
#
# It moves the real cursor: run it with nothing else on the desktop needing the mouse, and
# no other Cantus open (it refuses then). Windows only.
param(
    [Parameter(Mandatory)] [string]$Exe,
    [string]$Song = "",
    [int]$Seconds = 3
)
$ErrorActionPreference = 'Stop'
if (Get-Process cantus, 'cantus-x86-64-v*' -ErrorAction SilentlyContinue) {
    throw 'close every Cantus window first: the moves would go to it'
}
$d = Join-Path ([IO.Path]::GetTempPath()) ("cantus-input-flood-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force "$d\la" | Out-Null
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Diagnostics;
public static class InputFlood {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, IntPtr e);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  public static bool Front(IntPtr h) {
    keybd_event(0x12, 0, 0, IntPtr.Zero); keybd_event(0x12, 0, 2, IntPtr.Zero);
    ShowWindow(h, 3); SetForegroundWindow(h); System.Threading.Thread.Sleep(300);
    return GetForegroundWindow() == h;
  }
  public static int Flood(int ms) {
    var sw = Stopwatch.StartNew(); int n = 0; long next = 0;
    while (sw.ElapsedMilliseconds < ms) {
      if (sw.ElapsedTicks >= next) {
        int dx = ((n / 200) % 2 == 0) ? 2 : -2;
        mouse_event(0x0001, dx, (n % 2 == 0) ? 1 : -1, 0, IntPtr.Zero);
        n++; next += Stopwatch.Frequency / 1000;
      }
    }
    return n;
  }
}
"@
[void][InputFlood]::SetProcessDPIAware()
$psi = New-Object System.Diagnostics.ProcessStartInfo $Exe
$psi.UseShellExecute = $false
if ($Song) { $psi.Arguments = "`"$Song`"" }
$psi.EnvironmentVariables['LOCALAPPDATA'] = "$d\la"
$psi.EnvironmentVariables['CANTUS_REPAINT_LOG'] = "$d\repaint.log"
$p = [System.Diagnostics.Process]::Start($psi)
try {
    Start-Sleep -Seconds 10
    $p.Refresh()
    if (-not [InputFlood]::Front($p.MainWindowHandle)) { throw 'the window did not come to the front: nothing sent' }
    [void][InputFlood]::SetCursorPos(1100, 650)
    Start-Sleep -Milliseconds 800
    $before = ((Get-Date) - $p.StartTime).TotalSeconds
    $n = [InputFlood]::Flood($Seconds * 1000)
    $sent = ((Get-Date) - $p.StartTime).TotalSeconds - $before
    Start-Sleep -Seconds 8
} finally {
    if (-not $p.HasExited) { $p.Kill() }
}
Start-Sleep -Milliseconds 500
$rows = Get-Content "$d\repaint.log" | ForEach-Object {
    $c = $_ -split "`t"
    [pscustomobject]@{ t = [double]$c[0]; ev = [int]($c[1] -replace 'ev=', '') }
}
"Sent $n moves over {0:F2} s." -f $sent
$rows | Group-Object { [math]::Floor($_.t) } | ForEach-Object {
    "{0,3} s: {1,4} frames, {2,5} input events" -f $_.Name, $_.Count, ($_.Group | Measure-Object ev -Sum).Sum
}
$busy = $rows | Where-Object { $_.ev -gt 0 } | Group-Object { [math]::Floor($_.t) } |
    Where-Object { ($_.Group | Measure-Object ev -Sum).Sum -gt 20 }
if ($busy) {
    $from = [double]($busy | Select-Object -First 1).Name
    $to = [double]($busy | Select-Object -Last 1).Name + 1
    "Input handled over ~{0:F0} s for {1:F1} s of moves (more than a second over means a backlog)." -f ($to - $from), $sent
}
Remove-Item $d -Recurse -Force -ErrorAction SilentlyContinue
