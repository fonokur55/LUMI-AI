# LUMI Linux telepítési útmutató

A LUMI Linuxon **háromféle** csomagolásban kapható:
- `.deb` — Debian / Ubuntu / Pop!_OS / Mint
- `.rpm` — Fedora / openSUSE / RHEL
- `.AppImage` — bármilyen modern Linux distro (futtatható egy fájl)

A legtöbb felhasználónak az **AppImage** a legegyszerűbb. A `.deb` és `.rpm` rendszerintegrációt ad (Start menü, frissítés a csomagkezelőből), de néha extra dependency-k kellenek hozzá.

---

## A) AppImage — ajánlott

```bash
# 1. Töltsd le: LUMI_0.1.1_amd64.AppImage
# 2. Tedd futtathatóvá:
chmod +x LUMI_0.1.1_amd64.AppImage

# 3. Indítsd:
./LUMI_0.1.1_amd64.AppImage
```

**Megjegyzés:** Egyes (régebbi Debian-alapú) disztrókon a `libfuse2` dependency kell az AppImage-runtime-hoz:

```bash
sudo apt install libfuse2
```

Ha nem akarsz duplaklikkkel indítani, csinálj asztali parancsikont:

```bash
mkdir -p ~/.local/bin
mv LUMI_0.1.1_amd64.AppImage ~/.local/bin/lumi
chmod +x ~/.local/bin/lumi
# Most már a `lumi` parancs bárhonnan indítja
```

---

## B) `.deb` (Debian / Ubuntu)

```bash
# 1. Töltsd le: lumi_0.1.1_amd64.deb
# 2. Telepítsd:
sudo dpkg -i lumi_0.1.1_amd64.deb

# 3. Ha hiányzó dependency-k:
sudo apt --fix-broken install

# 4. Indítás:
lumi
# vagy az alkalmazás-menüből
```

**Szükséges dependency-k** (alapból az új Ubuntu-kon NINCSENEK telepítve):

```bash
sudo apt install libwebkit2gtk-4.1-0 libappindicator3-1
```

---

## C) `.rpm` (Fedora / openSUSE / RHEL)

```bash
# 1. Töltsd le: lumi-0.1.1-1.x86_64.rpm
# 2. Telepítsd:
sudo dnf install ./lumi-0.1.1-1.x86_64.rpm

# 3. Indítás:
lumi
# vagy az alkalmazás-menüből
```

---

## Hibakeresés

### „dpkg: error processing package lumi (--install)"
Hiányzó dependency. Futtasd: `sudo apt --fix-broken install`.

### Az AppImage nem nyílik meg dupla-kattintásra
- Tedd futtathatóvá: `chmod +x LUMI_*.AppImage`
- Telepítsd a `libfuse2`-t a fenti módon

### „Nem található webkit2gtk runtime"
Telepítsd a megfelelő csomagot:

```bash
# Ubuntu / Debian
sudo apt install libwebkit2gtk-4.1-0

# Fedora
sudo dnf install webkit2gtk4.1
```

### Adatok helye
A `data/` mappa az `~/.local/share/com.nomad.atman/data/` alatt jön létre (telepített verziónál), vagy az AppImage MELLETT (portable módban). Memória, beszélgetések és profil itt élnek — frissítéskor érintetlen marad.
