$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$DefaultReleaseBase = 'https://github.com/Atingaii/token-monitor/releases/download/v1.1.0'
$ApiAssetBase = 'https://api.github.com/repos/Atingaii/token-monitor/releases/assets'
$Base = if ($env:TOKEN_MONITOR_RELEASE_BASE) { $env:TOKEN_MONITOR_RELEASE_BASE.TrimEnd('/') } else { $DefaultReleaseBase }

if ($PSVersionTable.PSEdition -eq 'Desktop') {
  try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
  } catch {}
}

function Get-TokenMonitorArchitecture {
  $raw = $env:PROCESSOR_ARCHITEW6432
  if ([string]::IsNullOrWhiteSpace($raw)) { $raw = $env:PROCESSOR_ARCHITECTURE }
  if (-not [string]::IsNullOrWhiteSpace($raw)) {
    switch ($raw.Trim().ToUpperInvariant()) {
      'AMD64' { return 'x64' }
      'ARM64' { return 'arm64' }
      'X86' {
        if ([Environment]::Is64BitOperatingSystem) { return 'x64' }
        return 'x86'
      }
    }
  }
  if ([Environment]::Is64BitOperatingSystem) { return 'x64' }
  return 'x86'
}

$Arch = Get-TokenMonitorArchitecture
switch ($Arch) {
  'x64' {
    $AssetArch = 'x86_64'
    $AssetId = '531094714'
    $ChecksumId = '531094708'
  }
  'arm64' {
    $AssetArch = 'aarch64'
    $AssetId = '531094703'
    $ChecksumId = '531094700'
  }
  default { throw "Unsupported Windows architecture: $Arch (Token Monitor requires 64-bit Windows)" }
}

$Stem = "token-monitor-windows-$AssetArch"
$Asset = "$Stem.zip"
$Checksum = "$Stem.sha256"

$LocalAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
if ([string]::IsNullOrWhiteSpace($LocalAppData)) { $LocalAppData = $env:LOCALAPPDATA }
if ([string]::IsNullOrWhiteSpace($LocalAppData)) {
  $profileHome = [Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)
  if ([string]::IsNullOrWhiteSpace($profileHome)) { throw 'Cannot determine the current Windows user profile directory.' }
  $LocalAppData = Join-Path $profileHome 'AppData\Local'
}
$InstallDir = if ($env:TOKEN_MONITOR_INSTALL_DIR) { $env:TOKEN_MONITOR_INSTALL_DIR } else { Join-Path $LocalAppData 'TokenMonitor\bin' }
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ("token-monitor-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Temp, $InstallDir | Out-Null

function Normalize-PathEntry([string]$Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) { return '' }
  try { return ([System.IO.Path]::GetFullPath($Value)).TrimEnd('\').ToLowerInvariant() }
  catch { return $Value.TrimEnd('\').ToLowerInvariant() }
}

function Get-GitHubApiAsset([string]$Id, [string]$OutFile) {
  $Headers = @{
    Accept = 'application/octet-stream'
    'X-GitHub-Api-Version' = '2022-11-28'
    'User-Agent' = 'token-monitor-installer/1.1.0'
  }
  Invoke-WebRequest -UseBasicParsing -Headers $Headers -Uri "$ApiAssetBase/$Id" -OutFile $OutFile
}

try {
  $Zip = Join-Path $Temp $Asset
  $ChecksumPath = Join-Path $Temp $Checksum
  if ($env:TOKEN_MONITOR_RELEASE_BASE) {
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile $Zip
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Checksum" -OutFile $ChecksumPath
  } else {
    Write-Host "Downloading $Asset from GitHub Releases API..."
    Get-GitHubApiAsset $AssetId $Zip
    Get-GitHubApiAsset $ChecksumId $ChecksumPath
  }

  $Expected = ((Get-Content -Raw $ChecksumPath).Trim() -split '\s+')[0].ToLowerInvariant()
  $Actual = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
  if ([string]::IsNullOrWhiteSpace($Expected) -or $Expected -ne $Actual) {
    throw "SHA-256 mismatch for $Asset. Expected $Expected, got $Actual"
  }

  Expand-Archive -Force -Path $Zip -DestinationPath $Temp
  $Binary = Join-Path $InstallDir 'token-monitor.exe'
  Copy-Item -Force (Join-Path $Temp 'token-monitor.exe') $Binary

  & $Binary --version
  if ($LASTEXITCODE -ne 0) { throw "Downloaded token-monitor.exe failed to run (exit $LASTEXITCODE)" }

  $Wanted = Normalize-PathEntry $InstallDir
  $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if ($null -eq $UserPath) { $UserPath = '' }
  $UserParts = @($UserPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  $UserNormalized = @($UserParts | ForEach-Object { Normalize-PathEntry $_ })
  if ($UserNormalized -notcontains $Wanted) {
    $NewPath = (($UserParts + $InstallDir) -join ';')
    [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
    Write-Host "Added $InstallDir to your user PATH."
  }

  $ProcessPath = if ($null -eq $env:Path) { '' } else { $env:Path }
  $ProcessParts = @($ProcessPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  $ProcessNormalized = @($ProcessParts | ForEach-Object { Normalize-PathEntry $_ })
  if ($ProcessNormalized -notcontains $Wanted) {
    $env:Path = if ($ProcessPath) { "$InstallDir;$ProcessPath" } else { $InstallDir }
  }

  Write-Host "`nInstalled: $Binary"
  Write-Host 'First device:'
  Write-Host '  token-monitor setup'
  Write-Host 'Existing v1.0 device:'
  Write-Host '  token-monitor password'
  Write-Host '  token-monitor sync --full'
  Write-Host 'If this installer ran in a child PowerShell and PATH is not visible yet, use:'
  Write-Host "  & `"$Binary`" setup"
  Write-Host "Additional device: paste the 'token-monitor join ...' command printed by an existing device"
}
finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Temp
}
