$ErrorActionPreference = 'Stop'

$version = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
$archive = "release-assets/WhaleSwitch-$version-windows-x64-portable.zip"
$scratch = Join-Path $env:RUNNER_TEMP ("whaleswitch-smoke-" + [guid]::NewGuid().ToString('N'))
Expand-Archive -LiteralPath $archive -DestinationPath $scratch

$env:CODEX_HOME = Join-Path $scratch 'fixtures/codex'
$env:CLAUDE_CONFIG_DIR = Join-Path $scratch 'fixtures/claude'
$env:XDG_CONFIG_HOME = Join-Path $scratch 'fixtures/config'
$executable = Join-Path $scratch 'WhaleSwitch.exe'
$webview = Join-Path $scratch 'WhaleSwitchData/webview/EBWebView'
$process = Start-Process -FilePath $executable -WorkingDirectory $scratch -WindowStyle Hidden -PassThru

try {
    $deadline = [DateTime]::UtcNow.AddSeconds(35)
    do {
        if ($process.HasExited) {
            throw "The portable app exited during startup ($($process.ExitCode))."
        }
        if (Test-Path -LiteralPath $webview -PathType Container) {
            Start-Sleep -Seconds 2
            if ($process.HasExited) {
                throw "The portable app exited after WebView2 initialized ($($process.ExitCode))."
            }
            Write-Output 'The portable package started with WebView2 data beside the executable.'
            return
        }
        Start-Sleep -Milliseconds 400
    } while ([DateTime]::UtcNow -lt $deadline)
    throw 'The portable app did not initialize its WebView2 data directory.'
} finally {
    if (-not $process.HasExited) {
        Stop-Process -InputObject $process
        $null = $process.WaitForExit(5000)
    }
    $process.Dispose()
}
