<p align="center">
  <img src="https://img.shields.io/badge/version-0.4.0-blue" alt="version">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS-lightgrey" alt="platform">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="license">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/upgrade-Full%20Immersion%20%244-ff90e8?style=flat" alt="upgrade"></a>
</p>

---

# 🎮 Universal DualSense Haptics

**Turn your PS5 DualSense into a PC haptic peripheral.** Static adaptive trigger resistance, native DualSense passthrough, and virtual Xbox 360 output — all without a PlayStation.

<p align="center">
  <img src="https://img.shields.io/badge/DualSense-USB%20%7C%20Bluetooth-0052CC?style=for-the-badge&logo=playstation&logoColor=white" alt="DualSense">
  <img src="https://img.shields.io/badge/Xbox-Virtual%20XInput-107C10?style=for-the-badge&logo=xbox&logoColor=white" alt="Xbox">
</p>

---

## ✨ Features

- **Static trigger resistance** — L2 and R2 hold firm while pressed
- **Native DualSense output** — games that support the controller read it directly
- **Virtual Xbox 360 mode** — XInput-only games see a controller via ViGEmBus
- **HidHide integration** — cloak the real pad so games see only the virtual one
- **Frameless UI** — clean, minimal window with custom titlebar
- **USB + Bluetooth** — auto-detects transport, CRC-32 valid on wireless
- **Zero config** — plug in, launch, it works

---

## 📦 Download

Grab the latest `.exe` from [Releases](https://github.com/AlteredAJ/dualsense-haptics/releases).

---

## 🔓 Free vs Full Immersion ($4)

This is the **free** version — static adaptive triggers, no telemetry. Want more?

| Free (this repo) | Full Immersion ($4 on Gumroad) |
|---|---|
| Static trigger resistance | Racing telemetry (Forza, F1, AC) |
| USB + Bluetooth | Gun recoil (9 weapon profiles) |
| Xbox virtual pad | Melee swing feedback |
| HidHide cloaking | Audio reactive rumble |
| Frameless UI | Minecraft per-item feels |
| MIT open source | The Lab — live preview + tuning |

[**Upgrade to Full Immersion — $4 on Gumroad**](https://alt3red.gumroad.com/l/universal-dualsense-haptics)

---

## 🔧 Requirements

| Component | Needed for |
|-----------|-----------|
| [ViGEmBus](https://github.com/nefarius/ViGEmBus/releases) | Xbox virtual pad output |
| [HidHide](https://github.com/nefarius/HidHide/releases) | Cloaking the physical DualSense from games |

---

## 🚀 Quick Start

1. Install **ViGEmBus** if you want Xbox output
2. Install **HidHide** if you want device cloaking (run as admin)
3. Launch `dualsense-haptics.exe`
4. Plug in your DualSense via USB or pair via Bluetooth
5. Toggle **DualSense** or **Xbox** output mode

---

## 🏗️ Build from Source

```powershell
git clone https://github.com/AlteredAJ/dualsense-haptics.git
cd dualsense-haptics/src-tauri
cargo build --release
```

Requires [Rust](https://rustup.rs) + [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/).

---

## 🧩 Tech Stack

| Layer | Tech |
|-------|------|
| Backend | Rust + Tauri 2 |
| Frontend | Vanilla JS + HTML + CSS |
| HID I/O | hidapi |
| Virtual Pad | ViGEmBus (vigem-client) |
| Bluetooth CRC | crc32fast |

---

## 📜 License

MIT — see [LICENSE](LICENSE).

---

<p align="center">
  <sub>Built with ❤️ for sim racers and controller nerds.</sub>
</p>
