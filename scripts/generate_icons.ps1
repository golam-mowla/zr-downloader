Add-Type -AssemblyName System.Drawing

$sourcePath = 'C:\Users\GM desktop\.gemini\antigravity\brain\ca7d5d30-435e-4eb6-9950-df68f728190b\.user_uploaded\media_1793632007079_3c2704f0.jpg'
$srcImg = [System.Drawing.Image]::FromFile($sourcePath)

function Create-RoundedRectanglePath([System.Drawing.RectangleF]$rect, [float]$radius) {
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $diameter = $radius * 2.0
    $arc = New-Object System.Drawing.RectangleF($rect.X, $rect.Y, $diameter, $diameter)
    
    # Top-left arc
    $path.AddArc($arc, 180, 90)
    
    # Top-right arc
    $arc.X = $rect.Right - $diameter
    $path.AddArc($arc, 270, 90)
    
    # Bottom-right arc
    $arc.Y = $rect.Bottom - $diameter
    $path.AddArc($arc, 0, 90)
    
    # Bottom-left arc
    $arc.X = $rect.Left
    $path.AddArc($arc, 90, 90)
    
    $path.CloseFigure()
    return $path
}

function Generate-Squircle-Image($src, $w, $h, $dest, [bool]$isCircle = $false) {
    $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    $pad = [Math]::Max(1.0, [float]($w * 0.035))
    $rect = New-Object System.Drawing.RectangleF($pad, $pad, [float]($w - 2.0 * $pad), [float]($h - 2.0 * $pad))

    if ($isCircle) {
        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        $path.AddEllipse($rect)
    } else {
        # 22% squircle radius for smooth PotPlayer-like softness
        $radius = [float]($rect.Width * 0.22)
        $path = Create-RoundedRectanglePath $rect $radius
    }

    $g.SetClip($path)
    # Fill with base red color
    $redBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 212, 2, 3))
    $g.FillRectangle($redBrush, 0, 0, $w, $h)
    $redBrush.Dispose()

    # Draw source image scaled to the squircle area
    $g.DrawImage($src, $rect)
    $g.ResetClip()

    $dir = [System.IO.Path]::GetDirectoryName($dest)
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $bmp.Save($dest, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    $bmp.Dispose()
}

# Android Mipmaps
$androidMipmaps = @{
    'android/app/src/main/res/mipmap-mdpi/ic_launcher.png' = @{ Size = 48; Circle = $false }
    'android/app/src/main/res/mipmap-mdpi/ic_launcher_round.png' = @{ Size = 48; Circle = $true }
    'android/app/src/main/res/mipmap-mdpi/ic_launcher_foreground.png' = @{ Size = 108; Circle = $false }
    'android/app/src/main/res/mipmap-hdpi/ic_launcher.png' = @{ Size = 72; Circle = $false }
    'android/app/src/main/res/mipmap-hdpi/ic_launcher_round.png' = @{ Size = 72; Circle = $true }
    'android/app/src/main/res/mipmap-hdpi/ic_launcher_foreground.png' = @{ Size = 162; Circle = $false }
    'android/app/src/main/res/mipmap-xhdpi/ic_launcher.png' = @{ Size = 96; Circle = $false }
    'android/app/src/main/res/mipmap-xhdpi/ic_launcher_round.png' = @{ Size = 96; Circle = $true }
    'android/app/src/main/res/mipmap-xhdpi/ic_launcher_foreground.png' = @{ Size = 216; Circle = $false }
    'android/app/src/main/res/mipmap-xxhdpi/ic_launcher.png' = @{ Size = 144; Circle = $false }
    'android/app/src/main/res/mipmap-xxhdpi/ic_launcher_round.png' = @{ Size = 144; Circle = $true }
    'android/app/src/main/res/mipmap-xxhdpi/ic_launcher_foreground.png' = @{ Size = 324; Circle = $false }
    'android/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png' = @{ Size = 192; Circle = $false }
    'android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_round.png' = @{ Size = 192; Circle = $true }
    'android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_foreground.png' = @{ Size = 432; Circle = $false }
}

foreach ($path in $androidMipmaps.Keys) {
    $info = $androidMipmaps[$path]
    Generate-Squircle-Image $srcImg $info.Size $info.Size $path $info.Circle
    Write-Host "Generated $path"
}

