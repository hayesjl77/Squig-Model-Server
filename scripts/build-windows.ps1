# Squig Model Server - Build (Windows)
Write-Host "═══════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  Squig Model Server - Build (Windows)" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════" -ForegroundColor Cyan

$ProjectDir = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $ProjectDir

# ─── Build UI ────────────────────────────────────────
Write-Host ""
Write-Host "→ Building dashboard UI..." -ForegroundColor Green
Set-Location ui

if (-not (Test-Path "node_modules")) {
    Write-Host "  Installing npm dependencies..."
    npm install
}

npm run build
Write-Host "  ✓ UI built to ui\dist\" -ForegroundColor Green

Set-Location $ProjectDir

# ─── Build Rust server ───────────────────────────────
Write-Host ""
Write-Host "→ Building Rust server (release)..." -ForegroundColor Green
cargo build --release

Write-Host ""
Write-Host "✓ Build complete!" -ForegroundColor Green
Write-Host "  Binary: target\release\squig-model-server.exe"
Write-Host ""
Write-Host "To run:"
Write-Host "  .\target\release\squig-model-server.exe"
Write-Host ""
Write-Host "To cross-compile for Linux from Windows (via WSL):"
Write-Host "  wsl bash scripts/build-linux.sh"
