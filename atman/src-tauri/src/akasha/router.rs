use super::types::AkashaSlot;
use crate::portable::config::{AkashaArsenalConfig, AkashaConfig};
use std::path::Path;

const BRAIN_RAM_MB: u64 = 7000;
const CREATIVE_RAM_MB: u64 = 4500;
/// Minimális érvényes GGUF méret (sérült / részleges letöltés kiszűrése)
const MIN_GGUF_BYTES: u64 = 50_000_000;

pub struct RouteInput<'a> {
    pub prompt: &'a str,
    pub available_ram_mb: u64,
    pub is_critical: bool,
    pub launch_root: &'a Path,
    pub akasha: &'a AkashaConfig,
}

pub struct RouteResult {
    pub slot: AkashaSlot,
    pub model_filename: String,
    pub resource_limited: bool,
}

pub fn route_prompt(input: &RouteInput) -> RouteResult {
    let arsenal = &input.akasha.arsenal;
    let mut slot = classify_prompt(input.prompt);
    let mut resource_limited = false;

    let brain_path = input
        .akasha
        .arsenal_file_path(input.launch_root, &arsenal.brain);
    let creative_path = input
        .akasha
        .arsenal_file_path(input.launch_root, &arsenal.creative);
    let eco_path = input
        .akasha
        .arsenal_file_path(input.launch_root, &arsenal.eco);

    let eco_ok = model_available(&eco_path);
    let brain_ok = model_available(&brain_path);
    let creative_ok = model_available(&creative_path);

    if input.is_critical {
        if slot == AkashaSlot::Brain && brain_ok && input.available_ram_mb >= BRAIN_RAM_MB {
            // egyértelmű brain kérés maradhat
        } else {
            slot = AkashaSlot::Eco;
            resource_limited = true;
        }
    }

    match slot {
        AkashaSlot::Brain => {
            if !brain_ok {
                slot = pick_available_slot(eco_ok, brain_ok, creative_ok, AkashaSlot::Eco);
            } else if input.available_ram_mb < BRAIN_RAM_MB {
                slot = pick_available_slot(eco_ok, brain_ok, creative_ok, AkashaSlot::Eco);
                resource_limited = true;
            }
        }
        AkashaSlot::Creative => {
            if !creative_ok {
                slot = pick_available_slot(eco_ok, brain_ok, creative_ok, AkashaSlot::Eco);
            } else if input.available_ram_mb < CREATIVE_RAM_MB {
                slot = pick_available_slot(eco_ok, brain_ok, creative_ok, AkashaSlot::Eco);
                resource_limited = true;
            }
        }
        AkashaSlot::Eco => {
            if !eco_ok {
                slot = pick_available_slot(eco_ok, brain_ok, creative_ok, AkashaSlot::Brain);
            }
        }
    }

    let model_filename = filename_for_slot(slot, arsenal);
    RouteResult {
        slot,
        model_filename,
        resource_limited,
    }
}

fn model_available(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() >= MIN_GGUF_BYTES)
        .unwrap_or(false)
}

/// Előny: eco → brain → creative; ha a preferált slot elérhető, azt választja.
fn pick_available_slot(eco: bool, brain: bool, creative: bool, preferred: AkashaSlot) -> AkashaSlot {
    let ok = |s: AkashaSlot| match s {
        AkashaSlot::Eco => eco,
        AkashaSlot::Brain => brain,
        AkashaSlot::Creative => creative,
    };
    if ok(preferred) {
        return preferred;
    }
    if eco {
        AkashaSlot::Eco
    } else if brain {
        AkashaSlot::Brain
    } else if creative {
        AkashaSlot::Creative
    } else {
        AkashaSlot::Eco
    }
}

fn filename_for_slot(slot: AkashaSlot, arsenal: &AkashaArsenalConfig) -> String {
    match slot {
        AkashaSlot::Eco => arsenal.eco.clone(),
        AkashaSlot::Brain => arsenal.brain.clone(),
        AkashaSlot::Creative => arsenal.creative.clone(),
    }
}

fn classify_prompt(text: &str) -> AkashaSlot {
    let lower = text.to_lowercase();

    // Magyar kulcsszavak STEM-formában (rövidített tövek), hogy a ragozott
    // alakok is illeszkedjenek: "mesét" → "mese", "függvényt" → "függvény",
    // "javítani" → "javít", stb.
    let brain_score = score_keywords(
        &lower,
        &[
            // angol kód-fogalmak
            "function", "class", "import", "def", "rust", "typescript",
            "javascript", "python", "sql", "api", "bug", "compile", "git",
            "refactor", "debug", "variable", "struct", "enum", "async", "await",
            "code", "algorithm",
            // magyar tövek
            "kód", "hiba", "javít", "program", "algoritmus", "matek", "egyenlet",
            "excel", "logika", "adatbázis", "függvény", "változó", "típus",
            "tipus", "osztály", "objektum", "fordít", "kompilál", "fejleszt",
            "script", "szkript",
        ],
    );
    let creative_score = score_keywords(
        &lower,
        &[
            // angol kreatív-fogalmak
            "story", "creative", "write a", "write me", "imagine", "roleplay",
            "poem", "novel",
            // magyar tövek (a stem-matching miatt mindenféle ragozás illeszkedik:
            // "mese" → "mesét", "mesét", "mesével"; "írj" → "írj", "írjál"; stb.)
            "történet", "törté", "kreatív", "írj", "ír egy", "vers", "poéma",
            "szereplő", "fantáz", "beszélges", "érzel", "szabadon", "képzel",
            "mese", "regény", "humor", "novella", "költ", "tündér", "varázs",
        ],
    );

    // Bármely egyértelmű találat dönt - a régi "ha rövid a prompt → mindig Eco"
    // szabály ide-oda dobálta a triviális üzeneteket (pl. "Írj mesét egy
    // farkasról" 34 karakter → eco, holott egyértelmű creative kérés).
    if brain_score > creative_score && brain_score > 0 {
        return AkashaSlot::Brain;
    }
    if creative_score > brain_score && creative_score > 0 {
        return AkashaSlot::Creative;
    }
    if brain_score > 0 && creative_score > 0 {
        // Döntetlen + mindkettő talált: programozási kontextus erősebb.
        return AkashaSlot::Brain;
    }
    // Semmi specifikus keyword → marad az eco (gyors, általános beszélgetés).
    AkashaSlot::Eco
}

/// Stem-matching keyword scorer. A promptot szavakra bontja és minden
/// kulcsszót akkor talál, ha bármelyik szó a prompt-ban a kulcsszó tövével
/// kezdődik. Így a magyar ragozott alakok is illeszkednek: "mesét"
/// `starts_with` "mese" → match.
fn score_keywords(lower: &str, keywords: &[&str]) -> u32 {
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    keywords
        .iter()
        .filter(|k| {
            // Egyszavas keyword: prefix-match. Többszavas (pl. "write a"):
            // teljes substring-match a prompton.
            if k.contains(' ') {
                lower.contains(*k)
            } else {
                words.iter().any(|w| w.starts_with(*k))
            }
        })
        .count() as u32
}
