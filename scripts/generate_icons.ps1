Add-Type -AssemblyName System.Drawing
$sourcePath = 'C:\Users\GM desktop\.gemini\antigravity\brain\ca7d5d30-435e-4eb6-9950-df68f728190b\.user_uploaded\media_1793632007079_3c2704f0.jpg'
$srcImg = [System.Drawing.Image]::FromFile($sourcePath)

# Sample background color from corner
$sampleBmp = New-Object System.Drawing.Bitmap($srcImg)
$px = $sampleBmp.GetPixel(10, 10)
$hexColor = '#{0:X2}{1:X2}{2:X2}' -f $px.R, $px.G, $px.B
Write-Host "Corner background color: $hexColor"
$sampleBmp.Dispose()

function Resize-Image($src, $w, $h, $dest) {
    $bmp = New-Object System.Drawing.Bitmap($w, $h)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($src, 0, 0, $w, $h)
    $dir = [System.IO.Path]::GetDirectoryName($dest)
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $bmp.Save($dest, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    $bmp.Dispose()
}

$androidMipmaps = @{
    'android/app/src/main/res/mipmap-mdpi/ic_launcher.png' = 48
    'android/app/src/main/res/mipmap-mdpi/ic_launcher_round.png' = 48
    'android/app/src/main/res/mipmap-mdpi/ic_launcher_foreground.png' = 108
    'android/app/src/main/res/mipmap-hdpi/ic_launcher.png' = 72
    'android/app/src/main/res/mipmap-hdpi/ic_launcher_round.png' = 72
    'android/app/src/main/res/mipmap-hdpi/ic_launcher_foreground.png' = 162
    'android/app/src/main/res/mipmap-xhdpi/ic_launcher.png' = 96
    'android/app/src/main/res/mipmap-xhdpi/ic_launcher_round.png' = 96
    'android/app/src/main/res/mipmap-xhdpi/ic_launcher_foreground.png' = 216
    'android/app/src/main/res/mipmap-xxhdpi/ic_launcher.png' = 144
    'android/app/src/main/res/mipmap-xxhdpi/ic_launcher_round.png' = 144
    'android/app/src/main/res/mipmap-xxhdpi/ic_launcher_foreground.png' = 324
    'android/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png' = 192
    'android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_round.png' = 192
    'android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_foreground.png' = 432
}

foreach ($path in $androidMipmaps.Keys) {
    $s = $androidMipmaps[$path]
    Resize-Image $srcImg $s $s $path
    Write-Host "Generated $path"
}

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
    'public/goat-logo.png' = 512
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
    Resize-Image $srcImg $s $s $path
    Write-Host "Generated $path"
}

$srcImg.Save('public/goat-logo.jpg', [System.Drawing.Imaging.ImageFormat]::Jpeg)

# Generate icon.ico
$icoBmp = New-Object System.Drawing.Bitmap($srcImg, 256, 256)
$hIcon = $icoBmp.GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($hIcon)
$fs = New-Object System.IO.FileStream('src-tauri/icons/icon.ico', [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
$fs = New-Object System.IO.FileStream('public/favicon.ico', [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
$icoBmp.Dispose()

$srcImg.Dispose()
Write-Host 'All icons generated successfully!'
