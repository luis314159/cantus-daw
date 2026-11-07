<# Opens the real Cantus (a release build) on a data folder of its own, maximises it, and
   captures its window at physical resolution at the interface zoom of 100, 125 and 150 %
   (Ctrl+= steps 1.0 -> 1.1 -> 1.25 -> 1.5) — round 37's check of text sharpness in the
   real renderer against the CPU shots (the finish builder, ADR 0103). DPI-aware. Example:
   pwsh -File tools/look/shots_of_cantus.ps1 -Exe <target>\release\cantus.exe -OutDir <dir> -DataDir <empty dir>
   The data folder keeps the machine's own settings, songs and audio choices out of it. #>
param([string]$Exe, [string]$OutDir, [string]$DataDir, [int]$Settle = 10)
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $OutDir | Out-Null
New-Item -ItemType Directory -Force $DataDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public static class CW {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static IntPtr Find(uint pid) {
    IntPtr f = IntPtr.Zero;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true; f = h; return false; }, IntPtr.Zero);
    return f;
  }
}
"@
[void][CW]::SetProcessDPIAware()
function Shot($h, $name) {
  $r = New-Object CW+RECT; [void][CW]::GetWindowRect($h, [ref]$r)
  $w = $r.R - $r.L; $ht = $r.B - $r.T
  $bmp = New-Object System.Drawing.Bitmap $w, $ht
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc(); [void][CW]::PrintWindow($h, $dc, 2); $g.ReleaseHdc($dc); $g.Dispose()
  $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
  Write-Output "$name ${w}x$ht"
}
function ZoomIn($n) {
  for ($i = 0; $i -lt $n; $i++) {
    [CW]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [CW]::keybd_event(0xBB,0,0,[UIntPtr]::Zero)
    [CW]::keybd_event(0xBB,0,2,[UIntPtr]::Zero); [CW]::keybd_event(0x11,0,2,[UIntPtr]::Zero)
    Start-Sleep -Milliseconds 700
  }
  Start-Sleep 2
}
$env:CANTUS_DATA_DIR = $DataDir
$p = Start-Process -FilePath $Exe -PassThru
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 90 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $h = [CW]::Find([uint32]$p.Id) }
if ($h -eq [IntPtr]::Zero) { Stop-Process -Id $p.Id -Force; throw "no Cantus window" }
Start-Sleep $Settle
[void][CW]::ShowWindow($h, 3); Start-Sleep 3
$ws = New-Object -ComObject WScript.Shell
[void]$ws.AppActivate($p.Id); [void][CW]::SetForegroundWindow($h); Start-Sleep 1
# The first start's audio setup and the verse: Escape closes them.
for ($i = 0; $i -lt 2; $i++) {
  [CW]::keybd_event(0x1B,0,0,[UIntPtr]::Zero); [CW]::keybd_event(0x1B,0,2,[UIntPtr]::Zero); Start-Sleep 1
}
Start-Sleep 2
Shot $h "real-100"
ZoomIn 2; Shot $h "real-125"
ZoomIn 1; Shot $h "real-150"
Stop-Process -Id $p.Id -Force
Write-Output "done"
