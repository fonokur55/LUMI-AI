//! LUMI Adaptív Védelmi Protokoll - Fázis 1: hardware-profilozás.
//!
//! A felhasználó gépét 4 szintbe (Tier) sorolja a RAM, CPU-mag, és AVX2-támogatás
//! alapján. A frontend ez alapján tud felhasználóbarát üzenetet mutatni és (Fázis
//! 2-ben) tier-megfelelő modellt választani.
//!
//! Filozófia: LUMI soha nem fagyaszt le gépet és sosem mutat technikai
//! hibakódot. Mindig dolgozik, ha kell lassan és kis modellel - a felhasználó
//! megnyugtató, magyar nyelvű üzeneteket kap.

use super::hardware::HardwareSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Túl gyenge gép: <3 GB szabad RAM vagy <2 mag vagy nincs AVX2.
    /// LUMI nem indul; a user be kell zárjon programokat.
    Blocked,
    /// Védett mód (3-6 GB RAM, 2-4 mag): legkisebb modell, alacsony prioritás.
    /// Más programok problémamentesen futhatnak.
    Limp,
    /// Alap mód (6-12 GB RAM, 4-8 mag): közepes modell, normál prioritás.
    Standard,
    /// Csúcs mód (12+ GB RAM, 8+ mag): teljes modell, teljes teljesítmény.
    Pro,
}

