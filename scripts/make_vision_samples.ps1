# Renders visual-only test screenshots for the vision-model spike (bench_llm.py --images).
param([string]$Out = ".spikes")
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force $Out | Out-Null

# A 5-colour palette with hex labels, like a Coolors/Dribbble screenshot (1200x700).
$bmp = New-Object System.Drawing.Bitmap 1200, 700
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$g.Clear([System.Drawing.Color]::White)
$hex = "#264653", "#2A9D8F", "#E9C46A", "#F4A261", "#E76F51"
$font = New-Object System.Drawing.Font("Segoe UI", 22)
for ($i = 0; $i -lt 5; $i++) {
    $g.FillRectangle((New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml($hex[$i]))), $i * 240, 0, 240, 580)
    $g.DrawString($hex[$i], $font, [System.Drawing.Brushes]::Black, $i * 240 + 50, 610)
}
$bmp.Save((Join-Path $Out "vision-palette.png"), [System.Drawing.Imaging.ImageFormat]::Png)

# A simple bar chart with minimal text (1200x800).
$bmp = New-Object System.Drawing.Bitmap 1200, 800
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = "AntiAlias"; $g.TextRenderingHint = "AntiAliasGridFit"
$g.Clear([System.Drawing.ColorTranslator]::FromHtml("#0f172a"))
$white = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml("#e2e8f0"))
$g.DrawString("Monthly active users (millions)", (New-Object System.Drawing.Font("Segoe UI", 26)), $white, 60, 40)
$vals = 12, 18, 23, 31, 44, 61; $labels = "Jan", "Feb", "Mar", "Apr", "May", "Jun"
$bar = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml("#38bdf8"))
$small = New-Object System.Drawing.Font("Segoe UI", 18)
for ($i = 0; $i -lt 6; $i++) {
    $h = $vals[$i] * 9; $x = 110 + $i * 170
    $g.FillRectangle($bar, $x, 700 - $h, 110, $h)
    $g.DrawString($labels[$i], $small, $white, $x + 30, 712)
    $g.DrawString("$($vals[$i])", $small, $white, $x + 35, 660 - $h)
}
$bmp.Save((Join-Path $Out "vision-chart.png"), [System.Drawing.Imaging.ImageFormat]::Png)
