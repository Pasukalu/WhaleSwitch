param([string]$PortableZip, [string]$Installer)

$ErrorActionPreference = 'Stop'

function Assert-WebViewLaunch([string]$Executable, [string]$Profile, [string]$Variant) {
    $process = Start-Process -FilePath $Executable -WorkingDirectory ([IO.Path]::GetDirectoryName($Executable)) -WindowStyle Hidden -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(35)
        do {
            if ($process.HasExited) {
                throw "The $Variant app exited during startup ($($process.ExitCode))."
            }
            if (Test-Path -LiteralPath (Join-Path $Profile 'EBWebView') -PathType Container) {
                Start-Sleep -Seconds 2
                if ($process.HasExited) {
                    throw "The $Variant app exited after WebView2 initialized ($($process.ExitCode))."
                }
                Write-Output "The $Variant package started and initialized its WebView2 profile."
                return
            }
            Start-Sleep -Milliseconds 400
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "The $Variant app did not initialize its WebView2 profile."
    } finally {
        if (-not $process.HasExited) {
            Stop-Process -InputObject $process
            $null = $process.WaitForExit(5000)
        }
        $process.Dispose()
    }
}

if (-not $PortableZip) {
    $version = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
    $PortableZip = "release-assets/WhaleSwitch-$version-windows-x64-portable.zip"
}

$scratch = Join-Path $env:RUNNER_TEMP ("whaleswitch-smoke-" + [guid]::NewGuid().ToString('N'))
$portableDirectory = Join-Path $scratch 'portable'
Expand-Archive -LiteralPath $PortableZip -DestinationPath $portableDirectory
$env:CODEX_HOME = Join-Path $scratch 'fixtures/codex'
$env:CLAUDE_CONFIG_DIR = Join-Path $scratch 'fixtures/claude'
$env:XDG_CONFIG_HOME = Join-Path $scratch 'fixtures/config'
$portableExe = Join-Path $portableDirectory 'WhaleSwitch.exe'
Assert-WebViewLaunch $portableExe (Join-Path $portableDirectory 'WhaleSwitchData/webview') 'portable'

if ($Installer) {
    $installDirectory = Join-Path $scratch 'install'
    $setup = Start-Process -FilePath (Resolve-Path -LiteralPath $Installer).Path -ArgumentList "/S /D=$installDirectory" -WindowStyle Hidden -PassThru -Wait
    if ($setup.ExitCode -ne 0) { throw "The NSIS install failed ($($setup.ExitCode))." }
    $installedExe = Join-Path $installDirectory 'whaleswitch.exe'
    $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $scratch 'installed-webview'
    Assert-WebViewLaunch $installedExe $env:WEBVIEW2_USER_DATA_FOLDER 'installed'
    foreach ($file in @($Installer, $installedExe, $portableExe)) {
        $signature = Get-AuthenticodeSignature -LiteralPath $file
        Write-Output ("Authenticode status for " + [IO.Path]::GetFileName($file) + ": " + $signature.Status)
    }
}
