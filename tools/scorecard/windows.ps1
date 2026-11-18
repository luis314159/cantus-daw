<#
.SYNOPSIS
  What a window of its own costs (ADR 0151): Cantus with 1, 2 and 3 windows, at rest and
  playing — memory, the GPU driver's memory, CPU, and each window's frames.

.DESCRIPTION
  -Data holds w1, w2 and w3 (`cargo run -p cantus-ui --example multimon_measure -- <Data>
  <width> <height> <scale>`): the same configured Cantus whose layout opens the main window
  alone, with the mixer in a window of its own, and with the piano roll in a third.
  For each, -Runs starts at rest and -Runs starts with --play (the start-up song playing).
  Each start is measured from -RestAt to -RestAt + -RestFor seconds after spawn:
    cpu       the share of one core (thread times)
    private   private bytes at the end, and the working set
    gpu       the driver's memory charged to the process (the GPU Process Memory counters:
              dedicated and shared, summed over the process's adapters)
    frames    each window's passes a second and mean pass time, from Cantus's own log
              (CANTUS_WINDOW_STATS=1 writes a line every 5 s to <data>\logs\cantus.log)
  Prints one JSON line per start and the best of the runs per case, and writes them to -Out.
  The machine's other load is printed beside each start (the share of every core the rest
  of the machine used): measure with nothing else running when the numbers matter.

.EXAMPLE
  ./windows.ps1 -Exe F:\target\debug\cantus.exe -Data C:\tmp\mm -Out windows.json
#>
param(
  [Parameter(Mandatory)] [string] $Exe,
  [Parameter(Mandatory)] [string] $Data,
  [int] $Runs = 3,
  [int] $RestAt = 30,
  [int] $RestFor = 10,
  [string] $Out = ""
)
$ErrorActionPreference = "Stop"
$cores = [Environment]::ProcessorCount

function Gpu-Mb([int] $procId) {
  $ded = 0.0; $sha = 0.0
  try {
    $set = Get-Counter -Counter "\GPU Process Memory(pid_$($procId)_*)\Dedicated Usage", "\GPU Process Memory(pid_$($procId)_*)\Shared Usage" -ErrorAction Stop
    foreach ($s in $set.CounterSamples) {
      if ($s.Path -like "*dedicated usage") { $ded += $s.CookedValue } else { $sha += $s.CookedValue }
    }
  } catch { }
  return @{ dedicated_mb = [math]::Round($ded / 1MB, 1); shared_mb = [math]::Round($sha / 1MB, 1) }
}

function Machine-Busy {
  $c = (Get-Counter "\Processor(_Total)\% Processor Time" -ErrorAction SilentlyContinue).CounterSamples
  if ($c) { return [math]::Round($c[0].CookedValue, 1) } else { return $null }
}

function One([string] $dir, [bool] $play) {
  $env:CANTUS_DATA_DIR = $dir
  $env:CANTUS_WINDOW_STATS = "1"
  $args2 = @{ FilePath = $Exe; PassThru = $true }
  if ($play) { $args2.ArgumentList = "--play" }
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process @args2
  while ($sw.Elapsed.TotalSeconds -lt $RestAt) { Start-Sleep -Milliseconds 200 }
  $busy = Machine-Busy
  $p.Refresh(); $cpu0 = $p.TotalProcessorTime.TotalMilliseconds; $t0 = $sw.Elapsed.TotalMilliseconds
  while ($sw.Elapsed.TotalSeconds -lt $RestAt + $RestFor) { Start-Sleep -Milliseconds 200 }
  $p.Refresh(); $cpu1 = $p.TotalProcessorTime.TotalMilliseconds; $t1 = $sw.Elapsed.TotalMilliseconds
  $gpu = Gpu-Mb $p.Id
  $res = [ordered]@{
    case = (Split-Path $dir -Leaf); play = $play
    cpu_core_pct = [math]::Round(100.0 * ($cpu1 - $cpu0) / ($t1 - $t0), 2)
    private_mb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
    working_mb = [math]::Round($p.WorkingSet64 / 1MB, 1)
    gpu_dedicated_mb = $gpu.dedicated_mb; gpu_shared_mb = $gpu.shared_mb
    machine_busy_pct = $busy
  }
  if (-not $p.HasExited) {
    $p.CloseMainWindow() | Out-Null
    if (-not $p.WaitForExit(20000)) { $p.Kill($true) }
  }
  Start-Sleep -Milliseconds 500
  $log = Join-Path $dir "logs\cantus.log"
  if (Test-Path $log) {
    $lines = Select-String -Path $log -Pattern "window stats over" | Select-Object -Last 2
    if ($lines) { $res.frames = ($lines[0].Line -replace '^.*window stats over ', '') }
  }
  Remove-Item Env:\CANTUS_WINDOW_STATS
  return $res
}

$all = @()
foreach ($case in "w1", "w2", "w3") {
  $dir = Join-Path $Data $case
  foreach ($play in $false, $true) {
    # One unmeasured start puts the program in the file cache.
    if ($case -eq "w1" -and -not $play) { One $dir $false | Out-Null }
    for ($i = 0; $i -lt $Runs; $i++) {
      $r = One $dir $play
      $r.run = $i + 1
      $line = $r | ConvertTo-Json -Compress
      Write-Output $line
      $all += $r
    }
  }
}
Write-Output "--- best of $Runs (lowest CPU; memory of that run) ---"
$best = @()
foreach ($g in ($all | Group-Object { "$($_.case) play=$($_.play)" })) {
  $b = $g.Group | Sort-Object cpu_core_pct | Select-Object -First 1
  $pm = ($g.Group | Measure-Object private_mb -Minimum).Minimum
  $b.private_min_mb = $pm
  $best += $b
  Write-Output ($b | ConvertTo-Json -Compress)
}
if ($Out) { @{ runs = $all; best = $best } | ConvertTo-Json -Depth 4 | Set-Content -Path $Out -Encoding utf8 }
