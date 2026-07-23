# Setup — first time only

## 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# follow prompts (default install)
source ~/.cargo/env

## 2. Install Tauri CLI system deps (already have Node + npm)
npm install   # installs @tauri-apps/cli locally

## 3. Add macOS targets (for universal binary)
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin

## 4. Dev run (hot-reload frontend)
npm run dev

## 5. Release build (universal .app)
npm run build -- --target universal-apple-darwin
# Output: src-tauri/target/universal-apple-darwin/release/bundle/macos/Universal DualSense Haptics.app

## Dev bypass (skip license check)
DUALSENSE_DEV=1 npm run dev

## Notes
- First `cargo build` takes ~5 min (compiling all deps from scratch)
- Subsequent builds are incremental, much faster
- The .app bundle is ~10-15 MB (no bundled Node runtime)
- Gatekeeper: same xattr workaround as before until you set up code signing
