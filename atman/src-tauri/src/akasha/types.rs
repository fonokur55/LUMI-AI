use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AkashaSlot {
    Eco,
    Brain,
    Creative,
}

impl AkashaSlot {
    pub fn label(self) -> &'static str {
        match self {
            AkashaSlot::Eco => "Eco",
            AkashaSlot::Brain => "Brain",
            AkashaSlot::Creative => "Creative",
        }
    }

    pub fn profile_domain(self) -> &'static str {
        match self {
            AkashaSlot::Eco => "general",
            AkashaSlot::Brain => "code",
            AkashaSlot::Creative => "writing",
        }
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
    let base = match slot {
        AkashaSlot::Brain => "Te AKASHA vagy, a NOMAD LUMI fő intelligenciája - szakértő programozó, \
            matematikus és üzleti logikai asszisztens. Adj pontos, strukturált, futtatható megoldásokat. \
            Kódot mindig teljes blokkokban adj.",
        AkashaSlot::Creative => "Te AKASHA vagy, a NOMAD LUMI kreatív intelligenciája. \
            Természetes, meleg, organikus magyar stílusban válaszolj. \
            Mintha egy megbízható beszélgetőpartner lennél, aki nem ítélkezik.",
        AkashaSlot::Eco => "Te AKASHA vagy, a NOMAD LUMI gyors öko-asszisztense. \
            Tömör, gyakorlati, barátságos válaszokat adj magyarul. \
            Kerüld a felesleges bővenlést - a felhasználó értékeli a gyorsaságot.",
    };
    let mut prompt = format!("{base}{PRIVACY_PREAMBLE}");
    if resource_limited {
        prompt.push_str(
            "\n\n[Erőforrás-korlátozott mód: a rendszer RAM/CPU terhelése miatt tömör válasz preferált.]"
        );
    }
    prompt
}

/// Legacy domain mapping for profile stats
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
        AkashaSlot::Brain => TaskDomain::Code,
        AkashaSlot::Creative => TaskDomain::Writing,
        AkashaSlot::Eco => TaskDomain::General,
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
