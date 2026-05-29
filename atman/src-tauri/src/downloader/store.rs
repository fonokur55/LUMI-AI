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

use crate::akasha::perf::Tier;
use crate::downloader::catalog::{self, ModelEntry, Slot};
use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

// =========================================================================
//  Setup status check
// =========================================================================

/// Egy konkrét tier × slot cella telepítettsége. A frontend a teljes
/// 9-cellás mátrixot kapja meg (még a nem-recommended tier-eket is,
/// hogy a Beállítások › Modellek táblázatában tudja mutatni).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub tier: Tier,
    pub slot: Slot,
    pub installed: bool,
    /// Megjeleníthető név (a katalógusból, pl. "Qwen 2.5 Coder 7B")
    pub display_name: &'static str,
    /// Méret GB-ban
    pub size_gb: f32,
}

/// A meglévő komponensek és az ajánlott tier állapota. A frontend ez
/// alapján dönti el:
///  - megjeleníti-e a first-run download wizardot
///  - a Beállítások › Modellek táblázatában mit mutat „telepítve" / „letöltendő"
///  - a ChatView slot-választójában mit disable-l
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    /// A llama-server bináris elérhető-e (>=10 MB)
    pub runtime_installed: bool,
    /// A hardware-detektálás alapján ajánlott tier (Limp/Standard/Pro).
    /// Blocked esetén a wizard nem nyílik meg, az app figyelmeztet.
    pub recommended_tier: Tier,
    /// A 9 cella állapota (3 tier × 3 slot)
    pub models: Vec<ModelStatus>,
    /// A KÖTELEZŐ minimum: runtime + recommended_tier 3 modellje
    pub minimum_ready: bool,
}

impl SetupStatus {
    /// Visszaadja, hogy egy konkrét tier × slot kombináció telepítve van-e.
    pub fn is_installed(&self, tier: Tier, slot: Slot) -> bool {
        self.models
            .iter()
            .any(|m| m.tier == tier && m.slot == slot && m.installed)
    }

    /// Egy adott tier 3 modellje (Eco+Brain+Creative) mind telepítve van-e.
    pub fn tier_pack_ready(&self, tier: Tier) -> bool {
        self.is_installed(tier, Slot::Eco)
            && self.is_installed(tier, Slot::Brain)
            && self.is_installed(tier, Slot::Creative)
    }
}

/// Akkor tekintünk egy modell-fájlt "telepítettnek", ha legalább 50 MB
/// (sérült/részleges letöltés kiszűrése).
const MODEL_MIN_BYTES: u64 = 50_000_000;
/// A llama-server bináris is legalább ekkora kell legyen.
const RUNTIME_MIN_BYTES: u64 = 10_000_000;

fn file_at_least(path: &Path, min_bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() >= min_bytes)
        .unwrap_or(false)
}

