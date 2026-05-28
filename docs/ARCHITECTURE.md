# LUMI — Rendszerarchitektúra

## Áttekintés

```
┌─────────────────────────────────────────────────────────┐
│  React UI (LUMI) — chat, memória, profil, beállítások │
└──────────────────────────┬──────────────────────────────┘
                           │ Tauri IPC + Events
┌──────────────────────────▼──────────────────────────────┐
│  Rust Core — portable, akasha, memory, profile           │
└──────┬─────────────────────────────┬────────────────────┘
       │ HTTP localhost              │ SQLite
┌──────▼──────┐              ┌──────▼──────────────────┐
│ llama-server │              │ data/memory/vectors.db  │
│  + MoE GGUF  │              │ data/profile/atman.db   │
└─────────────┘              └─────────────────────────┘
```

## Modulok

### `portable`

- Indítási gyökér (`current_exe` szülő mappája)
- `data/` bootstrap: config, chats, memory, profile, logs
- `config.toml` betöltés és mentés

### `akasha` (Fázis 2 — hibrid arzenál)

- `llama-server` **router mód**: `--models-dir models/akasha --models-max 1`
- Három GGUF slot: **Eco**, **Brain**, **Creative** (`arsenal.rs` → `POST /models/load`)
- Belső **prompt router** (`router.rs`): kulcsszó + regex, hardver-fallback Eco-ra
- **HardwareMonitor** + **DynamicThrottle**: RAM/CPU poll, process priority (Windows), soha nem kill/timeout generálás közben
- **EtaEstimator**: `akasha-gen-start` / `akasha-gen-tick` események a UI visszaszámlálóhoz
- OpenAI-kompatibilis streaming (`/v1/chat/completions`) aktív `model` mezővel
- Slot-specifikus system promptok; gamification domain a slotból származik

### `memory` (TOTAL MEMÓRIA)

- Dokumentum import, chunking, embedding
- Vektor tárolás SQLite-ben (BLOB), cosine keresés
- RAG kontextus injektálás a chat promptba

### `profile`

- Felhasználói profil, domain órák, jelvények
- Események: `bugs_fixed`, `messages_sent`, stb.

## Biztonság

- Alapértelmezés: nincs külső hálózati hívás
- Inferencia csak `127.0.0.1`
- Felhasználói adatok a portable `data/` mappában

## Platformok (MVP)

- Windows x64
- macOS (Apple Silicon + Intel universal ajánlott)
