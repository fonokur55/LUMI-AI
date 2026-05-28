use super::store::MemoryStore;
use crate::portable::config::MemoryConfig;

pub fn build_rag_context(store: &MemoryStore, query: &str, cfg: &MemoryConfig) -> Option<String> {
    let hits = store.search(query, cfg.top_k).ok()?;
    if hits.is_empty() {
        return None;
    }
    let mut ctx = String::from(
        "\n\n[RELEVÁNS MEMÓRIA - a felhasználó saját, lokálisan tárolt \
         dokumentumaiból. Akkor használd ezeket, ha tényleg illeszkednek \
         a kérdéshez.]\n",
    );
    for (i, h) in hits.iter().enumerate() {
        ctx.push_str(&format!("[{}] {}\n", i + 1, h));
    }
    Some(ctx)
}
