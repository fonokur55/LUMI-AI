use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeInfo {
    pub id: String,
    pub title: String,
    pub description: String,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
}

pub struct BadgeDef {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

pub const BADGE_DEFINITIONS: &[BadgeDef] = &[
    BadgeDef {
        id: "bug_vadasz",
        title: "Bug Vadász",
        description: "10 hibajavítási esemény rögzítve",
    },
    BadgeDef {
        id: "code_sage",
        title: "Code Sage",
        description: "50 óra programozási domain használat",
    },
    BadgeDef {
        id: "akashic_scholar",
        title: "Akashic Scholar",
        description: "100 dokumentum chunk a TOTAL MEMÓRIÁban",
    },
];

pub fn check_badges(
    bugs_fixed: i64,
    code_seconds: i64,
    memory_chunks: i64,
    already: &[String],
) -> Vec<String> {
    let mut newly = Vec::new();
    let code_hours = code_seconds / 3600;

    if bugs_fixed >= 10 && !already.contains(&"bug_vadasz".to_string()) {
        newly.push("bug_vadasz".into());
    }
    if code_hours >= 50 && !already.contains(&"code_sage".to_string()) {
        newly.push("code_sage".into());
    }
    if memory_chunks >= 100 && !already.contains(&"akashic_scholar".to_string()) {
        newly.push("akashic_scholar".into());
    }
    newly
}
