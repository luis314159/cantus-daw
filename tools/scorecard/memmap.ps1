<#
.SYNOPSIS
  What a running program's private memory is made of, read from outside (MemMap.cs; with
  -Detail, MemDetail.cs: every byte of the commit charge under an owner — each heap, each
  thread's stack, the modules' writable pages, and the rest by the module that points at it).

.EXAMPLE
  ./memmap.ps1 -Exe target\release\cantus.exe -After 20      # start it, wait, report, close
  ./memmap.ps1 -ProcessId 1234                                 # a program already running
  ./memmap.ps1 -Exe target\release\cantus.exe -Arguments song.cantus -DataDir F:\tmp\data -Detail
#>
param(
  [string] $Exe = "",
  [string] $Arguments = "",
  [int] $ProcessId = 0,
  [int] $After = 20,
  [int] $Top = 25,
  # A fresh data folder for the start (CANTUS_DATA_DIR), set up as a configured copy is:
  # the audio set-up done, and the plug-in cache copied from -Plugins when it is given.
  [string] $DataDir = "",
  [string] $Plugins = "",
  [switch] $Detail,
  # A census build's file (CANTUS_HEAP_CENSUS, ADR 0120 §1): set for the start, read when
  # mapped, its addresses named from outside (CensusNames.cs), written beside it as .named.txt.
  [string] $Census = "",
  # Leave the program running (to map it again, or to look at it).
  [switch] $Keep
)
$ErrorActionPreference = "Stop"
Add-Type -Path (Join-Path $PSScriptRoot "MemMap.cs")
if ($Detail) { Add-Type -Path (Join-Path $PSScriptRoot "MemDetail.cs") }
$started = $null
if ($Exe) {
  if ($DataDir) {
    Remove-Item -Recurse -Force $DataDir -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $DataDir | Out-Null
    Set-Content -Path (Join-Path $DataDir "settings.json") -Value '{"audio_setup":{"setup_done":true}}' -NoNewline
    if ($Plugins) { Copy-Item $Plugins (Join-Path $DataDir "plugins.json") }
    $env:CANTUS_DATA_DIR = $DataDir
  }
  if ($Census) { Remove-Item $Census -ErrorAction SilentlyContinue; $env:CANTUS_HEAP_CENSUS = $Census }
  $a = @{ FilePath = (Resolve-Path $Exe).Path; PassThru = $true }
  if ($Arguments) { $a.ArgumentList = $Arguments }
  $started = Start-Process @a
  $ProcessId = $started.Id
  Start-Sleep -Seconds $After
}
$p = Get-Process -Id $ProcessId
"private bytes {0:F1} MB, working set {1:F1} MB, threads {2}" -f ($p.PrivateMemorySize64 / 1MB), ($p.WorkingSet64 / 1MB), $p.Threads.Count
if ($Detail) {
  [MemDetail]::Report($ProcessId, [int[]]($p.Threads | ForEach-Object { $_.Id }), $Top)
} else {
  [MemMap]::Report($ProcessId, $Top)
}
if ($Census) {
  Add-Type -Path (Join-Path $PSScriptRoot "CensusNames.cs")
  $deadline = (Get-Date).AddSeconds(15)
  while (-not (Test-Path $Census) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
  $named = [CensusNames]::Name((Get-Content -Raw $Census), $ProcessId, (Split-Path -Parent $p.Path))
  $out = [IO.Path]::ChangeExtension($Census, ".named.txt")
  Set-Content -Path $out -Value $named -Encoding utf8
  "census named: $out"
}
if ($started -and -not $Keep) {
  $started.CloseMainWindow() | Out-Null
  if (-not $started.WaitForExit(20000)) { $started.Kill($true) }
}
