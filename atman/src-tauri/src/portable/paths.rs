use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub launch_root: String,
    pub data_dir: String,
    pub config_path: String,
    pub chats_dir: String,
    pub chats_db: String,
    pub chats_attachments: String,
    pub memory_dir: String,
    pub memory_documents: String,
    pub vectors_db: String,
    pub memory_notes_db: String,
    pub profile_dir: String,
    pub profile_db: String,
    pub logs_dir: String,
    pub models_akasha: String,
    pub models_embed: String,
    pub runtime_llama: String,
}

pub fn get_launch_root() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Nem sikerült meghatározni az indítási gyökeret.".to_string())
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

pub fn ensure_portable_layout() -> Result<AppPaths, String> {
    let launch_root = get_launch_root()?;
    let data_dir = launch_root.join("data");
    let chats_dir = data_dir.join("chats");
    let chats_attachments = chats_dir.join("attachments");
    let memory_dir = data_dir.join("memory");
    let memory_documents = memory_dir.join("documents");
    let profile_dir = data_dir.join("profile");
    let logs_dir = data_dir.join("logs");

    for dir in [
        &data_dir,
        &chats_dir,
        &chats_attachments,
        &memory_dir,
        &memory_documents,
        &profile_dir,
        &logs_dir,
        &launch_root.join("models").join("akasha"),
        &launch_root.join("models").join("embed"),
    ] {
        ensure_dir(dir)?;
    }

    let paths = AppPaths {
        launch_root: launch_root.display().to_string(),
        data_dir: data_dir.display().to_string(),
        config_path: data_dir.join("config.toml").display().to_string(),
        chats_dir: chats_dir.display().to_string(),
        chats_db: chats_dir.join("chats.db").display().to_string(),
        chats_attachments: chats_attachments.display().to_string(),
        memory_dir: memory_dir.display().to_string(),
        memory_documents: memory_documents.display().to_string(),
        vectors_db: memory_dir.join("vectors.db").display().to_string(),
        memory_notes_db: memory_dir.join("notes.db").display().to_string(),
        profile_dir: profile_dir.display().to_string(),
        profile_db: profile_dir.join("atman.db").display().to_string(),
        logs_dir: logs_dir.display().to_string(),
        models_akasha: launch_root
            .join("models")
            .join("akasha")
            .display()
            .to_string(),
        models_embed: launch_root
            .join("models")
            .join("embed")
            .display()
            .to_string(),
        runtime_llama: runtime_binary_path(&launch_root)?.display().to_string(),
    };

    Ok(paths)
}

pub fn runtime_binary_path(launch_root: &Path) -> Result<PathBuf, String> {
    let name = if cfg!(target_os = "windows") {
        "llama-server.exe"
    } else {
        "llama-server"
    };

    let sub = if cfg!(target_os = "windows") {
        "win-x64"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let path = launch_root.join("runtime").join(sub).join(name);
    if path.exists() {
        return Ok(path);
    }

    // Dev fallback: keressük felfelé a runtime/ mappát (pl. TOTAL AI gyökér)
    let mut dir: PathBuf = launch_root.to_path_buf();
    for _ in 0..6 {
        let candidate = dir.join("runtime").join(sub).join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }

    Ok(path)
}

pub fn resolve_path(launch_root: &Path, relative: &str) -> PathBuf {
    let p = Path::new(relative);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        launch_root.join(p)
    }
}

/// Windows-on visszaadja a path 8.3 short formáját (ASCII karakterek),
/// hogy az ékezetes / szóközös path-ot átadhassuk olyan külső process-eknek
/// (pl. llama-server), amelyek nem támogatják az UTF-8 fájlneveket Windowson.
/// Ha a konverzió nem sikerül (pl. a fájl/mappa még nem létezik, vagy a köteten
/// le van tiltva a 8.3 név), akkor None-t ad vissza.
/// Nem-Windows platformokon mindig az eredeti path-szal tér vissza.
#[cfg(windows)]
pub fn to_short_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf = vec![0u16; 1024];
    let len = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(buf.as_mut_slice())) };
    if len == 0 || (len as usize) > buf.len() {
        return None;
    }
    let os = OsString::from_wide(&buf[..len as usize]);
    Some(PathBuf::from(os))
}

#[cfg(not(windows))]
pub fn to_short_path(path: &Path) -> Option<PathBuf> {
    Some(path.to_path_buf())
}
