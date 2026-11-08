use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Request {
    PickFolder {
        initial_directory: Option<String>,
    },
    MoveFile {
        source: String,
        destination_directory: String,
        #[serde(default = "default_collision")]
        collision_action: String,
    },
    PathStatus {
        path: String,
    },
    AssistSaveDialog {
        destination_directory: String,
        #[serde(default = "default_assist_timeout")]
        timeout_ms: u64,
    },
}

fn default_collision() -> String {
    "uniquify".into()
}

fn default_assist_timeout() -> u64 {
    10_000
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exists: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_directory: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    armed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn path(path: PathBuf) -> Self {
        Self {
            ok: true,
            path: Some(path.to_string_lossy().into_owned()),
            exists: None,
            is_directory: None,
            armed: None,
            completed: None,
            error: None,
        }
    }
    fn error(error: impl ToString) -> Self {
        Self {
            ok: false,
            path: None,
            exists: None,
            is_directory: None,
            armed: None,
            completed: None,
            error: Some(error.to_string()),
        }
    }

    fn armed() -> Self {
        Self {
            ok: true,
            path: None,
            exists: None,
            is_directory: None,
            armed: Some(true),
            completed: None,
            error: None,
        }
    }

    fn assist_result(result: Result<PathBuf, String>) -> Self {
        match result {
            Ok(path) => Self {
                ok: true,
                path: Some(path.to_string_lossy().into_owned()),
                exists: None,
                is_directory: None,
                armed: None,
                completed: Some(true),
                error: None,
            },
            Err(error) => Self {
                ok: false,
                path: None,
                exists: None,
                is_directory: None,
                armed: None,
                completed: Some(true),
                error: Some(error),
            },
        }
    }
}

fn unique_destination(directory: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let original = directory.join(file_name);
    if !original.exists() {
        return original;
    }
    let source = Path::new(file_name);
    let stem = source.file_stem().unwrap_or(file_name).to_string_lossy();
    let extension = source.extension().map(|value| value.to_string_lossy());
    for number in 1..=10_000 {
        let name = match &extension {
            Some(ext) => format!("{stem} ({number}).{ext}"),
            None => format!("{stem} ({number})"),
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{}-{}", stem, std::process::id()))
}

fn move_file(source: &Path, directory: &Path, collision_action: &str) -> Result<PathBuf, String> {
    if !source.is_file() {
        return Err(format!("Source file does not exist: {}", source.display()));
    }
    if !directory.is_dir() {
        return Err(format!(
            "Destination directory is unavailable: {}",
            directory.display()
        ));
    }
    let file_name = source.file_name().ok_or("Source has no filename")?;
    let direct = directory.join(file_name);

    let same_directory = source
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .zip(directory.canonicalize().ok())
        .is_some_and(|(left, right)| left == right);
    if source == direct || same_directory {
        return Ok(source.to_path_buf());
    }
    let destination = if direct.exists() {
        match collision_action {
            "uniquify" => unique_destination(directory, file_name),
            "error" => return Err(format!("Destination already exists: {}", direct.display())),
            _ => return Err("Unsupported collision action; use uniquify or error".into()),
        }
    } else {
        direct
    };

    match fs::rename(source, &destination) {
        Ok(()) => Ok(destination),
        Err(rename_error) => {
            let mut input = File::open(source).map_err(|error| {
                format!("Move failed ({rename_error}); source could not be opened ({error})")
            })?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|copy_error| {
                    format!(
                        "Move failed ({rename_error}); safe copy fallback failed ({copy_error})"
                    )
                })?;
            let copied = match io::copy(&mut input, &mut output) {
                Ok(value) => value,
                Err(error) => {
                    drop(output);
                    let _ = fs::remove_file(&destination);
                    return Err(format!("Copy fallback failed: {error}"));
                }
            };
            if let Err(error) = output.sync_all() {
                drop(output);
                let _ = fs::remove_file(&destination);
                return Err(format!("Copied file could not be flushed: {error}"));
            }
            let expected = match fs::metadata(source) {
                Ok(metadata) => metadata.len(),
                Err(error) => {
                    drop(output);
                    let _ = fs::remove_file(&destination);
                    return Err(error.to_string());
                }
            };
            if copied != expected {
                drop(output);
                let _ = fs::remove_file(&destination);
                return Err(format!(
                    "Copy was incomplete: copied {copied} of {expected} bytes"
                ));
            }
            drop(input);
            drop(output);
            fs::remove_file(source).map_err(|error| {
                format!(
                    "File was copied to {} but the original could not be removed: {error}",
                    destination.display()
                )
            })?;
            Ok(destination)
        }
    }
}

#[cfg(windows)]
fn pick_folder(initial: Option<&str>) -> Result<PathBuf, String> {
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
        FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
                    .map_err(|error| error.to_string())?;
            let options = dialog.GetOptions().map_err(|error| error.to_string())?;
            dialog
                .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
                .map_err(|error| error.to_string())?;
            if let Some(path) = initial.filter(|value| Path::new(value).is_dir()) {
                let wide = HSTRING::from(path);
                if let Ok(item) =
                    SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide.as_ptr()), None)
                {
                    let _ = dialog.SetFolder(&item);
                }
            }
            dialog.Show(None).map_err(|error| {
                if error.code().0 as u32 == 0x800704C7 {
                    "Folder selection canceled".into()
                } else {
                    error.to_string()
                }
            })?;
            let item = dialog.GetResult().map_err(|error| error.to_string())?;
            let raw: PWSTR = item
                .GetDisplayName(SIGDN_FILESYSPATH)
                .map_err(|error| error.to_string())?;
            let value = raw.to_string().map_err(|error| error.to_string());
            CoTaskMemFree(Some(raw.0.cast()));
            value.map(PathBuf::from)
        })();
        CoUninitialize();
        result
    }
}

