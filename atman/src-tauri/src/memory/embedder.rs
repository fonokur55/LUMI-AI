use std::collections::HashMap;

const DIM: usize = 256;

/// Egyszerű helyi embedding (hash-alapú) - működik embed modell nélkül is.
pub fn embed_text(text: &str) -> Vec<f32> {
    let mut vec = vec![0f32; DIM];
    for token in tokenize(text) {
        let h = fnv_hash(&token) % DIM;
        vec[h] += 1.0;
    }
    normalize(&mut vec);
    vec
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0f32;
    let mut na = 0f32;
    let mut nb = 0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 2)
        .map(|s| s.to_string())
        .collect()
}

fn fnv_hash(s: &str) -> usize {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h as usize
}

fn normalize(v: &mut [f32]) {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

pub fn embedding_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[allow(dead_code)]
pub fn term_overlap_score(query: &str, doc: &str) -> f32 {
    let q: HashMap<_, _> = tokenize(query).into_iter().map(|t| (t, 1)).collect();
    let d = tokenize(doc);
    if d.is_empty() {
        return 0.0;
    }
    let hits = d.iter().filter(|t| q.contains_key(*t)).count();
    hits as f32 / d.len() as f32
}
