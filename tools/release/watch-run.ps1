# Watch a run of cantus-daw (a release or its rehearsal): print each job as it finishes,
# then the run's end, one poll every five minutes (docs/RELEASING.md L8). Made for a
# Monitor or a terminal; filter it with `grep --line-buffered`.
#   pwsh -NoProfile -File tools/release/watch-run.ps1 -RunId <id>
param([Parameter(Mandatory)][string]$RunId)
$api = Join-Path $PSScriptRoot 'gh-api.ps1'
$seen = @{}
while ($true) {
    try {
        $j = (& $api GET "repos/luis314159/cantus-daw/actions/runs/$RunId/jobs?per_page=50") | ConvertFrom-Json
        foreach ($job in $j.jobs) {
            if ($job.status -eq 'completed' -and -not $seen.ContainsKey($job.name)) {
                $seen[$job.name] = 1
                Write-Output "job '$($job.name)': $($job.conclusion)"
            }
        }
        $r = (& $api GET "repos/luis314159/cantus-daw/actions/runs/$RunId") | ConvertFrom-Json
        if ($r.status -eq 'completed') { Write-Output "RUN $RunId completed: $($r.conclusion)"; break }
    } catch { Write-Output "poll error: $($_.Exception.Message)" }
    Start-Sleep -Seconds 300
}
