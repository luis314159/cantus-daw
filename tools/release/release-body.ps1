# A published release's text on cantus-daw: read it to a file, or replace it with a file's
# and check that what GitHub holds is that file to the byte (2026-11-20: beta.15's text still
# offered a macOS disk image that was never published). Editing a published release is
# public: only with the owner's OK.
#
#   pwsh tools/release/release-body.ps1 get v0.1.0-beta.15 body.md
#   pwsh tools/release/release-body.ps1 set v0.1.0-beta.15 body.md
#
# The token is the owner's fine-grained one for cantus-daw, read from Windows Credential
# Manager by gh-api.ps1 (default ~/.cantus-tools/gh-api.ps1, or $env:CANTUS_GH_API) — never
# printed. `set` refuses a text that quotes the RVR1960: public text carries the RV1909 (the
# RVR1960's permission is the owner's open item; tools/public/publish.sh rewrites the history
# the same way), checked against the private es.txt's Colossians 3:23 when it is present.
param(
    [Parameter(Mandatory)][ValidateSet('get', 'set')][string]$Action,
    [Parameter(Mandatory)][string]$Tag,
    [Parameter(Mandatory)][string]$File
)
$ErrorActionPreference = 'Stop'
$api = if ($env:CANTUS_GH_API) { $env:CANTUS_GH_API } else { Join-Path $HOME '.cantus-tools/gh-api.ps1' }
$repo = 'repos/luis314159/cantus-daw'
$utf8 = New-Object Text.UTF8Encoding $false
$release = & $api GET "$repo/releases/tags/$Tag" | ConvertFrom-Json
if ($Action -eq 'get') {
    $path = [IO.Path]::GetFullPath($File, (Get-Location).Path)
    [IO.File]::WriteAllText($path, $release.body, $utf8)
    "saved $Tag's text ($($release.body.Length) characters, release id $($release.id)) to $path"
    "assets: " + (($release.assets | ForEach-Object name) -join ', ')
    return
}
$body = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $File).Path, [Text.Encoding]::UTF8)
# The RVR1960 must not be in a public text: the rules of tools/public/motto_replacements.py
# (Colossians 3:23 up to "Señor", Psalm 33:3's first five words, the "(RVR1960)" mark), the
# quotations read from the private es.txt — no RVR1960 wording is written here, as the public
# repository never holds it (publish.sh's check refuses a blob that does). Without es.txt
# (the public tree), only the "(RVR1960)" mark is checked.
$root = (& git rev-parse --show-toplevel).Trim()
$es = Join-Path $root 'crates/cantus-ui/locales/verses/es.txt'
$quotes = @('(RVR1960)')
if (Test-Path $es) {
    $verse = @{}
    foreach ($l in Get-Content -LiteralPath $es -Encoding utf8) {
        $parts = $l -split ' \| ', 2
        if ($parts.Count -eq 2) { $verse[$parts[0]] = $parts[1].Trim().TrimEnd(';', '.') }
    }
    if ($verse['COL 3:23']) {
        $w = $verse['COL 3:23'] -split '\s+'
        $i = [array]::FindIndex($w, [Predicate[string]] { param($x) $x.Trim(',', ';', '.') -eq 'Señor' })
        if ($i -ge 0) {
            $quotes += ($w[0..$i] -join ' ').TrimEnd(',')
            # The clause alone, from "hacedlo" (motto_replacements.py's fifth rule).
            $h = [array]::FindIndex($w, [Predicate[string]] { param($x) $x.StartsWith('hacedlo') })
            if ($h -ge 0 -and $h -le $i) { $quotes += ($w[$h..$i] -join ' ').TrimEnd(',') }
        }
    }
    if ($verse['PSA 33:3']) { $quotes += ((($verse['PSA 33:3'] -split '\s+')[0..4]) -join ' ').TrimEnd(',', ';', ':', '.') }
}
foreach ($q in $quotes) {
    $pattern = (($q -split '\s+') | ForEach-Object { [regex]::Escape($_) }) -join '\s+'
    if ($body -match $pattern) {
        throw "the text quotes the RVR1960 ('$q'): public text carries the RV1909 — «hacedlo de ánimo, como al Señor»"
    }
}
$json = @{ body = $body } | ConvertTo-Json -Compress
$null = & $api PATCH "$repo/releases/$($release.id)" $json
$now = & $api GET "$repo/releases/$($release.id)" | ConvertFrom-Json
if ($now.body -cne $body) { throw "GitHub's text differs from $File after the update: look at the release" }
"$Tag's text replaced; GitHub holds $File to the byte ($($body.Length) characters)"
