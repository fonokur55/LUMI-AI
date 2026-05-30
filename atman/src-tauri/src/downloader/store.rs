// =========================================================================
//  Downloader - first-run modell és runtime letöltő
// =========================================================================
//  Az LUMI app első indításakor (vagy bármikor a Beállításokból) letölti
//  a hiányzó komponenseket:
//
//   - llama-server bináris (~30-46 MB, OS-tól függően) - a llama.cpp
//     GitHub release legfrissebbjéből
//
//   - Modellek tier × slot mátrixa (9 cella, lásd `catalog.rs`):
//       Light   × {Eco, Brain, Creative}  → ~4.2 GB összesen
//       Standard × {Eco, Brain, Creative} → ~14 GB összesen
//       Pro     × {Eco, Brain, Creative}  → ~22 GB összesen
//
//  Az első indításkor a hardware-detektálás alapján AJÁNLOTT tier
//  3 modelljét (Eco + Brain + Creative) tölti le egyben - minimum_ready
//  a recommended_tier teljes csomagja + a runtime megléte.
//
//  Más tier modelljei a Beállítások › Modellek menüből manuálisan
//  letölthetők, ha a user pl. forced_tier-rel egy másik tier-re vált.
//
//  Megszakíthatóság: ha a user kilép letöltés közben, a part-fájl nem
//  marad meg - a következő indításkor 0-ról kezdi az adott komponenst.
//  Az ELKÉSZÜLT komponensek megmaradnak.
// =========================================================================

use crate::akasha::types::AkashaSlot;
use crate::downloader::catalog::{self, ExpertEntry};
use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

// =========================================================================
//  Setup status check - v0.2.0
// =========================================================================
//  A v0.1.x 9-cellás mátrixa eltűnt. Most a 3 expert mindegyikéhez
//  egyetlen állapot tartozik (telepítve / nincs telepítve), + a runtime.
// =========================================================================

/// Egy expert telepítettsége. A frontend mind a 3-at megkapja:
///   - first-run / background-download UI ebből látja mi van még hátra
///   - ChatView slot-választó: ha `installed=false`, disable-li
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpertStatus {
    /// Slot azonosító (lowercase: "szoveg" / "logika" / "kod")
    pub slot: AkashaSlot,
    pub installed: bool,
    /// A `<slot>.gguf` fájl tényleges mérete a lemezen (vagy 0 ha nincs).
    /// Részleges letöltés monitorozására hasznos.
    pub installed_bytes: u64,
    /// Megjeleníthető név a katalógusból (pl. "Szöveg — Gemma 2 2B")
    pub display_name: &'static str,
    /// Rövid leírás
    pub description: &'static str,
    /// Várt méret GB-ban
    pub size_gb: f32,
    /// True, ha bundle-elt (telepítőben szállítva)
    pub bundled: bool,
}

/// A v0.2.0 SetupStatus: a 3 expert + a llama-server runtime állapota.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    /// A llama-server bináris megvan-e (>=50 KB)
    pub runtime_installed: bool,
    /// 3 expert állapota (fix sorrend: szoveg / logika / kod)
    pub experts: Vec<ExpertStatus>,
    /// True, ha a runtime + a SZÖVEG expert is telepítve van. Ennyi kell
    /// ahhoz, hogy a user beszélgethessen — a Logika+Kód jöhet háttérben.
    pub minimum_ready: bool,
    /// True, ha mind a 3 expert + a runtime is telepítve van.
    pub all_ready: bool,
}

impl SetupStatus {
    pub fn is_installed(&self, slot: AkashaSlot) -> bool {
        self.experts.iter().any(|e| e.slot == slot && e.installed)
    }
}

/// Akkor tekintünk egy modell-fájlt "telepítettnek", ha legalább 50 MB
/// (sérült/részleges letöltés kiszűrése).
const MODEL_MIN_BYTES: u64 = 50_000_000;
/// A llama-server bináris alsó küszöbe. v0.1.6-ban 500 KB volt, de az
/// új llama.cpp release-eknél a `llama-server.exe` egy ~80-150 KB-os
/// shim, ami DLL-ekből húzza a valódi logikát. A küszöb most 50 KB -
/// még a legkisebb shim is nagyobb ennél, üres/sérült fájlt viszont
/// továbbra is kiszűr.
const RUNTIME_MIN_BYTES: u64 = 50_000;

