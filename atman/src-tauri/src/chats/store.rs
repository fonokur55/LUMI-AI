use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPreview {
    pub id: String,
    pub title: String,
    pub group_id: Option<String>,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageRecord {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFull {
    pub preview: ChatPreview,
    pub messages: Vec<ChatMessageRecord>,
}

pub struct ChatStore {
    db_path: PathBuf,
}

impl ChatStore {
    pub fn open(db_path: &str) -> Result<Self, String> {
        let store = Self {
            db_path: PathBuf::from(db_path),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn conn(&self) -> Result<Connection, String> {
        let conn = Connection::open(&self.db_path).map_err(|e| e.to_string())?;
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| e.to_string())?;
        Ok(conn)
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL,
                icon TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                group_id TEXT,
                pinned INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE SET NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                chat_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_messages_chat ON messages(chat_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_chats_updated ON chats(updated_at DESC);
            "#,
        )
        .map_err(|e| e.to_string())
    }

    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    // === GROUPS ===

    pub fn groups_list(&self) -> Result<Vec<Group>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, color, icon, sort_order, created_at FROM groups \
                 ORDER BY sort_order ASC, created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    icon: row.get(3)?,
                    sort_order: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn group_create(
        &self,
        name: &str,
        color: &str,
        icon: &str,
    ) -> Result<Group, String> {
        let id = Uuid::new_v4().to_string();
        let created = Self::now();
        let conn = self.conn()?;
        let max_sort: i64 = conn
            .query_row("SELECT COALESCE(MAX(sort_order), -1) FROM groups", [], |r| {
                r.get(0)
            })
            .unwrap_or(-1);
        let sort_order = max_sort + 1;
        conn.execute(
            "INSERT INTO groups (id, name, color, icon, sort_order, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, color, icon, sort_order, created],
        )
        .map_err(|e| e.to_string())?;
        Ok(Group {
            id,
            name: name.into(),
            color: color.into(),
            icon: icon.into(),
            sort_order,
            created_at: created,
        })
    }

    pub fn group_update(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
        icon: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn()?;
        if let Some(n) = name {
            conn.execute("UPDATE groups SET name = ?1 WHERE id = ?2", params![n, id])
                .map_err(|e| e.to_string())?;
        }
        if let Some(c) = color {
            conn.execute("UPDATE groups SET color = ?1 WHERE id = ?2", params![c, id])
                .map_err(|e| e.to_string())?;
        }
        if let Some(i) = icon {
            conn.execute("UPDATE groups SET icon = ?1 WHERE id = ?2", params![i, id])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn group_delete(&self, id: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM groups WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // === CHATS ===

    pub fn chats_list(&self) -> Result<Vec<ChatPreview>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                r#"SELECT c.id, c.title, c.group_id, c.pinned, c.created_at, c.updated_at,
                          (SELECT COUNT(*) FROM messages m WHERE m.chat_id = c.id) as cnt,
                          (SELECT content FROM messages m WHERE m.chat_id = c.id
                           ORDER BY m.created_at DESC LIMIT 1) as last_msg
                   FROM chats c
                   ORDER BY c.pinned DESC, c.updated_at DESC"#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let preview: Option<String> = row.get::<_, Option<String>>(7)?.map(|s| {
                    let mut t: String = s.chars().take(80).collect();
                    if s.chars().count() > 80 {
                        t.push('…');
                    }
                    t
                });
                Ok(ChatPreview {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    group_id: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? != 0,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    message_count: row.get(6)?,
                    preview,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn chat_get(&self, id: &str) -> Result<ChatFull, String> {
        let conn = self.conn()?;
        let preview = conn
            .query_row(
                r#"SELECT id, title, group_id, pinned, created_at, updated_at,
                          (SELECT COUNT(*) FROM messages m WHERE m.chat_id = chats.id) as cnt
                   FROM chats WHERE id = ?1"#,
                params![id],
                |row| {
                    Ok(ChatPreview {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        group_id: row.get(2)?,
                        pinned: row.get::<_, i64>(3)? != 0,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                        message_count: row.get(6)?,
                        preview: None,
                    })
                },
            )
            .map_err(|e| format!("Beszélgetés nem található: {e}"))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, role, content, created_at FROM messages \
                 WHERE chat_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![id], |row| {
                Ok(ChatMessageRecord {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let messages = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        Ok(ChatFull { preview, messages })
    }

    /// INSERT OR IGNORE - ha létezik a `chat_id`, csak az `updated_at`-ot frissíti.
    pub fn chat_ensure(&self, id: &str, title: &str) -> Result<(), String> {
        let conn = self.conn()?;
        let now = Self::now();
        let changed = conn
            .execute(
                "INSERT OR IGNORE INTO chats (id, title, group_id, pinned, created_at, updated_at) \
                 VALUES (?1, ?2, NULL, 0, ?3, ?3)",
                params![id, title, now],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            // Már létezik - friss updated_at.
            conn.execute(
                "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn chat_rename(&self, id: &str, title: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, Self::now(), id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn chat_delete(&self, id: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM chats WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn chat_pin(&self, id: &str, pinned: bool) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE chats SET pinned = ?1 WHERE id = ?2",
            params![pinned as i64, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn chat_set_group(&self, id: &str, group_id: Option<&str>) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE chats SET group_id = ?1 WHERE id = ?2",
            params![group_id, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn chat_search(&self, query: &str, limit: usize) -> Result<Vec<ChatPreview>, String> {
        let conn = self.conn()?;
        let q = format!("%{}%", query.to_lowercase());
        let mut stmt = conn
            .prepare(
                r#"SELECT DISTINCT c.id, c.title, c.group_id, c.pinned, c.created_at, c.updated_at,
                          (SELECT COUNT(*) FROM messages m WHERE m.chat_id = c.id) as cnt,
                          (SELECT content FROM messages m WHERE m.chat_id = c.id
                           ORDER BY m.created_at DESC LIMIT 1) as last_msg
                   FROM chats c
                   LEFT JOIN messages m ON m.chat_id = c.id
                   WHERE LOWER(c.title) LIKE ?1
                      OR LOWER(m.content) LIKE ?1
                   ORDER BY c.pinned DESC, c.updated_at DESC
                   LIMIT ?2"#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![q, limit as i64], |row| {
                let preview: Option<String> = row.get::<_, Option<String>>(7)?.map(|s| {
                    let mut t: String = s.chars().take(80).collect();
                    if s.chars().count() > 80 {
                        t.push('…');
                    }
                    t
                });
                Ok(ChatPreview {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    group_id: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? != 0,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    message_count: row.get(6)?,
                    preview,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn message_append(
        &self,
        chat_id: &str,
        role: &str,
        content: &str,
    ) -> Result<ChatMessageRecord, String> {
        let conn = self.conn()?;
        let id = Uuid::new_v4().to_string();
        let now = Self::now();
        conn.execute(
            "INSERT INTO messages (id, chat_id, role, content, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, chat_id, role, content, now],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
            params![now, chat_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(ChatMessageRecord {
            id,
            role: role.into(),
            content: content.into(),
            created_at: now,
        })
    }
}
