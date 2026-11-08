[CmdletBinding()]
param([switch]$SkipBuild, [string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    if (-not $SkipBuild) {
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
    & $Python -X utf8 scripts/integrations.py build --output dist/integrations
    if ($LASTEXITCODE -ne 0) { throw 'Integration build failed.' }
    foreach ($name in @('model.onnx', 'config.json', 'tokenizer.json', 'tokenizer_config.json', 'special_tokens_map.json', 'manifest.json', 'LICENSE.txt')) {
        if (-not (Test-Path -LiteralPath (Join-Path $root ('resource\models\bge-small-zh-v1.5\' + $name)) -PathType Leaf)) { throw "Missing bundled embedding asset: $name" }
    }
    foreach ($name in @('onnxruntime.dll', 'manifest.json', 'LICENSE', 'ThirdPartyNotices.txt')) {
        if (-not (Test-Path -LiteralPath (Join-Path $root ('resource\runtime\windows-x86_64\' + $name)) -PathType Leaf)) { throw "Missing bundled CPU runtime asset: $name" }
    }
    $stage = Join-Path $root ('dist\lexington-windows-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path (Join-Path $stage 'scripts') -Force | Out-Null
    Copy-Item -LiteralPath target/release/lexington.exe, resource, README.md -Destination $stage -Recurse
    New-Item -ItemType Directory -Path (Join-Path $stage 'config') -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $stage 'config\config.json'), "{}" + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath scripts/install-windows.ps1 -Destination (Join-Path $stage 'scripts')
    New-Item -ItemType Directory -Path (Join-Path $stage 'dist') -Force | Out-Null
    Copy-Item -LiteralPath dist/integrations -Destination (Join-Path $stage 'dist') -Recurse
    $archive = Join-Path $root ('dist\lexington-' + $env:PROCESSOR_ARCHITECTURE.ToLowerInvariant() + '-windows.zip')
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force
    Write-Output $archive
} finally { Pop-Location }
