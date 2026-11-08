use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

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
}

fn default_collision() -> String {
    "uniquify".into()
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
    error: Option<String>,
}

impl Response {
    fn path(path: PathBuf) -> Self {
        Self {
            ok: true,
            path: Some(path.to_string_lossy().into_owned()),
            exists: None,
            is_directory: None,
            error: None,
        }
    }
    fn error(error: impl ToString) -> Self {
        Self {
            ok: false,
            path: None,
            exists: None,
            is_directory: None,
            error: Some(error.to_string()),
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
                error: None,
            }
        }
    }
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
    let output = serde_json::to_vec(&handle(request)).map_err(|error| error.to_string())?;
    io::stdout()
        .write_all(&(output.len() as u32).to_le_bytes())
        .map_err(|error| error.to_string())?;
    io::stdout()
        .write_all(&output)
        .map_err(|error| error.to_string())?;
    io::stdout().flush().map_err(|error| error.to_string())
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
