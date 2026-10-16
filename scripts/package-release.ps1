# Run cargo build --release first. All outputs stay in target/release.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$releaseRoot = Join-Path $projectRoot 'target/release'
$executable = Join-Path $releaseRoot 'mori.exe'
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw 'Missing target/release/mori.exe. Run cargo build --release first.'
}

$staging = Join-Path $releaseRoot ('package-' + [Guid]::NewGuid().ToString('N'))
$archive = Join-Path $releaseRoot 'mori-windows-x86_64.zip'
try {
    New-Item -ItemType Directory -Path (Join-Path $staging 'assets') | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $staging 'mori.exe')
    foreach ($name in @('README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $staging
    }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'assets/licenses') -Destination (Join-Path $staging 'assets') -Recurse
    New-Item -ItemType Directory -Path (Join-Path $staging 'docs') | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/RELEASING.md') -Destination (Join-Path $staging 'docs/RELEASING.md')
    $items = @(Get-ChildItem -LiteralPath $staging | ForEach-Object { $_.FullName })
    Compress-Archive -LiteralPath $items -DestinationPath $archive -CompressionLevel Optimal -Force
    $checksums = foreach ($file in @($executable, $archive)) {
        $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $([IO.Path]::GetFileName($file))"
    }
    [IO.File]::WriteAllText((Join-Path $releaseRoot 'SHA256SUMS.txt'), ($checksums -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
}
finally {
    if (Test-Path -LiteralPath $staging) {
        $resolvedStaging = (Resolve-Path -LiteralPath $staging).Path
        $resolvedRelease = (Resolve-Path -LiteralPath $releaseRoot).Path
        if (-not $resolvedStaging.StartsWith($resolvedRelease + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Refusing to remove staging files outside target/release.'
        }
        Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
    }
}
Write-Output "Release files: $releaseRoot"
