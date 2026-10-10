# Builds the manual's HTML (xtask manual) and dresses it as the site: web/assets/manual/{en,es}.
# Run from anywhere; needs cargo. The output is generated, so it is not committed.
$ErrorActionPreference = "Stop"
$repo = Resolve-Path "$PSScriptRoot\.."
Push-Location $repo
try { cargo run -q -p xtask -- manual } finally { Pop-Location }
$src = Join-Path $repo "target\manual"
$out = Join-Path $PSScriptRoot "assets\manual"
if (Test-Path $out) { Get-ChildItem $out -Recurse | Remove-Item -Recurse -Force }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "$src\*" $out -Recurse -Force
Copy-Item "$PSScriptRoot\assets\manual.css" "$out\manual.css" -Force
$theme = '<script>(function(){var t=null;try{t=localStorage.getItem("cantus-theme")}catch(e){}if(t!=="light"&&t!=="dark")t=matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light";document.documentElement.setAttribute("data-theme",t)})()</script>'
foreach ($lang in "en", "es") {
  $f = Join-Path $out "$lang\index.html"
  $h = [IO.File]::ReadAllText($f, [Text.Encoding]::UTF8)
  $h = [regex]::Replace($h, '<style>.*?</style>', "<link rel=`"stylesheet`" href=`"../manual.css`">`n$theme", 'Singleline')
  $back = if ($lang -eq "es") { "Volver al sitio" } else { "Back to the site" }
  $crumb = if ($lang -eq "es") { "Manual" } else { "Manual" }
  $bar = "<header class=`"mbar`"><a class=`"brand`" href=`"../../`"><img src=`"../../logo.svg`" alt=`"`" width=`"64`" height=`"64`">Cantus</a><span class=`"crumb`">$crumb</span><a class=`"back`" href=`"../../`">$back</a></header>"
  $h = $h -replace '<body>', "<body>`n$bar"
  [IO.File]::WriteAllText($f, $h, (New-Object Text.UTF8Encoding($false)))
}
"manual ready in $out"
