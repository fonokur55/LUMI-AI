# AKASHA — Modell-katalógus (v0.1.3+)

Az LUMI motorja, **AKASHA**, **9-cellás tier × slot** modell-mátrixból dolgozik. A **tier** a gép képességeit fogalmazza meg (Light/Standard/Pro a hardware-detektálás alapján), a **slot** pedig a kérdés-típust (Eco/Brain/Creative, amit a router választ a prompt-kulcsszavak alapján).

A felhasználó gépe első indításkor 1 tier-t kap, és annak 3 modellje (Eco + Brain + Creative) töltődik le egyszerre. Más tier modelljei a **Beállítások › Modellek** menüből manuálisan letölthetők.

## A 9 modell

| Tier (Hardware) | Eco (általános beszélgetés) | **Brain (kód, matek)** | Creative (kreatív írás) |
|---|---|---|---|
| **🟢 Light** (3-6 GB RAM, 2-4 mag) | Gemma 2 2B Q4_K_M (~1.6 GB) | **Qwen Coder 1.5B Q4_K_M (~1.0 GB)** | Gemma 2 2B abliterated Q4_K_M (~1.6 GB) |
| **🟡 Standard** (6-12 GB RAM, 4-8 mag) | Gemma 2 9B Q3_K_M (~3.8 GB) | **Qwen Coder 7B Q4_K_M (~4.7 GB)** | Gemma 2 9B abliterated Q4_K_M (~5.5 GB) |
| **🔴 Pro** (12+ GB RAM, 8+ mag) | Gemma 2 9B Q5_K_M (~6.5 GB) | **Qwen Coder 14B Q4_K_M (~9.0 GB)** | Gemma 2 9B abliterated Q5_K_M (~6.5 GB) |

**Tier-csomag összméretek:** Light ~4.2 GB · Standard ~14 GB · Pro ~22 GB.

## Filozófia

- **Eco és Creative — Gemma 2 család** (`google/gemma-2-*`). A Gemma kifejezetten erős magyarul, és a 9B Q4/Q5 közeli GPT-3.5 minőség lokál futtatáskor.
- **Brain — Qwen 2.5 Coder család** (`Qwen/Qwen2.5-Coder-*`). A Brain mindenhol dedikált kód-specifikus fine-tune. A 7B-es benchmarkokon közel áll a GPT-3.5-Turbo-hoz, a 14B-es Pro-tier-en GPT-4 közeli.
- **Creative — abliterated változat** (`mlabonne/*-abliterated` re-uploadok). Az "abliterated" azt jelenti, hogy a fine-tune-ban eltávolították a refusal direction-t — a modell hajlandóbb kreatív/szabad válaszokra (történet, vers, roleplay). **Felhasználói felelősség: helyi, offline használatra.**

## Konkrét HuggingFace források

| Tier × Slot | Repó | Fájl |
|---|---|---|
| Light · Eco | `bartowski/gemma-2-2b-it-GGUF` | `gemma-2-2b-it-Q4_K_M.gguf` |
| Light · Brain | `bartowski/Qwen2.5-Coder-1.5B-Instruct-GGUF` | `Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf` |
| Light · Creative | `bartowski/gemma-2-2b-it-abliterated-GGUF` | `gemma-2-2b-it-abliterated-Q4_K_M.gguf` |
| Standard · Eco | `bartowski/gemma-2-9b-it-GGUF` | `gemma-2-9b-it-Q3_K_M.gguf` |
| Standard · Brain | `Qwen/Qwen2.5-Coder-7B-Instruct-GGUF` (hivatalos) | `qwen2.5-coder-7b-instruct-q4_k_m.gguf` |
| Standard · Creative | `bartowski/gemma-2-9b-it-abliterated-GGUF` | `gemma-2-9b-it-abliterated-Q4_K_M.gguf` |
| Pro · Eco | `bartowski/gemma-2-9b-it-GGUF` | `gemma-2-9b-it-Q5_K_M.gguf` |
| Pro · Brain | `bartowski/Qwen2.5-Coder-14B-Instruct-GGUF` | `Qwen2.5-Coder-14B-Instruct-Q4_K_M.gguf` |
| Pro · Creative | `bartowski/gemma-2-9b-it-abliterated-GGUF` | `gemma-2-9b-it-abliterated-Q5_K_M.gguf` |

A források a `atman/src-tauri/src/downloader/catalog.rs` `CATALOG` konstansában is rögzítve.

## Helyi fájl-elhelyezés

A 9 modell a `models/akasha/` mappában tárolódik, **tier-előtaggal**:

```
models/akasha/
  light-eco.gguf
  light-brain.gguf
  light-creative.gguf
  standard-eco.gguf
  standard-brain.gguf
  standard-creative.gguf
  pro-eco.gguf
  pro-brain.gguf
  pro-creative.gguf
```

A `models/embed/` mappa a TOTAL MEMÓRIA RAG embedding-modelljét tartja külön (`nomic-embed-text` vagy `bge-small`).

## Router (szoftveres slot-választás)

A felhasználó kérdéséből a Rust kód `route_prompt` függvénye kulcsszó-stem-matching alapján választ slot-ot:

- **Brain:** `kód`, `función`, `class`, `import`, `rust`, `python`, `bug`, `matek`, `egyenlet`, `excel`, …
- **Creative:** `mese`, `törté`, `vers`, `írj`, `kreatív`, `fantáz`, `roleplay`, `novella`, …
- **Eco:** minden más (rövid prompt, üdvözlés, fallback)

A user manuálisan is választhat slot-ot a chat-mező slot-dropdown-jából. A nem-telepített slotok disabled-elve jelennek meg.

## Hardware-tier detektálás

A `compute_profile()` függvény ([`perf.rs`](../atman/src-tauri/src/akasha/perf.rs)) automatikusan választja a tier-t:

- **Light** (Limp): 3-6 GB szabad RAM, 2-4 mag
- **Standard:** 6-12 GB RAM, 4-8 mag
- **Pro:** 12+ GB RAM, 8+ mag
- **Blocked:** <3 GB RAM vagy <2 mag vagy nincs AVX2 — LUMI nem indul

A user a **Beállítások › Teljesítmény › Mód kézi felülírása** menüben felülírhatja a detektált tier-t. Ha az új tier modelljei hiányoznak, az LUMI figyelmeztet és a Modellek szekcióba irányít.

## Licencelés

| Modell-család | Licenc |
|---|---|
| Gemma 2 (Google) | Gemma License (kereskedelmileg használható, "fair use" megszorításokkal) |
| Qwen 2.5 / Qwen Coder (Alibaba) | Apache 2.0 |
| Llama-3.x alapú fine-tune-ok | Llama Community License |
| llama.cpp (futtatómotor) | MIT |

**Abliterated modellek** csak helyi, offline használatra ajánlottak — a felhasználó felelőssége.

## Telepítés

A modelleket a **first-run wizard** automatikusan letölti az ajánlott tier-re. Manuális letöltéshez a régi script is működik:

```powershell
.\scripts\fetch-akasha-arsenal.ps1   # legacy 3-slot Qwen+Dolphin
```

(A v0.1.3-tól a frontend `Beállítások › Modellek` UI-ja a preferált letöltési felület.)
