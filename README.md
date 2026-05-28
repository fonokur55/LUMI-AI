# TOTAL AI — LUMI

**NOMAD** anyavállalat · **LUMI** hordozható személyi intelligencia · **AKASHA** lokális MoE motor · **TOTAL MEMÓRIA** helyi RAG

A LUMI egy előfizetés nélküli, teljesen privát AI-alkalmazás, amely USB-ről vagy NVMe-ről futtatható, internet és felhő nélkül. A rendszer szíve az **AKASHA**: három GGUF slot (Eco / Brain / Creative), belső routerrel és `llama-server` router módban (`--models-max 1` — egyszerre egy modell a RAM-ban).

> ℹ️ **Megjegyzés a kódbeli neveken:** a forráskódban néhol még az „atman"/„AtmanConfig" elnevezés szerepel (Rust crate, struct, Tauri bundle ID, SQLite fájlnevek). Ez szándékos — ezek a build- és telepítési pipeline-ban azonosítók, az átírásuk az updater és a meglévő telepítések kompatibilitását törné. Csak a **felhasználói arc** (ablakcím, system prompt, márkázás) változott LUMI-ra.

## Gyors indítás (fejlesztő)

### Előfeltételek

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/)
- Windows 10+ vagy macOS 12+

### 1. llama-server bináris

```powershell
# Windows
.\scripts\fetch-llama.ps1
```

```bash
# macOS
chmod +x scripts/fetch-llama.sh && ./scripts/fetch-llama.sh
```

### 2. AKASHA arzenál (3× GGUF)

```powershell
.\scripts\fetch-akasha-arsenal.ps1
```

```bash
chmod +x scripts/fetch-akasha-arsenal.sh && ./scripts/fetch-akasha-arsenal.sh
```

Cél mappa (~12 GB összesen lemezre):

```
models/akasha/
  eco.Q4_K_M.gguf
  brain.Q4_K_M.gguf
  creative.Q4_K_M.gguf
```

Ha már van `akasha-moe.Q4_K_M.gguf`, az első indításkor `eco.Q4_K_M.gguf`-ra másolódik. Részletek: [docs/MODELS.md](docs/MODELS.md)

### 3. Alkalmazás

```bash
cd atman
npm install
npm run tauri dev
```

## Portable elrendezés

Első indításkor az alkalmazás a futtatható fájl mellett létrehozza a `data/` mappát (config, memória, profil). Részletek: [docs/PORTABLE.md](docs/PORTABLE.md)

## Dokumentáció

| Dokumentum | Tartalom |
|------------|----------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Rendszerarchitektúra |
| [docs/BRAND.md](docs/BRAND.md) | NOMAD / ATMAN brand |
| [docs/PORTABLE.md](docs/PORTABLE.md) | USB / NVMe telepítés |
| [docs/MODELS.md](docs/MODELS.md) | Támogatott MoE modellek |

## Licenc

Proprietary — NOMAD © 2026
