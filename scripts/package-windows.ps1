[CmdletBinding()]
param([switch]$SkipBuild, [string]$Python = 'python', [string]$Wix = '', [switch]$PortableZip)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') { throw 'Windows packages currently require an x64 build host and x64 release binary.' }
    if (-not $Wix) {
        $Wix = Join-Path $root '.tools\wix\wix.exe'
        if (-not (Test-Path -LiteralPath $Wix)) {
            $command = Get-Command wix -ErrorAction SilentlyContinue
            if (-not $command) { throw 'Install WiX: dotnet tool install wix --version 5.0.2 --tool-path .tools/wix' }
            $Wix = $command.Source
        }
    }
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
    & $Python -X utf8 scripts/windows_codex.py --destination (Join-Path $stage 'codex')
    if ($LASTEXITCODE -ne 0) { throw 'Native Codex staging failed.' }
    New-Item -ItemType Directory -Path (Join-Path $stage 'config') -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $stage 'config\config.json'), "{}" + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath scripts/install-windows.ps1 -Destination (Join-Path $stage 'scripts')
    New-Item -ItemType Directory -Path (Join-Path $stage 'dist') -Force | Out-Null
    Copy-Item -LiteralPath dist/integrations -Destination (Join-Path $stage 'dist') -Recurse
    $authoring = Join-Path $stage 'installer.wxs'
    & $Python -X utf8 scripts/package-windows.py --stage $stage --output $authoring --cargo Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'MSI authoring failed.' }
    $builtMsi = Join-Path $stage 'lexington.msi'
    & $Wix build $authoring -arch x64 -pdbtype none -intermediatefolder (Join-Path $stage 'obj') -out $builtMsi
    if ($LASTEXITCODE -ne 0) { throw 'MSI build failed.' }
    $msi = Join-Path $root 'dist\lexington-amd64-windows.msi'
    if (Test-Path -LiteralPath $msi) {
        [IO.File]::Replace($builtMsi, $msi, [NullString]::Value)
    } else {
        [IO.File]::Move($builtMsi, $msi)
    }
    Write-Output $msi
    if (-not $PortableZip) { return }
    $archive = Join-Path $root ('dist\lexington-' + $env:PROCESSOR_ARCHITECTURE.ToLowerInvariant() + '-windows.zip')
    $archiveDeadline = [DateTime]::UtcNow.AddSeconds(12)
    while ($true) {
        try {
            $portableFiles = @('lexington.exe', 'resource', 'codex', 'README.md', 'config', 'scripts', 'dist') | ForEach-Object { Join-Path $stage $_ }
            Compress-Archive -Path $portableFiles -DestinationPath $archive -Force
            break
        } catch {
            $archiveError = $_.Exception.HResult -band 0xffff
            if ($archiveError -notin @(32, 33) -or [DateTime]::UtcNow -ge $archiveDeadline) { throw }
            Start-Sleep -Milliseconds 200
        }
    }
    Write-Output $archive
} finally { Pop-Location }
