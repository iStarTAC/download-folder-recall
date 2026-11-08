[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$HostName = 'com.example.smart_download_folder_memory'
$RegistryPath = "HKCU:\Software\Mozilla\NativeMessagingHosts\$HostName"
$InstallDirectory = Join-Path $env:LOCALAPPDATA 'SmartDownloadFolderMemory'
if (Test-Path -LiteralPath $RegistryPath) { Remove-Item -LiteralPath $RegistryPath -Recurse -Force }
if (Test-Path -LiteralPath $InstallDirectory) { Remove-Item -LiteralPath $InstallDirectory -Recurse -Force }
Write-Host 'Download Folder Recall native helper uninstalled.'
