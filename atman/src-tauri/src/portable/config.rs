use super::paths::{ensure_portable_layout, get_launch_root, resolve_path, AppPaths};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// v0.2.0 arsenal: 3 fix expert (Szöveg / Logika / Kód).
///
/// A régi `eco/brain/creative` mezők is megmaradtak deszerializálható
/// alias-ként, hogy a v0.1.x-ből frissítő felhasználók config.toml-ja
/// se crash-eljen — egyszerűen figyelmen kívül hagyjuk a régi értékeket
/// (úgyis új fájlnevek és új modellek vannak).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AkashaArsenalConfig {
    #[serde(default = "default_szoveg_file")]
    pub szoveg: String,
    #[serde(default = "default_logika_file")]
    pub logika: String,
    #[serde(default = "default_kod_file")]
    pub kod: String,
    /// v0.1.x legacy mezők - elnyelés, hogy a régi config ne hibázzon.
    /// Ezeket NEM használjuk semmire a v0.2.0+ kódban.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eco: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creative: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AkashaThrottleConfig {
    #[serde(default = "default_ram_warning")]
    pub ram_warning_mb: u64,
    #[serde(default = "default_ram_critical")]
    pub ram_critical_mb: u64,
    #[serde(default = "default_cpu_critical")]
    pub cpu_critical_percent: f32,
    #[serde(default = "default_min_threads")]
    pub min_threads: u32,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AkashaConfig {
    #[serde(default = "default_models_dir")]
    pub models_dir: String,
    #[serde(default = "default_models_max")]
    pub models_max: u32,
    #[serde(default)]
    pub arsenal: AkashaArsenalConfig,
    #[serde(default)]
    pub throttle: AkashaThrottleConfig,
    #[serde(default)]
    pub n_threads: u32,
    #[serde(default = "default_ctx")]
    pub n_ctx: u32,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    /// Legacy - migrálva eco-ra
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<String>,
}

impl AkashaConfig {
    pub fn models_dir_path(&self, launch_root: &Path) -> PathBuf {
        resolve_existing_path(launch_root, &self.models_dir)
    }

    pub fn arsenal_file_path(&self, launch_root: &Path, filename: &str) -> PathBuf {
        self.models_dir_path(launch_root).join(filename)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfig {
    #[serde(default = "default_embed_path")]
    pub embed_model_path: String,
    #[serde(default = "default_chunk")]
    pub chunk_size: usize,
    #[serde(default = "default_overlap")]
    pub chunk_overlap: usize,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConfig {
    #[serde(default = "default_name")]
    pub display_name: String,
}

/// Adaptív Védelmi Protokoll beállítások (Fázis 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceConfig {
    /// Ha `true` (alap), a LUMI automatikusan throttle-ol RAM/CPU
    /// terhelés esetén. Ha `false`, a user kifejezetten kérte hogy
    /// MAXIMÁLIS teljesítményen fusson akkor is, ha kockázatos.
    #[serde(default = "default_protection")]
    pub hardware_protection_enabled: bool,
    /// Opcionális kézi tier-felülírás:
    /// `None`/üres = detektált tier-t használjuk.
    /// `"limp"` / `"standard"` / `"pro"` = kézi mód.
    #[serde(default)]
    pub forced_tier: Option<String>,
    /// RAM-takarékos mód: ha `true` (alap), minden válasz után automatikusan
    /// kiakasztjuk a modellt a memóriából. A következő üzenetre újratöltjük
    /// (~10-15 mp 5 GB modellnél). Cserébe semmi RAM nem foglalt amíg nem
    /// chat-elsz, és más programok zavartalanul használhatják a gépet.
    ///
    /// Ha `false`: a modell a RAM-ban marad, sebesség-prioritás - gyorsabb
    /// következő válasz, de 5+ GB RAM állandóan foglalt.
    #[serde(default = "default_unload_after_response")]
    pub unload_model_after_response: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            hardware_protection_enabled: default_protection(),
            forced_tier: None,
            unload_model_after_response: default_unload_after_response(),
        }
    }
}

fn default_protection() -> bool {
    true
}

fn default_unload_after_response() -> bool {
    true
}

/// Megjelenés / téma beállítások.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    /// "light" (alap) vagy "dark"
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}

