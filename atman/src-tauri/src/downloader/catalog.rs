// =========================================================================
//  v0.2.0 katalógus - 3 SPECIALIZÁLT EXPERT
// =========================================================================
//  A v0.1.x 9-cellás tier × slot mátrixát egy tiszta 3-modell architektúrára
//  cseréltük. Mindegyik expert egy szakterületre optimalizált, és Q4_K_M
//  kvantálású — a kis modellek (1.5–3B) ennél lentebb (Q3) észlelhetően
//  butulnak, ezért feljebb tartjuk a minőségi sávot.
//
//  EXPERTEK:
//    SZÖVEG (Gemma 2 2B-it, ~1.6 GB) - általános beszélgetés, kreatív írás,
//                                       marketing. Magyar nyelvi erősség.
//                                       BUNDLE-ELT a telepítőben (azonnal
//                                       használható telepítés után).
//    LOGIKA (Qwen 2.5 Math 1.5B-Instruct, ~1.0 GB) - matek, logika,
//                                                     Chain-of-Thought
//    KÓD    (Qwen 2.5 Coder 3B-Instruct, ~2.0 GB) - programozás
//
//  Összesen: ~4.6 GB (vs. régi 22 GB).
//  Egy időpontban RAM-ban: ~1.5–2.8 GB (a router unload-olja a többit).
// =========================================================================

use crate::akasha::types::AkashaSlot;
use serde::Serialize;

/// Egy expert leírása a katalógusban.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpertEntry {
    pub slot: AkashaSlot,
    /// HuggingFace repó (pl. "bartowski/gemma-2-2b-it-GGUF")
    pub repo: &'static str,
    /// A repón belüli GGUF fájl neve
    pub file: &'static str,
    /// Felhasználóbarát megjelenítendő név (Settings + first-run modal)
    pub display_name: &'static str,
    /// Rövid leírás a UI-ban ("Mit tud?")
    pub description: &'static str,
    /// Közelítő letöltött méret GB-ban (progress + figyelmeztetés)
    pub size_gb: f32,
    /// Bundled = a telepítőben szállítva (azonnal elérhető telepítés után).
    /// Jelenleg csak a SZÖVEG.
    pub bundled: bool,
}

impl ExpertEntry {
    /// A lemezen tárolt fájl neve: `<slot>.gguf` (pl. `szoveg.gguf`).
    /// Ez független a HuggingFace forrásfájl nevétől, így ha később
    /// modellt cserélünk, csak a `repo`+`file` változik, a path stabil
    /// marad.
    pub fn local_filename(&self) -> String {
        format!("{}.gguf", self.slot.key())
    }

    /// HuggingFace public URL a letöltéshez.
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            self.repo, self.file
        )
    }
}

// =========================================================================
//  A 3 EXPERT
// =========================================================================
pub const CATALOG: &[ExpertEntry] = &[
    ExpertEntry {
        slot: AkashaSlot::Szoveg,
        repo: "bartowski/gemma-2-2b-it-GGUF",
        file: "gemma-2-2b-it-Q4_K_M.gguf",
        display_name: "Szöveg — Gemma 2 2B",
        description: "Általános beszélgetés, kreatív írás, magyar nyelvi erősség",
        size_gb: 1.6,
        bundled: true,
    },
    ExpertEntry {
        slot: AkashaSlot::Logika,
        // v0.2.3 (B opció): Phi-3.5 mini → Gemma 2 9B Q3_K_M.
        // A Phi-3.5 mini magyarul érthető szöveget adott, de a matek-
        // levezetésben hibázott (a `(2x-3)(x+5)` egyenletre `x=22.5`-öt
        // adott, miközben nincs valós megoldás — diszkrimináns = -28).
        // A Gemma 2 9B *natívan* tudja a magyart (a kis 2B-s testvérétől
        // örökölt erősség, csak nagyobb kapacitással), és a 9B mérettől
        // a Gemma család tényleg tud matekot. RAM-igény ~5 GB egyszerre,
        // a router unload-flow miatt 8 GB-os gépeken is OK.
        // Méret: 2.3 GB → 4.2 GB (Q3_K_M).
        repo: "bartowski/gemma-2-9b-it-GGUF",
        file: "gemma-2-9b-it-Q3_K_M.gguf",
        display_name: "Logika — Gemma 2 9B",
        description: "Matek, logika, érvelés magyarul (Google Gemma 2 9B)",
        size_gb: 4.2,
        bundled: false,
    },
    ExpertEntry {
        slot: AkashaSlot::Kod,
        // v0.2.2 (A-mínusz csomag): Qwen 2.5 Coder 3B → Qwen 2.5 Coder 7B.
        // A 3B Coder magyar magyarázata pocsék volt ("magyarosz termékek",
        // "Életvények"). A 7B verziónál jobb a magyar nyelvi tudás +
        // jelentősen erősebb kód-generálás (a Coder család 7B-től kezd
        // tényleg produktív lenni). RAM-igény ~4.5 GB egyszerre, ami a
        // router unload-flow miatt OK 8 GB-os gépeken is.
        repo: "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF",
        file: "Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf",
        display_name: "Kód — Qwen 2.5 Coder 7B",
        description: "Programozás: Rust, Python, TypeScript, JS, C++, SQL",
        size_gb: 4.4,
        bundled: false,
    },
];

/// Kikeresi a megadott slot expertjét.
pub fn lookup(slot: AkashaSlot) -> Option<&'static ExpertEntry> {
    CATALOG.iter().find(|e| e.slot == slot)
}

/// A teljes csomag mérete GB-ban (3 expert összesen).
pub fn total_size_gb() -> f32 {
    CATALOG.iter().map(|e| e.size_gb).sum()
}

/// Csak a NEM-bundled expertek mérete (amit első indításkor le kell
/// tölteni a háttérben).
pub fn background_download_size_gb() -> f32 {
    CATALOG.iter().filter(|e| !e.bundled).map(|e| e.size_gb).sum()
}
