<#
.SYNOPSIS
  Where a program spends its CPU after it starts, sampled from outside (Sampler.cs).

.DESCRIPTION
  Starts -Exe (with -Arguments and the environment it inherits), samples every thread that
  used CPU every -PeriodMs for -Seconds, then writes one line per sample to -Out:
  "<ms since spawn>\t<thread id>\t<thread name>\t<innermost frame>;<caller>;…".
  `profile_report.py` folds the file into a per-second, per-thread, per-function report.
  No administrator rights; the program's PDB (beside it) gives the names.

.EXAMPLE
  $env:CANTUS_DATA_DIR = "F:\tmp\data"; ./profile.ps1 -Exe target\release\cantus.exe -Seconds 45 -Out cantus.samples
#>
param(
  [Parameter(Mandatory)] [string] $Exe,
  [string] $Arguments = "",
  [double] $Seconds = 45,
  [int] $PeriodMs = 5,
  [Parameter(Mandatory)] [string] $Out,
  [string] $Stderr = "",
  [switch] $Every
)
$ErrorActionPreference = "Stop"
Add-Type -Path (Join-Path $PSScriptRoot "Sampler.cs")
$exeFull = (Resolve-Path $Exe).Path
$sym = Split-Path $exeFull
$args2 = @{ FilePath = $exeFull; PassThru = $true }
if ($Arguments) { $args2.ArgumentList = $Arguments }
if ($Stderr) { $args2.RedirectStandardError = $Stderr }
$p = Start-Process @args2
$result = [Sampler]::Run($p.Id, $Seconds, $PeriodMs, [IO.Path]::GetFullPath($Out), $sym, $Every.IsPresent)
Write-Output $result
if (-not $p.HasExited) {
  $p.CloseMainWindow() | Out-Null
  if (-not $p.WaitForExit(20000)) { $p.Kill($true) }
}
