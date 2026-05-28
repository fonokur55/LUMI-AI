# LUMI release-folyamat

A LUMI Tauri-alapú auto-update-tel megy ki. Ez a dokumentum lépésről lépésre leírja, mit kell tenned **az első release előtt** (egyszer), és **minden új verziónál** (rutinszerűen).

---

## Egyszeri beállítás (csak az első release előtt)

### 1. Signing kulcs generálása

A Tauri-updater minden frissítést **digitálisan aláír**. A privát kulcsoddal aláírod, a publikus kulcsot pedig a `tauri.conf.json`-be ágyazzuk. A LUMI a beágyazott publikus kulccsal verifikálja a letöltött csomagot — enélkül nem fogadja el. Ez kötelező biztonsági lépés.

```powershell
cd atman
npm run tauri signer generate -- -w $HOME/.lumi/lumi.key
```

Ez generál:
- **`$HOME/.lumi/lumi.key`** — privát kulcs (KRITIKUS, soha ne pusholjd!)
- **`$HOME/.lumi/lumi.key.pub`** — publikus kulcs

A parancs kiírja a publikus kulcsot a konzolra is. Add hozzá a `tauri.conf.json`-höz:

```json
"plugins": {
  "updater": {
    "endpoints": ["https://github.com/fonokur55/LUMI-AI/releases/latest/download/latest.json"],
    "pubkey": "ITT_LEGYEN_A_PUBLIKUS_KULCS",
    ...
  }
}
```

Commit + push:
```powershell
git add atman/src-tauri/tauri.conf.json
git commit -m "chore: pubkey for updater"
git push
```

### 2. GitHub Secrets feltöltése

A GitHub Actions workflow-nak (`.github/workflows/release.yml`) szüksége van a privát kulcsodra a build-folyamatban való aláíráshoz. Két secret kell:

A `fonokur55/LUMI-AI` repón menj a **Settings → Secrets and variables → Actions → New repository secret**. Hozz létre kettőt:

| Secret név                                | Érték                                                                  |
|-------------------------------------------|------------------------------------------------------------------------|
| `TAURI_SIGNING_PRIVATE_KEY`               | A `$HOME/.lumi/lumi.key` fájl **teljes tartalma** (`cat`-eld ki belőle) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`      | A kulcs jelszava, amit a `signer generate` futtatásakor adtál meg     |

Ha a `signer generate` parancsnál **üres jelszót** adtál meg (default), akkor a password secret-et is üresen hagyhatod, vagy üres stringgel töltsd.

> **FONTOS:** A privát kulcs elvesztése azt jelenti, hogy **soha többé nem tudsz update-et kiadni a meglévő telepítéseknek** (mert új kulcsból jövő aláírást nem fogadnak el). Mentsd el biztonságos helyre (password manager, USB-stick, két helyen).

### 3. Backup tipp

```powershell
# Csomagold össze a kulcsfájlt, és tedd egy biztonságos helyre
Compress-Archive -Path $HOME/.lumi -DestinationPath $HOME/lumi-key-backup.zip
```

---

## Új release kiadása (rutinszerű)

Minden release egy git tag-pusholás. A GitHub Actions automatikusan build-el, aláír és felteszi a release-t.

### 1. Verzió frissítése a kódban

Három helyen kell összhangban lennie:

```powershell
# atman/package.json
"version": "0.2.0",

# atman/src-tauri/Cargo.toml
version = "0.2.0"

