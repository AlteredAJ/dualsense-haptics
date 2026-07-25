<p align="center">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/release-v0.4.0-4fc3ff?style=for-the-badge&logo=playstation&logoColor=white" alt="release"></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="platform">
  <img src="https://img.shields.io/badge/engine-Rust_%2B_Tauri_2-dea584?style=for-the-badge&logo=rust&logoColor=white" alt="rust">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/lifetime_license-%244-ff90e8?style=for-the-badge" alt="price"></a>
</p>

<br>

<p align="center">
  <img src="https://raw.githubusercontent.com/AlteredAJ/dualsense-haptics-paid/main/src-tauri/icons/128x128.png" width="96" alt="icon">
</p>

# Universal DualSense Haptics — Full Immersion

<p align="center"><strong>Six curated haptic profiles. Real telemetry. Lifetime license.</strong> Connect your PS5 DualSense over USB and unlock adaptive triggers, voice-coil rumble, and lightbar feedback driven by real game data — on any game, no PS5 required.</p>

<br>

> [!TIP]
> **[Demo GIF coming soon]** — each profile in action: triggers pushing back during Forza braking, gun recoil patterns, Minecraft per-item vibrations, audio reactive bass response.

---

## Contents

- [At a Glance](#at-a-glance)
- [Before & After](#before--after)
- [Profiles](#profiles)
  - [Racing](#racing)
  - [Gun](#gun)
  - [Melee](#melee)
  - [Audio Reactive](#audio-reactive)
  - [Minecraft](#minecraft)
  - [Static](#static)
- [The Lab](#the-lab)
- [DSP Engine](#dsp-engine)
- [Architecture](#architecture)
- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Tech Stack](#tech-stack)

---

## At a Glance

<table align="center">
<tr>
<td align="center" width="25%"><h3>6</h3><sub>Profiles</sub></td>
<td align="center" width="25%"><h3>60fps</h3><sub>Haptic loop</sub></td>
<td align="center" width="25%"><h3>9</h3><sub>Gun feels</sub></td>
<td align="center" width="25%"><h3>10</h3><sub>Melee weapons</sub></td>
</tr></table>

[⬆ Back to top](#contents)

---

## Before & After

| DualSense on PC *without* this | *With* Universal Haptics |
|---|---|
| Triggers feel dead — no resistance | Adaptive triggers push back with real tension |
| Rumble is generic on/off vibration | Voice-coil motors deliver per-profile envelopes |
| Lightbar stays static blue | Lightbar reacts to game state in real time |
| No game-aware feedback | Forza telemetry, F1 data, MC item detection |
| Controller is just a controller | Controller becomes a force-feedback instrument |

[⬆ Back to top](#contents)

---

## Profiles

> [!NOTE]
> **[Screenshots coming soon]** — each profile with the app UI showing the profile selector, trigger visualization, and tuning sliders.

### Racing

Real UDP telemetry from **Forza Horizon**, **Forza Motorsport**, **F1 23**, **Assetto Corsa**.

- **Brake pedal** firms up with speed — aero downforce simulation
- **Throttle** stiffens at high RPM, lightens at droop, near-zero when airborne
- **Tire slip** — wheelspin flutter (50–80 Hz) and lockup judder (35 Hz)
- **ABS pump** — rhythmic pulse at lockup threshold
- **Gear shifts** — mechanical clunk through both triggers, rev-matched intensity
- **Rev-limiter bounce** at redline
- **Suspension bumps** — stereophonic kerb strikes and road texture to grip motors
- **Trailbraking** oversteer feedback
- Five drivetrain tuning presets: **AWD · RWD · FWD · Hybrid Electric · Default**

[⬆ Back to top](#contents)

### Gun

Nine weapon profiles, each with its own trigger break, recoil curve, and fire rate.

| Weapon | Behavior |
|---|---|
| Pistol | Semi-auto break at 43% pull, sharp kick |
| Revolver | Heavier break, longer recoil thump |
| Rifle | Controlled snap, fast reset |
| Burst | Configurable 2–5 round bursts, intra-burst timing |
| AR | Continuous full-auto, adjustable fire rate |
| SMG | Fast cyclic rate, light buzz |
| LMG | Heavy sustained rumble |
| Shotgun | Wide recoil pulse, slow pump return |
| Sniper | Deliberate heavy break, long recovery |

[⬆ Back to top](#contents)

### Melee

Ten weapon profiles. Resting swing heft builds resistance as you pull R2. At full draw, release fires a connect-kick impact thump through the trigger and both grip motors.

> Fists · Knife · Machete · Katana · Axe · Cleaver · Knuckles · Bat · Spear · Sledgehammer

[⬆ Back to top](#contents)

### Audio Reactive

System audio drives the haptics in real time.

- **Sub-bass** (<90 Hz) → left grip motor (kicks, explosions)
- **Engine band** (90–280 Hz) → right grip motor (revs, synth tones)
- **TRUE haptics mode** — streams waveform directly into the DualSense's USB audio channels (Windows channels 3/4), the same signal path PS5 games use. Not simulated rumble.

[⬆ Back to top](#contents)

### Minecraft

Fabric mod (included) connects over local TCP (`localhost:27812`).

- Sword/Axe — swing impact with directional thump
- Pickaxe — mining grind textured per block hardness
- Bow — progressive draw tension, release snap, critical twang
- Shield — brace pulse on block
- Food — eating gulp haptic
- Health — low-health heartbeat, damage jolt scaled by severity
- Sprint — footfall vibrations through grip motors
- Lightbar — recolors in real time per held item

[⬆ Back to top](#contents)

### Static

Fixed trigger resistance and custom lightbar color. No game required. USB + Bluetooth support.

[⬆ Back to top](#contents)

---

## The Lab

Live preview any profile without launching a game. Tune every weapon's feel in real time.

- **Per-weapon sliders** — kick strength, rumble intensity, swing heft, fire rate
- **Racing Lab** — custom brake/throttle curves with live tach preview
- **Save presets** — persist your tuned profiles across sessions

[⬆ Back to top](#contents)

---

## DSP Engine

| Component | Description |
|---|---|
| Low-pass filter | Smooths telemetry noise before haptic synthesis |
| EWMA envelope | Exponential weighted moving average for attack/decay shaping |
| Pacejka model | Tire slip force curve modeling for realistic lateral grip feel |
| Slew limiting | Prevents jarring instantaneous force jumps |
| Slip crossover | Normal slip → high-frequency flutter; deep slip → low-frequency judder |
| Suspension coupling | Pedals go lighter at full droop (0.80×), firmer at full compression (1.20×), near-zero airborne (0.30×) |

[⬆ Back to top](#contents)

---

## Architecture

```
dualsense-haptics/
├── src/                    # Frontend (vanilla HTML/CSS/JS)
│   ├── index.html          # Profile selector, controller viz, Racing Lab, The Lab
│   ├── styles.css          # Glass-morphism design system, dark/light themes
│   ├── main.js             # UI logic, profile state, render loop
│   └── dualsense.svg       # Inline DualSense controller outline
│
├── src-tauri/              # Backend (Rust, Tauri 2)
│   ├── src/
│   │   ├── hid.rs          # Haptics engine — HID output reports, 60fps frame loop
│   │   ├── signal.rs       # DSP functions — low-pass, EWMA, Pacejka, slew, crossover
│   │   ├── forza.rs        # Forza Data Out UDP telemetry parser
│   │   ├── f123.rs         # F1 23 telemetry parser
│   │   ├── acc.rs          # Assetto Corsa telemetry stub
│   │   ├── mc.rs           # Minecraft TCP bridge
│   │   ├── license.rs      # License validation (Cloudflare Worker)
│   │   ├── settings.rs     # Persisted settings & presets
│   │   ├── feels.rs        # Per-weapon feel tuning state
│   │   ├── xinput.rs       # ViGEmBus virtual Xbox pad (XInput)
│   │   └── hidhide.rs      # HidHide device cloaking
│   └── icons/              # App icons (Windows, iOS, Android)
│
├── worker/                 # Cloudflare Worker (license + beta key server)
│   └── src/index.ts        # /activate, /validate, /version, /admin/*
│
└── minecraft-mod/          # Fabric mod (Minecraft 1.20.1)
```

[⬆ Back to top](#contents)

---

## Requirements

| Component | Needed For | Download |
|---|---|---|
| `Windows 10/11 x64` | Operating system | — |
| `Sony DualSense` | USB-C recommended, Bluetooth supported | — |
| `ViGEmBus` | Xbox virtual pad output | [Releases](https://github.com/nefarius/ViGEmBus/releases) |
| `HidHide` | Device cloaking (optional) | [Releases](https://github.com/nefarius/HidHide/releases) |
| `Minecraft Fabric 1.20.1` | For Minecraft profile | [Fabric](https://fabricmc.net/) |

[⬆ Back to top](#contents)

---

## Quick Start

```
1. Buy a license on Gumroad ($4 lifetime)
2. Download dualsense-haptics.exe — portable, no installer
3. Enter license key — binds to one machine
4. Install ViGEmBus (optional, for Xbox output) + HidHide (optional, for cloaking)
5. Plug in DualSense via USB, pick a profile, launch your game
```

[⬆ Back to top](#contents)

---

## Tech Stack

<p align="center">
  <img src="https://img.shields.io/badge/Backend-Rust_%2B_Tauri_2-dea584?logo=rust&logoColor=white&style=for-the-badge" alt="rust">
  <img src="https://img.shields.io/badge/Frontend-Vanilla_JS_%2B_HTML_%2B_CSS-f7df1e?logo=javascript&logoColor=black&style=for-the-badge" alt="js">
  <img src="https://img.shields.io/badge/HID-hidapi-333?style=for-the-badge" alt="hidapi">
  <img src="https://img.shields.io/badge/Virtual_Pad-ViGEmBus-107c10?logo=xbox&logoColor=white&style=for-the-badge" alt="vigem">
  <img src="https://img.shields.io/badge/Audio-cpal-ff6a4d?style=for-the-badge" alt="cpal">
  <img src="https://img.shields.io/badge/License_Server-Cloudflare_Workers-f38020?logo=cloudflare&logoColor=white&style=for-the-badge" alt="cloudflare">
</p>

[⬆ Back to top](#contents)

---

<p align="center">
  <a href="https://alt3red.gumroad.com/l/universal-dualsense-haptics"><img src="https://img.shields.io/badge/🔓_Get_Full_Immersion_$4-4fc3ff?style=for-the-badge&logo=playstation&logoColor=white" alt="buy"></a>
  <a href="https://github.com/AlteredAJ/dualsense-haptics"><img src="https://img.shields.io/badge/Try_the_free_version-2ea043?style=for-the-badge" alt="free"></a>
  <a href="https://alteredaj.github.io/dualsense-haptics/"><img src="https://img.shields.io/badge/Landing_Page-0078D6?style=for-the-badge" alt="site"></a>
</p>