# Desktop PNG Icons
$desktopIcons = @{
    'src-tauri/icons/32x32.png' = 32
    'src-tauri/icons/64x64.png' = 64
    'src-tauri/icons/128x128.png' = 128
    'src-tauri/icons/128x128@2x.png' = 256
    'src-tauri/icons/256x256.png' = 256
    'src-tauri/icons/icon.png' = 512
    'src-tauri/icons/Square30x30Logo.png' = 30
    'src-tauri/icons/Square44x44Logo.png' = 44
    'src-tauri/icons/Square71x71Logo.png' = 71
    'src-tauri/icons/Square89x89Logo.png' = 89
    'src-tauri/icons/Square107x107Logo.png' = 107
    'src-tauri/icons/Square142x142Logo.png' = 142
    'src-tauri/icons/Square150x150Logo.png' = 150
    'src-tauri/icons/Square284x284Logo.png' = 284
    'src-tauri/icons/Square310x310Logo.png' = 310
    'src-tauri/icons/StoreLogo.png' = 50
    'public/zr-logo.png' = 512
    'public/icon.png' = 512
    'public/pwa-192x192.png' = 192
    'public/pwa-512x512.png' = 512
    'browser-extension/icons/icon16.png' = 16
    'browser-extension/icons/icon32.png' = 32
    'browser-extension/icons/icon48.png' = 48
    'browser-extension/icons/icon128.png' = 128
}

foreach ($path in $desktopIcons.Keys) {
    $s = $desktopIcons[$path]
    Generate-Squircle-Image $srcImg $s $s $path $false
    Write-Host "Generated $path"
}

# Generate multi-frame Windows icon.ico with PNG frames
$tempDir = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "zr_icons_" + [System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

$icoSizes = @(16, 24, 32, 48, 64, 128, 256)
$icoPngFiles = @()

foreach ($size in $icoSizes) {
    $tmpPng = [System.IO.Path]::Combine($tempDir, "icon_$size.png")
    Generate-Squircle-Image $srcImg $size $size $tmpPng $false
    $icoPngFiles += $tmpPng
}

function Save-MultiResolutionIco($pngList, $destIco) {
    $streams = @()
    foreach ($png in $pngList) {
        $bytes = [System.IO.File]::ReadAllBytes($png)
        $bmp = [System.Drawing.Image]::FromFile($png)
        $w = if ($bmp.Width -ge 256) { [byte]0 } else { [byte]$bmp.Width }
        $h = if ($bmp.Height -ge 256) { [byte]0 } else { [byte]$bmp.Height }
        $bmp.Dispose()
        $streams += @{ Width = $w; Height = $h; Bytes = $bytes }
    }
    
    $count = $streams.Count
    $headerSize = 6 + ($count * 16)
    $offset = $headerSize
    
    $fs = [System.IO.File]::Create($destIco)
    $bw = New-Object System.IO.BinaryWriter($fs)
    
    # ICONDIR
    $bw.Write([uint16]0) # Reserved
    $bw.Write([uint16]1) # Type (1 = Icon)
    $bw.Write([uint16]$count) # Count
    
    # ICONDIRENTRY list
    foreach ($item in $streams) {
        $bw.Write([byte]$item.Width)
        $bw.Write([byte]$item.Height)
        $bw.Write([byte]0)   # Color count
        $bw.Write([byte]0)   # Reserved
        $bw.Write([uint16]1) # Color planes
        $bw.Write([uint16]32) # Bits per pixel
        $bw.Write([uint32]$item.Bytes.Length)
        $bw.Write([uint32]$offset)
        $offset += $item.Bytes.Length
    }
    
    # Image data (PNG blocks)
    foreach ($item in $streams) {
        $bw.Write($item.Bytes)
    }
    
    $bw.Flush()
    $fs.Close()
    Write-Host "Created multi-resolution ICO: $destIco ($count sizes)"
}

Save-MultiResolutionIco $icoPngFiles 'src-tauri/icons/icon.ico'
Save-MultiResolutionIco $icoPngFiles 'public/favicon.ico'

Remove-Item -Recurse -Force $tempDir

$srcImg.Dispose()
Write-Host 'All icons with soft rounded squircle edges generated successfully!'
