$ErrorActionPreference = 'Stop'

$version = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
if ($version -notmatch '^\d+\.\d+\.\d+$') {
    throw 'Expected an app version in X.Y.Z format.'
}
$members = [ordered]@{
    'WhaleSwitch.exe' = (Resolve-Path -LiteralPath 'src-tauri/target/release/whaleswitch.exe').Path
    'README-Portable.md' = (Resolve-Path -LiteralPath 'docs/PORTABLE.md').Path
    'LICENSE' = (Resolve-Path -LiteralPath 'LICENSE').Path
}
$marker = 'WhaleSwitch.portable'
$outputDirectory = (New-Item -ItemType Directory -Path 'release-assets' -Force).FullName
$portableName = "WhaleSwitch-$version-windows-x64-portable.zip"
$destination = Join-Path $outputDirectory $portableName
$staged = Join-Path $outputDirectory ("portable-" + [guid]::NewGuid().ToString('N') + '.zip')

try {
    $archive = [IO.Compression.ZipFile]::Open($staged, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($member in $members.GetEnumerator()) {
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
                $archive, $member.Value, $member.Key, [IO.Compression.CompressionLevel]::Optimal
            ) | Out-Null
        }
        $archive.CreateEntry($marker) | Out-Null
    } finally {
        $archive.Dispose()
    }

    $verification = [IO.Compression.ZipFile]::OpenRead($staged)
    try {
        $expected = @($members.Keys) + $marker
        $actual = @($verification.Entries | ForEach-Object FullName)
        if ($actual.Length -ne $expected.Length -or (Compare-Object $expected $actual -CaseSensitive)) {
            throw 'The portable ZIP entry set is incorrect.'
        }
        if ($verification.GetEntry($marker).Length -ne 0) {
            throw 'The portable mode marker must be empty.'
        }
        foreach ($member in $members.GetEnumerator()) {
            if ($verification.GetEntry($member.Key).Length -ne (Get-Item -LiteralPath $member.Value).Length) {
                throw 'A portable ZIP entry has a different size from its source.'
            }
        }
    } finally {
        $verification.Dispose()
    }

    if ([IO.Path]::GetDirectoryName($destination) -ne $outputDirectory -or
        [IO.Path]::GetDirectoryName($staged) -ne $outputDirectory) {
        throw 'Unexpected portable ZIP destination.'
    }
    [IO.File]::Move($staged, $destination, $true)
} finally {
    if (Test-Path -LiteralPath $staged -PathType Leaf) {
        Remove-Item -LiteralPath $staged
    }
}
