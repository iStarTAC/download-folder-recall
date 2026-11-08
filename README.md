# Download Folder Recall

A Windows-focused Firefox extension that recalls download directories using exact URLs, site-specific identities, URL prefixes/wildcards, or hostname rules.

![Download Folder Recall icon](extension/icons/icon-128.png)

Firefox does **not** expose a supported way for an extension to choose the initial absolute directory of its native Save As dialog. See [the feasibility assessment](docs/feasibility.md) for the researched API boundary and design rationale.

## What works

- Normal Firefox downloads keep their normal Save As behavior.
- On completion, the extension learns the selected directory from Firefox's download record.
- GitHub releases, tags, repository pages, API asset URLs, raw files, and codeload links normalize to `owner/repository` when that identity is available.
- Nexus Mods pages and file tabs normalize by game slug, so all mods for `phantombladezero` can share one game folder.
- The toolbar popup shows the current page's identity and mapping. A native folder picker can create or update the mapping.
- Right-clicking a link and choosing **Download link to remembered folder** performs one Firefox download and then moves it to the mapped absolute directory.
- An optional setting intercepts ordinary left-clicks on recognized direct GitHub release and Nexus archive links. Modifier-key clicks retain normal Firefox behavior.
- The toolbar badge shows `✓` when the current page has a usable remembered folder and `!` when its helper or directory needs attention.
- Exact, site identity (GitHub repository or Nexus Mods game), prefix/wildcard, and hostname rules resolve in that order.
- Rules can be added/deleted, sites disabled, learning disabled, and state imported/exported as JSON.
- Destination collisions are uniquified; files are not overwritten.

## Requirements

- Windows 11
- Firefox 142 or newer
- Rust toolchain (only needed to build the native helper from source)

## Install from a release

1. Install the signed `download-folder-recall-*.xpi` in Firefox.
2. Download `download-folder-recall-helper-windows-x64-*.zip` from [GitHub Releases](https://github.com/iStarTAC/download-folder-recall/releases/latest).
3. Extract the helper ZIP, open PowerShell in that directory, and run:

   ```powershell
   Set-ExecutionPolicy -Scope Process Bypass
   .\install.ps1 -SkipBuild
   ```

4. Restart Firefox. The toolbar shows `✓` on a mapped page when the helper and destination directory are ready.

## Install for development

1. In PowerShell from this repository, install the helper:

   ```powershell
   Set-ExecutionPolicy -Scope Process Bypass
   .\native\install.ps1
   ```

2. Open `about:debugging#/runtime/this-firefox` in Firefox.
3. Choose **Load Temporary Add-on**.
4. Select `extension/manifest.json`.
5. Pin **Download Folder Recall** to the toolbar if desired.

Temporary add-ons are removed when Firefox exits. For permanent installation, the extension must be packaged and signed by Mozilla; the fixed extension ID in `manifest.json` must remain the same because the native host allowlist uses it.

## Use

1. Download normally and choose a destination in Firefox. After completion, the extension learns it under the detected rule.
2. Open the toolbar popup to review or change the current page mapping. **Browse…** requires the native helper; a path can also be entered directly.
3. On a future page in the same repository/rule, right-click a download link and choose **Download link to remembered folder**.
4. Open **Manage rules** from the popup for custom rules, disabled sites, backup, and reset.

The context-menu command does not show Save As: it is the explicit alternative used because Firefox cannot seed Save As with the remembered absolute folder. Ordinary clicks still show Save As unless the opt-in left-click interception setting recognizes a direct download link.

## Development and tests

Run JavaScript checks and URL/rule tests:

```powershell
npm test
npm run check
```

Build and test the native helper:

```powershell
cargo test --manifest-path .\native\Cargo.toml
cargo build --release --manifest-path .\native\Cargo.toml
```

For extension debugging, use **Inspect** beside the temporary add-on in `about:debugging`. Native helper stderr appears in Firefox's Browser Console. The helper is registered at:

`HKCU\Software\Mozilla\NativeMessagingHosts\com.example.smart_download_folder_memory`

Uninstall it with:

```powershell
.\native\uninstall.ps1
```

## Validation status

Automated tests cover repository normalization across GitHub URL forms, Nexus Mods game normalization, owner separation, matching priority, wildcard/disabled-host behavior, configurable learning scopes, and native destination collision handling.

These scenarios require manual Firefox testing because they depend on browser UI, authentication, redirects, or the Windows filesystem:

1. Learn repository A into directory A.
2. Learn repository B into directory B, then explicitly download again from A.
3. Download multiple release assets from one repository.
4. Exercise a live GitHub asset redirect and an authenticated/API download.
5. Change a mapping with the popup.
6. Delete a destination and confirm the staged download remains in Firefox's download folder with an error badge/status.
7. Create a duplicate destination and confirm `(1)` uniquification.
8. Disable automatic learning or the entire extension and confirm normal Firefox behavior.

The project cannot automatically validate Firefox's native dialog behavior from unit tests, and it intentionally contains no UI automation or dialog hooks.

## Known limitations

- Firefox cannot open native Save As at an arbitrary extension-supplied absolute path.
- Opaque `objects.githubusercontent.com` URLs need GitHub page/referrer context or a custom rule; the repository cannot be recovered reliably from the opaque URL alone.
- Referrers may be empty. A download initiated outside a recognizable repository context falls back conservatively instead of guessing.
- Firefox's download history contains the staging path after a smart move; the native helper cannot rewrite Firefox's download record to the final path.
- Export itself is a normal browser download and may be observed by automatic learning; disable learning temporarily if that is undesirable.