#[cfg(not(windows))]
fn pick_folder(_: Option<&str>) -> Result<PathBuf, String> {
    Err("The folder picker is available on Windows only".into())
}

#[cfg(windows)]
fn window_class(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let mut buffer = [0_u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

#[cfg(windows)]
fn process_is_firefox(process_id: u32) -> bool {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
    {
        Ok(value) => value,
        Err(_) => return false,
    };
    let mut buffer = [0_u16; 1024];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    if result.is_err() {
        return false;
    }
    Path::new(&String::from_utf16_lossy(&buffer[..length as usize]))
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("firefox.exe"))
}

#[cfg(windows)]
unsafe extern "system" fn collect_firefox_dialog(
    hwnd: windows::Win32::Foundation::HWND,
    parameter: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};

    if IsWindowVisible(hwnd).as_bool() && window_class(hwnd) == "#32770" {
        let mut process_id = 0_u32;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_is_firefox(process_id) {
            let dialogs = &mut *(parameter.0 as *mut Vec<windows::Win32::Foundation::HWND>);
            dialogs.push(hwnd);
        }
    }
    windows::core::BOOL(1)
}

#[cfg(windows)]
fn firefox_file_dialogs() -> Result<Vec<windows::Win32::Foundation::HWND>, String> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;

    let mut dialogs = Vec::new();
    unsafe {
        EnumWindows(
            Some(collect_firefox_dialog),
            LPARAM((&mut dialogs as *mut Vec<_>) as isize),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(dialogs)
}

#[cfg(windows)]
fn send_key_chord(
    modifier: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };

    let keyboard = |virtual_key, flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [
        keyboard(modifier, Default::default()),
        keyboard(key, Default::default()),
        keyboard(key, KEYEVENTF_KEYUP),
        keyboard(modifier, KEYEVENTF_KEYUP),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(format!(
            "Windows accepted only {sent} of {} keyboard events",
            inputs.len()
        ))
    }
}

