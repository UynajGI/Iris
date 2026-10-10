# SPDX-License-Identifier: GPL-3.0-or-later
param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

function New-InstallerBitmap([string]$Name, [int]$Width, [int]$Height, [scriptblock]$Paint) {
    $bitmap = [Drawing.Bitmap]::new($Width * 3, $Height * 3, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.ScaleTransform(3, 3)
        $graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
        & $Paint $graphics
        $final = [Drawing.Bitmap]::new($Width, $Height, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
        $scaled = [Drawing.Graphics]::FromImage($final)
        try {
            $scaled.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $scaled.DrawImage($bitmap, 0, 0, $Width, $Height)
            $final.Save((Join-Path $OutputDirectory $Name), [Drawing.Imaging.ImageFormat]::Bmp)
        } finally { $scaled.Dispose(); $final.Dispose() }
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}

function Write-InstallerText($Graphics, [string]$Text, [float]$Size, [string]$Color, [float]$X, [float]$Y, [string]$Family = 'Microsoft YaHei UI') {
    $font = [Drawing.Font]::new($Family, $Size, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
    $brush = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml($Color))
    try { $Graphics.DrawString($Text, $font, $brush, $X, $Y) }
    finally { $font.Dispose(); $brush.Dispose() }
}

New-InstallerBitmap 'sidebar.bmp' 164 314 {
    param($g)
    $g.Clear([Drawing.ColorTranslator]::FromHtml('#242424'))
    Write-InstallerText $g '伊人' 29 '#EDEDED' 18 29
    Write-InstallerText $g 'Iris' 16 '#BDBDBD' 20 76 'Segoe UI'
    $pen = [Drawing.Pen]::new([Drawing.ColorTranslator]::FromHtml('#595959'), 1)
    $accent = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#7563AD'))
    try { $g.DrawLine($pen, 22, 118, 142, 118); $g.FillRectangle($accent, 22, 117, 28, 2) }
    finally { $pen.Dispose(); $accent.Dispose() }
    Write-InstallerText $g '照片，在你眼前。' 12 '#EDEDED' 20 137
    Write-InstallerText $g '选择，由你决定。' 12 '#BDBDBD' 20 160
    Write-InstallerText $g 'LOCAL PHOTO WORKSPACE' 8 '#BDBDBD' 20 275 'Segoe UI'
}

New-InstallerBitmap 'header.bmp' 150 57 {
    param($g)
    $g.Clear([Drawing.ColorTranslator]::FromHtml('#242424'))
    Write-InstallerText $g '伊人 / Iris' 21 '#EDEDED' 8 14
}
