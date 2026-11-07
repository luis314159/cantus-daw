<# Captures the real Cantus window (a release build) set up as the README's hero shot —
   `CANTUS_SHOT_SCENE=hero` (crates/cantus-ui/src/shot_scene.rs) — in each theme asked for,
   maximised, at the screen's own resolution, DPI-aware: the same way FL Studio's reference
   shots were taken (`shots_of_window.ps1`), so the blind comparisons are captures against
   captures (round 37, the finish builder, ADR 0103). Each run uses a fresh data folder so
   the machine's own settings, songs and audio choices stay out of it. Example:
   pwsh -File tools/look/shots_of_scene.ps1 -Exe <target>\release\cantus.exe -OutDir <dir> -DataRoot <dir>
#>
param(
    [string]$Exe,
    [string]$OutDir,
    [string]$DataRoot,
    [string[]]$Themes = @("cantus_night", "cantus_paper"),
    # A folder holding Cantus's own pack archives (cantus-loops-1.zip, cantus-one-shots-1.zip):
    # the hero installs them from there and shows their sounds (none in the repository).
    [string]$Packs = "",
    [int]$Settle = 14
)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class SW {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  public static IntPtr Find(uint pid) {
    IntPtr f = IntPtr.Zero; long best = 0;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true;
      RECT r; GetWindowRect(h, out r); long a = (long)(r.R - r.L) * (r.B - r.T);
      if (a > best) { best = a; f = h; } return true; }, IntPtr.Zero);
    return f;
  }
}
"@
[void][SW]::SetProcessDPIAware()
# The client area only (no title bar, no borders), from the screen with the window brought
# to the top (PrintWindow returns black for a GPU-drawn window): what the app drew.
function Shot($h, $name) {
  [void][SW]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x0003)
  Start-Sleep 1
  $r = New-Object SW+RECT; [void][SW]::GetClientRect($h, [ref]$r)
  $p = New-Object SW+POINT; $p.X = 0; $p.Y = 0; [void][SW]::ClientToScreen($h, [ref]$p)
  $w = $r.R - $r.L; $ht = $r.B - $r.T
  $bmp = New-Object System.Drawing.Bitmap $w, $ht
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($p.X, $p.Y, 0, 0, (New-Object System.Drawing.Size $w, $ht))
  $g.Dispose()
  $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
  [void][SW]::SetWindowPos($h, [IntPtr](-2), 0, 0, 0, 0, 0x0003)
  Write-Output "$name ${w}x$ht"
}
foreach ($theme in $Themes) {
  $data = Join-Path $DataRoot $theme
  if (Test-Path $data) { Remove-Item -Recurse -Force $data }
  New-Item -ItemType Directory -Force $data | Out-Null
  $env:CANTUS_DATA_DIR = $data
  $env:CANTUS_SHOT_SCENE = "hero"
  $env:CANTUS_SHOT_THEME = $theme
  if ($Packs) { $env:CANTUS_SHOT_PACKS = $Packs }
  $p = Start-Process -FilePath $Exe -PassThru
  $h = [IntPtr]::Zero
  for ($i = 0; $i -lt 90 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $h = [SW]::Find([uint32]$p.Id) }
  if ($h -eq [IntPtr]::Zero) { Stop-Process -Id $p.Id -Force; throw "no Cantus window" }
  [void][SW]::ShowWindow($h, 3); Start-Sleep 3
  $ws = New-Object -ComObject WScript.Shell
  [void]$ws.AppActivate($p.Id); [void][SW]::SetForegroundWindow($h); Start-Sleep 1
  # The scene itself closes the first start's audio questions and the verse.
  Start-Sleep $Settle
  # Maximised, checked: the window sometimes restores its own size after the first ask.
  for ($i = 0; $i -lt 10; $i++) {
    [void][SW]::ShowWindow($h, 3); Start-Sleep 2
    $cr = New-Object SW+RECT; [void][SW]::GetClientRect($h, [ref]$cr)
    if (($cr.R - $cr.L) -ge 1900) { break }
    [void][SW]::ShowWindow($h, 9); Start-Sleep 1
  }
  # The pointer off the window (the taskbar's corner): no hover hint in the picture.
  [void][SW]::SetCursorPos(0, 4000)
  Start-Sleep 3
  Shot $h "real-$theme"
  # Where each area was (the scene writes it): the judge's crops follow the layout.
  $layout = Join-Path $data "shot-layout.json"
  if (Test-Path $layout) { Copy-Item $layout (Join-Path $OutDir "layout-$theme.json") }
  Stop-Process -Id $p.Id -Force
  Start-Sleep 2
}
Remove-Item Env:CANTUS_SHOT_SCENE, Env:CANTUS_SHOT_THEME, Env:CANTUS_DATA_DIR, Env:CANTUS_SHOT_PACKS -ErrorAction SilentlyContinue
Write-Output "done"
