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
             main window appeared (the program has stopped loading); -AlsoShare gives the
             same rule at a second threshold, read from the same samples
    memory   the private bytes and working set at "settled"
    at rest  the share of one core the process used over -RestFor seconds starting -RestAt
             seconds after spawn (30 and 10 by default; -RestAt 0 skips it), and the private
             bytes then: what the program costs while it sits open and nothing is done. Two
             readings: the thread times (counted by Windows in 15.6 ms ticks — ±0.16 % of a
             core in 10 s) and the cycles (QueryProcessCycleTime over the rated clock: exact)

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
  [double] $AlsoShare = 0,
  [int] $RestAt = 30,
  [int] $RestFor = 10,
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
  [DllImport("kernel32.dll")] public static extern bool QueryProcessCycleTime(IntPtr h, out ulong cycles);
  [DllImport("kernel32.dll")] static extern bool GetSystemTimes(out long idle, out long kernel, out long user);
  // The machine's busy time so far, all cores, in milliseconds (kernel + user − idle).
  public static double BusyMs() { long i, k, u; return GetSystemTimes(out i, out k, out u) ? (k + u - i) / 10000.0 : 0; }
  // Every thread's cycles so far: exact, where the thread times are counted in 15.6 ms ticks.
  public static ulong Cycles(IntPtr h) { ulong c; return QueryProcessCycleTime(h, out c) ? c : 0; }

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
  $busy0 = [ScoreWin]::BusyMs()
  $p = if ($Arguments) { Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru } else { Start-Process -FilePath $Exe -PassThru }
  $window = $null; $settled = $null; $h = [IntPtr]::Zero
  $cores = [Environment]::ProcessorCount
  $quietSince = $null; $lastCpu = 0.0; $lastT = 0.0
  # The same rule at a second threshold, from the same samples (-AlsoShare; 0 = none).
  $quiet2 = $null; $settled2 = $null
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
    if (-not $settled) {
      if ($share -lt $IdleShare) {
        if (-not $quietSince) { $quietSince = $t - 250 }
        if ($t - $quietSince -ge 1500) { $settled = $quietSince }
      } else { $quietSince = $null }
    }
    if ($AlsoShare -gt 0 -and -not $settled2) {
      if ($share -lt $AlsoShare) {
        if (-not $quiet2) { $quiet2 = $t - 250 }
        if ($t - $quiet2 -ge 1500) { $settled2 = $quiet2 }
      } else { $quiet2 = $null }
    }
    if ($settled -and ($AlsoShare -le 0 -or $settled2)) { break }
  }
  $p.Refresh()
  $memSettled = if (-not $p.HasExited) { @{ private_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1); working_mb = [math]::Round($p.WorkingSet64 / 1MB, 1) } } else { @{ private_mb = $null; working_mb = $null } }
  # At rest: the share of a core over -RestFor s from -RestAt s after spawn.
  $rest = $null; $restMem = $null; $restCycles = $null
  if ($RestAt -gt 0 -and -not $p.HasExited) {
    while ($sw.Elapsed.TotalSeconds -lt $RestAt) { Start-Sleep -Milliseconds 100 }
    $p.Refresh(); $c0 = $p.TotalProcessorTime.TotalMilliseconds; $t0 = $sw.Elapsed.TotalMilliseconds
    $y0 = [ScoreWin]::Cycles($p.Handle)
    Start-Sleep -Seconds $RestFor
    $p.Refresh()
    if (-not $p.HasExited) {
      $y1 = [ScoreWin]::Cycles($p.Handle); $t1 = $sw.Elapsed.TotalMilliseconds
      $restCycles = ($y1 - $y0) / ($TscMHz * 1000.0 * ($t1 - $t0))
      $rest = ($p.TotalProcessorTime.TotalMilliseconds - $c0) / ($sw.Elapsed.TotalMilliseconds - $t0)
      $restMem = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
    }
  }
  $mem = $memSettled
  # What everything else on the machine did meanwhile, in cores: the conditions of this start.
  $p.Refresh()
  $ownMs = if (-not $p.HasExited) { $p.TotalProcessorTime.TotalMilliseconds } else { 0 }
  $others = (([ScoreWin]::BusyMs() - $busy0) - $ownMs) / $sw.Elapsed.TotalMilliseconds
  # Close politely, then make sure.
  if (-not $p.HasExited -and $h -ne [IntPtr]::Zero) { [void][ScoreWin]::PostMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
  if (-not $p.WaitForExit(20000)) { try { $p.Kill($true) } catch { } ; [void]$p.WaitForExit(5000) }
  Start-Sleep -Seconds 2
  [pscustomobject]@{
    name = $Name; measured = $measured
    window_ms = if ($window) { [math]::Round($window) } else { $null }
    settled_ms = if ($settled) { [math]::Round($settled) } else { $null }
    settled_also_ms = if ($settled2) { [math]::Round($settled2) } else { $null }
    private_mb = $mem.private_mb; working_mb = $mem.working_mb
    rest_share = if ($rest -ne $null) { [math]::Round($rest, 4) } else { $null }
    rest_share_cycles = if ($restCycles -ne $null) { [math]::Round($restCycles, 4) } else { $null }
    rest_private_mb = $restMem
    others_cores = [math]::Round($others, 2)
  }
}

$version = (Get-Item $Exe).VersionInfo.ProductVersion
# The processor's rated clock: the cycle counter's rate (an invariant TSC), for at rest in cycles.
$TscMHz = Get-ItemPropertyValue -Path "HKLM:\HARDWARE\DESCRIPTION\System\CentralProcessor\0" -Name "~MHz"
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
  also_share = $AlsoShare; settled_also_ms_median = Med ($rows.settled_also_ms)
  private_mb_median = Med ($rows.private_mb); working_mb_median = Med ($rows.working_mb)
  rest_at = $RestAt; rest_for = $RestFor
  rest_share_median = Med ($rows.rest_share); rest_private_mb_median = Med ($rows.rest_private_mb)
  rest_share_cycles_median = Med ($rows.rest_share_cycles); tsc_mhz = $TscMHz
  starts = $rows
}
$summary | Select-Object name, version, window_ms_median, window_ms_best, settled_ms_median, settled_also_ms_median, private_mb_median, working_mb_median, rest_share_median, rest_share_cycles_median, rest_private_mb_median | Format-List | Out-String | Write-Output
if ($Out) { $summary | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 $Out }
