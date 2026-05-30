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
            // === Programozási nyelvek és keretrendszerek ===
            "rust", "typescript", "javascript", "python", "java", "kotlin",
            "swift", "go", "ruby", "php", "scala", "haskell", "elixir",
            "c++", "c#", "csharp",
            // Web-stack
            "html", "css", "scss", "sass", "tailwind", "bootstrap",
            "react", "vue", "angular", "svelte", "nextjs", "next.js",
            "node", "nodejs", "node.js", "express", "fastapi", "django",
            "flask", "rails", "spring", "tauri",
            // Tooling
            "git", "github", "gitlab", "npm", "yarn", "pnpm", "cargo",
            "pip", "poetry", "docker", "kubernetes", "k8s", "webpack",
            "vite", "rollup", "babel", "eslint", "prettier",
            // === Általános kód-fogalmak (angol) ===
            "function", "class", "import", "def", "let", "const", "var",
            "void", "return", "struct", "enum", "trait", "interface",
            "impl", "async", "await", "promise", "callback", "lambda",
            "regex", "regexp", "json", "xml", "yaml", "toml",
            "api", "rest", "graphql", "sql", "nosql", "mongodb", "postgres",
            "mysql", "redis", "sqlite",
            "bug", "fix", "refactor", "debug", "compile", "build",
            "deploy", "test", "unittest", "pytest", "jest",
            "code", "snippet", "algorithm", "data structure",
            // === Magyar tövek (stem-matching) ===
            // alap programozási
            "kód", "kódol", "programoz", "fejleszt", "implementál",
            "szkript", "script",
            // hibakezelés / refaktor
            "hiba", "bug", "javít", "kijavít", "refaktorál", "tesztel",
            // szintaxis
            "függvény", "metódus", "osztály", "objektum", "változó",
            "típus", "tipus", "interfész", "modul", "csomag",
            // web/UI
            "weboldal", "website", "honlap", "webapp", "frontend",
            "backend", "felület", "design",
            // adat
            "adatbázis", "lekérdezés", "tábla", "rekord", "séma",
            // egyéb tipikus magyar promptok
            "fordít", "kompilál", "futtatás", "indítás", "telepít",
            "excel", "csv", "json", "algoritmus", "automatiz",
            // tipikus felhasználói kérések
            "csinálj", "építsd", "építs", "hozz létre", "készíts",
            "implementálj", "íjr egy", "írj egy",
        ],
    );

    let logika_score = score_keywords(
        &lower,
        &[
            // === Angol matek/logika ===
            "math", "mathematics", "equation", "calculate", "solve",
            "derivative", "integral", "logarithm", "logic", "proof",
            "theorem", "geometry", "algebra", "trigonometry",
            "probability", "statistics", "variance", "expectation",
            "matrix", "vector", "tensor", "graph", "set theory",
            "permutation", "combination", "factorial",
            // === Magyar matek/logika tövek ===
            "matek", "matem", "egyenlet", "egyenlőtlen", "számít",
            "számol", "számítás", "összead", "kivon", "szoroz",
            "oszt", "négyzetgyök", "köbgyök", "hatvány",
            "törtek", "tört", "tizedes", "kerekít", "százalék",
            "arány", "hányad", "képlet", "tétel", "bizonyít",
            "derivált", "integrál", "logaritmus", "logika", "logikai",
            "logikus", "érvel", "következtetés", "okosk", "gondolatmenet",
            "geometria", "geometriai", "algebra", "trigon",
            "valószínűség", "statiszt", "átlag", "medián", "szórás",
            "egyenes", "kör", "háromszög", "négyszög", "körvonal",
            "vektor", "mátrix", "halmaz", "halmaze", "függvénytan",
            // Tipikus tankönyv-kifejezések
            "mennyi", "hány", "hány százalék", "hány az", "mi az érték",
            "mekkora", "milyen hosszú", "hány darab", "hány milyen",
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