fn file_at_least(path: &Path, min_bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() >= min_bytes)
        .unwrap_or(false)
}

/// A runtime telepítettségének robusztus ellenőrzése. Először a kanonikus
/// `paths.runtime_llama` útvonalat nézi (pl. `runtime/win-x64/llama-server.exe`).
/// Ha az nincs meg, akkor a szülőmappát végigpásztázza, és minden olyan
/// fájlt elfogad amelynek a neve `llama-server`-rel kezdődik és átmegy
/// a méret-küszöbön. Ez kezeli azokat az eseteket, amikor:
///   - a llama.cpp release elnevezési konvenciója megváltozik
///   - a tar.gz/zip-ben a binárist más basename-mel kapjuk meg
///   - egy frissítés átírja a fájl nevét
fn is_runtime_installed(runtime_llama_path: &Path) -> bool {
    if file_at_least(runtime_llama_path, RUNTIME_MIN_BYTES) {
        return true;
    }
    let Some(parent) = runtime_llama_path.parent() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        // `llama-server`, `llama-server.exe`, `llama-server-vXYZ` stb.
        if name.starts_with("llama-server") && file_at_least(&path, RUNTIME_MIN_BYTES) {
            return true;
        }
    }
    false
}

/// v0.2.0 setup ellenőrzés: 3 expert + runtime.
/// Minimum_ready = runtime + Szöveg expert (a bundled). Ennyi kell ahhoz,
/// hogy a user már beszélgethessen, miközben Logika+Kód háttérben tölt.
pub fn check_setup_status(
    runtime_llama_path: &Path,
    models_akasha_dir: &Path,
) -> SetupStatus {
    let runtime_installed = is_runtime_installed(runtime_llama_path);

    let experts: Vec<ExpertStatus> = catalog::CATALOG
        .iter()
        .map(|entry: &ExpertEntry| {
            let path = models_akasha_dir.join(entry.local_filename());
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            ExpertStatus {
                slot: entry.slot,
                installed: bytes >= MODEL_MIN_BYTES,
                installed_bytes: bytes,
                display_name: entry.display_name,
                description: entry.description,
                size_gb: entry.size_gb,
                bundled: entry.bundled,
            }
        })
        .collect();

    let szoveg_ready = experts
        .iter()
        .find(|e| e.slot == AkashaSlot::Szoveg)
        .map(|e| e.installed)
        .unwrap_or(false);
    let all_models_ready = experts.iter().all(|e| e.installed);

    // #region agent log
    crate::debug_log::dlog(
        "store.rs:check_setup_status",
        "H1",
        "setup status",
        serde_json::json!({
            "runtime_llama_path": runtime_llama_path.display().to_string(),
            "runtime_installed": runtime_installed,
            "models_akasha_dir": models_akasha_dir.display().to_string(),
            "szoveg_ready": szoveg_ready,
            "all_ready": all_models_ready,
            "expert_status": experts.iter().map(|e| (e.slot.key(), e.installed, e.installed_bytes)).collect::<Vec<_>>(),
        }),
    );
    // #endregion

    SetupStatus {
        runtime_installed,
        experts,
        minimum_ready: runtime_installed && szoveg_ready,
        all_ready: runtime_installed && all_models_ready,
    }
}