# atman/src-tauri/tauri.conf.json
"version": "0.2.0",
```

> Tipp: csinálj `npm version 0.2.0`-t a `package.json`-höz, és kézzel frissítsd a többit.

### 2. Commit + tag + push

```powershell
git add -A
git commit -m "release: v0.2.0"
git tag v0.2.0
git push
git push --tags
```

### 3. A workflow lefut

A `tag v*` triggerre a `.github/workflows/release.yml` automatikusan elindul. A folyamatot a GitHub repón a **Actions** fülön követheted. ~15-25 percig tart Windows-on. Amit csinál:

1. Cargo + npm cache betöltése
2. `npm run tauri build -- --bundles nsis,updater`
3. **Aláírja** az NSIS csomagot a `TAURI_SIGNING_PRIVATE_KEY` secrettel → `.nsis.zip` + `.nsis.zip.sig`
4. **Portable ZIP** csomagolása az `atman.exe`-vel
5. **`latest.json`** generálása (verzió, URL, aláírás)
6. Mindent felad a `v0.2.0` release-re a GitHub-on

### 4. Mit kapnak a felhasználók

- **Régi telepítések** legközelebbi indításkor ~12 mp-cel azután csendben lekérdezik a `latest.json`-t. Ha újabb verzió van mint az övék, **jobb-alsó téglalap** jön elő "Új frissítés elérhető" szöveggel.
- A user **„Letöltés"**-re nyom → letölti az aláírt NSIS csomagot, ellenőrzi az aláírást a beágyazott publikus kulccsal, telepíti, és újraindítja az appot.
- A user **„Letöltés később"**-re nyom → a banner eltűnik, és session-ig nem zaklatja újra erre a verzióra.
- A **`data/` mappa** változatlan marad (config, memória, beszélgetések, profil). Az új verzió a meglévő DB-ket migrálja a `init_schema` `ALTER TABLE` parancsokkal.

### 5. Portable user-eknek

A portable verziónak NINCS auto-update — a jelenleg integrált flow csak telepített verziónál működik. A portable user-ek:
- Manuálisan letölthetik az `LUMI-x.y.z-windows-x64-portable.zip`-et a release-oldalról
- Kibontják, lecserélik a régi `atman.exe`-t
- A `data/` az exe MELLETT van, változatlan marad

Ezt a portable-flow-t később lehet teljes-automatára cserélni (lásd a kód-kommentek a `UpdateBanner.tsx`-ben + a `src-tauri/src/lib.rs`-ben).

### 6. USB Installer

Minden release-hez a workflow elkészít egy `LUMI-USB-Installer-x.y.z.exe` fájlt is, ami egy önálló mini-app:
- A user letölti és lefuttatja
- Listázza a csatlakoztatott pendrive-okat és külső meghajtókat
- A user kiválaszt egyet → "Telepítés" gomb → a beágyazott portable LUMI ZIP kibontva települ a `<DRIVE>:\LUMI` mappába
- Sikeres telepítés után megnyitható az Intézőben vagy közvetlenül elindítható

A beágyazott ZIP-et a workflow `Embed portable ZIP into USB Installer` lépése másolja a `lumi-usb-installer/src-tauri/embedded/lumi-portable.zip` helyre **közvetlenül a build előtt** — így a USB Installer mindig az aktuális release-hez tartozó LUMI-t tartalmazza.

**Helyi USB Installer build:**
```powershell
# 1. Először buildeld az atman portable ZIP-jét (vagy másolj kézzel egyet)
cd atman
npm run tauri build -- --bundles nsis
# Csomagold a target/release/atman.exe-t ZIP-be...

# 2. Másold a ZIP-et a beágyazási helyre
Copy-Item "release-staging/LUMI-x.y.z-portable.zip" `
  "lumi-usb-installer/src-tauri/embedded/lumi-portable.zip"

# 3. Build a USB Installert
cd ../lumi-usb-installer
npm install
npm run tauri build -- --bundles nsis
# Output: src-tauri/target/release/bundle/nsis/LUMI USB Telepítő_x.y.z_x64-setup.exe
```

---

## Hibakeresés

### „signature: invalid" hiba a userek gépén

- A `tauri.conf.json` `pubkey` nem stimmel a build idejében használt privát kulccsal. Mindig **ugyanazt a privát kulcsot** használd minden release-hez.

### „network error" / 404 a letöltésnél

- A `latest.json` URL-je nem mutat helyes manifestre. Ellenőrizd a `tauri.conf.json` `endpoints` mezőjét; a release-eknek **publikus** repón kell lennie (vagy CDN-en).

### Az auto-updater nem jelenik meg

- Az új verziónak **magasabb semver-számúnak** kell lennie mint a felhasználó jelenlegi verziójának. `0.1.0` → `0.2.0` OK, `0.1.0` → `0.1.0` NEM.
- Nézd meg a Console-t F12-vel; a `UpdateBanner` `console.warn`-t logol ha a check() sikertelen.

### A build elhasal a CI-n „signing key not set" hibával

- A `TAURI_SIGNING_PRIVATE_KEY` secret nincs feltöltve a GitHub repón, vagy üres. Lásd az „Egyszeri beállítás" 2. lépését.
