<#
.SYNOPSIS
  The website's performance budget, against a local build (round 39, P3) - the same checks
  as .github/workflows/site-budget.yml.

.DESCRIPTION
  Builds web/ with trunk and pre-renders every page (unless -NoBuild), then:
    - the WebAssembly <= 250 000 bytes gzipped (web/tools/wasm_budget.py);
    - Lighthouse CI on its mobile preset, three runs, the median held to
      web/lighthouserc.json: first visit <= 1 MB, LCP <= 2.0 s, CLS <= 0.02,
      accessibility 100.
  Reports land in web/target/lhci (ignored by git). Exit code 0 only when every line passes.
  Needs Python 3, Node (npx) and Chrome or Edge; uses Edge when Chrome is not installed.

.EXAMPLE
  powershell -File tools/web-budget.ps1
  powershell -File tools/web-budget.ps1 -NoBuild
#>
param(
    [switch]$NoBuild,
    [string]$Trunk = "",
    [string]$LhciVersion = "0.15.1"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$web = Join-Path $root "web"
$dist = Join-Path $web "dist"
$rc = Join-Path $web "lighthouserc.json"
$out = Join-Path $web "target\lhci"
$run = Join-Path $web "target\lhci-run"

if (-not $NoBuild) {
    if (-not $Trunk) {
        $found = Get-Command trunk -ErrorAction SilentlyContinue
        $Trunk = if ($found) { $found.Source } else { "F:\projects\daw-r39\downloads\web\trunk\trunk.exe" }
    }
    if (-not (Test-Path $Trunk)) { throw "trunk not found: pass -Trunk <path to trunk.exe>" }
    # trunk reads NO_COLOR as a flag and refuses "1" (site.yml sets it the same way).
    $env:NO_COLOR = "false"
    Push-Location $web
    try {
        & $Trunk build --release
        if ($LASTEXITCODE -ne 0) { throw "trunk build failed" }
        # What site.yml publishes: every page pre-rendered (round 39, P1-1).
        & cargo run --features ssr -- prerender dist
        if ($LASTEXITCODE -ne 0) { throw "pre-render failed" }
    } finally { Pop-Location }
}
if (-not (Test-Path (Join-Path $dist "index.html"))) { throw "no build in $dist (run without -NoBuild)" }

$failed = @()

& python -I (Join-Path $web "tools\wasm_budget.py") $dist
if ($LASTEXITCODE -ne 0) { $failed += "WebAssembly" }

if (-not $env:CHROME_PATH) {
    $chrome = @(
        "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($chrome) { $env:CHROME_PATH = $chrome }
}

# Lighthouse CI keeps its working files (.lighthouseci) in the current folder: one under target/.
New-Item -ItemType Directory -Force $run | Out-Null
Push-Location $run
try {
    & npx --yes "@lhci/cli@$LhciVersion" autorun "--config=$rc" "--collect.staticDistDir=$dist" "--upload.outputDir=$out"
    if ($LASTEXITCODE -ne 0) { $failed += "Lighthouse" }
} finally { Pop-Location }

if ($failed.Count -gt 0) {
    Write-Host "web-budget: over budget - $($failed -join ', '). Reports: $out"
    exit 1
}
Write-Host "web-budget: every line within budget. Reports: $out"