#[cfg(windows)]
fn focused_address_edit(
    dialog: windows::Win32::Foundation::HWND,
) -> Result<windows::Win32::Foundation::HWND, String> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetGUIThreadInfo, GetWindowRect, GetWindowThreadProcessId, IsChild, GUITHREADINFO,
    };

    let thread_id = unsafe { GetWindowThreadProcessId(dialog, None) };
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(thread_id, &mut info) }.map_err(|error| error.to_string())?;
    let focus = info.hwndFocus;
    if focus.0.is_null()
        || !unsafe { IsChild(dialog, focus) }.as_bool()
        || window_class(focus) != "Edit"
    {
        return Err("The Save As address field did not receive focus.".into());
    }

    let mut dialog_rect = RECT::default();
    let mut focus_rect = RECT::default();
    unsafe { GetWindowRect(dialog, &mut dialog_rect) }.map_err(|error| error.to_string())?;
    unsafe { GetWindowRect(focus, &mut focus_rect) }.map_err(|error| error.to_string())?;
    let dialog_height = (dialog_rect.bottom - dialog_rect.top).max(1);
    let focus_middle = (focus_rect.top + focus_rect.bottom) / 2;
    if focus_middle > dialog_rect.top + (dialog_height * 45 / 100) {
        return Err(
            "Focused edit field was not in the Save As address area; assistance was canceled."
                .into(),
        );
    }
    Ok(focus)
}

#[cfg(windows)]
fn navigate_dialog_to(
    dialog: windows::Win32::Foundation::HWND,
    destination: &Path,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, VK_CONTROL, VK_D, VK_L, VK_MENU,
        VK_RETURN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, SendMessageW, SetForegroundWindow, WM_SETTEXT,
    };

    if !unsafe { SetForegroundWindow(dialog) }.as_bool() {
        return Err("Windows would not foreground the Firefox Save As dialog.".into());
    }
    std::thread::sleep(Duration::from_millis(100));
    if unsafe { GetForegroundWindow() } != dialog {
        return Err("Firefox Save As did not remain the foreground window.".into());
    }

    send_key_chord(VK_MENU, VK_D)?;
    std::thread::sleep(Duration::from_millis(100));
    let edit = match focused_address_edit(dialog) {
        Ok(value) => value,
        Err(_) => {
            send_key_chord(VK_CONTROL, VK_L)?;
            std::thread::sleep(Duration::from_millis(100));
            focused_address_edit(dialog)?
        }
    };

    let wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SendMessageW(edit, WM_SETTEXT, None, Some(LPARAM(wide.as_ptr() as isize)));
    }
    if focused_address_edit(dialog)? != edit {
        return Err("The Save As address field lost focus before navigation.".into());
    }

    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_RETURN,
                ..Default::default()
            },
        },
    };
    let release = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_RETURN,
                dwFlags: windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP,
                ..Default::default()
            },
        },
    };
    let sent = unsafe { SendInput(&[input, release], std::mem::size_of::<INPUT>() as i32) };
    if sent != 2 {
        return Err("Windows did not accept the address navigation key.".into());
    }
    Ok(())
}

#[cfg(windows)]
fn assist_save_dialog(
    destination: &Path,
    timeout_ms: u64,
    baseline: &HashSet<usize>,
) -> Result<PathBuf, String> {
    if !destination.is_dir() {
        return Err(format!(
            "Destination directory is unavailable: {}",
            destination.display()
        ));
    }
    let timeout = Duration::from_millis(timeout_ms.clamp(3_000, 15_000));
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let candidates: Vec<_> = firefox_file_dialogs()?
            .into_iter()
            .filter(|hwnd| !baseline.contains(&(hwnd.0 as usize)))
            .collect();
        match candidates.as_slice() {
            [] => std::thread::sleep(Duration::from_millis(50)),
            [dialog] => {
                std::thread::sleep(Duration::from_millis(150));
                let confirmed: Vec<_> = firefox_file_dialogs()?
                    .into_iter()
                    .filter(|hwnd| !baseline.contains(&(hwnd.0 as usize)))
                    .collect();
                if confirmed.len() != 1 || confirmed[0] != *dialog {
                    return Err("The Firefox Save As dialog changed while being identified.".into());
                }
                navigate_dialog_to(*dialog, destination)?;
                return Ok(destination.to_path_buf());
            }
            _ => {
                return Err(
                    "More than one new Firefox dialog appeared; assistance was canceled.".into(),
                )
            }
        }
    }
    Err("No new Firefox Save As dialog appeared before the assist timed out.".into())
}

