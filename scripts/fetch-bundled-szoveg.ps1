# =============================================================================
#  v0.2.0 bundled Szöveg expert letöltő (Gemma 2 2B-it Q4_K_M)
# =============================================================================
#  RELEASE BUILD ELŐTT FUTTATNI KELL — különben a telepítő nem tartalmazza
#  a Gemma modellt, és az első indításnál csak letöltési-flow lesz.
#
#  HASZNÁLAT:
#    .\scripts\fetch-bundled-szoveg.ps1
#
#  EREDMÉNY:
#    atman\src-tauri\resources\szoveg.gguf (~1.6 GB)
#
#  Ezt aztán a `tauri.conf.json` `bundle.resources` mezője csomagolja be a
#  telepítőbe. A user telepítés után azonnal beszélgethet a Szöveg expert-tel,
#  letöltési várakozás nélkül.
#
#  CI-ben: a release.yml workflow Windows + macOS + Linux runner-eken
#  ezt a scriptet (vagy a .sh párját) futtatja a `tauri build` előtt.
# =============================================================================

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Resources = Join-Path $RepoRoot 'atman\src-tauri\resources'
$Target = Join-Path $Resources 'szoveg.gguf'

# Forrás: bartowski Gemma 2 2B-it GGUF, Q4_K_M kvantálás (~1.6 GB)
$Url = 'https://huggingface.co/bartowski/gemma-2-2b-it-GGUF/resolve/main/gemma-2-2b-it-Q4_K_M.gguf'

if (Test-Path $Target) {
    $SizeMb = [math]::Round((Get-Item $Target).Length / 1MB, 1)
    if ($SizeMb -gt 1000) {
        Write-Host "OK: szoveg.gguf már létezik ($SizeMb MB), kihagyom a letöltést." -ForegroundColor Green
        Write-Host "    Töröld le ($Target), ha újra le akarod tölteni." -ForegroundColor DarkGray
        exit 0
    } else {
        Write-Host "FIGYELEM: szoveg.gguf túl kicsi ($SizeMb MB), valószínűleg sérült. Újraletöltés..." -ForegroundColor Yellow
        Remove-Item $Target -Force
    }
}

if (-not (Test-Path $Resources)) {
    New-Item -ItemType Directory -Path $Resources | Out-Null
}

Write-Host "Letöltés: $Url" -ForegroundColor Cyan
Write-Host "Cél:      $Target" -ForegroundColor Cyan
Write-Host "Méret:    ~1.6 GB (Gemma 2 2B-it Q4_K_M)" -ForegroundColor DarkGray
Write-Host ""

$ProgressPreference = 'Continue'
try {
    # Köztes .part fájl, csak siker esetén nevezzük át
    $Tmp = "$Target.part"
    Invoke-WebRequest -Uri $Url -OutFile $Tmp -UseBasicParsing

    $SizeMb = [math]::Round((Get-Item $Tmp).Length / 1MB, 1)
    if ($SizeMb -lt 1000) {
        Remove-Item $Tmp -Force
        throw "Letöltött fájl túl kicsi ($SizeMb MB) — valószínűleg sérült."
    }

    Move-Item -Path $Tmp -Destination $Target -Force
    Write-Host ""
    Write-Host "OK: szoveg.gguf letöltve ($SizeMb MB)." -ForegroundColor Green
    Write-Host "    Most már futtathatod: cd atman; npm run tauri build" -ForegroundColor DarkGray
} catch {
    Write-Host "HIBA: $_" -ForegroundColor Red
    exit 1
}
