# Makes the four sample wallpapers (sunset, aurora, graphite, dawn) that site/tools/render-shots.sh lays the WinUI 3 theme over.
# pwsh -File site/tools/make-wallpapers.ps1 -Dir <folder>   then   SDM_WALLPAPERS=<folder> bash site/tools/render-shots.sh <out>
param([string]$Dir)
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force $Dir | Out-Null
function Make($name, $g0, $g1, $blobs) {
    $w = 1920; $h = 1080
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $rect = New-Object System.Drawing.Rectangle 0, 0, $w, $h
    $ground = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::FromArgb(255, $g0[0], $g0[1], $g0[2])), ([System.Drawing.Color]::FromArgb(255, $g1[0], $g1[1], $g1[2])), 35.0
    $g.FillRectangle($ground, $rect)
    foreach ($b in $blobs) {
        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        $path.AddEllipse($b[0] - $b[2], $b[1] - $b[3], 2 * $b[2], 2 * $b[3])
        $brush = New-Object System.Drawing.Drawing2D.PathGradientBrush $path
        $brush.CenterColor = [System.Drawing.Color]::FromArgb($b[4], $b[5], $b[6], $b[7])
        $brush.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $b[5], $b[6], $b[7]))
        $g.FillPath($brush, $path)
        $brush.Dispose(); $path.Dispose()
    }
    $g.Dispose()
    $bmp.Save((Join-Path $Dir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}
# sunset: deep plum ground, orange and pink blooms
Make 'sunset' @(40,10,50) @(90,25,40) @(@(500,300,900,650,230,255,110,60), @(1500,350,800,600,210,230,50,110), @(1000,950,1000,560,190,255,170,70), @(250,900,700,500,170,150,40,170), @(1700,900,700,480,150,255,90,90))
# aurora: dark teal ground, green and cyan blooms
Make 'aurora' @(4,26,34) @(10,40,50) @(@(500,350,900,650,220,20,200,150), @(1500,300,800,620,190,40,150,230), @(1000,900,1000,560,190,30,210,120), @(300,950,700,520,160,90,60,200), @(1650,860,760,520,160,20,160,200))
# graphite: neutral dark ground, faint cool light
Make 'graphite' @(18,18,22) @(34,34,40) @(@(600,350,900,650,140,110,120,150), @(1500,400,800,600,120,90,100,130), @(1000,950,1000,500,110,70,80,100))
# dawn: bright airy light ground with pastel blooms
Make 'dawn' @(236,240,252) @(252,238,244) @(@(500,300,900,650,200,160,190,255), @(1500,300,800,600,190,255,190,210), @(1000,900,1000,560,170,170,230,255), @(300,900,700,500,150,255,215,180), @(1700,900,700,480,140,200,170,255))
Get-ChildItem $Dir | ForEach-Object { "$($_.Name) $($_.Length)" }
