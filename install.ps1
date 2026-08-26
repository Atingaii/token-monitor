$ErrorActionPreference = 'Stop'
$Repo = 'Atingaii/token-monitor'
$Base = "https://github.com/$Repo/releases/latest/download"

$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
switch ($Arch) {
  'x64' { $AssetArch = 'x86_64' }
  'arm64' { $AssetArch = 'aarch64' }
  default { throw "Unsupported Windows architecture: $Arch" }
}

$Asset = "token-monitor-windows-$AssetArch.zip"
$InstallDir = if ($env:TOKEN_MONITOR_INSTALL_DIR) { $env:TOKEN_MONITOR_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'TokenMonitor\bin' }
$Temp = Join-Path ([System.IO.Path]::GetTempPath()) ("token-monitor-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Temp, $InstallDir | Out-Null
try {
  $Zip = Join-Path $Temp $Asset
  Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile $Zip
  Expand-Archive -Force -Path $Zip -DestinationPath $Temp
  Copy-Item -Force (Join-Path $Temp 'token-monitor.exe') (Join-Path $InstallDir 'token-monitor.exe')

  $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  $Parts = @($UserPath -split ';' | Where-Object { $_ })
  if ($Parts -notcontains $InstallDir) {
    $NewPath = (($Parts + $InstallDir) -join ';')
    [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
    Write-Host "Added $InstallDir to your user PATH."
  }
  if (($env:Path -split ';') -notcontains $InstallDir) { $env:Path = "$InstallDir;$env:Path" }

  & (Join-Path $InstallDir 'token-monitor.exe') --version
  Write-Host "`nInstalled: $(Join-Path $InstallDir 'token-monitor.exe')"
  Write-Host 'First device: token-monitor setup'
  Write-Host "Additional device: paste the 'token-monitor join ...' command printed by an existing device"
}
finally {
  Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Temp
}
