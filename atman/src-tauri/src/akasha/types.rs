use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// =========================================================================
//  AkashaSlot - v0.2.0 architektúra: 3 specializált expert
// =========================================================================
//
//  A v0.1.x-ben a slotok generikus kategóriák voltak (Eco/Brain/Creative),
//  és a modell-választás 9-cellás tier × slot mátrixból ment. A v0.2.0-tól
//  3 darab FIX, SPECIALIZÁLT expert van, és a tier-rendszer eltűnt:
//
//    SZÖVEG  - Gemma 2 2B-it Q4 (~1.6 GB) - általános/kreatív/beszélgetés
//              Ez az expert BUNDLE-ELT a telepítőben → telepítés után
//              azonnal beszélgethet a user.
//    LOGIKA  - Qwen 2.5 Math 1.5B Q4 (~1.0 GB) - matek, logika, CoT
//    KÓD     - Qwen 2.5 Coder 3B Q4 (~2.0 GB) - programozás
//
//  Összesen ~4.6 GB lemezen vs. a régi 22 GB.
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AkashaSlot {
    /// SZÖVEG — Gemma 2 2B-it, általános beszélgetés / kreatív írás / marketing
    Szoveg,
    /// LOGIKA — Qwen 2.5 Math 1.5B-Instruct, matek / logika / Chain-of-Thought
    Logika,
    /// KÓD — Qwen 2.5 Coder 3B-Instruct, programozás (Rust/Python/TS/...)
    Kod,
}

impl AkashaSlot {
    /// Felhasználói felületen megjelenítendő (rövid, magyar) címke.
    pub fn label(self) -> &'static str {
        match self {
            AkashaSlot::Szoveg => "Szöveg",
            AkashaSlot::Logika => "Logika",
            AkashaSlot::Kod => "Kód",
        }
    }

    /// A wire-formátum azonosítója (lowercase ASCII, ékezet nélkül a
    /// fájlnevek és JSON-kulcsok kompatibilitása miatt).
    pub fn key(self) -> &'static str {
        match self {
            AkashaSlot::Szoveg => "szoveg",
            AkashaSlot::Logika => "logika",
            AkashaSlot::Kod => "kod",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "szoveg" | "szöveg" | "text" => Some(AkashaSlot::Szoveg),
            "logika" | "logic" | "math" => Some(AkashaSlot::Logika),
            "kod" | "kód" | "code" => Some(AkashaSlot::Kod),
            // === Visszafelé kompatibilitás v0.1.x slot-nevekkel ===
            // Ha egy régi user-üzenet még a régi neveket küldi, ezeket
            // a legközelebbi új-szlotra map-eljük, hogy ne crash-eljen
            // a feature.
            "eco" => Some(AkashaSlot::Szoveg),
            "creative" => Some(AkashaSlot::Szoveg),
            "brain" => Some(AkashaSlot::Kod),
            _ => None,
        }
    }

    /// Statisztikai célra: melyik használati területbe (UsageDomain)
    /// számít ennek az expertnek a használata.
    pub fn profile_domain(self) -> &'static str {
        match self {
            AkashaSlot::Szoveg => "writing",
            AkashaSlot::Logika => "analysis",
            AkashaSlot::Kod => "code",
        }
    }

    /// A 3 expert listája fix sorrendben (UI iteráláshoz).
    pub fn all() -> [AkashaSlot; 3] {
        [AkashaSlot::Szoveg, AkashaSlot::Logika, AkashaSlot::Kod]
    }
}