#[cfg(not(windows))]
fn assist_save_dialog(
    _: &Path,
    _: u64,
    _: &std::collections::HashSet<usize>,
) -> Result<PathBuf, String> {
    Err("Save As assistance is available on Windows only".into())
}

fn handle(request: Request) -> Response {
    match request {
        Request::PickFolder { initial_directory } => pick_folder(initial_directory.as_deref())
            .map(Response::path)
            .unwrap_or_else(Response::error),
        Request::MoveFile {
            source,
            destination_directory,
            collision_action,
        } => move_file(
            Path::new(&source),
            Path::new(&destination_directory),
            &collision_action,
        )
        .map(Response::path)
        .unwrap_or_else(Response::error),
        Request::PathStatus { path } => {
            let value = Path::new(&path);
            let exists = value.exists();
            let is_directory = value.is_dir();
            Response {
                ok: true,
                path: Some(path),
                exists: Some(exists),
                is_directory: Some(is_directory),
                armed: None,
                completed: None,
                error: None,
            }
        }
        Request::AssistSaveDialog { .. } => {
            Response::error("Save As assistance requires a native messaging port")
        }
    }
}

fn write_response(response: &Response) -> Result<(), String> {
    let output = serde_json::to_vec(response).map_err(|error| error.to_string())?;
    io::stdout()
        .write_all(&(output.len() as u32).to_le_bytes())
        .map_err(|error| error.to_string())?;
    io::stdout()
        .write_all(&output)
        .map_err(|error| error.to_string())?;
    io::stdout().flush().map_err(|error| error.to_string())
}

fn run() -> Result<(), String> {
    let mut length = [0_u8; 4];
    io::stdin()
        .read_exact(&mut length)
        .map_err(|error| error.to_string())?;
    let length = u32::from_le_bytes(length) as usize;
    if length > 4 * 1024 * 1024 {
        return Err("Request exceeds 4 MiB limit".into());
    }
    let mut input = vec![0_u8; length];
    io::stdin()
        .read_exact(&mut input)
        .map_err(|error| error.to_string())?;
    let request: Request = serde_json::from_slice(&input).map_err(|error| error.to_string())?;
    match request {
        Request::AssistSaveDialog {
            destination_directory,
            timeout_ms,
        } => {
            let destination = PathBuf::from(destination_directory);
            if !destination.is_dir() {
                return write_response(&Response::error(format!(
                    "Destination directory is unavailable: {}",
                    destination.display()
                )));
            }
            #[cfg(windows)]
            let baseline: HashSet<usize> = firefox_file_dialogs()?
                .into_iter()
                .map(|hwnd| hwnd.0 as usize)
                .collect();
            #[cfg(not(windows))]
            let baseline = std::collections::HashSet::new();
            write_response(&Response::armed())?;
            write_response(&Response::assist_result(assist_save_dialog(
                &destination,
                timeout_ms,
                &baseline,
            )))
        }
        other => write_response(&handle(other)),
    }
}

fn main() {
    if let Err(error) = run() {
        let response = serde_json::to_vec(&Response::error(error)).unwrap_or_default();
        let _ = io::stdout().write_all(&(response.len() as u32).to_le_bytes());
        let _ = io::stdout().write_all(&response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn uniquifies_collision_without_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "smart-download-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source_dir = root.join("source");
        let destination_dir = root.join("destination");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&destination_dir).unwrap();
        fs::write(source_dir.join("asset.zip"), b"new").unwrap();
        fs::write(destination_dir.join("asset.zip"), b"old").unwrap();
        let result =
            move_file(&source_dir.join("asset.zip"), &destination_dir, "uniquify").unwrap();
        assert_eq!(result.file_name().unwrap(), "asset (1).zip");
        assert_eq!(fs::read(destination_dir.join("asset.zip")).unwrap(), b"old");
        assert_eq!(fs::read(result).unwrap(), b"new");
        fs::remove_dir_all(root).unwrap();
    }
}
