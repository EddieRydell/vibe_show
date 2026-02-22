# Download the uv binary for Windows.
# Usage: .\scripts\download-uv.ps1

$ErrorActionPreference = "Stop"

$UV_VERSION = "0.6.6"
$DEST_DIR = "src-tauri\resources"

if (-not (Test-Path $DEST_DIR)) {
    New-Item -ItemType Directory -Path $DEST_DIR | Out-Null
}

# Determine architecture
$arch = if ([Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
} else {
    Write-Error "32-bit systems are not supported"
    exit 1
}

$archive = "uv-${arch}-pc-windows-msvc.zip"
$url = "https://github.com/astral-sh/uv/releases/download/${UV_VERSION}/${archive}"
$tempDir = Join-Path $env:TEMP "uv-download-$(Get-Random)"
$zipPath = Join-Path $tempDir $archive

New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

Write-Host "Downloading uv ${UV_VERSION} for windows/${arch}..."
Write-Host "  URL: ${url}"

Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

Write-Host "Extracting..."
Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

# Find uv.exe in extracted contents
$uvBin = Get-ChildItem -Path $tempDir -Filter "uv.exe" -Recurse | Select-Object -First 1

if (-not $uvBin) {
    Remove-Item -Recurse -Force $tempDir
    Write-Error "Could not find uv.exe in archive"
    exit 1
}

Copy-Item $uvBin.FullName -Destination (Join-Path $DEST_DIR "uv.exe") -Force
Remove-Item -Recurse -Force $tempDir

$finalPath = Join-Path $DEST_DIR "uv.exe"
Write-Host "Done! uv binary at ${finalPath}"
& $finalPath --version
