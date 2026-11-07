<#
.SYNOPSIS
  Start-up time and memory of any DAW, measured the same way for each (docs/scorecard/README.md §Start-up).

.DESCRIPTION
  Starts the program, then watches it from outside — no probe inside it, so Cantus, FL Studio,
  REAPER or any other program is measured by the same rule:

    window   spawn → the first visible top-level window of the process whose title matches
             -Title (the main window, not a splash) answers a message (WM_NULL within 100 ms)
    settled  spawn → the process has used under -IdleShare of one core (10 % by default: a
             DAW's audio engine keeps a little CPU busy at rest) for 1.5 s in a row after its
             main window appeared (the program has stopped loading)
    memory   the private bytes and working set at "settled"

  The first start is unmeasured (it puts the program in the file cache); then -Runs starts,
  each closed politely (WM_CLOSE, then killed after 20 s). Prints one JSON line per start and
  a summary, and writes them to -Out.

  Before a run: mains power, the power plan at maximum performance, nothing else running.
  Close any "what's new" or registration dialog once by hand before measuring a program, so
  every measured start is the same start.

.EXAMPLE
  ./startup.ps1 -Exe "C:\Program Files\Image-Line\FL Studio 2024\FL64.exe" -Title "FL Studio" -Name fl -Out fl.json
  ./startup.ps1 -Exe F:\projects\cantus\target\release\cantus.exe -Title "Cantus" -Name cantus -Out cantus.json
#>
param(
  [Parameter(Mandatory)] [string] $Exe,
  [Parameter(Mandatory)] [string] $Title,
  [Parameter(Mandatory)] [string] $Name,
  [string] $Arguments = "",
  [int] $Runs = 5,
  [int] $TimeoutSeconds = 120,
  [double] $IdleShare = 0.10,
  [string] $Out = ""
)

$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ScoreWin {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint msg, IntPtr w, IntPtr l, uint flags, uint ms, out IntPtr result);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);

  // The first visible, unowned top-level window of `pid` whose title contains `title`.
  public static IntPtr MainWindow(uint pid, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h) || GetWindow(h, 4) != IntPtr.Zero) return true;
      var sb = new StringBuilder(512); GetWindowText(h, sb, 512);
      if (sb.ToString().IndexOf(title, StringComparison.OrdinalIgnoreCase) >= 0) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }

  // Whether the window answers WM_NULL within `ms` (SMTO_ABORTIFHUNG).
  public static bool Answers(IntPtr h, uint ms) {
    IntPtr r; return SendMessageTimeout(h, 0, IntPtr.Zero, IntPtr.Zero, 2, ms, out r) != IntPtr.Zero;
  }
}
"@

function Measure-Start([bool] $measured) {
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $p = if ($Arguments) { Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru } else { Start-Process -FilePath $Exe -PassThru }
  $window = $null; $settled = $null; $h = [IntPtr]::Zero
  $cores = [Environment]::ProcessorCount
  $quietSince = $null; $lastCpu = 0.0; $lastT = 0.0
  while ($sw.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
    Start-Sleep -Milliseconds 10
    $p.Refresh()
    if ($p.HasExited) { break }
    $t = $sw.Elapsed.TotalMilliseconds
    if (-not $window) {
      $h = [ScoreWin]::MainWindow([uint32]$p.Id, $Title)
      if ($h -ne [IntPtr]::Zero -and [ScoreWin]::Answers($h, 100)) {
        $window = $sw.Elapsed.TotalMilliseconds
        $lastCpu = $p.TotalProcessorTime.TotalMilliseconds; $lastT = $t
      }
      continue
    }
    # After the window: sample the CPU every 250 ms.
    if ($t - $lastT -lt 250) { continue }
    $cpu = $p.TotalProcessorTime.TotalMilliseconds
    $share = ($cpu - $lastCpu) / ($t - $lastT)   # in cores
    $lastCpu = $cpu; $lastT = $t
    if ($share -lt $IdleShare) {
      if (-not $quietSince) { $quietSince = $t - 250 }
      if ($t - $quietSince -ge 1500) { $settled = $quietSince; break }
    } else { $quietSince = $null }
  }
  $p.Refresh()
  $mem = if (-not $p.HasExited) { @{ private_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); working_mb = [math]::Round($p.WorkingSet64 / 1MB, 1) } } else { @{ private_mb = $null; working_mb = $null } }
  # Close politely, then make sure.
  if (-not $p.HasExited -and $h -ne [IntPtr]::Zero) { [void][ScoreWin]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
  if (-not $p.WaitForExit(20000)) { try { $p.Kill($true) } catch { } ; [void]$p.WaitForExit(5000) }
  Start-Sleep -Seconds 2
  [pscustomobject]@{
    name = $Name; measured = $measured
    window_ms = if ($window) { [math]::Round($window) } else { $null }
    settled_ms = if ($settled) { [math]::Round($settled) } else { $null }
    private_mb = $mem.private_mb; working_mb = $mem.working_mb
  }
}

$version = (Get-Item $Exe).VersionInfo.ProductVersion
$rows = @()
[void](Measure-Start $false)
for ($i = 0; $i -lt $Runs; $i++) {
  $r = Measure-Start $true
  $r | ConvertTo-Json -Compress | Write-Output
  $rows += $r
}
function Med($xs) { $s = @($xs | Where-Object { $_ -ne $null } | Sort-Object); if ($s.Count -eq 0) { return $null }; $s[[int][math]::Floor(($s.Count - 1) / 2)] }
$summary = [pscustomobject]@{
  name = $Name; exe = $Exe; version = $version; runs = $Runs; idle_share = $IdleShare
  date = (Get-Date).ToString("yyyy-MM-dd HH:mm")
  cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
  window_ms_median = Med ($rows.window_ms); window_ms_best = ($rows.window_ms | Measure-Object -Minimum).Minimum
  settled_ms_median = Med ($rows.settled_ms); settled_ms_best = ($rows.settled_ms | Measure-Object -Minimum).Minimum
  private_mb_median = Med ($rows.private_mb); working_mb_median = Med ($rows.working_mb)
  starts = $rows
}
$summary | Select-Object name, version, window_ms_median, window_ms_best, settled_ms_median, private_mb_median, working_mb_median | Format-List | Out-String | Write-Output
if ($Out) { $summary | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 $Out }
