# ASCII only: Windows PowerShell 5 reads scripts as ANSI ("e circumflex" below is [char]0x00EA).
# Builds the NSIS installer images (24-bit BMP) from the app icon and the
# "Braise" colors of design/tokens.css. Run from the repo root:
#   powershell -ExecutionPolicy Bypass -File design/installer/make-installer-images.ps1
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$out = Join-Path $root 'src-tauri\icons\installer'
New-Item -ItemType Directory -Force $out | Out-Null
$icon = [System.Drawing.Image]::FromFile((Join-Path $root 'src-tauri\icons\128x128@2x.png'))

function Color([string]$hex, [int]$alpha = 255) {
  $c = [System.Drawing.ColorTranslator]::FromHtml($hex)
  [System.Drawing.Color]::FromArgb($alpha, $c.R, $c.G, $c.B)
}

function New-Canvas([int]$w, [int]$h) {
  $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'
  $g.InterpolationMode = 'HighQualityBicubic'
  $g.TextRenderingHint = 'AntiAliasGridFit'
  $g.PixelOffsetMode = 'HighQuality'
  @($bmp, $g)
}

# Ember glow: an ellipse whose center is accent orange fading to nothing.
function Draw-Glow($g, [float]$cx, [float]$cy, [float]$rx, [float]$ry, [int]$alpha) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $path.AddEllipse($cx - $rx, $cy - $ry, 2 * $rx, 2 * $ry)
  $brush = New-Object System.Drawing.Drawing2D.PathGradientBrush $path
  $brush.CenterColor = Color '#ff7a45' $alpha
  $brush.SurroundColors = @((Color '#ff7a45' 0))
  $g.FillPath($brush, $path)
  $brush.Dispose(); $path.Dispose()
}

function Draw-Centered($g, [string]$text, $font, $brush, [float]$width, [float]$y) {
  $size = $g.MeasureString($text, $font)
  $g.DrawString($text, $font, $brush, [float](($width - $size.Width) / 2), $y)
}

# ---- Sidebar (welcome / finish pages): 164 x 314.
$w = 164; $h = 314
$bmp, $g = New-Canvas $w $h
$bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush (New-Object System.Drawing.Point 0, 0), (New-Object System.Drawing.Point 0, $h), (Color '#1a1512'), (Color '#110d0b')
$g.FillRectangle($bg, 0, 0, $w, $h)
Draw-Glow $g 82 112 120 110 70
Draw-Glow $g 82 330 140 70 60
$g.DrawImage($icon, 42, 72, 80, 80)
$title = New-Object System.Drawing.Font 'Segoe UI', 21, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$small = New-Object System.Drawing.Font 'Segoe UI', 11, ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
Draw-Centered $g 'Ignite' $title (New-Object System.Drawing.SolidBrush (Color '#f3eae4')) $w 164
Draw-Centered $g 'Tous tes jeux PC' $small (New-Object System.Drawing.SolidBrush (Color '#a39187')) $w 196
Draw-Centered $g "au m$([char]0x00EA)me endroit" $small (New-Object System.Drawing.SolidBrush (Color '#a39187')) $w 211
# Accent line, like the active tab underline in the app.
$g.FillRectangle((New-Object System.Drawing.SolidBrush (Color '#ff7a45')), 62, 240, 40, 2)
$g.Dispose()
$bmp.Save((Join-Path $out 'sidebar.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp.Dispose()

# ---- Header (other pages, top right of a white bar): 150 x 57.
$w = 150; $h = 57
$bmp, $g = New-Canvas $w $h
$g.Clear((Color '#ffffff'))
$g.DrawImage($icon, 54, 9, 40, 40)
$g.Dispose()
$bmp.Save((Join-Path $out 'header.bmp'), [System.Drawing.Imaging.ImageFormat]::Bmp)
$bmp.Dispose()

$icon.Dispose()
Get-ChildItem $out | Select-Object Name, Length
