// =========================================================================
//  9-modelles tier × slot katalógus
// =========================================================================
//  A LUMI 3 hardware-tier-re (Light/Standard/Pro) és 3 slot-ra
//  (Eco/Brain/Creative) van bontva. Mindegyik kombinációhoz egy konkrét
//  GGUF-fájl tartozik a HuggingFace-en.
//
//  Filozófia:
//   - Eco és Creative: Gemma-család (magyar nyelvi erősség + abliterated
//     a kreatív szabadsághoz)
//   - Brain: Qwen2.5-Coder-család (kifejezetten kód-fókusszú fine-tune,
//     GPT-3.5 / GPT-4 közeli kódminőség lokál futtatáskor)
//
//  A modellek mind Apache 2.0 vagy hasonló kereskedelmileg engedélyezett
//  licenc alatt.
// =========================================================================

use crate::akasha::perf::Tier;
use serde::Serialize;

/// A 3 chat-slot, amit a router választ a prompt alapján.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    Eco,
    Brain,
    Creative,
}

impl Slot {
    pub fn key(&self) -> &'static str {
        match self {
            Slot::Eco => "eco",
            Slot::Brain => "brain",
            Slot::Creative => "creative",
        }
    }
    pub fn from_key(k: &str) -> Option<Self> {
        match k.to_lowercase().as_str() {
            "eco" => Some(Slot::Eco),
            "brain" => Some(Slot::Brain),
            "creative" => Some(Slot::Creative),
            _ => None,
        }
    }
}

/// Egy modell-bejegyzés a katalógusban.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub tier: Tier,
    pub slot: Slot,
    /// HuggingFace repó (pl. "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF")
    pub repo: &'static str,
    /// A repón belüli GGUF fájl neve
    pub file: &'static str,
    /// Felhasználóbarát megjelenítendő név (Settings + wizard)
    pub display_name: &'static str,
    /// Közelítő letöltött méret GB-ban (UI-progress + figyelmeztetés
    /// felhasználónak)
    pub size_gb: f32,
}

impl ModelEntry {
    /// A lemezen tárolt fájl neve: `<tier>-<slot>.gguf`.
    /// Pl. `light-eco.gguf`, `standard-brain.gguf`, `pro-creative.gguf`.
    pub fn local_filename(&self) -> String {
        format!("{}-{}.gguf", tier_key(self.tier), self.slot.key())
    }

    /// HuggingFace public URL a letöltéshez.
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            self.repo, self.file
        )
    }
}

/// A tier-kulcs amit a lemezfájlnévben használunk. `Limp` → `"light"`,
/// mert a UI-ban "Light mód"-ként szerepel (a `Limp` belső név régebbi).
pub fn tier_key_for_filename(tier: Tier) -> &'static str {
    match tier {
        Tier::Limp => "light",
        Tier::Standard => "standard",
        Tier::Pro => "pro",
        Tier::Blocked => "blocked",
    }
}

fn tier_key(tier: Tier) -> &'static str {
    tier_key_for_filename(tier)
}

/// A 9 modell - 3 tier × 3 slot.
///
/// Brand-szempontok:
///  - Eco/Creative: Gemma-2 család (jó magyar)
///  - Brain: Qwen2.5-Coder család (jó kód)
///  - Pro-Brain: 14B Q4 - GPT-4-közeli kódminőség lokál
pub const CATALOG: &[ModelEntry] = &[
    // ===== Light tier (3-6 GB szabad RAM, kis modellek) =====
    ModelEntry {
        tier: Tier::Limp,
        slot: Slot::Eco,
        repo: "bartowski/gemma-2-2b-it-GGUF",
        file: "gemma-2-2b-it-Q4_K_M.gguf",
        display_name: "Gemma 2 2B (Light · Eco)",
        size_gb: 1.6,
    },
    ModelEntry {
        tier: Tier::Limp,
        slot: Slot::Brain,
        repo: "bartowski/Qwen2.5-Coder-1.5B-Instruct-GGUF",
        file: "Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf",
        display_name: "Qwen 2.5 Coder 1.5B (Light · Brain)",
        size_gb: 1.0,
    },
    ModelEntry {
        tier: Tier::Limp,
        slot: Slot::Creative,
        repo: "bartowski/gemma-2-2b-it-abliterated-GGUF",
        file: "gemma-2-2b-it-abliterated-Q4_K_M.gguf",
        display_name: "Gemma 2 2B abliterated (Light · Creative)",
        size_gb: 1.6,
    },
    // ===== Standard tier (6-12 GB, közepes modellek) =====
    ModelEntry {
        tier: Tier::Standard,
        slot: Slot::Eco,
        repo: "bartowski/gemma-2-9b-it-GGUF",
        file: "gemma-2-9b-it-Q3_K_M.gguf",
        display_name: "Gemma 2 9B (Standard · Eco)",
        size_gb: 3.8,
    },
    ModelEntry {
        tier: Tier::Standard,
        slot: Slot::Brain,
        repo: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
        file: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        display_name: "Qwen 2.5 Coder 7B (Standard · Brain)",
        size_gb: 4.7,
    },
    ModelEntry {
        tier: Tier::Standard,
        slot: Slot::Creative,
        repo: "bartowski/gemma-2-9b-it-abliterated-GGUF",
        file: "gemma-2-9b-it-abliterated-Q4_K_M.gguf",
        display_name: "Gemma 2 9B abliterated (Standard · Creative)",
        size_gb: 5.5,
    },
    // ===== Pro tier (12+ GB, csúcsminőség) =====
    ModelEntry {
        tier: Tier::Pro,
        slot: Slot::Eco,
        repo: "bartowski/gemma-2-9b-it-GGUF",
        file: "gemma-2-9b-it-Q5_K_M.gguf",
        display_name: "Gemma 2 9B Q5 (Pro · Eco)",
        size_gb: 6.5,
    },
    ModelEntry {
        tier: Tier::Pro,
        slot: Slot::Brain,
        repo: "bartowski/Qwen2.5-Coder-14B-Instruct-GGUF",
        file: "Qwen2.5-Coder-14B-Instruct-Q4_K_M.gguf",
        display_name: "Qwen 2.5 Coder 14B (Pro · Brain)",
        size_gb: 9.0,
    },
    ModelEntry {
        tier: Tier::Pro,
        slot: Slot::Creative,
        repo: "bartowski/gemma-2-9b-it-abliterated-GGUF",
        file: "gemma-2-9b-it-abliterated-Q5_K_M.gguf",
        display_name: "Gemma 2 9B abliterated Q5 (Pro · Creative)",
        size_gb: 6.5,
    },
];

/// Kikeresi a megadott tier+slot kombináció modell-bejegyzését.
pub fn lookup(tier: Tier, slot: Slot) -> Option<&'static ModelEntry> {
    CATALOG.iter().find(|m| m.tier == tier && m.slot == slot)
}

/// Egy adott tier 3 modellje (Eco, Brain, Creative).
pub fn tier_pack(tier: Tier) -> [&'static ModelEntry; 3] {
    let eco = lookup(tier, Slot::Eco).expect("Eco entry missing");
    let brain = lookup(tier, Slot::Brain).expect("Brain entry missing");
    let creative = lookup(tier, Slot::Creative).expect("Creative entry missing");
    [eco, brain, creative]
}

/// Egy tier teljes mérete GB-ban (Eco + Brain + Creative).
pub fn tier_total_size_gb(tier: Tier) -> f32 {
    tier_pack(tier).iter().map(|m| m.size_gb).sum()
}
