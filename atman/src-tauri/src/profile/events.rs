use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageDomain {
    Code,
    Writing,
    Analysis,
    General,
}

impl UsageDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            UsageDomain::Code => "code",
            UsageDomain::Writing => "writing",
            UsageDomain::Analysis => "analysis",
            UsageDomain::General => "general",
        }
    }

    pub fn from_task(s: &str) -> Self {
        match s {
            "code" => UsageDomain::Code,
            "writing" => UsageDomain::Writing,
            "analysis" => UsageDomain::Analysis,
            _ => UsageDomain::General,
        }
    }
}