fn default_theme() -> String {
    "light".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtmanConfig {
    #[serde(default)]
    pub akasha: AkashaConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub profile: ProfileConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

impl Default for AtmanConfig {
    fn default() -> Self {
        Self {
            akasha: AkashaConfig::default(),
            memory: MemoryConfig::default(),
            profile: ProfileConfig::default(),
            performance: PerformanceConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

impl Default for AkashaArsenalConfig {
    fn default() -> Self {
        Self {
            szoveg: default_szoveg_file(),
            logika: default_logika_file(),
            kod: default_kod_file(),
            eco: None,
            brain: None,
            creative: None,
        }
    }
}

impl Default for AkashaThrottleConfig {
    fn default() -> Self {
        Self {
            ram_warning_mb: default_ram_warning(),
            ram_critical_mb: default_ram_critical(),
            cpu_critical_percent: default_cpu_critical(),
            min_threads: default_min_threads(),
            poll_interval_ms: default_poll_interval(),
        }
    }
}

impl Default for AkashaConfig {
    fn default() -> Self {
        Self {
            models_dir: default_models_dir(),
            models_max: default_models_max(),
            arsenal: AkashaArsenalConfig::default(),
            throttle: AkashaThrottleConfig::default(),
            n_threads: 0,
            n_ctx: default_ctx(),
            host: default_host(),
            port: 0,
            model_path: None,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            embed_model_path: default_embed_path(),
            chunk_size: default_chunk(),
            chunk_overlap: default_overlap(),
            top_k: default_top_k(),
        }
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            display_name: default_name(),
        }
    }
}

fn default_models_dir() -> String {
    "models/akasha".into()
}
fn default_models_max() -> u32 {
    1
}
// v0.2.0 default arsenal fájlnevek - tiszta `<slot>.gguf` séma,
// nincs többé tier-prefix vagy quant-suffix. A fájlnév stabil; a mögötte
// lévő konkrét modell a `downloader::catalog::CATALOG`-ban változhat.
fn default_szoveg_file() -> String {
    "szoveg.gguf".into()
}
fn default_logika_file() -> String {
    "logika.gguf".into()
}
fn default_kod_file() -> String {
    "kod.gguf".into()
}
fn default_ram_warning() -> u64 {
    2048
}
fn default_ram_critical() -> u64 {
    1024
}
fn default_cpu_critical() -> f32 {
    90.0
}
fn default_min_threads() -> u32 {
    2
}
fn default_poll_interval() -> u64 {
    500
}
fn default_embed_path() -> String {
    "models/embed/embed.Q4_K_M.gguf".into()
}
fn default_ctx() -> u32 {
    4096
}
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_chunk() -> usize {
    512
}
fn default_overlap() -> usize {
    64
}
fn default_top_k() -> usize {
    5
}
fn default_name() -> String {
    "felhasználó".into()
}

/// v0.2.0 first-run migráció + bundled-modell telepítés.
///
/// HÁROM dolgot kezel egyetlen hívással:
///
///   1. **Bundled Szöveg modell kicsomagolása**: ha a telepítő tartalmaz
///      egy `resources/szoveg.gguf`-ot (Tauri bundle resource), és a
///      `models/akasha/szoveg.gguf` még nincs meg, akkor átmásolja oda.
///      Ezzel a user telepítés UTÁN azonnal chat-elhet a Szöveg expert-tel
///      letöltési várakozás nélkül.
///
///   2. **Régi v0.1.x slot-fájlok megőrzése**: a régi `{tier}-{slot}.gguf`
///      fájlokat NEM töröljük — a user manuálisan kitakaríthatja a
///      Beállítások › Modellek menüből (későbbi feature). Csak az új
///      `szoveg/logika/kod.gguf` fájlokra koncentrálunk.
///
///   3. **Legacy `akasha-moe.Q4_K_M.gguf` (v0.0.x maradványa)**: ha még
///      megvan a régi-régi monolitikus fájl, ezt sem bántjuk — nem hivatkozunk
///      rá többé.
pub fn migrate_eco_model(launch_root: &Path) {
    let models_dir = launch_root.join("models").join("akasha");
    if let Err(e) = fs::create_dir_all(&models_dir) {
        crate::debug_log::dlog(
            "config.rs:migrate_eco_model",
            "H1",
            "models_dir mkdir failed",
            serde_json::json!({ "error": e.to_string() }),
        );
        return;
    }

    let target = models_dir.join("szoveg.gguf");
    if target.exists() {
        return;
    }

    // Bundled Szöveg keresés — a Tauri bundle "resources" mappa különböző
    // OS-eken eltérő helyre kerül; több jelölt path-on is megpróbáljuk.
    let candidates = [
        // Windows + Linux: a binárissal egy szinten van a resources/ mappa
        launch_root.join("resources").join("szoveg.gguf"),
        // macOS .app bundle: Contents/Resources/
        launch_root.join("..").join("Resources").join("szoveg.gguf"),
        // Fallback: közvetlenül a launch_root mellé téve
        launch_root.join("szoveg.gguf"),
    ];

    for src in &candidates {
        if !src.exists() || !src.is_file() {
            continue;
        }
        match fs::copy(src, &target) {
            Ok(bytes) => {
                crate::debug_log::dlog(
                    "config.rs:migrate_eco_model",
                    "H1",
                    "bundled szoveg telepítve",
                    serde_json::json!({
                        "src": src.display().to_string(),
                        "dst": target.display().to_string(),
                        "bytes": bytes,
                    }),
                );
                return;
            }
            Err(e) => {
                crate::debug_log::dlog(
                    "config.rs:migrate_eco_model",
                    "H1",
                    "bundled szoveg copy failed",
                    serde_json::json!({
                        "src": src.display().to_string(),
                        "error": e.to_string(),
                    }),
                );
            }
        }
    }
}

pub fn load_config(paths: &AppPaths) -> Result<AtmanConfig, String> {
    let config_path = Path::new(&paths.config_path);
    if !config_path.exists() {
        let cfg = AtmanConfig::default();
        save_config(paths, &cfg)?;
        let root = get_launch_root()?;
        migrate_eco_model(&root);
        return Ok(cfg);
    }
    let raw = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
    let mut cfg: AtmanConfig = toml::from_str(&raw).map_err(|e| format!("config.toml hiba: {e}"))?;
    let root = get_launch_root()?;
    migrate_eco_model(&root);
    cfg.memory.embed_model_path = resolve_existing_path(&root, &cfg.memory.embed_model_path)
        .display()
        .to_string();
    Ok(cfg)
}

/// Igazi tartalom-e: a fájl/mappa létezik ÉS nem üres mappa.
fn path_has_content(p: &Path) -> bool {
    if !p.exists() {
        return false;
    }
    if p.is_file() {
        return true;
    }
    p.read_dir()
        .map(|mut it| it.next().is_some())
        .unwrap_or(false)
}

/// Megpróbálja megtalálni a `relative` útvonalat a `launch_root`-tól indulva,
/// és felfelé haladva is (dev módban a `target/debug` szülő szintekre nézünk).
/// Csak akkor fogadja el a találatot, ha van benne tartalom - így a Tauri
/// által dev futáskor létrehozott üres `target/debug/models/akasha` mappa
/// nem fedi el a tényleges, fájlokat tartalmazó `models/akasha`-t a repo gyökerében.
fn resolve_existing_path(launch_root: &Path, relative: &str) -> PathBuf {
    let primary = resolve_path(launch_root, relative);
    if path_has_content(&primary) {
        return primary;
    }
    // Fallback: keressünk felfelé akár 6 szintig (target/debug → ... → repo gyökér).
    let mut dir = launch_root.to_path_buf();
    for _ in 0..6 {
        if !dir.pop() {
            break;
        }
        let candidate = dir.join(relative);
        if path_has_content(&candidate) {
            return candidate;
        }
    }
    primary
}

pub fn save_config(paths: &AppPaths, cfg: &AtmanConfig) -> Result<(), String> {
    let out = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&paths.config_path, out).map_err(|e| e.to_string())
}

pub fn init_config() -> Result<(AppPaths, AtmanConfig), String> {
    let paths = ensure_portable_layout()?;
    let cfg = load_config(&paths)?;
    Ok((paths, cfg))
}
