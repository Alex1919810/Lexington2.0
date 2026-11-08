[CmdletBinding()]
param([string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
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
    foreach ($name in @('install-windows.ps1', 'package-windows.ps1', 'integrations.py')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $source 'scripts')
    }
    Copy-Item -LiteralPath (Join-Path $root 'integrations') -Destination $source -Recurse
    [IO.File]::WriteAllText((Join-Path $source 'README.md'), 'fixture')
    [IO.File]::WriteAllText((Join-Path $source 'Cargo.toml'), 'fixture')
    [IO.File]::WriteAllText((Join-Path $source 'target\release\lexington.exe'), 'fixture version one')
    [IO.File]::WriteAllText((Join-Path $source 'resource\fixture.txt'), 'resource one')
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
    $refused = $false
    try { & $installer -InstallDir $source -SkipBuild -NoShortcuts -NoIntegrations } catch { $refused = $true }
    Assert-True $refused 'Installer accepted the source checkout as destination.'
    [IO.File]::WriteAllText((Join-Path $source 'config\config.json'), '{"codex":{"apiKey":"fixture-only-not-real"}}')
    & (Join-Path $source 'scripts\package-windows.ps1') -SkipBuild -Python $Python
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
    } finally { $archive.Dispose() }
    $unpacked = Join-Path $fixture 'unpacked release'
    Expand-Archive -LiteralPath $archivePath -DestinationPath $unpacked
    $packageInstall = Join-Path $fixture 'package installed'
    & (Join-Path $unpacked 'scripts\install-windows.ps1') -InstallDir $packageInstall -NoShortcuts -NoIntegrations -NoRestart -Python 'deliberately-unavailable-python'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $packageInstall 'lexington.exe')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $installed 'lexington.exe')).Hash) 'Source-free package install mismatch.'
    Assert-True ((Get-Content -LiteralPath (Join-Path $packageInstall 'config\config.json') -Raw).Trim() -eq '{}') 'Source-free install configuration mismatch.'
    Assert-True ((Get-Content -LiteralPath (Join-Path $packageInstall 'resource\agents\fixture.md') -Raw) -eq 'fixture persona two') 'Source-free install lost agent Markdown.'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $packageInstall 'resource\agents\characters.json')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $agentDirectory 'characters.json')).Hash) 'Source-free install lost agent mapping.'
    Write-Output 'Windows install/upgrade/preservation/hash/package fixture checks passed.'
} finally {
    $resolved = [IO.Path]::GetFullPath($fixture)
    if ($resolved.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf).StartsWith('lexington-windows-test-')) {
        if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
    }
}
