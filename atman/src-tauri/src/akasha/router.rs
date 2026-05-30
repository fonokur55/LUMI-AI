use super::types::AkashaSlot;
use crate::portable::config::AkashaConfig;
use std::path::Path;

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

/// Kiválasztja a prompthoz illő expertet (Szöveg / Logika / Kód).
///
/// Routing logika:
///   1. classify_prompt → kulcsszó-alapú kategorizálás
///   2. Ha a kiválasztott expert nincs letöltve, visszaesés a Szövegre
///      (mindig elérhető, mert bundled)
///   3. Kritikus erőforráshelyzetben mindig a Szöveg (legkisebb, leggyorsabb)
pub fn route_prompt(input: &RouteInput) -> RouteResult {
    let mut slot = classify_prompt(input.prompt);
    let mut resource_limited = false;

    // Kritikus RAM/CPU → mindig a Szöveg (a legkisebb, leggyorsabb)
    if input.is_critical {
        if slot != AkashaSlot::Szoveg {
            slot = AkashaSlot::Szoveg;
            resource_limited = true;
        }
    }

    // Ellenőrizzük a kiválasztott expert letöltöttségét; ha nincs meg,
    // visszaesés a Szövegre (bundled, mindig elérhető)
    if !slot_available(slot, input.launch_root, input.akasha) {
        slot = AkashaSlot::Szoveg;
    }

    let model_filename = format!("{}.gguf", slot.key());
    RouteResult {
        slot,
        model_filename,
        resource_limited,
    }
}

fn slot_available(slot: AkashaSlot, launch_root: &Path, akasha: &AkashaConfig) -> bool {
    let filename = format!("{}.gguf", slot.key());
    let path = akasha.arsenal_file_path(launch_root, &filename);
    std::fs::metadata(&path)
        .map(|m| m.len() >= MIN_GGUF_BYTES)
        .unwrap_or(false)
}

/// Kulcsszó-alapú prompt-kategorizálás. STEM-matching: a magyar ragozott
/// alakok is illeszkednek (pl. "függvényt" → "függvény").
///
/// Algoritmus:
///   - score_kod  (kód kulcsszavak találata)
///   - score_logika (matek/logika kulcsszavak)
///   - A nagyobb score nyer; ha mindkettő 0, fallback Szöveg.
///   - Döntetlen + mindkettő talált: kód-kontextus erősebb (egyetlen
///     "rust struct"-példa még matek-szavakkal együtt is kód).
fn classify_prompt(text: &str) -> AkashaSlot {
    let lower = text.to_lowercase();

    let kod_score = score_keywords(
        &lower,
        &[
            // angol kód-fogalmak
            "function", "class", "import", "def", "rust", "typescript",
            "javascript", "python", "sql", "api", "bug", "compile", "git",
            "refactor", "debug", "variable", "struct", "enum", "async",
            "await", "code", "algorithm", "regex", "tauri", "react",
            "node", "npm", "cargo", "docker", "json", "yaml",
            // magyar tövek
            "kód", "hiba", "javít", "program", "algoritmus", "adatbázis",
            "függvény", "változó", "típus", "tipus", "osztály", "objektum",
            "fordít", "kompilál", "fejleszt", "script", "szkript", "excel",
            "lekérdezés", "tábla", "metódus", "interfész",
        ],
    );

    let logika_score = score_keywords(
        &lower,
        &[
            // angol matek/logika
            "math", "equation", "calculate", "solve", "derivative",
            "integral", "logarithm", "logic", "proof", "theorem",
            "geometry", "algebra", "probability", "statistics",
            // magyar tövek
            "matek", "egyenlet", "számít", "számol", "osztály", "törtek",
            "derivált", "integrál", "logaritmus", "logika", "bizonyít",
            "tétel", "geometria", "algebra", "valószínűség", "statiszt",
            "számítás", "kerekít", "százalék", "arány", "képlet",
            "egyenes", "vektor", "mátrix", "halmaz",
        ],
    );

    if kod_score > logika_score && kod_score > 0 {
        return AkashaSlot::Kod;
    }
    if logika_score > kod_score && logika_score > 0 {
        return AkashaSlot::Logika;
    }
    if kod_score > 0 && logika_score > 0 {
        // Döntetlen + mindkettő talált: kód-kontextus erősebb.
        return AkashaSlot::Kod;
    }
    // Semmi specifikus keyword → Szöveg (általános/kreatív beszélgetés).
    AkashaSlot::Szoveg
}

/// Stem-matching keyword scorer. A promptot szavakra bontja és minden
/// kulcsszót akkor talál, ha bármelyik szó a prompt-ban a kulcsszó tövével
/// kezdődik.
fn score_keywords(lower: &str, keywords: &[&str]) -> u32 {
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    keywords
        .iter()
        .filter(|k| {
            if k.contains(' ') {
                lower.contains(*k)
            } else {
                words.iter().any(|w| w.starts_with(*k))
            }
        })
        .count() as u32
}
