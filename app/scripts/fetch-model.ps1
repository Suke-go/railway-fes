# Windows equivalent of fetch-model.sh.
# Usage: pwsh scripts/fetch-model.ps1            # downloads small
#        $env:MODEL='medium'; pwsh scripts/fetch-model.ps1

$ErrorActionPreference = 'Stop'
$Model   = if ($env:MODEL) { $env:MODEL } else { 'small' }
$DestDir = Join-Path $PSScriptRoot '..\src-tauri\resources\models'
$DestFile = Join-Path $DestDir "ggml-$Model.bin"
$Url     = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-$Model.bin"

New-Item -ItemType Directory -Force $DestDir | Out-Null

if (Test-Path $DestFile) {
  Write-Host "model already present: $DestFile"
  exit 0
}

Write-Host "downloading $Model model to $DestFile"
Invoke-WebRequest -Uri $Url -OutFile $DestFile
Write-Host "done."
