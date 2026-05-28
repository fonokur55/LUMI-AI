# Downloads AKASHA MoE GGUF (Q4_K_M) to models/akasha/akasha-moe.Q4_K_M.gguf
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$OutFile = Join-Path $Root "models\akasha\akasha-moe.Q4_K_M.gguf"
New-Item -ItemType Directory -Force -Path (Split-Path $OutFile) | Out-Null

if (Test-Path $OutFile) {
    $size = (Get-Item $OutFile).Length
    if ($size -gt 1GB) {
        Write-Host "Modell mar letezik: $OutFile ($([math]::Round($size/1GB, 2)) GB)"
        exit 0
    }
}

$Repo = "matrixportalx/Qwen2.5-MOE-2X1.5B-DeepSeek-Uncensored-Censored-4B-GGUF"
$File = "qwen2.5-moe-2x1.5b-deepseek-uncensored-censored-4b-q4_k_m.gguf"
$Url = "https://huggingface.co/$Repo/resolve/main/$File"

Write-Host "AKASHA MoE modell letoltese (~2.5 GB)..."
Write-Host "Forras: $Repo"
Write-Host "Cel: $OutFile"

$Temp = "$OutFile.part"
if (Test-Path $Temp) { Remove-Item $Temp -Force }

# curl: resume tamogatas
curl.exe -L --retry 5 --retry-delay 5 -C - -o $Temp $Url
if ($LASTEXITCODE -ne 0) {
    Write-Host "Letoltes sikertelen (curl exit $LASTEXITCODE)."
    exit 1
}

Move-Item -Force $Temp $OutFile
Write-Host "Kesz: $OutFile ($([math]::Round((Get-Item $OutFile).Length/1GB, 2)) GB)"
