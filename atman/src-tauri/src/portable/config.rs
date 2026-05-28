use super::paths::{ensure_portable_layout, get_launch_root, resolve_path, AppPaths};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AkashaArsenalConfig {
    #[serde(default = "default_eco_file")]
    pub eco: String,
    #[serde(default = "default_brain_file")]
    pub brain: String,
    #[serde(default = "default_creative_file")]
    pub creative: String,
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
            eco: default_eco_file(),
            brain: default_brain_file(),
            creative: default_creative_file(),
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
fn default_eco_file() -> String {
    "eco.Q4_K_M.gguf".into()
}
fn default_brain_file() -> String {
    "brain.Q4_K_M.gguf".into()
}
fn default_creative_file() -> String {
    "creative.Q4_K_M.gguf".into()
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

pub fn migrate_eco_model(launch_root: &Path) {
    let dir = launch_root.join("models").join("akasha");
    let eco = dir.join(default_eco_file());
    if eco.exists() {
        return;
    }
    let legacy = dir.join("akasha-moe.Q4_K_M.gguf");
    if legacy.exists() {
        let _ = fs::copy(&legacy, &eco);
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
