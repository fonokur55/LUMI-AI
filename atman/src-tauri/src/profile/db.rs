use super::badges::{check_badges, BadgeInfo, BADGE_DEFINITIONS};
use super::events::UsageDomain;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileData {
    pub display_name: String,
    pub domain_hours: DomainHours,
    pub badges: Vec<BadgeInfo>,
    pub bugs_fixed: i64,
    pub messages_sent: i64,
    /// 1-12 vagy None ha még nem adta meg.
    pub birth_month: Option<u32>,
    /// 1-31 vagy None.
    pub birth_day: Option<u32>,
    /// Az avatar PNG-fájl elérési útja a data/profile/ mappában; None ha nincs.
    pub avatar_path: Option<String>,
}

/// First-run wizard státusz: a frontend ez alapján dönti el hogy mutatja-e
/// az induló bekérő modálot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSetupStatus {
    pub has_name: bool,
    pub has_birthday: bool,
}

/// Születésnap-info az induló köszöntő logikához.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BirthdayCheck {
    /// Igaz, ha MA van a user születésnapja.
    pub is_birthday_today: bool,
    /// Igaz, ha ma MÉG NEM köszöntöttük (akkor a UI confetti-vel + cím-cserével
    /// köszönt; aztán meghívja `profile_mark_birthday_greeted`-et).
    pub needs_greeting: bool,
    /// A user neve a köszöntő üzenethez (vagy default).
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainHours {
    pub code: f64,
    pub writing: f64,
    pub analysis: f64,
    pub general: f64,
}

pub struct ProfileStore {
    db_path: String,
}

