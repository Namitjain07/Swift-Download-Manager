# Signs Windows files with the project's code-signing key.
#
#   $env:SDM_PFX_PATH = 'C:\path\to\swift-download-manager-codesign-2.pfx'
#   $env:SDM_PFX_PASSWORD = '...'
#   powershell -File packaging\windows\sign.ps1 dist\SwiftDownloadManager.exe dist\setup.exe
#
# Uses signtool.exe (Windows SDK) when it is installed, otherwise PowerShell's own Set-AuthenticodeSignature.
# Every signature gets a timestamp, so it stays valid after the certificate has expired. A timestamp server
# that does not answer within 90 seconds is given up on and the next one is tried (the system's own calls
# have no time limit, and one stalled server would otherwise hang a whole build). The key is never written
# anywhere by this script. Exit code 0 only when every file was signed.
param([Parameter(Mandatory, ValueFromRemainingArguments)] [string[]] $Files)

$ErrorActionPreference = 'Stop'
$pfx = $env:SDM_PFX_PATH
$password = $env:SDM_PFX_PASSWORD
if (-not $pfx -or -not (Test-Path -LiteralPath $pfx)) { throw 'Set SDM_PFX_PATH to the code-signing .pfx file.' }
if (-not $password) { throw 'Set SDM_PFX_PASSWORD to the password of the .pfx file.' }

$servers = 'http://timestamp.digicert.com', 'http://timestamp.sectigo.com', 'http://timestamp.globalsign.com/tsa/r6advanced1'
$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\*\signtool.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.Directory.Name -in 'x64', 'x86', 'arm64' } |
    Sort-Object { try { [version]$_.Directory.Parent.Name } catch { [version]'0.0' } }, { $_.Directory.Name -eq 'x64' } -Descending |
    Select-Object -First 1
$signtoolPath = if ($signtool) { $signtool.FullName } else { '' }

# One attempt, in a job of its own so that it can be stopped when it takes too long.
$attempt = {
    param($file, $pfx, $password, $timestamp, $signtoolPath)
    if ($signtoolPath) {
        & $signtoolPath sign /fd SHA256 /f $pfx /p $password /tr $timestamp /td SHA256 /d 'Swift Download Manager' /du 'https://github.com/Namitjain07/Swift-Download-Manager' $file 2>&1 | Out-Null
        return ($LASTEXITCODE -eq 0)
    }
    $cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($pfx, $password)
    $r = Set-AuthenticodeSignature -FilePath $file -Certificate $cert -HashAlgorithm SHA256 -TimestampServer $timestamp
    # The status says "not trusted" for a self-signed certificate; what counts is that it signed and timestamped.
    return [bool]($r.SignerCertificate -and $r.TimeStamperCertificate)
}

foreach ($file in $Files) {
    $file = (Resolve-Path -LiteralPath $file.Trim('"')).Path
    $done = $false
    foreach ($ts in $servers) {
        $job = Start-Job -ScriptBlock $attempt -ArgumentList $file, $pfx, $password, $ts, $signtoolPath
        if (Wait-Job $job -Timeout 90) {
            $ok = Receive-Job $job -ErrorAction SilentlyContinue
            Remove-Job $job -Force
            if ($ok -contains $true) { $done = $true; break }
            Write-Warning "Signing $file with timestamp server $ts failed; trying the next one"
        } else {
            Stop-Job $job
            Remove-Job $job -Force
            Write-Warning "Timestamp server $ts did not answer in 90 seconds; trying the next one"
        }
    }
    if (-not $done) { throw "Could not sign $file" }
    Write-Host "Signed $file"
}
