# A rectangle of the screen as a PNG (round 35, ADR 0091): what the desktop shows where the
# stage's window is — to see with one's own eyes that it is see-through.
#
#   powershell -NoProfile -File tools/obs/grab.ps1 -X 1500 -Y 600 -W 340 -H 380 -Out shot.png
param(
    [int]$X, [int]$Y, [int]$W, [int]$H,
    [string]$Out
)
Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap $W, $H
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($X, $Y, 0, 0, (New-Object System.Drawing.Size $W, $H))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
