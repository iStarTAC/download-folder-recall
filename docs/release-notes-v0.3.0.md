## Download Folder Recall 0.3.0

- Adds opt-in experimental Save As folder assistance on Windows.
- Arms the helper only for recognized GitHub release and Nexus Mods archive clicks with a usable mapping.
- Waits for exactly one new Firefox common file dialog and changes only its address-bar folder.
- Preserves the filename and leaves Save or Cancel entirely to the user.
- Excludes pre-existing dialogs and aborts on ambiguity, timeout, missing destinations, or an unsafe focus position.
- Keeps the existing download-then-move mode available as a separate fallback.

### Windows helper

Download and extract `download-folder-recall-helper-windows-x64-0.3.0.zip`, then run `install.ps1 -SkipBuild` from PowerShell. The helper is currently unsigned, so Windows may show a publisher warning.

Save As assistance is disabled by default. Enable Firefox’s **Always ask you where to save files** setting, then enable **Experimental: open Firefox Save As in the remembered folder** from the add-on’s rule-management page.

### Firefox extension

Install the Mozilla-signed `download-folder-recall-0.3.0.xpi`. The helper must be installed separately for folder selection, mapped-folder checks, explicit moves, and experimental Save As assistance.
