# AKASHA — Hibrid arzenál (Fázis 2)

Az AKASHA **három GGUF slotot** használ (Eco / Brain / Creative). Egyszerre **csak egy modell** van betöltve a RAM-ba (`llama-server --models-max 1`). A választást belső szabályalapú router végzi — nincs modellválasztó a UI-ban.

## Slotok

| Slot | Szerep | Alapértelmezett fájl | Méret (irányadó) |
|------|--------|----------------------|------------------|
| **Eco** | Rutin, rövid kérdések, alacsony RAM | `models/akasha/eco.Q4_K_M.gguf` | ~2.2 GB |
| **Brain** | Kód, matek, logika, üzlet | `models/akasha/brain.Q4_K_M.gguf` | ~5.5–8.5 GB |
| **Creative** | Kreatív/chat, szabad hang | `models/akasha/creative.Q4_K_M.gguf` | ~3.5–4.5 GB |

Összesen a lemezen ~12 GB (mindhárom Q4_K_M).

## Ajánlott források (Hugging Face)

Paraméterezhető a `scripts/fetch-akasha-arsenal.*` scriptekben.

| Slot | Példa repó / modell |
|------|---------------------|
| **Eco** | Meglévő `akasha-moe.Q4_K_M.gguf` → másolás `eco.Q4_K_M.gguf`-ra (automatikus migráció induláskor) |
| **Brain** | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` Q4_K_M; gyengébb gépre: `deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct` GGUF |
| **Creative** | `DavidAU/Llama-3.2-8X3B-MOE` közeli MoE GGUF, vagy `cognitivecomputations/dolphin-*` Q4_K_M |

## Hardver táblázat

| Slot (Q4_K_M) | RAM (min. futáshoz) | Megjegyzés |
|---------------|---------------------|------------|
| Eco | 6–8 GB | Első válaszhoz előtöltve induláskor |
| Brain | 16 GB+ ajánlott | 8 GB gépen a router Eco-ra esik vissza |
| Creative | 10 GB+ | MoE / nagyobb modelleknél több RAM |

A **throttle** csökkenti a process prioritást és a következő betöltés szálait — **nem** szakítja meg a futó generálást.

## Fájlelhelyezés

```
models/akasha/
  eco.Q4_K_M.gguf
  brain.Q4_K_M.gguf
  creative.Q4_K_M.gguf
```

Legacy: `akasha-moe.Q4_K_M.gguf` → első indításkor másolódik `eco.Q4_K_M.gguf`-ra, ha az még nem létezik.

Config (`data/config.toml`):

```toml
[akasha]
models_dir = "models/akasha"
models_max = 1

[akasha.arsenal]
eco = "eco.Q4_K_M.gguf"
brain = "brain.Q4_K_M.gguf"
creative = "creative.Q4_K_M.gguf"

[akasha.throttle]
ram_warning_mb = 2048
ram_critical_mb = 1024
cpu_critical_percent = 90
min_threads = 2
poll_interval_ms = 500
```

## Router (szoftveres)

- **Brain:** kód, SQL, hiba, matek, API kulcsszavak (HU/EN)
- **Creative:** narratíva, kreatív kérés, hosszú személyes hang
- **Eco:** rövid prompt, üdvözlés, fallback
- **Hardver:** kritikus RAM/CPU → Eco (kivéve egyértelmű Brain prompt + elég RAM)

## Embedding modell (TOTAL MEMÓRIA)

```
models/embed/embed.Q4_K_M.gguf
```

Ajánlott: `nomic-embed-text` vagy `bge-small` GGUF. Ha nincs embed modell, egyszerű szöveg-hasonlóság fallback.

## llama-server verzió

Router mód (`--models-dir`, `POST /models/load`) legalább **b9285+** release. Telepítés:

```powershell
.\scripts\fetch-llama.ps1
```

Arzenál letöltés:

```powershell
.\scripts\fetch-akasha-arsenal.ps1
```

## Felelősség

Uncensored / abliterated modellek csak **helyi, offline** használatra — a felhasználó felelőssége.
