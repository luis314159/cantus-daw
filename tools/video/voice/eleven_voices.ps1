# The ElevenLabs voices of the launch video: the library searches that found them, and adding
# the chosen ones to the account (an account must hold a library voice before it speaks).
# Searches and adds spend no characters. The key is read by eleven-api.ps1 from Windows
# Credential Manager (target "cantus-elevenlabs"), never printed or written.
#
#   pwsh -File tools/video/voice/eleven_voices.ps1 -Out F:\projects\video-work\voice\eleven [-Search] [-Add]
#
# The owner's choice (2026-11-21): English Jon, Spanish Alejandro (Mexican narrator); Victoria
# and Gabriela were the other two heard. The searches below are the ones that listed them.
param(
    [Parameter(Mandatory)][string]$Out,
    [switch]$Search,
    [switch]$Add
)
$ErrorActionPreference = 'Stop'
$api = Join-Path $PSScriptRoot 'eleven-api.ps1'
New-Item -ItemType Directory -Force $Out | Out-Null

# The shared library searches (sorted by a year's use), each saved as JSON for the record.
$queries = @(
    'language=es&accent=mexican&gender=male',
    'language=es&accent=mexican&gender=female',
    'language=es&accent=latin%20american&gender=male',
    'language=es&accent=latin%20american&gender=female',
    'language=en&accent=american&gender=male&use_cases=narrative_story',
    'language=en&accent=american&gender=female&use_cases=narrative_story'
)
if ($Search) {
    & $api GET v1/user/subscription | Set-Content -Encoding utf8 (Join-Path $Out 'subscription.json')
    & $api GET v1/models | Set-Content -Encoding utf8 (Join-Path $Out 'models.json')
    foreach ($q in $queries) {
        $f = Join-Path $Out ('shared-' + ($q -replace '[^a-z]', '_') + '.json')
        & $api GET "v1/shared-voices?page_size=30&sort=usage_character_count_1y&$q" | Set-Content -Encoding utf8 $f
        $j = Get-Content $f -Raw | ConvertFrom-Json
        "== $q : $($j.voices.Count)"
        $j.voices | Select-Object -First 8 | ForEach-Object { '{0} | {1} | {2} | {3}' -f $_.voice_id, $_.name, $_.accent, $_.use_case }
    }
}

# The four voices heard (owner id, voice id, the name it takes in the account).
$voices = @(
    @('b45c1524a1f360319535f16b301e5b6f37edf26fd1d59861eda66206ff25cdc9', 'MFZUKuGQUsGJPQjTS4wC', 'Cantus EN Jon'),
    @('58a5f76980f0b3db1d9d23fbe598ebff118ad113cf99d1b5afdd78d50666b46e', 'qSeXEcewz7tA0Q0qk9fH', 'Cantus EN Victoria'),
    @('197bd00cc0ed5de14d1251c841b1016a1c7464a5d4f1fd13f87c4f2cea1b0597', 'NWqMOQLlMBaUbjKYdhbW', 'Cantus ES Alejandro'),
    @('af26f59825a133f0adb7714a7153938018eca4fcdecf9548fb48d4ad5914b9fc', 'hHjbwzYZW17oh0p05AKv', 'Cantus ES Gabriela')
)
if ($Add) {
    $have = (& $api GET v1/voices | ConvertFrom-Json).voices.voice_id
    foreach ($v in $voices) {
        if ($have -contains $v[1]) { "already in the account: $($v[2])"; continue }
        $body = Join-Path $Out 'add.json'
        [IO.File]::WriteAllText($body, (@{ new_name = $v[2] } | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
        & $api POST "v1/voices/add/$($v[0])/$($v[1])" -BodyFile $body
        Remove-Item $body
    }
}
