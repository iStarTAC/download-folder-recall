# Download Folder Recall

A Windows-focused Firefox extension that recalls download directories using exact URLs, site-specific identities, URL prefixes/wildcards, or hostname rules.

![Download Folder Recall icon](extension/icons/icon-128.png)

Firefox does **not** expose a supported way for an extension to choose the initial absolute directory of its native Save As dialog. See [the feasibility assessment](docs/feasibility.md) for the researched API boundary and design rationale. The experimental branch includes an opt-in Windows dialog assist as a deliberately narrow workaround.

## What works

- Normal Firefox downloads keep their normal Save As behavior.
- On completion, the extension learns the selected directory from Firefox's download record.
- GitHub releases, tags, repository pages, API asset URLs, raw files, and codeload links normalize to `owner/repository` when that identity is available.
- Nexus Mods pages and file tabs normalize by game slug, so all mods for `phantombladezero` can share one game folder.
- The toolbar popup shows the current page's identity and mapping. A native folder picker can create or update the mapping.
- Right-clicking a link and choosing **Download link to remembered folder** performs one Firefox download and then moves it to the mapped absolute directory.
- An optional setting intercepts ordinary left-clicks on recognized direct GitHub release and Nexus archive links. Modifier-key clicks retain normal Firefox behavior.
- An experimental Windows-only setting can instead arm the helper for one recognized click, wait for one new Firefox Save As dialog, and change only its address-bar folder. It never presses Save or changes the filename.
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

### Experimental Save As assistance

1. In Firefox settings, enable **Always ask you where to save files**.
2. In **Manage rules**, enable **Experimental: open Firefox Save As in the remembered folder**.
3. Left-click a recognized direct GitHub release or Nexus Mods archive link.
4. Confirm that the Save As dialog opened in the remembered directory, then choose whether to save or cancel normally.

The assist is one-shot and opt-in. Before replaying the click, the helper snapshots existing dialogs. It then accepts only one new visible `firefox.exe` common file dialog, focuses the upper address edit, verifies that focus did not land in the filename field, enters the mapped directory, and sends Enter to navigate the address field. It never invokes the dialog’s Save or Cancel button. It aborts on timeout, an ambiguous dialog, a missing directory, or a failed focus check. Modifier-key clicks bypass it.

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

9. Enable experimental Save As assistance and confirm that only the folder changes; the filename remains intact and Save is never activated automatically.
10. Open another Firefox dialog before clicking and confirm that an ambiguous or pre-existing dialog is not controlled.
11. Confirm that a modifier-key click and an unrecognized link retain normal Firefox behavior.

The project cannot automatically validate Firefox's native dialog behavior from unit tests. Experimental Save As assistance therefore requires these manual checks on each supported Windows/Firefox combination.

## Known limitations

- Firefox cannot open native Save As at an arbitrary extension-supplied absolute path.
- Opaque `objects.githubusercontent.com` URLs need GitHub page/referrer context or a custom rule; the repository cannot be recovered reliably from the opaque URL alone.
- Referrers may be empty. A download initiated outside a recognizable repository context falls back conservatively instead of guessing.
- Firefox's download history contains the staging path after a smart move; the native helper cannot rewrite Firefox's download record to the final path.
- Experimental Save As assistance depends on Windows common-dialog structure and keyboard focus behavior. Firefox or Windows UI changes can break it, so it is disabled by default and must fail closed.
- Export itself is a normal browser download and may be observed by automatic learning; disable learning temporarily if that is undesirable.