// =========================================================================
//  Progress eventek
// =========================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    /// "runtime" | "eco" | "brain" | "creative"
    component: String,
    /// 0..100
    percent: f32,
    /// Letöltött byte-ok eddig
    downloaded_bytes: u64,
    /// Összes várt byte (lehet 0 ha nem ismert)
    total_bytes: u64,
    /// Sebesség MB/s (utolsó ~1 mp átlaga)
    speed_mbps: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadDone {
    component: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadError {
    component: String,
    message: String,
}

// =========================================================================
//  Online check - mielőtt letöltés indul, hadd lássuk van-e net
// =========================================================================

pub async fn is_online() -> bool {
    // Egyszerű HEAD a GitHub API-ra. 3 mp timeout - ha nincs net,
    // gyorsan kiderül, a wizard hibajelzéssel állít meg.
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .head("https://api.github.com")
        .send()
        .await
        .map(|r| r.status().is_success() || r.status().is_redirection())
        .unwrap_or(false)
}

// =========================================================================
//  Llama-server runtime letöltés (GitHub release - llama.cpp)
// =========================================================================

#[derive(serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize)]
struct GhRelease {
    assets: Vec<GhAsset>,
}

/// Aktuális OS + architektúra alapján kiválasztja a megfelelő llama.cpp
/// release asset-jét.
///
/// FORMÁTUMOK a llama.cpp release-en (b9413+):
///   - Windows: `.zip` (pl. `llama-b9413-bin-win-cpu-x64.zip`)
///   - macOS:   `.tar.gz` (pl. `llama-b9413-bin-macos-arm64.tar.gz`)
///   - Linux:   `.tar.gz` (pl. `llama-b9413-bin-ubuntu-x64.tar.gz`)
///
/// A `llama_asset_extension()` mondja meg melyik formátumot várjuk, és
/// a download_runtime az extension alapján választja a zip vagy tar.gz
/// extractort.
fn pick_llama_asset_name() -> Vec<&'static str> {
    if cfg!(target_os = "windows") {
        // Preferenciasor: tiszta CPU build → Vulkan → fallback.
        // Tipikus minta: "llama-b9413-bin-win-cpu-x64.zip"
        vec!["bin-win-cpu-x64", "bin-win-vulkan-x64", "bin-win-x64"]
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            // macOS ARM: "llama-b9413-bin-macos-arm64.tar.gz"
            vec!["bin-macos-arm64", "bin-macos-aarch64"]
        } else {
            vec!["bin-macos-x64", "bin-macos-amd64"]
        }
    } else {
        // Linux: "llama-b9413-bin-ubuntu-x64.tar.gz"
        vec!["bin-ubuntu-x64", "bin-linux-x64"]
    }
}

/// A platform-natív llama.cpp asset-formátuma.
/// - Windows: `.zip`
/// - macOS / Linux: `.tar.gz`
fn llama_asset_extension() -> &'static str {
    if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    }
}

pub async fn download_runtime(
    app: &AppHandle,
    runtime_dir: PathBuf,
) -> Result<(), String> {
    let component = "runtime";
    let _ = app.emit(
        "download-start",
        DownloadProgress {
            component: component.to_string(),
            percent: 0.0,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_mbps: 0.0,
        },
    );

    // 1. GitHub API: llama.cpp legutóbbi release-ének asset-listája
    let client = reqwest::Client::builder()
        .user_agent("LUMI/0.1 (downloader)")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("HTTP kliens hiba: {e}"))?;

    let release: GhRelease = client
        .get("https://api.github.com/repos/ggml-org/llama.cpp/releases/latest")
        .send()
        .await
        .map_err(|e| format!("Release-lista lekérése sikertelen: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Release-JSON parse hiba: {e}"))?;

    let patterns = pick_llama_asset_name();
    let expected_ext = llama_asset_extension(); // ".zip" Windows-on, ".tar.gz" máshol
    let asset = patterns
        .iter()
        .find_map(|pat| {
            release.assets.iter().find(|a| {
                let lower = a.name.to_lowercase();
                // SZIGORÚ feltételek:
                //   - tartalmazza a pattern-t (pl. "bin-macos-arm64")
                //   - az OS-natív formátum (".zip" Windows-on, ".tar.gz" máshol)
                //   - NEM aláírás vagy checksum (.sig, .sha256)
                //   - NEM CUDA-runtime csomag (nincs Nvidia GPU)
                a.name.contains(pat)
                    && lower.ends_with(expected_ext)
                    && !lower.ends_with(".sig")
                    && !lower.ends_with(".sha256")
                    && !a.name.starts_with("cudart-")
            })
        })
        .ok_or_else(|| {
            let available: Vec<&str> = release.assets.iter().map(|a| a.name.as_str()).collect();
            format!(
                "Nem található megfelelő {expected_ext} llama-server csomag az aktuális OS-hez. \
                 Próbáld manuálisan: https://github.com/ggml-org/llama.cpp/releases \n\
                 Keresett minták: {patterns:?}\nElérhető asset-ek: {available:?}"
            )
        })?;

    // 2. Letöltés egy temp fájlba (.zip vagy .tar.gz)
    let tmp_file = std::env::temp_dir().join(format!("lumi-llama-{}", asset.name));
    download_stream_to_file(
        app,
        component,
        &client,
        &asset.browser_download_url,
        &tmp_file,
    )
    .await?;

    // 3. Kibontás a runtime mappába - extension alapján zip vagy tar.gz
    let _ = app.emit(
        "download-extracting",
        DownloadDone {
            component: component.to_string(),
        },
    );
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("Mappa-létrehozás hiba: {e}"))?;
    if expected_ext == ".zip" {
        extract_zip_runtime(&tmp_file, &runtime_dir)?;
    } else {
        extract_tar_gz_runtime(&tmp_file, &runtime_dir)?;
    }
    // Temp fájl törlése
    let _ = std::fs::remove_file(&tmp_file);

    let _ = app.emit(
        "download-done",
        DownloadDone {
            component: component.to_string(),
        },
    );
    Ok(())
}

