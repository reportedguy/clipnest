# Renders screenshot-like PNGs with known text for the OCR spike (spike_ocr.rs).
#   powershell -File make_ocr_samples.ps1 -Out ..\.spikes
param([string]$Out = "..\.spikes")
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force $Out | Out-Null

function Render($name, $lines, $fontSize, $fg, $bg, $width) {
    $font = New-Object System.Drawing.Font("Segoe UI", $fontSize, [System.Drawing.GraphicsUnit]::Pixel)
    $lineH = [int]($fontSize * 1.6)
    $bmp = New-Object System.Drawing.Bitmap $width, ($lineH * $lines.Count + 40)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
    $g.Clear([System.Drawing.ColorTranslator]::FromHtml($bg))
    $brush = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml($fg))
    for ($i = 0; $i -lt $lines.Count; $i++) { $g.DrawString($lines[$i], $font, $brush, 20, 20 + $i * $lineH) }
    $bmp.Save((Join-Path $Out "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    [IO.File]::WriteAllText((Join-Path $Out "$name.txt"), ($lines -join "`n"))
}

Render "ocr-light" @(
    "Global Student Hackathon 2026",
    "Registrations close on 12/11/2026. Apply at https://hack.example.org/apply",
    "Prize pool: Rs 5,00,000 for the top three teams. Teams of up to 4 people.",
    "Themes: climate, health, education and open data."
) 16 "#1f2328" "#ffffff" 900

Render "ocr-dark" @(
    "How to make better thumbnails",
    "1. Use one clear subject and a bold color contrast.",
    "2. Keep text under five words; it must read at phone size.",
    "Watch the full video: youtube.com/watch?v=abc123XYZ"
) 15 "#d4d4d4" "#1e1e1e" 820

Render "ocr-small" @(
    "Note: The scholarship portal opens on 3 March and closes on 28 March 2027.",
    "Eligible: undergraduate students in their second or third year.",
    "Documents needed: transcript, two references, and a 500-word statement."
) 11 "#57606a" "#f6f8fa" 700