impl ProfileStore {
    pub fn open(db_path: &str) -> Result<Self, String> {
        let store = Self {
            db_path: db_path.to_string(),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn conn(&self) -> Result<Connection, String> {
        if let Some(parent) = Path::new(&self.db_path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        Connection::open(&self.db_path).map_err(|e| e.to_string())
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS profile (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                display_name TEXT NOT NULL,
                bugs_fixed INTEGER NOT NULL DEFAULT 0,
                messages_sent INTEGER NOT NULL DEFAULT 0
            );
            -- Üres név alapból (NEM 'felhasználó'), hogy a first-run modal
            -- meg tudja állapítani: a user még nem adta meg.
            INSERT OR IGNORE INTO profile (id, display_name) VALUES (1, '');
            CREATE TABLE IF NOT EXISTS usage_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                domain TEXT NOT NULL,
                seconds INTEGER NOT NULL,
                started_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS badges (
                id TEXT PRIMARY KEY,
                unlocked_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                payload TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .map_err(|e| e.to_string())?;

        // Idempotens oszlop-hozzáadás (régebbi DB-k frissítése). Ha már
        // megvan az oszlop, az ALTER hibára fut - azt elnyeljük.
        let conn = self.conn()?;
        for sql in [
            "ALTER TABLE profile ADD COLUMN birth_month INTEGER",
            "ALTER TABLE profile ADD COLUMN birth_day INTEGER",
            "ALTER TABLE profile ADD COLUMN avatar_path TEXT",
            // YYYY-MM-DD formátum, hogy ne köszöntsük újra ugyanazon a napon
            "ALTER TABLE profile ADD COLUMN last_birthday_greeted TEXT",
            // YYYY-MM-DD - AKASHA-val történt LEGUTÓBBI beszélgetés napja.
            // Ha ma még nem volt, AKASHA az első üzenetben köszönt.
            "ALTER TABLE profile ADD COLUMN last_chat_date TEXT",
        ] {
            let _ = conn.execute(sql, []);
        }
        Ok(())
    }

    /// Visszaadja MA az első chat-e a felhasználónak (mai dátum != last_chat_date),
    /// és egyúttal beállítja a `last_chat_date`-et a mai napra. A `true` válasz
    /// azt jelenti: AKASHA most köszöntse a felhasználót egyszer az aznap első
    /// üzenetében.
    pub fn check_and_mark_daily_first_chat(&self) -> Result<bool, String> {
        let today_iso = chrono::Local::now().format("%Y-%m-%d").to_string();
        let conn = self.conn()?;
        let last: Option<String> = conn
            .query_row(
                "SELECT last_chat_date FROM profile WHERE id = 1",
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);
        let is_first = last.as_deref() != Some(today_iso.as_str());
        if is_first {
            conn.execute(
                "UPDATE profile SET last_chat_date = ?1 WHERE id = 1",
                params![today_iso],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(is_first)
    }

    pub fn get_display_name(&self) -> Result<String, String> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT display_name FROM profile WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    }

    pub fn set_display_name(&self, name: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE profile SET display_name = ?1 WHERE id = 1",
            params![name],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn record_session(&self, domain: UsageDomain, seconds: u64) -> Result<(), String> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO usage_sessions (domain, seconds, started_at) VALUES (?1, ?2, ?3)",
            params![domain.as_str(), seconds as i64, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn record_event(&self, kind: &str, payload: Option<&str>) -> Result<(), String> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        if kind == "bugs_fixed" {
            conn.execute(
                "UPDATE profile SET bugs_fixed = bugs_fixed + 1 WHERE id = 1",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        if kind == "message_sent" {
            conn.execute(
                "UPDATE profile SET messages_sent = messages_sent + 1 WHERE id = 1",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        conn.execute(
            "INSERT INTO events (kind, payload, created_at) VALUES (?1, ?2, ?3)",
            params![kind, payload, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn domain_seconds(&self, domain: &str) -> Result<i64, String> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT COALESCE(SUM(seconds), 0) FROM usage_sessions WHERE domain = ?1",
            params![domain],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())
    }

    fn unlocked_badges(&self) -> Result<Vec<(String, String)>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT id, unlocked_at FROM badges")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn unlock_badges(&self, ids: &[String]) -> Result<(), String> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        for id in ids {
            conn.execute(
                "INSERT OR IGNORE INTO badges (id, unlocked_at) VALUES (?1, ?2)",
                params![id, now],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn evaluate_badges(&self, memory_chunks: i64) -> Result<Vec<String>, String> {
        let conn = self.conn()?;
        let bugs: i64 = conn
            .query_row("SELECT bugs_fixed FROM profile WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);
        let code_sec = self.domain_seconds("code")?;
        let unlocked: Vec<String> = self
            .unlocked_badges()?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let new = check_badges(bugs, code_sec, memory_chunks, &unlocked);
        self.unlock_badges(&new)?;
        Ok(new)
    }

    pub fn get_profile(&self, memory_chunks: i64) -> Result<ProfileData, String> {
        let _ = self.evaluate_badges(memory_chunks);
        let conn = self.conn()?;
        let display_name: String = conn
            .query_row("SELECT display_name FROM profile WHERE id = 1", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let bugs_fixed: i64 = conn
            .query_row("SELECT bugs_fixed FROM profile WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);
        let messages_sent: i64 = conn
            .query_row("SELECT messages_sent FROM profile WHERE id = 1", [], |r| r.get(0))
            .unwrap_or(0);
        let birth_month: Option<u32> = conn
            .query_row("SELECT birth_month FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None)
            .map(|v| v as u32);
        let birth_day: Option<u32> = conn
            .query_row("SELECT birth_day FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None)
            .map(|v| v as u32);
        let avatar_path: Option<String> = conn
            .query_row("SELECT avatar_path FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .unwrap_or(None);

        let domain_hours = DomainHours {
            code: self.domain_seconds("code")? as f64 / 3600.0,
            writing: self.domain_seconds("writing")? as f64 / 3600.0,
            analysis: self.domain_seconds("analysis")? as f64 / 3600.0,
            general: self.domain_seconds("general")? as f64 / 3600.0,
        };

        let unlocked_map: std::collections::HashMap<String, String> = self
            .unlocked_badges()?
            .into_iter()
            .collect();

        let badges: Vec<BadgeInfo> = BADGE_DEFINITIONS
            .iter()
            .map(|b| {
                let unlocked = unlocked_map.contains_key(b.id);
                BadgeInfo {
                    id: b.id.to_string(),
                    title: b.title.to_string(),
                    description: b.description.to_string(),
                    unlocked,
                    unlocked_at: unlocked_map.get(b.id).cloned(),
                }
            })
            .collect();

        Ok(ProfileData {
            display_name,
            domain_hours,
            badges,
            bugs_fixed,
            messages_sent,
            birth_month,
            birth_day,
            avatar_path,
        })
    }

    /// Beállítja a születésnapot (1-12 hónap, 1-31 nap).
    pub fn set_birthday(&self, month: u32, day: u32) -> Result<(), String> {
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return Err("Érvénytelen születésnap (hónap 1-12, nap 1-31).".into());
        }
        let conn = self.conn()?;
        conn.execute(
            "UPDATE profile SET birth_month = ?1, birth_day = ?2 WHERE id = 1",
            params![month as i64, day as i64],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Beállítja az avatar elérési útját (a data/profile/avatar.png-re mutat).
    pub fn set_avatar_path(&self, path: Option<&str>) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE profile SET avatar_path = ?1 WHERE id = 1",
            params![path],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// First-run wizard státusz: a name és birthday vannak-e?
    pub fn get_setup_status(&self) -> Result<ProfileSetupStatus, String> {
        let conn = self.conn()?;
        let display_name: String = conn
            .query_row("SELECT display_name FROM profile WHERE id = 1", [], |r| r.get(0))
            .unwrap_or_default();
        let bm: Option<i64> = conn
            .query_row("SELECT birth_month FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None);
        let bd: Option<i64> = conn
            .query_row("SELECT birth_day FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None);
        Ok(ProfileSetupStatus {
            has_name: !display_name.trim().is_empty(),
            has_birthday: bm.is_some() && bd.is_some(),
        })
    }

    /// Megnézi hogy ma van-e a user születésnapja, és hogy kell-e még köszönteni
    /// ma (UTC-Local napon: a `last_birthday_greeted` ISO-dátum bekerül a DB-be
    /// minden köszöntés után, és ma csak akkor köszöntünk újra, ha más nap az).
    pub fn check_birthday_today(&self) -> Result<BirthdayCheck, String> {
        let conn = self.conn()?;
        let display_name: String = conn
            .query_row("SELECT display_name FROM profile WHERE id = 1", [], |r| r.get(0))
            .unwrap_or_default();
        let bm: Option<i64> = conn
            .query_row("SELECT birth_month FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None);
        let bd: Option<i64> = conn
            .query_row("SELECT birth_day FROM profile WHERE id = 1", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .unwrap_or(None);
        let last_greeted: Option<String> = conn
            .query_row(
                "SELECT last_birthday_greeted FROM profile WHERE id = 1",
                [],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);

        use chrono::Datelike;
        let now = chrono::Local::now();
        let today_iso = now.format("%Y-%m-%d").to_string();

        let is_birthday_today = match (bm, bd) {
            (Some(m), Some(d)) => now.month() == m as u32 && now.day() == d as u32,
            _ => false,
        };
        let needs_greeting =
            is_birthday_today && last_greeted.as_deref() != Some(today_iso.as_str());

        Ok(BirthdayCheck {
            is_birthday_today,
            needs_greeting,
            display_name,
        })
    }

    /// A frontend hívja miután lefutott a konfetti+köszöntés animáció -
    /// elmenti hogy ma már köszöntöttünk.
    pub fn mark_birthday_greeted(&self) -> Result<(), String> {
        let today_iso = chrono::Local::now().format("%Y-%m-%d").to_string();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE profile SET last_birthday_greeted = ?1 WHERE id = 1",
            params![today_iso],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
