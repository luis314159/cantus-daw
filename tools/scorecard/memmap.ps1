<#
.SYNOPSIS
  What a running program's private memory is made of, read from outside (MemMap.cs).

.EXAMPLE
  ./memmap.ps1 -Exe target\release\cantus.exe -After 20      # start it, wait, report, close
  ./memmap.ps1 -ProcessId 1234                                 # a program already running
#>
param(
  [string] $Exe = "",
  [string] $Arguments = "",
  [int] $ProcessId = 0,
  [int] $After = 20,
  [int] $Top = 25
)
$ErrorActionPreference = "Stop"
Add-Type -Path (Join-Path $PSScriptRoot "MemMap.cs")
$started = $null
if ($Exe) {
  $a = @{ FilePath = (Resolve-Path $Exe).Path; PassThru = $true }
  if ($Arguments) { $a.ArgumentList = $Arguments }
  $started = Start-Process @a
  $ProcessId = $started.Id
  Start-Sleep -Seconds $After
}
$p = Get-Process -Id $ProcessId
"private bytes {0:F1} MB, working set {1:F1} MB" -f ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB)
[MemMap]::Report($ProcessId, $Top)
if ($started) {
  $started.CloseMainWindow() | Out-Null
  if (-not $started.WaitForExit(20000)) { $started.Kill($true) }
}
