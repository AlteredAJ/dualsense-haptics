# Windows Port + Xbox-Translated Output

This app runs on Windows via Tauri 2 (WebView2). On top of native DualSense haptics it
adds an **Xbox output mode**: a virtual Xbox 360 (XInput) gamepad that the DualSense's
inputs are forwarded into, so XInput-only games (Forza Horizon, etc.) detect the
controller. Haptics keep driving the real DualSense in both modes, so adaptive triggers
and rumble still work in Forza.

## One-time driver setup

The Xbox mode needs two kernel drivers (the same stack DS4Windows / DualSenseX use):

1. **ViGEmBus** — creates the virtual Xbox pad.
   https://github.com/nefarius/ViGEmBus/releases  → install the latest `.exe`.
2. **HidHide** — cloaks the real DualSense so the game sees ONLY the virtual Xbox pad
   (otherwise Forza gets double input).
   https://github.com/nefarius/HidHide/releases  → install, then open **HidHide
   Configuration Client**:
   - **Applications** tab → add this app's `dualsense-haptics.exe` to the whitelist (so
     WE can still read the DualSense while it's hidden from everything else).
   - **Devices** tab → tick the DualSense (Wireless Controller, both USB + Bluetooth
     entries if present) to hide it.
   - Enable **"Enable device hiding"** at the bottom.

Without HidHide the Xbox pad still appears, but the game may register both controllers.

## Build

Prereqs: Rust (stable, MSVC toolchain), Node, and the Tauri prereqs
(WebView2 runtime ships with Windows 10/11).

```powershell
cd dualsense-haptics
npm install
npm run tauri build      # release MSI/NSIS installer in src-tauri/target/release/bundle
# or for dev:
npm run tauri dev
```

The `vigem-client` crate and the `src-tauri/src/xinput.rs` module only compile on Windows
(`#[cfg(windows)]`), so they don't affect the macOS build.

## Using it

1. Plug in / pair the DualSense.
2. Launch the app. In the header, **Output** row: pick **Xbox**.
3. The virtual Xbox pad plugs in; switching back to **DualSense** unplugs it.
4. Launch Forza. It should see an Xbox controller. Pick a haptic profile (e.g. Racing or
   Game → Dead Island 2) as usual — the real DualSense gets the triggers/rumble.

## Input mapping (xinput.rs)

DualSense → Xbox: Cross→A, Circle→B, Square→X, Triangle→Y, L1/R1→bumpers, Create→Back,
Options→Start, L3/R3→thumbs, D-pad hat→D-pad, L2/R2 analog→triggers, sticks→thumbsticks
(Y axis inverted to XInput convention).

**If a button is wrong or a `vigem_client::XButtons` constant fails to compile**, the
const names differ slightly between vigem-client versions — adjust the mapping in
`xinput.rs::push()`. The bit masks for the DualSense side (face byte / shoulder byte) are
documented inline.

## What is NOT automated

HidHide whitelisting/cloaking is manual (above). A future version could shell out to
`HidHideCLI.exe` to automate it, but for now set it once in the HidHide client.
