# Builds ATMAN and assembles a portable folder layout under dist/portable/
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Atman = Join-Path $Root "atman"
$Out = Join-Path $Root "dist\portable"

Push-Location $Atman
npm run build
npm run tauri build
Pop-Location

$Bundle = Get-ChildItem (Join-Path $Atman "src-tauri\target\release\bundle") -Recurse -Filter "ATMAN*.exe" | Select-Object -First 1
if (-not $Bundle) {
    Write-Host "ATMAN.exe nem található a bundle kimenetben."
    exit 1
}

Remove-Item $Out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Out, (Join-Path $Out "models\akasha"), (Join-Path $Out "models\embed"), (Join-Path $Out "runtime\win-x64") | Out-Null

Copy-Item $Bundle.FullName (Join-Path $Out "ATMAN.exe")
if (Test-Path (Join-Path $Root "runtime\win-x64\llama-server.exe")) {
    Copy-Item (Join-Path $Root "runtime\win-x64\llama-server.exe") (Join-Path $Out "runtime\win-x64\")
}
Copy-Item (Join-Path $Root "models\akasha\.gitkeep") (Join-Path $Out "models\akasha\") -ErrorAction SilentlyContinue
Copy-Item (Join-Path $Root "docs\PORTABLE.md") (Join-Path $Out "README-PORTABLE.txt")

Write-Host "Portable csomag: $Out"
Write-Host "Helyezd ide az AKASHA MoE GGUF-ot: models\akasha\akasha-moe.Q4_K_M.gguf"
