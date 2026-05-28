# Downloads AKASHA Phase 2 arsenal (eco / brain / creative) to models/akasha/
param(
    [switch]$EcoOnly,
    [switch]$SkipEco,
    [string]$BrainRepo = "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
    [string]$BrainFile = "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    [string]$CreativeRepo = "dphn/dolphin-2.9.4-llama3.1-8b-gguf",
    [string]$CreativeFile = "dolphin-2.9.4-llama3.1-8b-Q4_K_M.gguf"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Dir = Join-Path $Root "models\akasha"
New-Item -ItemType Directory -Force -Path $Dir | Out-Null

function Get-HfUrl($Repo, $File) {
    "https://huggingface.co/$Repo/resolve/main/$File"
}

function Download-File($Url, $OutPath) {
    $minBytes = 50MB
    if (Test-Path $OutPath) {
        $size = (Get-Item $OutPath).Length
        if ($size -gt $minBytes) {
            Write-Host "Mar letezik, skip: $OutPath ($([math]::Round($size/1GB, 2)) GB)"
            return
        }
        Write-Host "Serult/kicsi fajl, ujratoltes: $OutPath"
        Remove-Item $OutPath -Force
    }
    Write-Host "Letoltes: $Url"
    Write-Host "Cel: $OutPath"
    $Temp = "$OutPath.part"
    if (Test-Path $Temp) { Remove-Item $Temp -Force }
    curl.exe -L --retry 5 --retry-delay 5 -C - -o $Temp $Url
    if ($LASTEXITCODE -ne 0) {
        throw "Letoltes sikertelen (curl exit $LASTEXITCODE)"
    }
    $dlSize = (Get-Item $Temp).Length
    if ($dlSize -lt $minBytes) {
        Remove-Item $Temp -Force
        throw "Letoltott fajl tul kicsi ($dlSize byte) - valoszinuleg hibas URL vagy HF hiba"
    }
    Move-Item -Force $Temp $OutPath
    Write-Host "Kesz: $OutPath"
}

$EcoOut = Join-Path $Dir "eco.Q4_K_M.gguf"
$Legacy = Join-Path $Dir "akasha-moe.Q4_K_M.gguf"

if (-not $SkipEco) {
    if (Test-Path $EcoOut) {
        Write-Host "Eco modell mar letezik: $EcoOut"
    }
    elseif (Test-Path $Legacy) {
        Write-Host "Eco: masolas legacy fajlbol..."
        Copy-Item -Force $Legacy $EcoOut
        Write-Host "Kesz: $EcoOut"
    }
    else {
        # MEGJEGYZES: a korabbi DeepSeek-distill alapu MoE modell egy reasoning
        # modell volt, amibol a kis (~4B osszparameter) miatt vegtelen
        # gondolkodasi hurkok jottek a casual chat eseten ("DeepSeek10",
        # "fitted with whom?" tipusu kvazi-velemenyek). Lecsereltuk egy modern,
        # tisztan utasitas-koveto kis modellre, ami eppen olyan gyors,
        # multilingual, es nem ragad reasoning ciklusokba.
        $EcoRepo = "Qwen/Qwen2.5-3B-Instruct-GGUF"
        $EcoFile = "qwen2.5-3b-instruct-q4_k_m.gguf"
        Download-File (Get-HfUrl $EcoRepo $EcoFile) $EcoOut
    }
}

if ($EcoOnly) { exit 0 }

$BrainOut = Join-Path $Dir "brain.Q4_K_M.gguf"
$CreativeOut = Join-Path $Dir "creative.Q4_K_M.gguf"

Download-File (Get-HfUrl $BrainRepo $BrainFile) $BrainOut
Download-File (Get-HfUrl $CreativeRepo $CreativeFile) $CreativeOut

Write-Host ""
Write-Host "Arzenal kesz. Fajlok:"
Get-ChildItem $Dir -Filter "*.gguf" | ForEach-Object {
    Write-Host "  $($_.Name) - $([math]::Round($_.Length/1GB, 2)) GB"
}
