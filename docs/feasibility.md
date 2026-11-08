# Feasibility assessment

Research date: 8 November 2026. This assessment separates Mozilla-documented behavior from design inferences.

## Conclusion

The ideal workflow is not implementable through supported Firefox WebExtensions APIs. An extension can learn the final absolute path selected for a completed download, but it cannot tell Firefox's native Save As dialog to open at an arbitrary absolute Windows directory. Native messaging does not add a browser-dialog control API; changing Firefox internals, automating the dialog, or hooking Windows UI would be unsupported and fragile.

The implemented alternative therefore has two deliberately separate modes:

1. A normal click follows normal Firefox behavior. After completion, the extension reads the final path and learns its directory. It never moves that download.
2. The user can explicitly right-click a link and choose **Download link to remembered folder**. An optional setting can perform the same action for recognized direct-download left-clicks. Firefox downloads the link once, and after completion the Rust helper moves that file to the mapped absolute directory. Existing files are uniquified, never overwritten.

This preserves intent: selecting a different location in Firefox's Save As dialog teaches the new location instead of being silently undone.

## API findings

| API or question | Documented capability | Consequence |
|---|---|---|
| `downloads.onCreated` | Fires when a download begins and supplies a `DownloadItem`. | Useful for capturing source URL/referrer metadata, but it does not offer a filename suggestion callback. |
| `downloads.onChanged` | Fires when properties including `filename`, `url`, and `state` change. | Completion can trigger learning, and `downloads.search()` can retrieve the final item. |
| `downloads.onDeterminingFilename` | Not part of Firefox's documented `downloads` API. Mozilla Bug 1245652 remains the implementation request. | A Chrome-oriented design using `suggest()` cannot be used in Firefox. |
| `DownloadItem.filename` | An absolute local path. | The containing directory selected by the user can be learned without opening or enumerating the filesystem. |
| `DownloadItem.url` / `referrer` | Absolute source URL and referrer string. | These are useful identity hints, but referrer can be empty and redirects can replace or obscure repository identity. |
| `downloads.download()` | Can show a file chooser with `saveAs`; its `filename` must be relative to the default Downloads directory. Absolute paths and `..` are rejected. | It cannot initialize Save As at `D:\Mods\ProjectA` or download directly to an arbitrary absolute path. |
| `FilenameConflictAction` | Supports `uniquify`, `overwrite`, and `prompt`. | The extension requests `uniquify` for its browser-side staging download; the helper independently uniquifies at the final destination. |
| `storage.local` | Local persistent extension storage. | Mappings remain local; there is no cloud account or telemetry. Uninstall normally clears them, so JSON export is provided. |
| `nativeMessaging` | Exchanges JSON with a separately installed native process. | It enables folder picking, path validation, and filesystem moves. It does not grant control of Firefox's internal native dialog. |

Primary Mozilla references:

- [downloads API](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/downloads)
- [DownloadItem](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/downloads/DownloadItem)
- [downloads.download](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/downloads/download)
- [downloads.onCreated](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/downloads/onCreated)
- [downloads.onChanged](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/downloads/onChanged)
- [FilenameConflictAction](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/downloads/FilenameConflictAction)
- [storage.local](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/storage/local)
- [Native messaging](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging)
- [Mozilla Bug 1245652: implement onDeterminingFilename](https://bugzilla.mozilla.org/show_bug.cgi?id=1245652)

## Native Save As answers

1. **Read the selected absolute directory?** Yes, after the download record exposes its final `filename`. The implementation waits for completion and then derives the parent directory. It does not need native filesystem access to learn it.
2. **Influence the dialog's initial directory?** Not to an arbitrary directory through a supported WebExtensions API. `saveAs: true` opens a chooser; `filename` is only a default-download-directory-relative path.
3. **Reuse arbitrary absolute Windows paths?** Not as `downloads.download().filename`. A native application can use such paths for its own filesystem operations.
4. **Can native messaging influence Save As?** Native messaging itself only transports messages. A helper could attempt UI automation or manipulate undocumented Firefox state, but that would be a fragile unsupported hack and is intentionally not implemented.
5. **Learn from completed downloads without filesystem access?** Yes. `DownloadItem.filename` supplies the absolute path from the browser download record. Its `exists` field can be stale; learning the path does not depend on it.

## Redirects and GitHub

GitHub page, release, tag, and direct `github.com/{owner}/{repo}/...` URLs normalize to lowercase `owner/repository`. The same identity is extracted from:

- `api.github.com/repos/{owner}/{repo}/...`
- `raw.githubusercontent.com/{owner}/{repo}/...`
- `codeload.github.com/{owner}/{repo}/...`

`objects.githubusercontent.com` asset URLs generally contain opaque storage identifiers and cannot reliably reveal a repository. The extension therefore prefers the GitHub page/referrer and records recent link-click context on `github.com`. For the explicit context-menu action, `pageUrl` supplies a strong repository context. If neither context nor a recognizable URL is present, it deliberately falls back to a user rule or hostname rather than guessing a repository.

Referrers can be suppressed by policy, API downloads may be initiated without a repository page, and a redirect chain is not fully exposed by the downloads API. Those cases are limitations, not silently inferred identities.

## Nexus Mods

Nexus Mods URLs such as `www.nexusmods.com/phantombladezero/`, its mod pages, and file tabs normalize to the game slug `phantombladezero`. The alternate `/games/{game}` form is also recognized. Shared routes such as `/Core/...`, user pages, and generic site sections are excluded because treating those route names as games would create incorrect mappings.

Nexus download endpoints and CDN URLs do not always carry the game slug. As with opaque GitHub asset URLs, the implementation records the originating Nexus page when a link is clicked and uses the context-menu `pageUrl` for explicit smart downloads. If the browser supplies neither recognizable source nor page/referrer context, the game cannot be recovered reliably and the extension does not guess.

## Architecture choice

- Manifest V3 Firefox extension with a non-persistent module background page. Firefox supports MV3 background `scripts` but not `background.service_worker`; the manifest uses the Firefox-supported form.
- Plain JavaScript and browser APIs, with no runtime dependencies.
- `storage.local` for mappings and settings; `storage.session` for in-progress download actions so an event-page suspension does not lose a pending move.
- A short-lived Rust native-messaging executable. Firefox launches one instance per request. There is no daemon or background service.
- A Windows COM folder picker in Rust and standard-library file operations. Cross-volume moves use a checked copy-then-delete fallback.

## Security and data-loss properties

- Normal downloads are never moved.
- Smart moving requires the explicit context-menu command or the opt-in interception of a recognized direct-download left-click. Modifier-key clicks bypass interception.
- The destination must already exist and be a directory.
- Final-name collisions use `name (1).ext`, `name (2).ext`, and so on; no overwrite path is exposed in the UI.
- A failed move leaves the Firefox-downloaded file in place. A cross-volume copy is size-checked before removal of the source.
- Private downloads are not automatically learned.
- No network service, telemetry, or external account is used.
