use super::chunker::chunk_text;
use super::embedder::{bytes_to_embedding, cosine_similarity, embed_text, embedding_to_bytes};
use crate::portable::config::MemoryConfig;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub id: String,
    pub name: String,
    pub chunk_count: i64,
    pub created_at: String,
}

pub struct MemoryStore {
    db_path: PathBuf,
    documents_dir: PathBuf,
}

impl MemoryStore {
    pub fn open(vectors_db: &str, documents_dir: &str) -> Result<Self, String> {
        let store = Self {
            db_path: PathBuf::from(vectors_db),
            documents_dir: PathBuf::from(documents_dir),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn conn(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path).map_err(|e| e.to_string())
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                doc_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                FOREIGN KEY (doc_id) REFERENCES documents(id) ON DELETE CASCADE
            );
            "#,
        )
        .map_err(|e| e.to_string())
    }

    pub fn import_file(&self, source: &Path, cfg: &MemoryConfig) -> Result<DocumentInfo, String> {
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document")
            .to_string();
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        let text = fs::read_to_string(source).map_err(|e| format!("Olvasás sikertelen: {e}"))?;

        let doc_id = Uuid::new_v4().to_string();
        let dest = self.documents_dir.join(format!("{doc_id}.{ext}"));
        fs::copy(source, &dest).map_err(|e| e.to_string())?;

        let chunks = chunk_text(&text, cfg.chunk_size, cfg.chunk_overlap);
        let conn = self.conn()?;
        let created = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO documents (id, name, path, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![doc_id, name, dest.display().to_string(), created],
        )
        .map_err(|e| e.to_string())?;

        for (i, chunk) in chunks.iter().enumerate() {
            let emb = embed_text(chunk);
            let blob = embedding_to_bytes(&emb);
            let chunk_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO chunks (id, doc_id, chunk_index, text, embedding) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![chunk_id, doc_id, i as i64, chunk, blob],
            )
            .map_err(|e| e.to_string())?;
        }

        Ok(DocumentInfo {
            id: doc_id,
            name,
            chunk_count: chunks.len() as i64,
            created_at: created,
        })
    }

    pub fn list_documents(&self) -> Result<Vec<DocumentInfo>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                r#"SELECT d.id, d.name, d.created_at, COUNT(c.id) as cnt
                   FROM documents d LEFT JOIN chunks c ON c.doc_id = d.id
                   GROUP BY d.id ORDER BY d.created_at DESC"#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok(DocumentInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    chunk_count: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn delete_document(&self, doc_id: &str) -> Result<(), String> {
        let conn = self.conn()?;
        let path: Option<String> = conn
            .query_row(
                "SELECT path FROM documents WHERE id = ?1",
                params![doc_id],
                |row| row.get(0),
            )
            .ok();
        conn.execute("DELETE FROM chunks WHERE doc_id = ?1", params![doc_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
            .map_err(|e| e.to_string())?;
        if let Some(p) = path {
            let _ = fs::remove_file(p);
        }
        Ok(())
    }

    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<String>, String> {
        let q_emb = embed_text(query);
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT text, embedding FROM chunks")
            .map_err(|e| e.to_string())?;

        let mut scored: Vec<(f32, String)> = Vec::new();
        let rows = stmt
            .query_map([], |row| {
                let text: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((text, blob))
            })
            .map_err(|e| e.to_string())?;

        for row in rows.flatten() {
            let emb = bytes_to_embedding(&row.1);
            let score = cosine_similarity(&q_emb, &emb);
            scored.push((score, row.0));
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored
            .into_iter()
            .take(top_k)
            .filter(|(s, _)| *s > 0.01)
            .map(|(_, t)| t)
            .collect())
    }

    pub fn chunk_count(&self) -> Result<i64, String> {
        let conn = self.conn()?;
        conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    }
}
