<p align="center">
  <a href="https://github.com/AlteredAJ/dualsense-haptics/releases"><img src="https://img.shields.io/github/v/release/AlteredAJ/dualsense-haptics?color=4fc3ff&style=for-the-badge" alt="release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=for-the-badge" alt="license"></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="platform">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/upgrade-%244%20Full%20Immersion-ff90e8?style=for-the-badge&logo=playstation&logoColor=white" alt="upgrade"></a>
</p>

<br>

<p align="center">
  <img src="https://raw.githubusercontent.com/AlteredAJ/dualsense-haptics/release-free/src-tauri/icons/128x128.png" width="96" alt="icon">
</p>

# Universal DualSense Haptics

<p align="center"><strong>Turn your PS5 controller into a PC haptic peripheral.</strong> Static adaptive trigger resistance, native DualSense passthrough, virtual Xbox 360 output — no PlayStation required.</p>

<br>

> [!TIP]
> **[Demo GIF coming soon]** — screen recording of the app in action, controller triggers visibly reacting to game input.

---

## Contents

- [Free vs Full Immersion](#free-vs-full-immersion)
- [Before & After](#before--after)
- [Features](#features)
- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Build from Source](#build-from-source)
- [Tech Stack](#tech-stack)

---

## Free vs Full Immersion

<table>
<tr>
<td width="50%" valign="top">

### Free (this repo)

Static adaptive triggers. USB + Bluetooth auto-detection with CRC-32 validation. Xbox virtual pad via ViGEmBus. HidHide device cloaking. Frameless window with custom titlebar. Zero config — plug in, launch, it works. **MIT licensed.**

<p align="center">
  <a href="https://github.com/AlteredAJ/dualsense-haptics/releases"><img src="https://img.shields.io/badge/⬇_Download_Free-2ea043?style=for-the-badge" alt="download"></a>
</p>

</td>
<td width="50%" valign="top">

### Full Immersion ($4)

Everything in Free, plus **six curated haptic profiles**:

- Racing — Forza/F1/AC telemetry drives triggers and rumble
- Gun — 9 weapon profiles, per-trigger recoil
- Melee — 10 swing-to-impact feels
- Audio Reactive — TRUE haptics over USB audio channels
- Minecraft — per-item feels via Fabric mod
- Static — fixed resistance, custom lightbar

Includes **The Lab**: live preview any profile, per-weapon feel sliders, custom brake/throttle curves, save presets. **Lifetime license.** No subscription.

<p align="center">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/🔓_Unlock_$4-4fc3ff?style=for-the-badge" alt="buy"></a>
</p>

</td>
</tr></table>

[⬆ Back to top](#contents)

---

## Before & After

| Without Universal Haptics | With Universal Haptics |
|---|---|
| Triggers feel dead — no resistance | Adaptive triggers push back with real tension |
| Rumble is generic on/off vibration | Voice-coil motors deliver per-profile envelopes |
| Lightbar stays static blue | Lightbar reacts to game state in real time |
| No game-aware feedback | Forza telemetry, F1 data, MC item detection |
| Controller is just a controller | Controller becomes a force-feedback instrument |

[⬆ Back to top](#contents)

---

## Features

> [!NOTE]
> **[Screenshots coming soon]** — app UI, controller visualization, and profile selector.

- **Static trigger resistance** — L2 and R2 hold firm while pressed
- **Native DualSense output** — games that support the controller read it directly
- **Virtual Xbox 360 mode** — XInput-only games see a controller via ViGEmBus
- **HidHide integration** — cloak the physical pad so games see only the virtual one
- **Frameless UI** — clean, minimal window with custom titlebar
- **USB + Bluetooth** — auto-detects transport, CRC-32 checks on wireless
- **Zero config** — plug in, launch, it works

[⬆ Back to top](#contents)

---

## Requirements

| Component | Needed For | Download |
|---|---|---|
| `Windows 10/11 x64` | Operating system | — |
| `Sony DualSense` | USB-C recommended, Bluetooth supported | — |
| `ViGEmBus` | Xbox virtual pad output | [Releases](https://github.com/nefarius/ViGEmBus/releases) |
| `HidHide` | Device cloaking (optional) | [Releases](https://github.com/nefarius/HidHide/releases) |

[⬆ Back to top](#contents)

---

## Quick Start

```powershell
# 1. Install ViGEmBus (for Xbox output) + HidHide (optional, for cloaking)

# 2. Download latest dualsense-haptics.exe
#    https://github.com/AlteredAJ/dualsense-haptics/releases

# 3. Launch the app
.\dualsense-haptics.exe

# 4. Toggle DualSense or Xbox output mode
# 5. Pick a profile — play your game
```

[⬆ Back to top](#contents)

---

## Build from Source

```powershell
git clone https://github.com/AlteredAJ/dualsense-haptics.git
cd dualsense-haptics/src-tauri
cargo build --release
```

Requires [Rust](https://rustup.rs) + [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/).

[⬆ Back to top](#contents)

---

## Tech Stack

<p align="center">
  <img src="https://img.shields.io/badge/Backend-Rust_%2B_Tauri_2-dea584?logo=rust&logoColor=white&style=for-the-badge" alt="rust">
  <img src="https://img.shields.io/badge/Frontend-Vanilla_JS_%2B_HTML_%2B_CSS-f7df1e?logo=javascript&logoColor=black&style=for-the-badge" alt="js">
  <img src="https://img.shields.io/badge/HID-hidapi-333?style=for-the-badge" alt="hidapi">
  <img src="https://img.shields.io/badge/Virtual_Pad-ViGEmBus-107c10?logo=xbox&logoColor=white&style=for-the-badge" alt="vigem">
</p>

[⬆ Back to top](#contents)

---

<p align="center">
  <sub><a href="https://github.com/AlteredAJ/dualsense-haptics">Free on GitHub</a> · <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics">Full Immersion $4 on Gumroad</a> · <a href="https://alteredaj.github.io/dualsense-haptics/">Landing page</a></sub>
</p>
