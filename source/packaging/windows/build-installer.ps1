# Builds the Windows installer (Inno Setup), and signs it when a code-signing key is given.
#
#   powershell -File packaging\windows\build-installer.ps1                    # x64, unsigned
#   powershell -File packaging\windows\build-installer.ps1 -Arch arm64        # needs: rustup target add aarch64-pc-windows-msvc
#   $env:SDM_PFX_PATH = '...\swift-download-manager-codesign-2.pfx'; $env:SDM_PFX_PASSWORD = '...'
#   powershell -File packaging\windows\build-installer.ps1 -Sign              # program, installer and uninstaller signed
#
# Needs Rust (unless -SkipBuild) and Inno Setup 6 (winget install JRSoftware.InnoSetup). Run it from the source folder
# (the one with Cargo.toml) or anywhere: it finds its own way. Result: dist\SwiftDownloadManager-<version>-setup-<arch>.exe
# and the program itself, dist\SwiftDownloadManager.exe (x64) or dist\SwiftDownloadManager-arm64.exe.
param(
    [ValidateSet('x64', 'arm64')] [string] $Arch = 'x64',
    [switch] $Sign,
    # Use a program that is already built (-Exe), instead of building one.
    [switch] $SkipBuild,
    [string] $Exe
)

$ErrorActionPreference = 'Stop'
$source = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $source

$version = (Select-String -Path 'Cargo.toml' -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$triple = if ($Arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$staged = if ($Arch -eq 'arm64') { 'dist\SwiftDownloadManager-arm64.exe' } else { 'dist\SwiftDownloadManager.exe' }
New-Item -ItemType Directory -Force dist | Out-Null
Write-Host "Swift Download Manager $version, $Arch"

if (-not $SkipBuild) {
    Write-Host "Building $version for $triple ..."
    cargo build --release --locked --target $triple
    if ($LASTEXITCODE -ne 0) { throw 'The build failed' }
    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
    $Exe = Join-Path $targetDir "$triple\release\swift_download_manager.exe"
}
if (-not $Exe) { $Exe = $staged }
if (-not (Test-Path -LiteralPath $Exe)) { throw "The program was not found: $Exe" }
if ((Resolve-Path $Exe).Path -ne (Join-Path $source $staged)) { Copy-Item -LiteralPath $Exe $staged -Force }

Write-Host "Program ready: $staged"
$signScript = Join-Path $PSScriptRoot 'sign.ps1'
# The newer PowerShell (pwsh) when there is one, else the one that comes with Windows.
$shell = if (Get-Command pwsh.exe -ErrorAction SilentlyContinue) { 'pwsh.exe' } else { 'powershell.exe' }
if ($Sign) {
    # A program that was already signed (the release workflow does that first) is left as it is.
    if ((Get-AuthenticodeSignature -LiteralPath $staged).SignerCertificate) {
        Write-Host "$staged is already signed"
    } else {
        & $shell -NoProfile -ExecutionPolicy Bypass -File $signScript $staged
        if ($LASTEXITCODE -ne 0) { throw 'The program could not be signed' }
    }
}

$iscc = @(
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source,
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $iscc) { throw 'Inno Setup 6 was not found. Install it with: winget install JRSoftware.InnoSetup' }

Write-Host "Compiling the installer with $iscc"
$isccArgs = @("/DAppVersion=$version", "/DArch=$Arch", "/DExe=..\..\$staged")
if ($Sign) {
    # $q is a quote mark and $f the file, to Inno Setup; the signing key comes from the environment.
    $isccArgs += '/DSign=1'
    $isccArgs += "/Ssdmsign=$shell -NoProfile -ExecutionPolicy Bypass -File `$q$signScript`$q `$f"
}
& $iscc @isccArgs (Join-Path $PSScriptRoot 'installer.iss')
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup failed' }

$setup = "dist\SwiftDownloadManager-$version-setup-$Arch.exe"
Write-Host "Made $setup"
