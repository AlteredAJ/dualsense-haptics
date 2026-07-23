# build-windows.ps1 — one-shot Windows build for DualSense Haptics.
#
# Prereqs (install once, see docs/WINDOWS.md):
#   - Rust (stable, MSVC toolchain)         https://rustup.rs
#   - Node.js LTS                           https://nodejs.org
#   - WebView2 runtime                      (ships with Win 10/11)
#   - ViGEmBus driver                       https://github.com/nefarius/ViGEmBus/releases
#   - HidHide driver                        https://github.com/nefarius/HidHide/releases
#
# Run from the project root in PowerShell:
#   ./build-windows.ps1
#
# Output installer lands in src-tauri/target/release/bundle/.

$ErrorActionPreference = "Stop"

Write-Host "==> Checking toolchain..." -ForegroundColor Cyan
foreach ($cmd in @("cargo", "npm")) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "$cmd not found on PATH. See docs/WINDOWS.md for setup."
    }
}

Write-Host "==> Installing npm deps..." -ForegroundColor Cyan
npm install

Write-Host "==> Building release bundle (this compiles vigem-client + hidhide)..." -ForegroundColor Cyan
npm run tauri build

Write-Host ""
Write-Host "Done. Installer is in src-tauri\target\release\bundle\." -ForegroundColor Green
Write-Host "After install, launch the app, pick Output -> Xbox, then start Forza." -ForegroundColor Green
Write-Host "HidHide cloaking is automated; the app must run elevated (admin) for it to take effect." -ForegroundColor Yellow
