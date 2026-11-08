[CmdletBinding()]
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$HostName = 'com.example.smart_download_folder_memory'
$InstallDirectory = Join-Path $env:LOCALAPPDATA 'SmartDownloadFolderMemory'

if (-not $SkipBuild) {
    cargo build --release --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'Rust helper build failed.' }
}

New-Item -ItemType Directory -Force -Path $InstallDirectory | Out-Null
$BundledExecutable = Join-Path $PSScriptRoot 'smart-download-native.exe'
$BuiltExecutable = Join-Path $PSScriptRoot 'target\release\smart-download-native.exe'
$Executable = if ($SkipBuild -and (Test-Path -LiteralPath $BundledExecutable)) { $BundledExecutable } else { $BuiltExecutable }
if (-not (Test-Path -LiteralPath $Executable)) { throw "Native helper not found at $Executable" }
Copy-Item -LiteralPath $Executable -Destination (Join-Path $InstallDirectory 'smart-download-native.exe') -Force

$ManifestPath = Join-Path $InstallDirectory "$HostName.json"
$Manifest = @{
    name = $HostName
    description = 'Native helper for Download Folder Recall'
    path = (Join-Path $InstallDirectory 'smart-download-native.exe')
    type = 'stdio'
    allowed_extensions = @('smart-download-folder-memory@example.local')
} | ConvertTo-Json -Depth 4
Set-Content -LiteralPath $ManifestPath -Value $Manifest -Encoding utf8

$RegistryPath = "HKCU:\Software\Mozilla\NativeMessagingHosts\$HostName"
New-Item -Path $RegistryPath -Force | Out-Null
Set-Item -Path $RegistryPath -Value $ManifestPath
Write-Host "Installed native helper at $InstallDirectory"
Write-Host 'Download Folder Recall helper installed. Restart Firefox if it is currently open.'
