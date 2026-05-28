# Downloads llama.cpp llama-server + runtime DLLs for Windows x64 into runtime/win-x64/
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$OutDir = Join-Path $Root "runtime\win-x64"
$ExtractDir = Join-Path $env:TEMP "llama-cpp-extract-win"

function Select-WindowsAsset($assets) {
    # Előny: tiszta CPU build (irodai PC, portable) — NEM a cudart-* CUDA runtime csomag
    $preferred = @(
        "llama-.*-bin-win-cpu-x64\.zip$",
        "llama-.*-bin-win-vulkan-x64\.zip$",
        "llama-.*-bin-win-cuda-.*-x64\.zip$"
    )
    foreach ($pattern in $preferred) {
        $match = $assets | Where-Object { $_.name -match $pattern -and $_.name -notmatch "^cudart-" } | Select-Object -First 1
        if ($match) { return $match }
    }
    return $null
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (Test-Path $ExtractDir) { Remove-Item $ExtractDir -Recurse -Force }

Write-Host "llama.cpp legutobbi release lekerese..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest"
$Asset = Select-WindowsAsset $Release.assets

if (-not $Asset) {
    Write-Host "Nem talalhato Windows x64 binaris csomag."
    Write-Host "Manualis letoltes: https://github.com/ggml-org/llama.cpp/releases"
    Write-Host "Keress: llama-*-bin-win-cpu-x64.zip"
    exit 1
}

$ZipPath = Join-Path $env:TEMP "llama-cpp-win.zip"
Write-Host "Letoltes: $($Asset.name)"
Invoke-WebRequest -Uri $Asset.browser_download_url -OutFile $ZipPath
Expand-Archive -Path $ZipPath -DestinationPath $ExtractDir -Force

$Server = Get-ChildItem -Path $ExtractDir -Recurse -Filter "llama-server.exe" | Select-Object -First 1
if (-not $Server) {
    Write-Host "llama-server.exe nem talalhato a csomagban: $($Asset.name)"
    exit 1
}

# Az uj llama.cpp stub + DLL struktura: az egesz kicsomagolt mappat masoljuk
$BinRoot = $Server.Directory.FullName
Write-Host "Masolas: $BinRoot -> $OutDir"
Get-ChildItem -Path $BinRoot -File | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $OutDir $_.Name) -Force
}

$Dest = Join-Path $OutDir "llama-server.exe"
if (-not (Test-Path $Dest)) {
    Write-Host "Masolas sikertelen."
    exit 1
}

Write-Host "Kesz: $Dest"
Write-Host "DLL-ek is a runtime\win-x64\ mappaban - szuksegesek az inditashoz."
