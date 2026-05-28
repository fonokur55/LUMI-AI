# ATMAN Desktop App

Tauri 2 + React 19 alkalmazás. A teljes projekt dokumentációja a gyökér [README.md](../README.md).

## Fejlesztés

```bash
npm install
npm run tauri dev
```

A `data/` mappa a debug build mellett jön létre (`src-tauri/target/debug/data` helyett a launch root mellett — az exe `target/debug/` alatt fut, ezért dev-ben a portable root a debug mappa).

## Modell és runtime

1. `../scripts/fetch-llama.ps1` (Windows) vagy `fetch-llama.sh` (macOS)
2. MoE GGUF → `../models/akasha/akasha-moe.Q4_K_M.gguf`