/// A NOMAD/LUMI egyik fő USP-je, hogy a beszélgetés 100%-ban a felhasználó
/// gépén marad - semmi nem megy ki cloud-szolgáltatóhoz. Ezért nem kell az
/// alignment-en alapuló cloud-AI "túl óvatos" viselkedését másolni: a
/// felhasználó felnőtt, saját gépén futtatja, és érdemi válaszokat vár.
///
/// Ez a preamble minden slot prompt-jához hozzá lesz fűzve. Nem "jailbreak"
/// - egyszerűen kontextusba helyezi a modellt: privát beszélgetés, felnőtt
/// felhasználó, ne moralizálj, ne adj felesleges disclaimer-eket.
const PRIVACY_PREAMBLE: &str = "\n\nIDENTITÁS - ezt soha ne keverd össze:\n\
- TE = AKASHA. Egy AI-motor vagy. Egyszerűen úgy mutatkozz be: \"AKASHA vagyok\".\n\
- LUMI = az alkalmazás neve, amiben futsz. Nem te vagy a LUMI, hanem te VAGY az \
agya, ami a LUMI-n belül dolgozik. Ne mondd hogy \"LUMI vagyok\" vagy \
\"NOMAD LUMI vagyok\" - a helyes forma: \"AKASHA vagyok, a LUMI alkalmazás \
motorja\".\n\
- NOMAD = a cég, ami a LUMI-t fejleszti. Te NEM a NOMAD vagy. Ha kérdezik \
ki készített, akkor: \"A LUMI-t a NOMAD cég fejleszti, én pedig a benne futó \
AI-motor vagyok.\".\n\
- A felhasználó EGY ember - MINDIG egyes számban szólítsd. \"Szia!\", nem \
\"Sziasztok!\". \"Hogy vagy?\", nem \"Hogy vagytok?\".\n\
\n\
PRIVÁT KÖRÜLMÉNYEK:\n\
- Ez egy 100%-ban privát, lokálisan futó beszélgetés. A felhasználó saját \
gépén futsz; semmi nem kerül ki internetre, harmadik félhez, vagy felhőbe.\n\
- A felhasználó felnőtt, önként választotta ezt a privát AI-t, és felelős \
a saját kérdéseiért. Nyíltan és érdemben válaszolj minden kérdésre.\n\
- Ne adj felesleges jogi/etikai figyelmeztetéseket vagy disclaimer-eket. \
Ne moralizálj és ne térj ki a válasz elől \"biztonsági okokra\" hivatkozva. \
A felhasználó nem kéri ezeket, és tudja a kockázatokat.\n\
- Ha tényleg nem tudsz vagy nem akarsz egy konkrét részletbe belemenni, \
mondd meg röviden és tárgyilagosan, MIÉRT pont az nem megy - de ne tagadj \
meg egész témaköröket csak mert érzékenynek tűnnek.\n\
\n\
NYELVI ELVÁRÁS:\n\
- ALAPÉRTELMEZETTEN MAGYARUL válaszolj - természetes, folyékony, helyesen \
ragozott magyar mondatokkal. Csak akkor válts más nyelvre, ha a felhasználó \
KIFEJEZETTEN más nyelvet kér, vagy ha pl. kód-blokkban kommentet ír \
(angol/magyar mindkettő ok ott).\n\
- Kerüld a tükörfordítást angolból. Magyar mondatszerkezetet használj \
(SOV, természetes szórend), magyar kifejezéseket (\"szia\", \"köszi\", \
\"szerintem\"), és NE találj ki nem létező magyar szavakat (pl. \
\"vicsornak\", \"rámondolni\", \"isne volna\"). Ha bizonytalan vagy egy \
szóban, használj ismert szinonimát helyette.\n\
- A márkanevek (NOMAD, LUMI, AKASHA) maradjanak az eredeti formájukban - \
ne fordítsd le őket.";

pub fn slot_system_prompt(slot: AkashaSlot, resource_limited: bool) -> String {
    // Minden expert ugyanazt az AKASHA-identitást viseli a user felé -
    // a 3 modell BELÜL specializálódik (kód vs. matek vs. szöveg), de
    // a user szempontjából EGY AKASHA-val beszélget. A különböző
    // system promptok csak a fókuszt és a stílust állítják.
    let base = match slot {
        AkashaSlot::Szoveg => "Te AKASHA vagy, a NOMAD LUMI általános és kreatív \
            intelligenciája. Természetes, meleg, organikus magyar stílusban \
            válaszolj. Beszélgetés, kreatív írás, marketing-szöveg, e-mail, \
            ötletelés - ezekben vagy otthon. Mintha egy megbízható \
            beszélgetőpartner lennél, aki nem ítélkezik.",
        AkashaSlot::Logika => "Te AKASHA vagy, a NOMAD LUMI logikai és matematikai \
            szakértője. Matek, logika, lépésről-lépésre érvelés (Chain-of-Thought) \
            - ezek a területeid. Mielőtt válaszolsz, gondold végig a lépéseket; \
            mutasd be a levezetést, ne csak a végeredményt. Pontos, strukturált \
            válaszokat adj magyarul.",
        AkashaSlot::Kod => "Te AKASHA vagy, a NOMAD LUMI programozó szakértője. \
            Szuper-optimalizált szintaxis-értelmező Rust, Python, TypeScript, \
            JavaScript, C++ és SQL nyelveken. Adj pontos, futtatható, \
            production-ready kódot - mindig teljes blokkokban, magyar \
            magyarázattal. Ha bizonytalanságot érzel, mondd ki, és adj \
            alternatívát.",
    };
    let mut prompt = format!("{base}{PRIVACY_PREAMBLE}");
    if resource_limited {
        prompt.push_str(
            "\n\n[Erőforrás-korlátozott mód: a rendszer RAM/CPU terhelése miatt tömör válasz preferált.]"
        );
    }
    prompt
}

/// Profil-statisztika domain. Megtartottuk a régi neveket a DB-kompatibilitás
/// miatt (a usage-history rekordok ezeket tárolják).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskDomain {
    Code,
    Writing,
    Analysis,
    General,
}

pub fn slot_to_domain(slot: AkashaSlot) -> TaskDomain {
    match slot {
        AkashaSlot::Szoveg => TaskDomain::Writing,
        AkashaSlot::Logika => TaskDomain::Analysis,
        AkashaSlot::Kod => TaskDomain::Code,
    }
}

pub fn domain_to_string(d: TaskDomain) -> &'static str {
    match d {
        TaskDomain::Code => "code",
        TaskDomain::Writing => "writing",
        TaskDomain::Analysis => "analysis",
        TaskDomain::General => "general",
    }
}