impl Tier {
    pub fn label_hu(&self) -> &'static str {
        match self {
            Tier::Blocked => "Nem futtatható",
            Tier::Limp => "Light mód",
            Tier::Standard => "Standard mód",
            Tier::Pro => "Pro mód",
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            Tier::Blocked => "blocked",
            Tier::Limp => "limp",
            Tier::Standard => "standard",
            Tier::Pro => "pro",
        }
    }

    pub fn from_key(k: &str) -> Option<Self> {
        match k.to_lowercase().as_str() {
            "blocked" => Some(Tier::Blocked),
            "limp" => Some(Tier::Limp),
            "standard" => Some(Tier::Standard),
            "pro" => Some(Tier::Pro),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfile {
    /// Detektált tier (a hardware tényei alapján).
    pub detected_tier: Tier,
    /// Ténylegesen használt tier - egyenlő detektált-tal, kivéve ha a user
    /// kézzel felülírta a Settings-ben (`forced_tier`).
    pub effective_tier: Tier,
    /// Igaz, ha a user kézzel írta felül a detektálást.
    pub override_active: bool,
    /// A védelmi protokoll alapból be van kapcsolva. Ha a user kikapcsolta
    /// (`hardware_protection_enabled = false`), itt false-t adunk vissza -
    /// a frontend ez alapján mutathat figyelmeztetést.
    pub protection_enabled: bool,
    /// Felhasználóbarát magyar üzenet - közvetlenül ezt mutathatja a UI.
    pub message: String,
    // Diagnosztikai info - Settings panelban mutatható:
    pub total_ram_gb: f32,
    pub available_ram_gb: f32,
    pub cpu_cores: usize,
    pub cpu_has_avx2: bool,
    /// Javasolt modell-méret a tier-hez (Fázis 2-ben fogjuk használni).
    pub recommended_model: ModelRecommendation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRecommendation {
    pub display_name: String,
    pub size_gb: f32,
    pub n_ctx: u32,
    pub n_threads: u32,
}

/// Runtime AVX2 detektálás. A llama.cpp AVX2 nélkül radikálisan lassú, ezért
/// ez egy fontos jelző egy gép „futhatóságához".
fn cpu_has_avx2() -> bool {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    {
        // ARM stb. - nem AVX, de natív gyors instrukciókkal rendelkezik.
        // Az llama.cpp-nek megvan az ARM-NEON pathja.
        true
    }
}

/// A tényleges detektálási logika. RAM-ot, CPU-magot, AVX2-t mér.
fn detect_tier(snap: &HardwareSnapshot, has_avx2: bool) -> Tier {
    let avail_gb = snap.available_ram_mb as f32 / 1024.0;
    let cores = snap.cpu_cores;

    // 1) Kritikus alsó küszöb - alatta nem indítjuk
    if avail_gb < 3.0 || cores < 2 || !has_avx2 {
        return Tier::Blocked;
    }
    // 2) Light mód - szerény hardware, de használható
    if avail_gb < 6.0 || cores < 4 {
        return Tier::Limp;
    }
    // 3) Standard - átlagos friss laptop / asztali gép
    if avail_gb < 12.0 || cores < 8 {
        return Tier::Standard;
    }
    // 4) Pro - komoly desktop / workstation
    Tier::Pro
}

fn recommend(tier: Tier) -> ModelRecommendation {
    // v0.1.3+: a tier-szintű ajánlás a 9-cellás katalógus 3 modellének
    // ÖSSZmérete (Eco + Brain + Creative). A display_name a tier
    // arc-modellje (Eco) - a teljes csomag mérete a sizeGb-ben.
    match tier {
        Tier::Blocked => ModelRecommendation {
            display_name: "-".into(),
            size_gb: 0.0,
            n_ctx: 0,
            n_threads: 0,
        },
        Tier::Limp => ModelRecommendation {
            display_name: "Light: Gemma 2 2B + Qwen Coder 1.5B (~4.2 GB)".into(),
            size_gb: 4.2,
            n_ctx: 2048,
            n_threads: 2,
        },
        Tier::Standard => ModelRecommendation {
            display_name: "Standard: Gemma 2 9B + Qwen Coder 7B (~14 GB)".into(),
            size_gb: 14.0,
            n_ctx: 4096,
            n_threads: 4,
        },
        Tier::Pro => ModelRecommendation {
            display_name: "Pro: Gemma 2 9B Q5 + Qwen Coder 14B (~22 GB)".into(),
            size_gb: 22.0,
            n_ctx: 4096,
            n_threads: 0, // 0 = auto (összes mag)
        },
    }
}

/// A felhasználónak mutatott magyar üzenet. Tónus: megnyugtató, érdemi, nem
/// technikai. Mindig pozitív végződéssel ("dolgozunk", "fut", "indul").
fn message_for(tier: Tier, override_active: bool, protection_disabled: bool) -> String {
    let base = match tier {
        Tier::Blocked => {
            "Sajnos a géped jelenleg nem tud AKASHA-t futtatni. Túl kevés a szabad memória, \
             vagy a processzorod nem támogatja a szükséges utasításokat (AVX2). \
             Próbáld bezárni a böngészőt vagy más nehéz programokat, aztán nyomd meg az „Újra\" gombot."
        }
        Tier::Limp => {
            "A géped szerényebb teljesítményű, ezért AKASHA Light módban indul. \
             A válaszok valamivel lassabbak lesznek, de stabilan és megbízhatóan dolgozunk - \
             más programjaid közben gond nélkül futhatnak."
        }
        Tier::Standard => {
            "AKASHA Standard módban fut. Gyors, pontos válaszok kíméletes memória-használattal."
        }
        Tier::Pro => {
            "AKASHA Pro módban fut. Teljes teljesítményen dolgozom - csúcs-minőségű válaszok."
        }
    };

    let mut out = base.to_string();
    if override_active {
        out.push_str(
            "\n\n[Te kézzel állítottad be ezt a módot a Beállítások között. \
             A detektált alap-mód ettől eltérhet - kockázat saját felelősségedre.]",
        );
    } else if protection_disabled {
        out.push_str(
            "\n\n[A védelmi protokoll ki van kapcsolva - \
             AKASHA nem fog automatikusan kíméletes módra váltani memóriahiány esetén. \
             Bármikor visszakapcsolható a Beállításokban.]",
        );
    }
    out
}

/// Megpróbálja feloldani a tier-specifikus modell-elérési utat. A folder
/// konvenció: `models/akasha/{tier}/{slot}.gguf`. Ha nincs tier-specifikus
/// fájl (vagy Pro tier-en vagyunk), visszaesik a default arsenal útvonalra.
///
/// Példák:
/// - Pro tier + eco → `models/akasha/eco.Q4_K_M.gguf` (default)
/// - Limp tier + eco → `models/akasha/limp/eco.Q4_K_M.gguf` ha létezik,
///   különben `models/akasha/eco.Q4_K_M.gguf`
pub fn resolve_tier_model_path(
    base_models_dir: &std::path::Path,
    default_filename: &str,
    tier: Tier,
) -> std::path::PathBuf {
    let subdir = match tier {
        Tier::Limp => Some("limp"),
        Tier::Standard => Some("standard"),
        Tier::Pro | Tier::Blocked => None,
    };
    if let Some(sub) = subdir {
        let tiered = base_models_dir.join(sub).join(default_filename);
        if tiered.exists() {
            return tiered;
        }
    }
    base_models_dir.join(default_filename)
}

/// Fő belépési pont. A `forced_tier` (Settings-ből) felülírja a detektálást,
/// de a detektált értéket mindig visszaadjuk diagnosztikához.
pub fn compute_profile(
    snap: &HardwareSnapshot,
    forced_tier: Option<&str>,
    protection_enabled: bool,
) -> HardwareProfile {
    let has_avx2 = cpu_has_avx2();
    let detected = detect_tier(snap, has_avx2);

    let override_tier = forced_tier.and_then(Tier::from_key);
    let (effective, override_active) = match override_tier {
        Some(t) if t != detected => (t, true),
        _ => (detected, false),
    };

    HardwareProfile {
        detected_tier: detected,
        effective_tier: effective,
        override_active,
        protection_enabled,
        message: message_for(effective, override_active, !protection_enabled),
        total_ram_gb: snap.total_ram_mb as f32 / 1024.0,
        available_ram_gb: snap.available_ram_mb as f32 / 1024.0,
        cpu_cores: snap.cpu_cores,
        cpu_has_avx2: has_avx2,
        recommended_model: recommend(effective),
    }
}