/// A teljes 9-cellás mátrix telepítettségét + a runtime állapotát adja vissza.
/// A `recommended_tier` a hívó dolga (hardware-snapshot alapján).
pub fn check_setup_status(
    runtime_llama_path: &Path,
    models_akasha_dir: &Path,
    recommended_tier: Tier,
) -> SetupStatus {
    let runtime_installed = file_at_least(runtime_llama_path, RUNTIME_MIN_BYTES);

    let models: Vec<ModelStatus> = catalog::CATALOG
        .iter()
        .map(|entry: &ModelEntry| {
            let path = models_akasha_dir.join(entry.local_filename());
            ModelStatus {
                tier: entry.tier,
                slot: entry.slot,
                installed: file_at_least(&path, MODEL_MIN_BYTES),
                display_name: entry.display_name,
                size_gb: entry.size_gb,
            }
        })
        .collect();

    let recommended_pack_ready = catalog::tier_pack(recommended_tier).iter().all(|m| {
        let path = models_akasha_dir.join(m.local_filename());
        file_at_least(&path, MODEL_MIN_BYTES)
    });

    SetupStatus {
        runtime_installed,
        recommended_tier,
        models,
        minimum_ready: runtime_installed && recommended_pack_ready,
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
/// FONTOS: szigorúan a `.zip` végződésre szűrünk, mert egyes release-eken
/// vannak `.tar.gz` és `.zip.sig` asset-ek is, és ha a `find` egy nem-zip
/// fájlt fogna meg, az `extract_zip_runtime` "Could not find EOCD" hibát
/// adna. A preferenciasor a tiszta CPU build → Vulkan/Metal → fallback.
fn pick_llama_asset_name() -> Vec<&'static str> {
    if cfg!(target_os = "windows") {
        // Tipikus minta: "llama-b5050-bin-win-cpu-x64.zip"
        vec!["bin-win-cpu-x64", "bin-win-vulkan-x64", "bin-win-x64"]
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            // macOS ARM: "llama-bXXXX-bin-macos-arm64.zip"
            vec!["bin-macos-arm64", "bin-macos-aarch64"]
        } else {
            vec!["bin-macos-x64", "bin-macos-amd64"]
        }
    } else {
        // Linux: "llama-bXXXX-bin-ubuntu-x64.zip"
        vec!["bin-ubuntu-x64", "bin-linux-x64"]
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
    let asset = patterns
        .iter()
        .find_map(|pat| {
            release.assets.iter().find(|a| {
                // SZIGORÚ feltételek:
                //   - tartalmazza a pattern-t (pl. "bin-macos-arm64")
                //   - .zip végződésű (NEM .tar.gz, NEM .zip.sig, NEM .sha256)
                //   - NEM CUDA-runtime csomag (nincs Nvidia GPU)
                a.name.contains(pat)
                    && a.name.to_lowercase().ends_with(".zip")
                    && !a.name.to_lowercase().ends_with(".zip.sig")
                    && !a.name.to_lowercase().ends_with(".zip.sha256")
                    && !a.name.starts_with("cudart-")
            })
        })
        .ok_or_else(|| {
            // Diagnosztika a logba: lássuk milyen asset-ek voltak elérhetők
            let available: Vec<&str> = release.assets.iter().map(|a| a.name.as_str()).collect();
            format!(
                "Nem található megfelelő .zip llama-server csomag az aktuális OS-hez. \
                 Próbáld manuálisan: https://github.com/ggml-org/llama.cpp/releases \n\
                 Keresett minták: {patterns:?}\nElérhető asset-ek: {available:?}"
            )
        })?;

    // 2. Letöltés egy temp ZIP-be
    let tmp_zip = std::env::temp_dir().join(format!("lumi-llama-{}", asset.name));
    download_stream_to_file(
        app,
        component,
        &client,
        &asset.browser_download_url,
        &tmp_zip,
    )
    .await?;

    // 3. Kibontás a runtime mappába
    let _ = app.emit(
        "download-extracting",
        DownloadDone {
            component: component.to_string(),
        },
    );
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("Mappa-létrehozás hiba: {e}"))?;
    extract_zip_runtime(&tmp_zip, &runtime_dir)?;
    // Temp ZIP törlése
    let _ = std::fs::remove_file(&tmp_zip);

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
        // README/LICENSE-eket kihagyjuk.
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
    }
    Ok(())
}

// =========================================================================
//  Modell letöltés (HuggingFace public)
// =========================================================================

/// Egy konkrét tier × slot modell letöltése a katalógus szerint.
/// A `target_path` a `models/akasha/<tier>-<slot>.gguf` lesz, a `component`
/// pedig az event-azonosító (`light-eco`, `standard-brain` stb.).
pub async fn download_tier_model(
    app: &AppHandle,
    tier: Tier,
    slot: Slot,
    target_dir: &Path,
) -> Result<(), String> {
    let entry = catalog::lookup(tier, slot)
        .ok_or_else(|| format!("Nem található katalógus-bejegyzés: {tier:?}/{slot:?}"))?;
    let component = format!("{}-{}", catalog::tier_key_for_filename(tier), slot.key());
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

// =========================================================================
//  Modell-katalógus - hol találhatók a forrás-fájlok
// =========================================================================

pub struct ModelSource {
    pub repo: &'static str,
    pub file: &'static str,
}

pub fn model_source(slot: &str) -> Option<ModelSource> {
    match slot {
        "eco" => Some(ModelSource {
            repo: "Qwen/Qwen2.5-3B-Instruct-GGUF",
            file: "qwen2.5-3b-instruct-q4_k_m.gguf",
        }),
        "brain" => Some(ModelSource {
            repo: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
            file: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        }),
        "creative" => Some(ModelSource {
            repo: "dphn/dolphin-2.9.4-llama3.1-8b-gguf",
            file: "dolphin-2.9.4-llama3.1-8b-Q4_K_M.gguf",
        }),
        _ => None,
    }
}
