# LUMI — Portable telepítés (USB / NVMe)

## Mappa-struktúra

```
{LAUNCH_ROOT}/
├── LUMI.exe              # Windows
├── LUMI.app/             # macOS
├── runtime/
│   ├── win-x64/llama-server.exe
│   └── macos/llama-server
├── models/
│   ├── akasha/akasha-moe.Q4_K_M.gguf
│   └── embed/embed.Q4_K_M.gguf
└── data/                  # első indításkor jön létre
    ├── config.toml
    ├── chats/
    ├── memory/
    │   ├── documents/
    │   └── vectors.db
    ├── profile/atman.db
    └── logs/
```

## Első indítás

1. Csatlakoztasd a hordozható meghajtót (USB vagy Hard Edition NVMe).
2. Indítsd az LUMI futtathatót a gyökérből — **ne** másold Program Files-ba.
3. Az alkalmazás létrehozza a `data/` mappát és az alapértelmezett `config.toml`-t.
4. Helyezd el az AKASHA MoE modellt a `models/akasha/` alá (vagy állítsd be az útvonalat a Beállításokban).

## Windows

- A `data/` és `models/` ugyanazon a meghajtón legyen, ahol az exe — USB-n lassabb I/O, NVMe ajánlott nagy modellekhez.
- Windows Defender néha blokkolja az ismeretlen `llama-server.exe`-t — adj kivételt a portable mappára.

## macOS

Első futtatás előtt (terminál, a portable mappa gyökerében):

```bash
xattr -cr .
chmod +x runtime/macos/llama-server
```

Gatekeeper: jobb klikk → Megnyitás az első indításkor, ha a rendszer blokkolja.

## config.toml

```toml
[akasha]
model_path = "models/akasha/akasha-moe.Q4_K_M.gguf"
n_threads = 0          # 0 = auto (CPU magok)
n_ctx = 4096
host = "127.0.0.1"
port = 0             # 0 = dinamikus port

[memory]
embed_model_path = "models/embed/embed.Q4_K_M.gguf"
chunk_size = 512
chunk_overlap = 64
top_k = 5

[profile]
display_name = "felhasználó"
```

## Hard Edition (jövő)

A limitált NVMe kiadás ugyanezt a struktúrát tartalmazza előre feltöltve; az arany gomba fizikai formát a tokban örökíti meg a brandet.
