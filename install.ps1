$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'Atingaii/token-monitor'
$Base = if ($env:TOKEN_MONITOR_RELEASE_BASE) { $env:TOKEN_MONITOR_RELEASE_BASE.TrimEnd('/') } else { "https://github.com/$Repo/releases/latest/download" }

# Windows PowerShell 5.1 can otherwise negotiate an obsolete TLS protocol on
# older machines/profiles when downloading from GitHub.
if ($PSVersionTable.PSEdition -eq 'Desktop') {
  try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
  } catch {
    # Modern PowerShell/.NET chooses TLS automatically; failure here is harmless.
  }
}

function Get-TokenMonitorArchitecture {
  try {
    $value = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
    if ($value) { return $value }
  } catch {}

  $fallback = "$env:PROCESSOR_ARCHITECTURE".ToLowerInvariant()
  switch ($fallback) {
    'amd64' { return 'x64' }
    'x86' { return 'x86' }
    'arm64' { return 'arm64' }
    default { return $fallback }
  }
}

$Arch = Get-TokenMonitorArchitecture
switch ($Arch) {
  'x64' { $AssetArch = 'x86_64' }
  'amd64' { $AssetArch = 'x86_64' }
  'arm64' { $AssetArch = 'aarch64' }
  default { throw "Unsupported Windows architecture: $Arch" }
}

$Stem = "token-monitor-windows-$AssetArch"
$Asset = "$Stem.zip"
$Checksum = "$Stem.sha256"

$LocalAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
if (-not $LocalAppData) { $LocalAppData = $env:LOCALAPPDATA }
if (-not $LocalAppData) { $LocalAppData = Join-Path $HOME 'AppData\Local' }
$InstallDir = if ($env:TOKEN_MONITOR_INSTALL_DIR) { $env:TOKEN_MONITOR_INSTALL_DIR } else { Join-Path $LocalAppData 'TokenMonitor\bin' }
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ("token-monitor-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Temp, $InstallDir | Out-Null

function Normalize-PathEntry([string]$Value) {
  if (-not $Value) { return '' }
  try { return ([System.IO.Path]::GetFullPath($Value)).TrimEnd('\').ToLowerInvariant() } catch { return $Value.TrimEnd('\').ToLowerInvariant() }
}

try {
  $Zip = Join-Path $Temp $Asset
  $ChecksumPath = Join-Path $Temp $Checksum
  Invoke-WebRequest -Uri "$Base/$Asset" -OutFile $Zip
  Invoke-WebRequest -Uri "$Base/$Checksum" -OutFile $ChecksumPath

  $Expected = ((Get-Content -Raw $ChecksumPath).Trim() -split '\s+')[0].ToLowerInvariant()
  $Actual = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLowerInvariant()
  if (-not $Expected -or $Expected -ne $Actual) {
    throw "SHA-256 mismatch for $Asset. Expected $Expected, got $Actual"
  }

  Expand-Archive -Force -Path $Zip -DestinationPath $Temp
  $Binary = Join-Path $InstallDir 'token-monitor.exe'
  Copy-Item -Force (Join-Path $Temp 'token-monitor.exe') $Binary

  # Validate the native executable before mutating PATH.
  & $Binary --version
  if ($LASTEXITCODE -ne 0) { throw "Downloaded token-monitor.exe failed to run (exit $LASTEXITCODE)" }

  $Wanted = Normalize-PathEntry $InstallDir
  $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  $UserParts = @($UserPath -split ';' | Where-Object { $_ -and $_.Trim() })
  $UserNormalized = @($UserParts | ForEach-Object { Normalize-PathEntry $_ })
  if ($UserNormalized -notcontains $Wanted) {
    $NewPath = (($UserParts + $InstallDir) -join ';')
    [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
    Write-Host "Added $InstallDir to your user PATH."
  }

  $ProcessParts = @($env:Path -split ';' | Where-Object { $_ -and $_.Trim() })
  $ProcessNormalized = @($ProcessParts | ForEach-Object { Normalize-PathEntry $_ })
  if ($ProcessNormalized -notcontains $Wanted) {
    $env:Path = "$InstallDir;$env:Path"
  }

  Write-Host "`nInstalled: $Binary"
  Write-Host 'First device: token-monitor setup'
  Write-Host "If your host launched this installer in a child PowerShell, run the absolute command instead:"
  Write-Host "  `"$Binary`" setup"
  Write-Host "Additional device: paste the 'token-monitor join ...' command printed by an existing device"
}
finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Temp
}
