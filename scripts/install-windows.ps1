[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'lexington'),
    [switch]$SkipBuild,
    [switch]$NoShortcuts,
    [switch]$NoIntegrations,
    [switch]$NoRestart,
    [string]$Python = 'python'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$installRoot = [IO.Path]::GetFullPath($InstallDir)
if ($installRoot -eq [IO.Path]::GetPathRoot($installRoot) -or $installRoot -eq $projectRoot) {
    throw 'Choose a dedicated installation directory, not a drive root or the source checkout.'
}
Push-Location $projectRoot
$temporaryCodex = $null
try {
    $source = Join-Path $projectRoot 'target\release\lexington.exe'
    if (Test-Path -LiteralPath (Join-Path $projectRoot 'Cargo.toml')) {
        if (-not $SkipBuild) {
            if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Install Rust stable with the MSVC toolchain and Visual Studio C++ build tools first.' }
            & cargo build --release --locked
            if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
        }
    } else { $source = Join-Path $projectRoot 'lexington.exe' }
    if (-not (Test-Path -LiteralPath $source)) { throw "Missing release binary: $source" }
    $codexSource = Join-Path $projectRoot 'codex'
    if (-not (Test-Path -LiteralPath $codexSource -PathType Container)) {
        if (-not (Test-Path -LiteralPath (Join-Path $projectRoot 'Cargo.toml'))) { throw 'Packaged release is missing bundled native Codex.' }
        $codexSource = Join-Path $projectRoot ('.tools\codex-stage-' + [Guid]::NewGuid().ToString('N'))
        $temporaryCodex = $codexSource
        & $Python -X utf8 scripts/windows_codex.py --destination $codexSource
        if ($LASTEXITCODE -ne 0) { throw 'Native Codex staging failed.' }
    }
    if (-not (Test-Path -LiteralPath (Join-Path $codexSource 'vendor\x86_64-pc-windows-msvc\bin\codex.exe') -PathType Leaf)) { throw 'Bundled native Codex is incomplete.' }
    $destination = Join-Path $installRoot 'lexington.exe'
    $running = @(Get-Process lexington -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq $destination })
    foreach ($process in $running) {
        if (-not $process.CloseMainWindow()) { throw 'Cannot request a safe shutdown of the installed Lexington. Close it from its menu and retry.' }
        if (-not $process.WaitForExit(15000)) { throw 'Lexington has not exited. No processes were forcibly terminated.' }
    }
    New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
    function Install-Binary([string]$Target) {
        $temporary = Join-Path $installRoot ('.install-' + [Guid]::NewGuid().ToString('N') + '.exe')
        try {
            Copy-Item -LiteralPath $source -Destination $temporary
            if (Test-Path -LiteralPath $Target) { [IO.File]::Replace($temporary, $Target, [NullString]::Value) }
            else { [IO.File]::Move($temporary, $Target) }
        } finally {
            if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
        }
        if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash) { throw 'Installed binary checksum mismatch.' }
    }
    Install-Binary $destination
    Install-Binary (Join-Path $installRoot 'lexington-native-host.exe')
    $codexDestination = Join-Path $installRoot 'codex'
    New-Item -ItemType Directory -Path $codexDestination -Force | Out-Null
    Get-ChildItem -LiteralPath $codexSource | Copy-Item -Destination $codexDestination -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'resource') -Destination $installRoot -Recurse -Force
    $configDirectory = Join-Path $installRoot 'config'
    New-Item -ItemType Directory -Force -Path $configDirectory | Out-Null
    $configPath = Join-Path $configDirectory 'config.json'
    if (-not (Test-Path -LiteralPath $configPath)) {
        Copy-Item -LiteralPath (Join-Path $projectRoot 'config\config.json') -Destination $configPath
    }
    if (-not $NoIntegrations) {
        $artifacts = Join-Path $projectRoot 'dist\integrations'
        if (-not (Test-Path -LiteralPath (Join-Path $artifacts 'lexington-context.vsix'))) {
            if (-not (Get-Command $Python -ErrorAction SilentlyContinue)) { throw 'Python 3 is required to build integration archives from source. Packaged releases include them.' }
            & $Python -X utf8 scripts/integrations.py build --output $artifacts
            if ($LASTEXITCODE -ne 0) { throw 'Integration archive build failed.' }
        }
        $integrationRoot = Join-Path $installRoot 'integrations'
        New-Item -ItemType Directory -Force -Path $integrationRoot | Out-Null
        $installedArtifacts = Join-Path $integrationRoot 'artifacts'
        New-Item -ItemType Directory -Force -Path $installedArtifacts | Out-Null
        Get-ChildItem -LiteralPath $artifacts | Copy-Item -Destination $installedArtifacts -Recurse -Force
        $utf8 = [Text.UTF8Encoding]::new($false)
        $common = @{ name = 'com.lexington.context'; description = 'Lexington context menu bridge'; path = (Join-Path $installRoot 'lexington-native-host.exe'); type = 'stdio' }
        $firefox = $common.Clone()
        $firefox.allowed_extensions = @('lexington-context@lexington.local')
        $firefoxManifest = Join-Path $integrationRoot 'native-firefox.json'
        [IO.File]::WriteAllText($firefoxManifest, ($firefox | ConvertTo-Json), $utf8)
        $manifest = Get-Content -LiteralPath (Join-Path $artifacts 'chromium\manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
        $digest = [Security.Cryptography.SHA256]::Create().ComputeHash([Convert]::FromBase64String($manifest.key))
        $hex = -join ($digest[0..15] | ForEach-Object { $_.ToString('x2') })
        $extensionId = -join ($hex.ToCharArray() | ForEach-Object { [char](97 + [Convert]::ToInt32([string]$_, 16)) })
        $chromium = $common.Clone()
        $chromium.allowed_origins = @("chrome-extension://$extensionId/")
        $chromiumManifest = Join-Path $integrationRoot 'native-chromium.json'
        [IO.File]::WriteAllText($chromiumManifest, ($chromium | ConvertTo-Json), $utf8)
        foreach ($browser in @('Google\Chrome', 'Chromium', 'Microsoft\Edge', 'BraveSoftware\Brave-Browser', 'Mozilla')) {
            $key = "HKCU:\Software\$browser\NativeMessagingHosts\com.lexington.context"
            New-Item -Path $key -Force | Out-Null
            Set-Item -LiteralPath $key -Value $(if ($browser -eq 'Mozilla') { $firefoxManifest } else { $chromiumManifest })
        }
        foreach ($kind in @('*', 'Directory')) {
            $key = "HKCU:\Software\Classes\$kind\shell\Lexington"
            New-Item -Path "$key\command" -Force | Out-Null
            Set-Item -LiteralPath $key -Value ([string]::Concat([char]0x770b, [char]0x770b, 'Lexington', [char]0x600e, [char]0x4e48, [char]0x8bf4))
            New-ItemProperty -LiteralPath $key -Name Icon -Value $destination -PropertyType String -Force | Out-Null
            Set-Item -LiteralPath "$key\command" -Value ('"' + $destination + '" --ask-files "%1"')
        }
        $code = Get-Command code -ErrorAction SilentlyContinue
        if ($code) {
            & $code.Source --install-extension (Join-Path $artifacts 'lexington-context.vsix') --force
            if ($LASTEXITCODE -ne 0) { Write-Warning 'Install integrations/artifacts/lexington-context.vsix manually in VS Code.' }
        }
    }
    if (-not $NoShortcuts) {
        $shell = New-Object -ComObject WScript.Shell
        foreach ($folder in @([Environment]::GetFolderPath('DesktopDirectory'), (Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs'))) {
            New-Item -ItemType Directory -Force -Path $folder | Out-Null
            $shortcut = $shell.CreateShortcut((Join-Path $folder 'Lexington.lnk'))
            $shortcut.TargetPath = $destination
            $shortcut.WorkingDirectory = $installRoot
            $shortcut.IconLocation = Join-Path $installRoot 'resource\icon\Normal.ico'
            $shortcut.Save()
        }
    }
    if ($running.Count -gt 0 -and -not $NoRestart) {
        Start-Process -FilePath $destination -WorkingDirectory $installRoot -WindowStyle Hidden | Out-Null
    }
    Write-Output "Installed and SHA256 verified: $destination"
    Write-Output 'Codex: bundled native CLI is available automatically; existing CLI, login and configuration are preserved.'
} finally {
    if ($temporaryCodex -and (Test-Path -LiteralPath $temporaryCodex)) {
        $resolved = [IO.Path]::GetFullPath($temporaryCodex)
        $toolsRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot '.tools')) + [IO.Path]::DirectorySeparatorChar
        if (-not $resolved.StartsWith($toolsRoot, [StringComparison]::OrdinalIgnoreCase) -or -not (Split-Path $resolved -Leaf).StartsWith('codex-stage-')) { throw 'Unsafe Codex staging cleanup path.' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    Pop-Location
}