/// A llama.cpp ZIP-jén belül a `llama-server`(`.exe`) + futtatáshoz
/// szükséges DLL-ek/dylib-ek vannak egy almappában. Mindent egy szintre
/// másolunk a `runtime_dir`-be, hogy a meglévő `runtime_binary_path`
/// (paths.rs) megtalálja.
fn extract_zip_runtime(zip_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    // Sanity check: olvassuk be az első 4 byte-ot. Egy ZIP-fájl mindig
    // a "PK\x03\x04" signature-rel kezdődik. Ha mást találunk, sokkal
    // értelmesebb hibaüzenetet adunk mint a `zip` crate „Could not find
    // EOCD"-je (ami azt jelenti, a fájl egyáltalán nem ZIP).
    {
        use std::io::Read;
        let mut head = [0u8; 4];
        let n = std::fs::File::open(zip_path)
            .and_then(|mut f| f.read(&mut head))
            .map_err(|e| format!("Letöltött fájl nem nyitható meg: {e}"))?;
        if n < 4 || &head[..4] != b"PK\x03\x04" {
            return Err(format!(
                "A letöltött fájl nem ZIP formátumú (várt: PK\\x03\\x04, kapott: {:?}). \
                 Lehet, hogy a llama.cpp release-en az aktuális OS-hez .tar.gz formátum van, \
                 vagy a letöltés sérült. A fájl helye: {}",
                &head[..n],
                zip_path.display(),
            ));
        }
    }

    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("ZIP-megnyitás hiba: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("ZIP-olvasás hiba: {e}"))?;

    // Csak a fájlokat vesszük figyelembe (nem a könyvtárszerkezetet);
    // mindent a runtime_dir gyökerébe pakolunk a basename alapján.
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("ZIP-entry hiba: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        // A llama-server bináris és minden .dll/.dylib/.so kell;
        // README/LICENSE-eket kihagyjuk. A `.metallib` és `.metal`
        // macOS Metal GPU kernelek - Apple Silicon-on a llama-server
        // ezek nélkül startup-kor crash-el, így ezeket is megőrizzük
        // (Windows-on legfeljebb pár KB-os no-op, de safer így).
        let basename = Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let lower = basename.to_lowercase();
        let keep = lower.contains("llama-server")
            || lower.contains("llama.")
            || lower.ends_with(".dll")
            || lower.ends_with(".dylib")
            || lower.ends_with(".so")
            || lower.ends_with(".metallib")
            || lower.ends_with(".metal")
            || lower == "llama-server.exe"
            || lower == "llama-server";
        if !keep {
            continue;
        }
        let out_path = runtime_dir.join(basename);
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| format!("Írási hiba ({}): {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| format!("Kicsomagolási hiba: {e}"))?;
        drop(out_file);

        // Unix-on a llama-server-nek futtathatónak kell lennie.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if lower.contains("llama-server") || !lower.contains('.') {
                let _ = std::fs::set_permissions(
                    &out_path,
                    std::fs::Permissions::from_mode(0o755),
                );
            }
        }

        // macOS Gatekeeper fix - lásd extract_tar_gz_runtime kommentjét
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("xattr")
                .args(["-d", "com.apple.quarantine"])
                .arg(&out_path)
                .output();
        }
    }
    Ok(())
}

