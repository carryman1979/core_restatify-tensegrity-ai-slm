<#
.SYNOPSIS
    Richtet die lokale Tensegrity-Inferenzlaufzeit ein (ADR-0012).

.DESCRIPTION
    Laedt zwei Artefakte in %LOCALAPPDATA%\Tensegrity:
      - llama-cli (llama.cpp Release, win-cpu-x64) nach bin\
      - Qwen2.5-0.5B-Instruct GGUF (Q4_K_M) nach models\  (Apache 2.0)
    Das Modell wird per SHA-256 gegen den gepinnten Hash geprueft.
    Keines der Artefakte landet im Git-Repository.

.PARAMETER LlamaTag
    llama.cpp Release-Tag (Standard: b11734, Stand 2026-10).

.PARAMETER ModelSha256
    Erwarteter SHA-256 des Modells. Leer = nur warnen, nicht verifizieren.

.EXAMPLE
    pwsh scripts/fetch-local-runtime.ps1
#>
[CmdletBinding()]
param(
    [string]$LlamaTag = "b11734",
    [string]$ModelSha256 = "6EB923E7D26E9CEA28811E1A8E852009B21242FB157B26149D3B188F3A8C8653"
)

$ErrorActionPreference = 'Stop'

$root = Join-Path $env:LOCALAPPDATA 'Tensegrity'
$binDir = Join-Path $root 'bin'
$modelDir = Join-Path $root 'models'
$modelPath = Join-Path $modelDir 'qwen2.5-0.5b-instruct-q4_k_m.gguf'
$modelUrl = 'https://huggingface.co/bartowski/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf'

New-Item -ItemType Directory -Force -Path $binDir, $modelDir | Out-Null

# --- llama-cli -------------------------------------------------------------
$llamaCli = Join-Path $binDir 'llama-cli.exe'
if (Test-Path $llamaCli) {
    Write-Host "llama-cli bereits vorhanden: $llamaCli"
} else {
    $assetName = "llama-$LlamaTag-bin-win-cpu-x64.zip"
    $url = "https://github.com/ggml-org/llama.cpp/releases/download/$LlamaTag/$assetName"
    $zip = Join-Path $env:TEMP $assetName
    Write-Host "Lade $assetName ..."
    curl.exe -L -s -o $zip $url
    Expand-Archive -Path $zip -DestinationPath $binDir -Force
    Remove-Item $zip -Force
    Write-Host "llama-cli installiert nach $binDir"
}

# --- Modell ----------------------------------------------------------------
if (Test-Path $modelPath) {
    Write-Host "Modell bereits vorhanden: $modelPath"
} else {
    Write-Host "Lade Modell (~400 MB) ..."
    curl.exe -L -o $modelPath $modelUrl
}

if ($ModelSha256) {
    $actual = (Get-FileHash $modelPath -Algorithm SHA256).Hash
    if ($actual -ne $ModelSha256) {
        Remove-Item $modelPath -Force
        throw "Modell-Hash stimmt nicht! Erwartet $ModelSha256, erhalten $actual – Datei verworfen."
    }
    Write-Host "Modell-Hash verifiziert."
} else {
    Write-Warning "Kein Hash angegeben – Modell unverifiziert."
}

Write-Host ""
Write-Host "Fertig. Die App findet die Laufzeit automatisch (Standardpfade)."
Write-Host "Alternativ: TENSEGRITY_LLAMA_CLI / TENSEGRITY_MODEL_PATH setzen."
