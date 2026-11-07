<# Opens FL Studio on a project, maximises it, and captures its window at physical resolution
   after each key/click sequence (round 37's reference shots; DPI-aware). Example:
   pwsh -File tools/look/shots_of_window.ps1 -Project "<demo>.flp" -OutDir <dir> -KeyList "F12+F9,F12+F6"
   Tokens: F5..F12, CLICK:x:y, DCLICK:x:y, RCLICK:x:y, ESC; "+" chains, "," separates shots.
   The same Shot function captures any window; adapt the exe and title for Cantus. #>
param([string]$Project, [string]$OutDir, [string]$KeyList = "")
$Keys = if ($KeyList) { $KeyList.Split(",") } else { @() }
$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public static class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  public static IntPtr Find(uint pid, string title) {
    IntPtr f = IntPtr.Zero;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true;
      var sb = new StringBuilder(512); GetWindowText(h, sb, 512);
      if (sb.ToString().Contains(title)) { f = h; return false; } return true; }, IntPtr.Zero);
    return f;
  }
}
"@
[void][W]::SetProcessDPIAware()
function Shot($h, $name) {
  $r = New-Object W+RECT; [void][W]::GetWindowRect($h, [ref]$r)
  $w = $r.R - $r.L; $ht = $r.B - $r.T
  $bmp = New-Object System.Drawing.Bitmap $w, $ht
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc(); [void][W]::PrintWindow($h, $dc, 2); $g.ReleaseHdc($dc); $g.Dispose()
  $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
  Write-Output "$name ${w}x$ht"
}
$fl = "C:\Program Files\Image-Line\FL Studio 2024\FL64.exe"
$p = Start-Process -FilePath $fl -ArgumentList "`"$Project`"" -PassThru
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 120 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $h = [W]::Find([uint32]$p.Id, "FL Studio") }
if ($h -eq [IntPtr]::Zero) { throw "no FL window" }
Start-Sleep 12
[void][W]::ShowWindow($h, 3); Start-Sleep 2
$ws = New-Object -ComObject WScript.Shell
[void]$ws.AppActivate($p.Id); [void][W]::SetForegroundWindow($h); Start-Sleep 1
Shot $h "00-open"
$vk = @{ F5 = 0x74; F6 = 0x75; F7 = 0x76; F8 = 0x77; F9 = 0x78; F12 = 0x7B; F10 = 0x79 }
foreach ($seq in $Keys) {
  foreach ($k in $seq.Split("+")) {
    [void]$ws.AppActivate($p.Id); [void][W]::SetForegroundWindow($h); Start-Sleep -Milliseconds 400
    if ($k -like "CLICK:*" -or $k -like "DCLICK:*" -or $k -like "RCLICK:*") {
      $a = $k.Split(":"); [void][W]::SetCursorPos([int]$a[1], [int]$a[2]); Start-Sleep -Milliseconds 200
      $n = if ($a[0] -eq "DCLICK") { 2 } else { 1 }; $d = if ($a[0] -eq "RCLICK") { 8 } else { 2 }; $u = $d * 2
      for ($j = 0; $j -lt $n; $j++) { [W]::mouse_event($d,0,0,0,[UIntPtr]::Zero); [W]::mouse_event($u,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 80 }
      Start-Sleep 2; continue
    }
    if ($k -eq "ESC") { [W]::keybd_event(0x1B,0,0,[UIntPtr]::Zero); [W]::keybd_event(0x1B,0,2,[UIntPtr]::Zero); Start-Sleep 1; continue }
    $alt = $k -like "ALT_*"; $kk = $k -replace "^ALT_", ""
    if ($alt) { [W]::keybd_event(0x12,0,0,[UIntPtr]::Zero) }
    [W]::keybd_event([byte]$vk[$kk],0,0,[UIntPtr]::Zero); [W]::keybd_event([byte]$vk[$kk],0,2,[UIntPtr]::Zero)
    if ($alt) { [W]::keybd_event(0x12,0,2,[UIntPtr]::Zero) }
    Start-Sleep 2
  }
  Shot $h ($seq -replace "[+:]", "-")
}
Write-Output "pid $($p.Id)"
