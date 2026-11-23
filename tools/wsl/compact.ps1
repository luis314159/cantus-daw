<#
.SYNOPSIS
  Shrinks the WSL disk image (ext4.vhdx) to what it holds — run as administrator.

.DESCRIPTION
  A WSL 2 disk grows as Linux writes and never shrinks by itself: deleting files inside frees
  space for Linux but not for Windows. On this machine the image lives on C:, beside the
  paging file, and a Linux suite grew it from 123 to 135 GB with 72 GB in use inside, until C:
  had 1.6 GB left (round 42, 2026-11-22; tools/wsl/guarded.sh stopped it). diskpart's
  `compact vdisk` gives the unused blocks back: 146 -> 19 GB on 2026-11-13.

  Steps: shuts WSL down, clears the image's sparse flag (diskpart compacts a non-sparse
  image; a sparse one plus fstrim gave back only 1.5 GB), runs diskpart (select, attach
  read-only, compact, detach), and prints the image's size and C:'s free space before and
  after. Delete what Linux no longer needs first, inside WSL (the CI clone's target:
  `wsl -d Ubuntu-24.04 -- rm -rf ~/cantus-ci/target`) — compacting only returns free space.

  Needs administrator rights (diskpart): right-click PowerShell, "Run as administrator", then
    powershell -ExecutionPolicy Bypass -File F:\projects\cantus\tools\wsl\compact.ps1
  Nothing inside Linux is lost; WSL starts again on its next use.

.PARAMETER Distro
  The distribution whose image to compact (default Ubuntu-24.04).
#>
param([string]$Distro = "Ubuntu-24.04")

$ErrorActionPreference = "Stop"
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { throw "Run this as administrator (diskpart needs it)." }

$key = Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Lxss" |
    Where-Object { (Get-ItemProperty $_.PSPath).DistributionName -eq $Distro } | Select-Object -First 1
if (-not $key) { throw "No WSL distribution named $Distro." }
$base = (Get-ItemProperty $key.PSPath).BasePath -replace '^\\\\\?\\', ''
$vhdx = Join-Path $base "ext4.vhdx"
if (-not (Test-Path $vhdx)) { throw "No disk image at $vhdx." }

function Show($when) {
    $size = (Get-Item $vhdx).Length / 1GB
    $drive = (Split-Path $vhdx -Qualifier).TrimEnd(':')
    $free = (Get-PSDrive $drive).Free / 1GB
    Write-Host ("{0}: image {1:N1} GB, {2}: {3:N1} GB free" -f $when, $size, $drive, $free)
}

Show "before"
wsl --shutdown
Start-Sleep -Seconds 3
# diskpart compacts a non-sparse image; WSL may have made it sparse.
wsl --manage $Distro --set-sparse false 2>$null | Out-Null

$script = Join-Path $env:TEMP "cantus-compact-vhdx.txt"
@(
    "select vdisk file=`"$vhdx`"",
    "attach vdisk readonly",
    "compact vdisk",
    "detach vdisk"
) | Set-Content -Encoding ascii $script
try {
    diskpart /s $script
    if ($LASTEXITCODE -ne 0) { throw "diskpart failed ($LASTEXITCODE)." }
} finally {
    Remove-Item $script -ErrorAction SilentlyContinue
}
Show "after"
