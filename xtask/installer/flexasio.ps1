# Fetch the latest FlexASIO (MIT, https://github.com/dechamps/FlexASIO) and run its
# installer elevated. Called by the Cantus installer when the user ticks
# "Install FlexASIO"; never during a silent install or update. Nothing is bundled.
#
# Exit codes: 0 installed (or downloaded, with -DownloadOnly); 10 GitHub unreachable;
# 11 the network answered with a web page (a proxy's quarantine) — nothing is run;
# 12 no installer in the release; 13 the file is not what GitHub described;
# 14 the user declined elevation, or FlexASIO's installer failed.
#
# Invoke-WebRequest goes through Windows' own proxy settings (a PAC script included) and
# certificate store; TLS checks are never relaxed.

param(
    [string]$Dest = $env:TEMP,
    [switch]$DownloadOnly
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch { }

$api = 'https://api.github.com/repos/dechamps/FlexASIO/releases/latest'
$headers = @{ 'User-Agent' = 'cantus-setup'; 'Accept' = 'application/vnd.github+json' }

try {
    $answer = Invoke-WebRequest -UseBasicParsing -Uri $api -Headers $headers -TimeoutSec 60
} catch {
    Write-Output "FlexASIO: GitHub unreachable: $($_.Exception.Message)"
    exit 10
}
$text = [string]$answer.Content
if ($text.TrimStart().StartsWith('<')) {
    Write-Output 'FlexASIO: the network answered with a web page instead of the release.'
    exit 11
}
try {
    $release = $text | ConvertFrom-Json
} catch {
    Write-Output 'FlexASIO: the answer is not a release.'
    exit 12
}
$asset = $release.assets | Where-Object { $_.name -like 'FlexASIO-*.exe' } | Select-Object -First 1
if (-not $asset) {
    Write-Output "FlexASIO: no installer in $($release.tag_name)."
    exit 12
}

$file = Join-Path $Dest $asset.name
Write-Output "FlexASIO: downloading $($asset.name) ($([math]::Round($asset.size / 1MB, 1)) MB)"
try {
    Invoke-WebRequest -UseBasicParsing -Uri $asset.browser_download_url -OutFile $file `
        -Headers @{ 'User-Agent' = 'cantus-setup' } -TimeoutSec 600
} catch {
    Write-Output "FlexASIO: download failed: $($_.Exception.Message)"
    exit 10
}

# What arrived: a Windows program, of the size GitHub reported (and its digest when given).
$length = (Get-Item -LiteralPath $file).Length
$head = New-Object byte[] 2
$stream = [IO.File]::OpenRead($file)
try { [void]$stream.Read($head, 0, 2) } finally { $stream.Dispose() }
if ($head[0] -ne 0x4D -or $head[1] -ne 0x5A) {
    Remove-Item -LiteralPath $file -Force
    Write-Output 'FlexASIO: what arrived is not a program (a proxy page?); nothing was run.'
    exit 11
}
if ($length -ne [int64]$asset.size) {
    Remove-Item -LiteralPath $file -Force
    Write-Output "FlexASIO: $length bytes arrived, GitHub reported $($asset.size); nothing was run."
    exit 13
}
if ($asset.digest -and ($asset.digest -match '^sha256:([0-9a-fA-F]{64})$')) {
    $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
    if ($hash -ne $Matches[1].ToUpper()) {
        Remove-Item -LiteralPath $file -Force
        Write-Output 'FlexASIO: the SHA-256 does not match GitHub''s; nothing was run.'
        exit 13
    }
}
Write-Output "FlexASIO: verified ($length bytes)."
if ($DownloadOnly) {
    Write-Output $file
    exit 0
}

try {
    $p = Start-Process -FilePath $file -Verb RunAs -Wait -PassThru
} catch {
    Write-Output 'FlexASIO: not installed (elevation was declined).'
    exit 14
}
Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
if ($p.ExitCode -ne 0) {
    Write-Output "FlexASIO: its installer ended with code $($p.ExitCode)."
    exit 14
}
Write-Output 'FlexASIO: installed.'
exit 0