/// A llama.cpp macOS és Linux release-jei `.tar.gz` formátumúak. A
/// kibontás-logika ugyanaz mint a ZIP-é: minden `llama-server*`,
/// `*.dll`/`*.dylib`/`*.so` fájlt a `runtime_dir` gyökerébe pakolunk.
fn extract_tar_gz_runtime(tar_gz_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    // Sanity check: GZIP magic bytes (1f 8b)
    {
        use std::io::Read;
        let mut head = [0u8; 2];
        let n = std::fs::File::open(tar_gz_path)
            .and_then(|mut f| f.read(&mut head))
            .map_err(|e| format!("Letöltött fájl nem nyitható meg: {e}"))?;
        if n < 2 || head != [0x1f, 0x8b] {
            return Err(format!(
                "A letöltött fájl nem GZIP formátumú (várt: 1f8b, kapott: {head:?}). \
                 Lehet, hogy a letöltés sérült. A fájl helye: {}",
                tar_gz_path.display(),
            ));
        }
    }

    let file = std::fs::File::open(tar_gz_path)
        .map_err(|e| format!("Tar.gz-megnyitás hiba: {e}"))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let entries = archive
        .entries()
        .map_err(|e| format!("Tar-olvasás hiba: {e}"))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Tar-entry hiba: {e}"))?;

        // Csak a fájlokat fogadjuk (nem a könyvtárakat, nem symlinkeket)
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() {
            continue;
        }

        // A fájl path-ja a tar-ban (általában `build/bin/llama-server` vagy hasonló)
        let path_in_tar = entry
            .path()
            .map_err(|e| format!("Tar path hiba: {e}"))?
            .into_owned();
        let basename = path_in_tar
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let lower = basename.to_lowercase();

        // Csak a runtime-hoz szükséges fájlokat tartjuk meg.
        // FONTOS macOS-en: a `.metallib` (Apple Silicon Metal GPU kernel)
        // korábban kimaradt a filterből, ezért a llama-server startup-kor
        // crash-elt ("default.metallib not found"), és a frontend csak
        // "AKASHA szerver nem válaszolt időben"-ként látta. Most már bent
        // van. A `.metal` az újabb buildek runtime-kompilált forrása.
        let keep = lower.contains("llama-server")
            || lower.contains("llama.")
            || lower.ends_with(".dll")
            || lower.ends_with(".dylib")
            || lower.ends_with(".so")
            || lower.ends_with(".metallib")
            || lower.ends_with(".metal")
            || lower == "llama-server.exe"
            || lower == "llama-server";
        if !keep {
            continue;
        }

        let out_path = runtime_dir.join(basename);
        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| format!("Írási hiba ({}): {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out_file)
            .map_err(|e| format!("Tar kicsomagolási hiba: {e}"))?;
        drop(out_file); // Zárjuk be a file-handle-t mielőtt chmod/xattr

        // Unix-on a llama-server-nek futtathatónak kell lennie
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if lower.contains("llama-server") || !lower.contains('.') {
                let _ = std::fs::set_permissions(
                    &out_path,
                    std::fs::Permissions::from_mode(0o755),
                );
            }
        }

        // macOS Gatekeeper fix: az tar.gz-ből kibontott binárisok és
        // dylib-ek automatikusan `com.apple.quarantine` extended attribute-ot
        // kapnak, ami megakadályozza a futtatást ("nem ellenőrzött fejlesztő"
        // hibával vagy csendes-fail SIGKILL-lel). Ezt el kell távolítani.
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("xattr")
                .args(["-d", "com.apple.quarantine"])
                .arg(&out_path)
                .output();
            // Plusz a com.apple.metadata:kMDItemWhereFroms-t is, hogy
            // a Finder se mondja "letöltött a netről":
            let _ = std::process::Command::new("xattr")
                .args(["-d", "com.apple.metadata:kMDItemWhereFroms"])
                .arg(&out_path)
                .output();
        }
    }
    Ok(())
}

// =========================================================================
//  Modell letöltés (HuggingFace public)
// =========================================================================

/// v0.2.0: egy expert letöltése a katalógus szerint.
/// A `target_path` a `models/akasha/<slot>.gguf` lesz, a `component`
/// pedig az event-azonosító (`szoveg` / `logika` / `kod`).
pub async fn download_expert(
    app: &AppHandle,
    slot: AkashaSlot,
    target_dir: &Path,
) -> Result<(), String> {
    let entry = catalog::lookup(slot)
        .ok_or_else(|| format!("Nem található katalógus-bejegyzés: {:?}", slot))?;
    let component = slot.key().to_string();
    let target_path = target_dir.join(entry.local_filename());
    download_model(app, &component, target_path, entry.repo, entry.file).await
}

