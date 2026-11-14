[CmdletBinding()]
param([Parameter(Mandatory)][string]$Fixture, [Parameter(Mandatory)][string]$Source, [Parameter(Mandatory)][string]$Wix)
$ErrorActionPreference = 'Stop'
function Assert-Msi($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Read-MsiColumn($Database, [string]$Query) {
    $view = $Database.OpenView($Query)
    try {
        $view.Execute() | Out-Null
        while ($record = $view.Fetch()) {
            try { $record.StringData(1) } finally { [Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) | Out-Null }
        }
    } finally {
        $view.Close() | Out-Null
        [Runtime.InteropServices.Marshal]::FinalReleaseComObject($view) | Out-Null
    }
}
function Invoke-Msi([string]$Arguments) {
    $process = Start-Process -FilePath msiexec.exe -ArgumentList $Arguments -WindowStyle Hidden -PassThru -Wait
    if ($process.ExitCode -notin @(0, 3010)) { throw "Isolated MSI operation failed: $($process.ExitCode)" }
}
$fixtureRoot = [IO.Path]::GetFullPath($Fixture)
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
Assert-Msi ($fixtureRoot.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path $fixtureRoot -Leaf).StartsWith('lexington-windows-test-')) 'MSI tests require an isolated fixture directory.'
$engine = New-Object -ComObject WindowsInstaller.Installer
$productionMsi = Join-Path $Source 'dist\lexington-amd64-windows.msi'
$database = $engine.OpenDatabase($productionMsi, 0)
try {
$files = @(Read-MsiColumn $database 'SELECT `FileName` FROM `File`' | ForEach-Object { $_.Split('|')[-1] })
Assert-Msi ($files -contains 'lexington.exe' -and $files -contains 'lexington-native-host.exe') 'MSI executable aliases are missing.'
Assert-Msi (@($files | Where-Object { $_ -match 'lexington-context.vsix' }).Count -eq 1) 'MSI VSIX is missing.'
$properties = @(Read-MsiColumn $database 'SELECT `Property` FROM `Property`')
Assert-Msi ($properties -notcontains 'ALLUSERS') 'MSI must be per-user.'
Assert-Msi ((Read-MsiColumn $database 'SELECT `Value` FROM `Property` WHERE `Property` = ''RESTARTMANAGERCONTROL''') -eq 'DisableShutdown') 'MSI may not shut down unrelated applications.'
Assert-Msi (@(Read-MsiColumn $database 'SELECT `Root` FROM `Registry`' | Where-Object { $_ -ne '1' }).Count -eq 0) 'MSI writes outside HKCU.'
Assert-Msi (@(Read-MsiColumn $database 'SELECT `DirProperty` FROM `RemoveFile`' | Where-Object { $_ -eq 'config' }).Count -eq 0) 'MSI must not remove user configuration.'
} finally {
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($database) | Out-Null
    [Runtime.InteropServices.Marshal]::FinalReleaseComObject($engine) | Out-Null
}
$stage = Get-ChildItem -LiteralPath (Join-Path $Source 'dist') -Directory -Filter 'lexington-windows-*' | Select-Object -First 1
[xml]$authoring = Get-Content -LiteralPath (Join-Path $stage.FullName 'installer.wxs') -Raw -Encoding utf8
$namespace = New-Object Xml.XmlNamespaceManager($authoring.NameTable)
$namespace.AddNamespace('w', 'http://wixtoolset.org/schemas/v4/wxs')
$payload = @($authoring.SelectNodes('//w:File', $namespace))
foreach ($file in $payload) {
    $relative = $file.Source.Substring($stage.FullName.Length + 1).Replace('\', '/')
    Assert-Msi ($relative -notmatch '^(config|scripts|dist)/|(^|/)live2d-drafts/') 'MSI contains private configuration or authoring files.'
}
foreach ($required in @('resource/agents/fixture.md', 'resource/models/bge-small-zh-v1.5/model.onnx', 'resource/runtime/windows-x86_64/onnxruntime.dll')) {
    Assert-Msi (@($payload | Where-Object { $_.Source.Replace('\', '/').EndsWith('/' + $required) }).Count -eq 1) "MSI payload is missing $required"
}
$manifest = Get-Content -LiteralPath (Join-Path $stage.FullName 'integrations\native-chromium.json') -Raw -Encoding utf8 | ConvertFrom-Json
Assert-Msi ($manifest.path -eq '..\lexington-native-host.exe') 'MSI native manifest must be relocatable.'
$isolationId = [Guid]::NewGuid().ToString('N')
$registryPrefix = "Software\LexingtonFixture\$isolationId"
$package = $authoring.SelectSingleNode('//w:Package', $namespace)
$package.Name = "Lexington fixture $isolationId"
$package.UpgradeCode = [Guid]::NewGuid().ToString()
$authoring.SelectSingleNode('//w:Directory[@Id="INSTALLFOLDER"]', $namespace).Name = 'lexington-msi-fixture-' + $isolationId
foreach ($component in $authoring.SelectNodes('//w:Component', $namespace)) { $component.Guid = [Guid]::NewGuid().ToString() }
foreach ($registry in $authoring.SelectNodes('//w:RegistryValue', $namespace)) { $registry.Key = $registryPrefix + '\' + $registry.Key }
foreach ($search in $authoring.SelectNodes('//w:RegistrySearch', $namespace)) { $search.Key = $registryPrefix + '\' + $search.Key }
foreach ($shortcut in $authoring.SelectNodes('//w:Shortcut', $namespace)) { $shortcut.Directory = 'INSTALLFOLDER' }
$installed = Join-Path $fixtureRoot 'msi installed with spaces'
$activeProduct = $null
try {
    foreach ($version in @('2.0.0', '2.0.1', '2.0.1')) {
        $productCode = '{' + [Guid]::NewGuid().ToString().ToUpperInvariant() + '}'
        $package.SetAttribute('ProductCode', $productCode)
        $package.Version = $version
        $sequence = [Guid]::NewGuid().ToString('N')
        $wxs = Join-Path $fixtureRoot ($sequence + '.wxs')
        $msi = Join-Path $fixtureRoot ($sequence + '.msi')
        $authoring.Save($wxs)
        & $Wix build $wxs -arch x64 -pdbtype none -out $msi
        if ($LASTEXITCODE -ne 0) { throw 'Isolated MSI build failed.' }
        $log = Join-Path $fixtureRoot ($sequence + '.log')
        $destinationArgument = if ($version -eq '2.0.0') { " INSTALLFOLDER=`"$installed`"" } else { '' }
        Invoke-Msi "/i `"$msi`" /qn /norestart$destinationArgument /l*v `"$log`""
        $activeProduct = $productCode
        Assert-Msi ((Get-FileHash -LiteralPath (Join-Path $installed 'lexington.exe')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $stage.FullName 'lexington.exe')).Hash) 'MSI executable hash mismatch.'
        $config = Join-Path $installed 'config\config.json'
        $memory = Join-Path $installed 'config\roleplay-memory.json'
        if (-not (Test-Path -LiteralPath $config)) {
            New-Item -ItemType Directory -Path (Split-Path $config) -Force | Out-Null
            [IO.File]::WriteAllText($config, 'fixture-user-config-must-survive')
            [IO.File]::WriteAllText($memory, 'fixture-user-memory-must-survive')
        }
        Assert-Msi ((Get-Content -LiteralPath $config -Raw) -eq 'fixture-user-config-must-survive') 'MSI upgrade changed user configuration.'
        Assert-Msi ((Get-Content -LiteralPath $memory -Raw) -eq 'fixture-user-memory-must-survive') 'MSI upgrade changed user memory.'
        $key = "HKCU:\$registryPrefix\Software\Classes\*\shell\Lexington\command"
        Assert-Msi ((Get-Item -LiteralPath $key).GetValue('') -eq ('"' + $installed + '\lexington.exe" --ask-files "%1"')) 'MSI context command is not quoted correctly.'
        if ($version -eq '2.0.0') {
            [IO.File]::WriteAllText((Join-Path $stage.FullName 'lexington.exe'), 'fixture MSI upgrade binary')
            [IO.File]::WriteAllText((Join-Path $stage.FullName 'lexington-native-host.exe'), 'fixture MSI upgrade binary')
            [IO.File]::WriteAllText((Join-Path $stage.FullName 'resource\fixture.txt'), 'fixture MSI upgrade resource')
        } else {
            Assert-Msi ((Get-Content -LiteralPath (Join-Path $installed 'resource\fixture.txt') -Raw) -eq 'fixture MSI upgrade resource') 'MSI upgrade did not replace resource content.'
        }
    }
    $uninstallLog = Join-Path $fixtureRoot 'uninstall.log'
    Invoke-Msi "/x $activeProduct /qn /norestart /l*v `"$uninstallLog`""
    $activeProduct = $null
    Assert-Msi (-not (Test-Path -LiteralPath (Join-Path $installed 'lexington.exe'))) 'MSI uninstall left its executable behind.'
    Assert-Msi ((Get-Content -LiteralPath $config -Raw) -eq 'fixture-user-config-must-survive') 'MSI uninstall removed user configuration.'
    Assert-Msi ((Get-Content -LiteralPath $memory -Raw) -eq 'fixture-user-memory-must-survive') 'MSI uninstall removed user memory.'
    Assert-Msi (-not (Test-Path -LiteralPath $key)) 'MSI uninstall left a context command registered.'
    Write-Output 'Native MSI tables/install/upgrade/same-version upgrade/uninstall/config preservation checks passed.'
} finally {
    if ($activeProduct) { Invoke-Msi "/x $activeProduct /qn /norestart" }
    $testRegistry = "HKCU:\$registryPrefix"
    if ($testRegistry -like 'HKCU:\Software\LexingtonFixture\*' -and (Test-Path -LiteralPath $testRegistry)) {
        Remove-Item -LiteralPath $testRegistry -Recurse -Force
    }
}
