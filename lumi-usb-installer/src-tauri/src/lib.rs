use serde::Serialize;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

// =========================================================================
//  Beágyazott LUMI portable ZIP
// =========================================================================
// A build során a `lumi-usb-installer/src-tauri/embedded/lumi-portable.zip`
// fájl tartalmát BELESÜTI a binárisba a `include_bytes!` macro. Így az
// installer egyetlen fájl, semmi extra letöltés a user gépén.
//
// Helyi fejlesztéshez kézzel kell odamásolni egy ZIP-et; a CI workflow
// automatikusan teszi.
const LUMI_PORTABLE_ZIP: &[u8] =
    include_bytes!("../embedded/lumi-portable.zip");

// =========================================================================
//  Drive-lista
// =========================================================================

/// Egy elérhető drive a user gépén. A frontend ezeket listázza ki, hogy
/// melyikre települjön a LUMI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    /// pl. "F:"
    pub letter: String,
    /// pl. "F:\\"
    pub root: String,
    /// pl. "Kingston DataTraveler" vagy üres string ha nincs label
    pub label: String,
    /// "removable" (USB-pendrive, SD-kártya) / "fixed" (belső HDD/SSD,
    /// külső HDD USB-n) / "unknown"
    pub drive_type: String,
    /// Szabad tárhely byte-ban
    pub free_bytes: u64,
    /// Teljes tárhely byte-ban
    pub total_bytes: u64,
}

// Windows DRIVE_TYPE konstansok (a `GetDriveTypeW` által visszaadott u32
// értékek a Win32 spec szerint). A `windows` crate ezeket nem
// exportálja külön névvel, ezért itt definiáljuk.
#[cfg(windows)]
const DRIVE_REMOVABLE: u32 = 2;
#[cfg(windows)]
const DRIVE_FIXED: u32 = 3;

#[cfg(windows)]
fn list_drives_impl() -> Vec<DriveInfo> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives,
        GetVolumeInformationW,
    };

    let mut drives = Vec::new();
    let bitmask = unsafe { GetLogicalDrives() };

    for i in 0..26 {
        if (bitmask & (1 << i)) == 0 {
            continue;
        }
        let letter_char = (b'A' + i as u8) as char;
        let root = format!("{letter_char}:\\");
        let wide_root: Vec<u16> =
            root.encode_utf16().chain(std::iter::once(0)).collect();
        let pcwstr_root = PCWSTR(wide_root.as_ptr());

        let drive_type = unsafe { GetDriveTypeW(pcwstr_root) };
        let drive_type_str = match drive_type {
            DRIVE_REMOVABLE => "removable",
            DRIVE_FIXED => "fixed",
            _ => continue, // CD-ROM, hálózati, ramdisk - kihagyjuk
        };

        // Szabad / teljes hely
        let mut free_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;
        let free_ok = unsafe {
            GetDiskFreeSpaceExW(
                pcwstr_root,
                None,
                Some(&mut total_bytes),
                Some(&mut free_bytes),
            )
        };
        if free_ok.is_err() {
            // pl. üres kártyaolvasó, nem hozzáférhető - kihagyjuk
            continue;
        }

        // Volume label
        let mut name_buf = [0u16; 256];
        let label_result = unsafe {
            GetVolumeInformationW(
                pcwstr_root,
                Some(&mut name_buf),
                None,
                None,
                None,
                None,
            )
        };
        let label = if label_result.is_ok() {
            let len = name_buf.iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf16_lossy(&name_buf[..len])
        } else {
            String::new()
        };

        drives.push(DriveInfo {
            letter: format!("{letter_char}:"),
            root: root.clone(),
            label,
            drive_type: drive_type_str.to_string(),
            free_bytes,
            total_bytes,
        });
    }

    drives
}

#[cfg(not(windows))]
fn list_drives_impl() -> Vec<DriveInfo> {
    // Az USB-installer első körben Windows-only - macOS/Linux portable
    // élményt később adunk hozzá. Üres lista jelenleg.
    Vec::new()
}

#[tauri::command]
fn list_drives() -> Vec<DriveInfo> {
    let mut drives = list_drives_impl();
    // Removable-eket előre, aztán fixed (külső HDD-k), aztán a C: legalul.
    drives.sort_by_key(|d| match d.drive_type.as_str() {
        "removable" => 0,
        "fixed" => 1,
        _ => 2,
    });
    drives
}

// =========================================================================
//  Telepítés - a beágyazott ZIP kibontása a kiválasztott meghajtó-ra
// =========================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    /// 0..100
    percent: f32,
    /// Az éppen kibontott fájl neve (UI feedback)
    current_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub install_path: String,
}

