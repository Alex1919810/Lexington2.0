[CmdletBinding()]
param([string]$Python = 'python', [string]$Wix = '')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Wix) { $Wix = Join-Path $root '.tools\wix\wix.exe' }
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$fixture = Join-Path $temporaryRoot ('lexington-windows-test-' + [Guid]::NewGuid().ToString('N'))
if (-not $fixture.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe fixture path.' }
function Assert-True($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
try {
    $unicode = [string]::Concat([char]0x4e2d, [char]0x6587)
    $source = Join-Path $fixture ('source with spaces ' + $unicode)
    $installed = Join-Path $fixture ('installed with spaces ' + $unicode)
    foreach ($directory in @('scripts', 'resource', 'config', 'target\release')) {
        New-Item -ItemType Directory -Path (Join-Path $source $directory) -Force | Out-Null
    }
    foreach ($name in @('install-windows.ps1', 'package-windows.ps1', 'package-windows.py', 'integrations.py')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $source 'scripts')
    }
    Copy-Item -LiteralPath (Join-Path $root 'integrations') -Destination $source -Recurse
    [IO.File]::WriteAllText((Join-Path $source 'README.md'), 'fixture')
    [IO.File]::WriteAllText((Join-Path $source 'Cargo.toml'), "[package]`nversion = `"2.0.0`"`n")
    New-Item -ItemType Directory -Path (Join-Path $source 'resource\icon') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'resource\icon\Normal.ico') -Destination (Join-Path $source 'resource\icon')
    [IO.File]::WriteAllText((Join-Path $source 'target\release\lexington.exe'), 'fixture version one')
    [IO.File]::WriteAllText((Join-Path $source 'resource\fixture.txt'), 'resource one')
    $modelDirectory = Join-Path $source 'resource\models\bge-small-zh-v1.5'
    New-Item -ItemType Directory -Path $modelDirectory -Force | Out-Null
    foreach ($name in @('model.onnx', 'config.json', 'tokenizer.json', 'tokenizer_config.json', 'special_tokens_map.json', 'manifest.json', 'LICENSE.txt')) {
        [IO.File]::WriteAllText((Join-Path $modelDirectory $name), ('fixture embedding ' + $name))
    }
    $runtimeDirectory = Join-Path $source 'resource\runtime\windows-x86_64'
    New-Item -ItemType Directory -Path $runtimeDirectory -Force | Out-Null
    foreach ($name in @('onnxruntime.dll', 'manifest.json', 'LICENSE', 'ThirdPartyNotices.txt')) {
        [IO.File]::WriteAllText((Join-Path $runtimeDirectory $name), ('fixture runtime ' + $name))
    }
    $agentDirectory = Join-Path $source 'resource\agents'
    New-Item -ItemType Directory -Path $agentDirectory -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $agentDirectory 'characters.json'), '{"defaultCharacter":"fixture","characters":{"fixture":"fixture.md"}}')
    [IO.File]::WriteAllText((Join-Path $agentDirectory 'fixture.md'), 'fixture persona one')
    [IO.File]::WriteAllText((Join-Path $source 'config\config.json'), '{"codex":{"apiKey":"fixture-only-not-real"}}')
    $installer = Join-Path $source 'scripts\install-windows.ps1'
    & $installer -InstallDir $installed -SkipBuild -NoShortcuts -NoIntegrations -NoRestart
    $config = Join-Path $installed 'config\config.json'
    $originalHash = (Get-FileHash -LiteralPath $config).Hash
    [IO.File]::WriteAllText((Join-Path $source 'target\release\lexington.exe'), 'fixture version two')
    [IO.File]::WriteAllText((Join-Path $source 'config\config.json'), '{}')
    [IO.File]::WriteAllText((Join-Path $source 'resource\fixture.txt'), 'resource two')
    [IO.File]::WriteAllText((Join-Path $agentDirectory 'fixture.md'), 'fixture persona two')
    & $installer -InstallDir $installed -SkipBuild -NoShortcuts -NoIntegrations -NoRestart
    Assert-True ((Get-FileHash -LiteralPath $config).Hash -eq $originalHash) 'Upgrade overwrote the installed configuration.'
    Assert-True ((Get-Content -LiteralPath (Join-Path $installed 'resource\fixture.txt') -Raw) -eq 'resource two') 'Resources did not update.'
    Assert-True ((Get-Content -LiteralPath (Join-Path $installed 'resource\agents\fixture.md') -Raw) -eq 'fixture persona two') 'Agent Markdown did not update.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $installed 'lexington.exe')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $source 'target\release\lexington.exe')).Hash) 'Binary mismatch.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $installed 'lexington-native-host.exe')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $installed 'lexington.exe')).Hash) 'Host alias mismatch.'
    Assert-True (@(Get-ChildItem -LiteralPath $installed -Filter '.install-*').Count -eq 0) 'Temporary binary leaked.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $installed 'resource\models\bge-small-zh-v1.5\model.onnx')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $modelDirectory 'model.onnx')).Hash) 'Installed embedding model mismatch.'
    $refused = $false
    try { & $installer -InstallDir $source -SkipBuild -NoShortcuts -NoIntegrations } catch { $refused = $true }
    Assert-True $refused 'Installer accepted the source checkout as destination.'
    [IO.File]::WriteAllText((Join-Path $source 'config\config.json'), '{"codex":{"apiKey":"fixture-only-not-real"}}')
    $archiveProbe = @{ Attempts = 0 }
    function Compress-Archive {
        [CmdletBinding()]
        param([string[]]$Path, [string]$DestinationPath, [switch]$Force)
        $archiveProbe.Attempts++
        if ($archiveProbe.Attempts -eq 1) {
            throw [IO.IOException]::new('Fixture sharing conflict', -2147024864)
        }
        Microsoft.PowerShell.Archive\Compress-Archive @PSBoundParameters
    }
    try {
        & (Join-Path $source 'scripts\package-windows.ps1') -SkipBuild -Python $Python -Wix $Wix -PortableZip
        Assert-True ($archiveProbe.Attempts -eq 2) 'ZIP did not retry a transient sharing conflict.'
    } finally {
        Remove-Item -LiteralPath Function:\Compress-Archive
    }
    & (Join-Path $PSScriptRoot 'test-windows-msi.ps1') -Fixture $fixture -Source $source -Wix $Wix
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archivePath = (Get-ChildItem -LiteralPath (Join-Path $source 'dist') -Filter '*.zip' | Select-Object -First 1).FullName
    $archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
    try {
        $configEntry = $archive.Entries | Where-Object { $_.FullName.Replace('\', '/') -eq 'config/config.json' }
        Assert-True ($null -ne $configEntry) 'Package configuration is missing.'
        $reader = [IO.StreamReader]::new($configEntry.Open())
        try { $packagedConfig = $reader.ReadToEnd() } finally { $reader.Dispose() }
        Assert-True ($packagedConfig.Trim() -eq '{}') 'Package contains source credentials.'
        Assert-True (@($archive.Entries | Where-Object { $_.Name -eq 'lexington-context.vsix' }).Count -eq 1) 'VSIX is missing.'
        Assert-True (@($archive.Entries | Where-Object { $_.FullName.Replace('\', '/') -eq 'resource/agents/fixture.md' }).Count -eq 1) 'Agent Markdown is missing from ZIP.'
        Assert-True (@($archive.Entries | Where-Object { $_.FullName.Replace('\', '/') -eq 'resource/models/bge-small-zh-v1.5/model.onnx' }).Count -eq 1) 'Embedding model is missing from ZIP.'
        Assert-True (@($archive.Entries | Where-Object { $_.FullName.Replace('\', '/') -eq 'resource/runtime/windows-x86_64/onnxruntime.dll' }).Count -eq 1) 'CPU runtime is missing from ZIP.'
    } finally { $archive.Dispose() }
    $unpacked = Join-Path $fixture 'unpacked release'
    Expand-Archive -LiteralPath $archivePath -DestinationPath $unpacked
    $packageInstall = Join-Path $fixture 'package installed'
    & (Join-Path $unpacked 'scripts\install-windows.ps1') -InstallDir $packageInstall -NoShortcuts -NoIntegrations -NoRestart -Python 'deliberately-unavailable-python'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $packageInstall 'lexington.exe')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $installed 'lexington.exe')).Hash) 'Source-free package install mismatch.'
    Assert-True ((Get-Content -LiteralPath (Join-Path $packageInstall 'config\config.json') -Raw).Trim() -eq '{}') 'Source-free install configuration mismatch.'
    Assert-True ((Get-Content -LiteralPath (Join-Path $packageInstall 'resource\agents\fixture.md') -Raw) -eq 'fixture persona two') 'Source-free install lost agent Markdown.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $packageInstall 'resource\agents\characters.json')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $agentDirectory 'characters.json')).Hash) 'Source-free install lost agent mapping.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $packageInstall 'resource\models\bge-small-zh-v1.5\model.onnx')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $modelDirectory 'model.onnx')).Hash) 'Source-free install lost embedding model.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $packageInstall 'resource\runtime\windows-x86_64\onnxruntime.dll')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $runtimeDirectory 'onnxruntime.dll')).Hash) 'Source-free install lost CPU runtime.'
    Write-Output 'Windows install/upgrade/preservation/hash/package fixture checks passed.'
} catch {
    Write-Error $_ -ErrorAction Continue
    throw
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    if ($resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf).StartsWith('lexington-windows-test-')) {
        [GC]::Collect()
        [GC]::WaitForPendingFinalizers()
        $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(12)
        while (Test-Path -LiteralPath $resolved) {
            try { Remove-Item -LiteralPath $resolved -Recurse -Force } catch {
                if (($_.Exception.HResult -band 0xffff) -notin @(32, 33) -or [DateTime]::UtcNow -ge $cleanupDeadline) { throw }
                Start-Sleep -Milliseconds 200
            }
        }
    }
}
