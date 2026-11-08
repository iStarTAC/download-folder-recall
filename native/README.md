# Download Folder Recall Windows helper

The helper is a short-lived native-messaging application used for the Windows folder picker, directory availability checks, explicit user-authorized file moves, and the opt-in experimental Save As folder assist. It is not a service and does not remain running.

The Save As assist is deliberately limited: it snapshots existing windows, waits briefly for exactly one new visible common file dialog owned by `firefox.exe`, focuses and verifies the dialog's upper address edit, and navigates that edit to the mapped directory. It never invokes Save, Cancel, or a filename control. Any ambiguity or failed verification aborts the attempt.

## Install a release build

Extract the release ZIP, open PowerShell in the extracted directory, and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\install.ps1 -SkipBuild
```

This copies the executable and native manifest to `%LOCALAPPDATA%\SmartDownloadFolderMemory` and registers the manifest under the current user's `HKCU\Software\Mozilla\NativeMessagingHosts` key. Administrator access is not required.

## Uninstall

```powershell
.\uninstall.ps1
```

## Build from source

```powershell
cargo test --manifest-path .\Cargo.toml
cargo build --release --manifest-path .\Cargo.toml
.\install.ps1
```