/// HuggingFace publikus modell letöltése. A `repo` és `file` paramétereket
/// a hívó adja meg. Letöltés után a `target_path`-ra mentődik.
///
/// Sikeres letöltés végén `download-done` event-tel jelez a frontendnek.
pub async fn download_model(
    app: &AppHandle,
    component: &str,
    target_path: PathBuf,
    repo: &str,
    file: &str,
) -> Result<(), String> {
    let url = format!("https://huggingface.co/{repo}/resolve/main/{file}");

    // Target mappa biztosítása
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Mappa-létrehozás hiba: {e}"))?;
    }

    let client = reqwest::Client::builder()
        .user_agent("LUMI/0.1 (downloader)")
        .build()
        .map_err(|e| format!("HTTP kliens hiba: {e}"))?;

    let _ = app.emit(
        "download-start",
        DownloadProgress {
            component: component.to_string(),
            percent: 0.0,
            downloaded_bytes: 0,
            total_bytes: 0,
            speed_mbps: 0.0,
        },
    );

    // Köztes .part fájlba töltjük, sikeres végén átnevezés
    let tmp_path = target_path.with_extension("part");
    download_stream_to_file(app, component, &client, &url, &tmp_path).await?;

    // Ellenőrzés: legalább 50 MB-os legyen
    let size = std::fs::metadata(&tmp_path)
        .map(|m| m.len())
        .unwrap_or(0);
    if size < MODEL_MIN_BYTES {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!(
            "Letöltött fájl túl kicsi ({size} byte) - valószínűleg hibás URL vagy hálózati hiba."
        ));
    }

    // Végleges helyre átnevezés
    std::fs::rename(&tmp_path, &target_path).map_err(|e| {
        format!(
            "Átnevezés hiba ({} → {}): {e}",
            tmp_path.display(),
            target_path.display()
        )
    })?;

    let _ = app.emit(
        "download-done",
        DownloadDone {
            component: component.to_string(),
        },
    );
    Ok(())
}

// =========================================================================
//  Közös streaming-letöltő progress-event-ekkel
// =========================================================================

async fn download_stream_to_file(
    app: &AppHandle,
    component: &str,
    client: &reqwest::Client,
    url: &str,
    out_path: &Path,
) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Letöltés indítása sikertelen: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP {} - {}",
            response.status().as_u16(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("ismeretlen hiba")
        ));
    }

    let total_bytes = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(out_path)
        .await
        .map_err(|e| format!("Fájl-létrehozás hiba: {e}"))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    // Sebesség-mérés: utolsó ~1 mp átlagolva
    let mut last_tick = std::time::Instant::now();
    let mut bytes_since_tick: u64 = 0;
    let mut last_speed_mbps: f32 = 0.0;
    // Progress emit ritkítása: legfeljebb 8x/mp ne spam-eljük a frontendet
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            // Streaming hiba esetén töröljük a részfájlt - "egyszerűbb,
            // bezáráskor kezdje újra" stratégia.
            let _ = std::fs::remove_file(out_path);
            format!("Stream hiba: {e}")
        })?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Lemez-írás hiba: {e}"))?;
        downloaded += chunk.len() as u64;
        bytes_since_tick += chunk.len() as u64;

        let now = std::time::Instant::now();
        let since_tick = now.duration_since(last_tick).as_secs_f32();
        if since_tick >= 0.5 {
            last_speed_mbps = (bytes_since_tick as f32 / 1_048_576.0) / since_tick.max(0.001);
            bytes_since_tick = 0;
            last_tick = now;
        }

        if now.duration_since(last_emit).as_millis() >= 125 {
            let percent = if total_bytes > 0 {
                (downloaded as f32 / total_bytes as f32) * 100.0
            } else {
                0.0
            };
            let _ = app.emit(
                "download-progress",
                DownloadProgress {
                    component: component.to_string(),
                    percent,
                    downloaded_bytes: downloaded,
                    total_bytes,
                    speed_mbps: last_speed_mbps,
                },
            );
            last_emit = now;
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("Flush hiba: {e}"))?;
    Ok(())
}

// A v0.1.x ModelSource / model_source() pár obsolete a v0.2.0-tól -
// a `downloader::catalog::CATALOG` az egyetlen forrás-igazság.