#[tauri::command]
async fn install_to_drive(
    app: tauri::AppHandle,
    drive_root: String,
) -> Result<InstallResult, String> {
    // A felhasználó pl. "F:\\" gyökeret ad meg; a LUMI ennek a
    // `LUMI/` almappájába települ, hogy a pendrive többi tartalma
    // ne keveredjen össze.
    let install_dir = PathBuf::from(&drive_root).join("LUMI");
    std::fs::create_dir_all(&install_dir).map_err(|e| {
        format!("Nem sikerült létrehozni a mappát: {e}")
    })?;

    let cursor = Cursor::new(LUMI_PORTABLE_ZIP);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| format!("A beágyazott LUMI csomag sérült: {e}"))?;

    let total = archive.len();
    if total == 0 {
        return Err("A beágyazott LUMI csomag üres.".into());
    }

    // A ZIP-ben gyakran van egy szülő-mappa (pl. `LUMI-0.1.0-portable/`).
    // Ha az összes belépő ezzel kezdődik, "strip-eljük" - különben a
    // user `F:\LUMI\LUMI-0.1.0-portable\atman.exe`-t kapna a tisztább
    // `F:\LUMI\atman.exe` helyett.
    let common_prefix = detect_common_prefix(&mut archive)?;

    for i in 0..total {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("ZIP-olvasás hiba: {e}"))?;
        let name = file.name().to_string();

        // Strip-eljük a közös prefixet (ha van)
        let stripped = if let Some(p) = &common_prefix {
            name.strip_prefix(p).unwrap_or(&name).to_string()
        } else {
            name.clone()
        };
        if stripped.is_empty() {
            continue;
        }

        let out_path = install_dir.join(&stripped);
        if file.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| {
                format!("Mappa-létrehozás hiba ({}): {e}", out_path.display())
            })?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Mappa-létrehozás hiba ({}): {e}", parent.display())
                })?;
            }
            let mut out_file = std::fs::File::create(&out_path).map_err(|e| {
                format!("Írási hiba ({}): {e}", out_path.display())
            })?;
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = file
                    .read(&mut buf)
                    .map_err(|e| format!("Olvasási hiba: {e}"))?;
                if n == 0 {
                    break;
                }
                out_file
                    .write_all(&buf[..n])
                    .map_err(|e| format!("Írási hiba: {e}"))?;
            }
        }

        // Progress event 1%-onként legalább, de ne minden fájlnál spam-eljünk
        let percent = ((i + 1) as f32 / total as f32) * 100.0;
        let _ = app.emit(
            "install-progress",
            InstallProgress {
                percent,
                current_file: short_name(&stripped),
            },
        );
    }

    Ok(InstallResult {
        install_path: install_dir.display().to_string(),
    })
}

/// Megnyitja a Windows Intézőben a telepítés helyét. „Megnyitás" gomb.
#[tauri::command]
fn reveal_in_explorer(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
    Ok(())
}

/// Elindítja a frissen telepített `atman.exe`-t. „Elindítás" gomb.
#[tauri::command]
fn launch_lumi(install_path: String) -> Result<(), String> {
    let exe = PathBuf::from(&install_path).join("atman.exe");
    if !exe.exists() {
        return Err(format!("Nem található: {}", exe.display()));
    }
    std::process::Command::new(&exe)
        .current_dir(&install_path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// =========================================================================
//  Helpers
// =========================================================================

fn detect_common_prefix(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<Option<String>, String> {
    let mut prefix: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("ZIP-olvasás hiba: {e}"))?;
        let name = entry.name();
        let first_slash = name.find('/');
        let candidate = match first_slash {
            Some(idx) => &name[..=idx], // tartalmazza a `/`-t
            None => "",
        };
        if candidate.is_empty() {
            // gyökér-fájl, nincs közös prefix
            return Ok(None);
        }
        match &prefix {
            None => prefix = Some(candidate.to_string()),
            Some(p) if p == candidate => {}
            _ => return Ok(None),
        }
    }
    Ok(prefix)
}

fn short_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

// =========================================================================
//  Tauri bootstrap
// =========================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Pici hint: ha a beágyazott ZIP üres (a developer elfelejtette
            // odatenni a buildhez), próbáljunk legalább panic-mentes lenni.
            if LUMI_PORTABLE_ZIP.is_empty() {
                eprintln!("WARN: LUMI_PORTABLE_ZIP üres - lokál dev build?");
            }
            let _ = app.get_webview_window("main");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_drives,
            install_to_drive,
            reveal_in_explorer,
            launch_lumi,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
