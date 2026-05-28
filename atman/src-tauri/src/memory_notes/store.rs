use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Egy darab memória-kártya. Több ilyen tárolódik az "Memória"
/// szekcióban (Gemini-stílusban), és minden chat előtt az engedélyezett
/// kártyák tömör formában a system promptba kerülnek.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryNote {
    pub id: String,
    pub title: String,
    pub content: String,
    /// Ha `false`, a kártya KIHAGYÁSRA kerül a system prompt összeállításnál.
    /// A felhasználó így ideiglenesen kikapcsolhat egy kártyát anélkül,
    /// hogy törölné.
    pub enabled: bool,
    /// ISO 8601 timestamp - sorba rendezéshez.
    pub created_at: String,
    pub updated_at: String,
}

/// SQLite-alapú tár az adat/memory/notes.db fájlban. Az `MemoryNote`-okat
/// kezeli (CRUD + toggle). A schema idempotens: minden megnyitáskor
/// `CREATE TABLE IF NOT EXISTS`.
pub struct MemoryNotesStore {
    db_path: String,
}

impl MemoryNotesStore {
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
            CREATE TABLE IF NOT EXISTS memory_notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_notes_updated
                ON memory_notes(updated_at DESC);
            "#,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Visszaadja az összes kártyát - frissesség szerint csökkenő sorrendben.
    pub fn list_all(&self) -> Result<Vec<MemoryNote>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, content, enabled, created_at, updated_at
                 FROM memory_notes
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(MemoryNote {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    enabled: row.get::<_, i64>(3)? != 0,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Csak az ENGEDÉLYEZETT kártyák - ezt használja a chat-flow integráció,
    /// amikor a system promptba illeszti a memóriát.
    pub fn list_enabled(&self) -> Result<Vec<MemoryNote>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, content, enabled, created_at, updated_at
                 FROM memory_notes
                 WHERE enabled = 1
                 ORDER BY updated_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(MemoryNote {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    enabled: row.get::<_, i64>(3)? != 0,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn create(&self, title: &str, content: &str) -> Result<MemoryNote, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_iso();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO memory_notes (id, title, content, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?4)",
            params![id, title, content, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(MemoryNote {
            id,
            title: title.to_string(),
            content: content.to_string(),
            enabled: true,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn update(
        &self,
        id: &str,
        title: &str,
        content: &str,
    ) -> Result<(), String> {
        let now = now_iso();
        let conn = self.conn()?;
        let affected = conn
            .execute(
                "UPDATE memory_notes SET title = ?1, content = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![title, content, now, id],
            )
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            return Err(format!("Memória-kártya nem található: {id}"));
        }
        Ok(())
    }

    pub fn toggle_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let now = now_iso();
        let conn = self.conn()?;
        let affected = conn
            .execute(
                "UPDATE memory_notes SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![if enabled { 1 } else { 0 }, now, id],
            )
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            return Err(format!("Memória-kártya nem található: {id}"));
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let conn = self.conn()?;
        let affected = conn
            .execute("DELETE FROM memory_notes WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        if affected == 0 {
            return Err(format!("Memória-kártya nem található: {id}"));
        }
        Ok(())
    }
}

// =========================================================================
//  Promptba illesztés - token-takarékos formátum
// =========================================================================

/// Egy egyszerű karakter-alapú "token" becslés: ~4 karakter / token a magyar
/// és angol vegyes szövegre, hasonlóan a tipikus BPE arányhoz. Nem pontos,
/// de elegendő egy felső limit kikényszerítésére.
fn approx_tokens(s: &str) -> usize {
    s.chars().count().div_ceil(4)
}

/// Formázza az engedélyezett kártyákat egy KOMPAKT system-prompt blokká.
/// A token-limit alatt minden kártya benne van; ha túllépné, a régebbi
/// kártyák (alacsonyabb `updated_at`) kimaradnak.
///
/// Visszatérés:
///  - `Some(blokk)` ha van legalább egy kártya
///  - `None` ha nincs egyetlen engedélyezett sem (semmit ne adjunk a prompthoz)
pub fn format_for_prompt(notes: &[MemoryNote], max_tokens: usize) -> Option<String> {
    if notes.is_empty() {
        return None;
    }
    let header = "## A felhasználó memóriájából (figyelembe veendő):\n";
    let footer = "\n";
    let mut body = String::new();
    let mut used = approx_tokens(header) + approx_tokens(footer);
    // Frissesség szerint, frissebb előrébb. A `list_enabled` már így adja.
    for n in notes {
        let title = n.title.trim();
        let content = n.content.trim();
        if content.is_empty() {
            continue;
        }
        let entry = if title.is_empty() {
            format!("- {content}\n")
        } else {
            format!("- **{title}:** {content}\n")
        };
        let entry_tokens = approx_tokens(&entry);
        if used + entry_tokens > max_tokens {
            // A többi kártya már nem fér be - egyszerűen kihagyjuk őket
            // (a felhasználó UI-ból láthatja, hogy mennyi a határ).
            break;
        }
        used += entry_tokens;
        body.push_str(&entry);
    }
    if body.is_empty() {
        return None;
    }
    Some(format!("{header}{body}{footer}"))
}

// =========================================================================
//  Pici helperek - nem akarunk új crate-et csak ezekért
// =========================================================================

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}
