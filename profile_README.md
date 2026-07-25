<p align="center">
  <img src="https://raw.githubusercontent.com/AlteredAJ/dualsense-haptics/release-free/src-tauri/icons/128x128.png" width="80" alt="logo">
</p>

<h1 align="center">AJ</h1>

<p align="center"><strong>Systems & haptics engineer. Rust, Tauri, DSP, game telemetry.</strong></p>

<p align="center">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/Gumroad-Support-ff90e8?style=flat-square&logo=gumroad&logoColor=white" alt="gumroad"></a>
  <a href="https://alteredaj.github.io/dualsense-haptics/"><img src="https://img.shields.io/badge/Website-Landing_Page-4fc3ff?style=flat-square" alt="website"></a>
  <a href="https://www.linkedin.com/"><img src="https://img.shields.io/badge/LinkedIn-Connect-0a66c2?style=flat-square&logo=linkedin" alt="linkedin"></a>
  <img src="https://komarev.com/ghpvc/?username=AlteredAJ&style=flat-square&color=4fc3ff" alt="views">
</p>

---

### About

I build software that connects hardware to games. Currently shipping **[Universal DualSense Haptics](https://github.com/AlteredAJ/dualsense-haptics)** — a Windows desktop app that turns the PS5 DualSense into a full haptic peripheral, driving adaptive triggers and voice-coil rumble from real game telemetry at 60fps.

- **Rust + Tauri 2** — haptics engine, HID output, telemetry parsers
- **DSP** — low-pass filters, EWMA envelopes, Pacejka tire models, slip crossover
- **UDP telemetry** — Forza Data Out, F1 23, Assetto Corsa
- **TCP bridges** — Minecraft Fabric mod, custom JSON protocol
- **License infra** — Cloudflare Workers, Gumroad API, beta key minting
- **UI** — vanilla HTML/CSS/JS, glass-morphism design system, Manrope variable font

---

### Projects

<p align="center">
  <a href="https://github.com/AlteredAJ/dualsense-haptics">
    <img src="https://github-readme-stats.vercel.app/api/pin/?username=AlteredAJ&repo=dualsense-haptics&theme=github_dark&hide_border=true&description_lines_count=3" alt="dualsense-haptics">
  </a>
</p>

<table>
<tr>
<td width="50%" valign="top">

#### [Universal DualSense Haptics](https://github.com/AlteredAJ/dualsense-haptics)

Desktop app that drives DualSense adaptive triggers, voice-coil rumble, and lightbar from real game data.

- **Six haptic profiles** — Racing, Gun, Melee, Audio Reactive, Minecraft, Static
- **Telemetry-driven** — Forza Horizon/Motorsport, F1 23, Assetto Corsa via UDP
- **Minecraft mod** — Fabric bridge over TCP, per-item feels and lightbar colors
- **Virtual Xbox pad** — ViGEmBus XInput passthrough with HidHide cloaking
- **60fps haptic loop** — custom DSP pipeline in Rust
- **The Lab** — live preview, per-weapon tuning sliders, Racing Lab curves

</td>
<td width="50%" valign="top">

#### Key Metrics

| Metric | |
|---|---|
| Language | Rust, JS, CSS |
| License | MIT (free) / Proprietary (paid) |
| Platform | Windows 10/11 x64 |
| Engine | Tauri 2 + hidapi + ViGEmBus |
| Pricing | Free (static) · $4 Full Immersion |
| Site | [alteredaj.github.io/dualsense-haptics](https://alteredaj.github.io/dualsense-haptics/) |
| Gumroad | [alt3red.gumroad.com](https://alt3red.gumroad.com/l/universal-dualsense-haptics) |

</td>
</tr></table>

<p align="center">
  <a href="https://github.com/AlteredAJ/dualsense-haptics/releases"><img src="https://img.shields.io/badge/⬇_Download_Free-2ea043?style=for-the-badge" alt="free"></a>
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/🔓_Full_Immersion_$4-4fc3ff?style=for-the-badge" alt="paid"></a>
</p>

---

### Stack

<p align="center">
  <img src="https://img.shields.io/badge/Rust-dea584?style=for-the-badge&logo=rust&logoColor=white" alt="rust">
  <img src="https://img.shields.io/badge/Tauri_2-ffc131?style=for-the-badge&logo=tauri&logoColor=black" alt="tauri">
  <img src="https://img.shields.io/badge/JavaScript-f7df1e?style=for-the-badge&logo=javascript&logoColor=black" alt="js">
  <img src="https://img.shields.io/badge/CSS-glass_UI-4fc3ff?style=for-the-badge" alt="css">
  <img src="https://img.shields.io/badge/Cloudflare_Workers-f38020?style=for-the-badge&logo=cloudflare&logoColor=white" alt="cloudflare">
  <img src="https://img.shields.io/badge/HID-hidapi-333?style=for-the-badge" alt="hid">
</p>

---

<p align="center">
  <sub>
    <a href="https://github.com/AlteredAJ">github</a> ·
    <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics">gumroad</a> ·
    <a href="https://alteredaj.github.io/dualsense-haptics/">landing page</a>
  </sub>
</p>
