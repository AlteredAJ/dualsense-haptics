# Contributing to Universal DualSense Haptics

Thank you for your interest in contributing! Every contribution matters — whether it's fixing a typo, reporting a bug, or adding a new haptic profile.

---

## Code of Conduct

By participating, you agree to maintain a respectful and inclusive environment. Be kind and constructive in all interactions.

---

## Ways to Contribute

- **Report bugs** — Found something broken? Open an issue with reproduction steps (game, controller connection type, Windows version).
- **Suggest features** — Have an idea? Open a Feature Request issue describing the use case.
- **Improve documentation** — Fix typos, clarify setup steps, or add examples.
- **Write code** — Fix bugs, add telemetry sources, refine DSP curves, or improve the UI.

---

## Getting Started

### Prerequisites

- **Windows 10/11** (x64)
- **Rust** (via [rustup](https://rustup.rs))
- **WebView2** (included in Windows 10+  )
- **Node.js 18+** (for frontend dev)
- **Tauri CLI v2**: `cargo install tauri-cli --version "^2"`
- **DualSense controller** (USB-C for reliable HID during development)
- **ViGEmBus** (optional — for Xbox virtual pad testing)
- **HidHide** (optional — for device cloaking testing)

### Setup

```powershell
# 1. Fork the repository on GitHub

# 2. Clone your fork
git clone https://github.com/<your-username>/dualsense-haptics.git
cd dualsense-haptics

# 3. Install frontend dependencies
npm ci

# 4. Build the Rust backend
cd src-tauri
cargo build

# 5. Run in dev mode
npm run dev
```

> **Note:** The app talks to a physical DualSense over USB HID. Without one plugged in, the haptics engine won't start. Connect via USB-C before launching.

### License Worker (Cloudflare)

```powershell
cd worker
npm install
npm run typecheck
npx wrangler dev
```

Requires `GUMROAD_PRODUCT_ID`, `TOKEN_SECRET`, and `ADMIN_KEY` secrets. See `RELEASE.md` for setup.

---

## Development Workflow

### Branching

```powershell
git checkout main
git pull upstream main
git checkout -b feat/your-feature-name
```

| Prefix | Purpose |
|---|---|
| `feat/` | New feature (profile, telemetry source, UI component) |
| `fix/` | Bug fix |
| `docs/` | Documentation only |
| `tune/` | Haptic feel adjustments (trigger curves, DSP constants) |
| `perf/` | Performance improvements |
| `refactor/` | Code restructuring (no behavior change) |

### Commit Messages

Follow Conventional Commits:

```
<type>(<scope>): <short summary>

[optional body]
```

Examples:
```
feat(hid): add drivetrain-specific slip crossover tuning
fix(signal): correct throttle flutter frequency inversion
tune(gun): increase revolver recoil kick magnitude
docs(README): document ViGEmBus setup
```

---

## Project Structure

```
dualsense-haptics/
├── src/                    # Frontend (vanilla HTML/CSS/JS, no framework)
│   ├── index.html          # Main UI — profile selector, controller viz, Lab
│   ├── styles.css          # Glass-morphism design system, dark/light themes
│   ├── main.js             # UI logic, profile state, Tauri invoke calls
│   └── dualsense.svg       # Inline DualSense controller illustration
│
├── src-tauri/              # Backend (Rust, Tauri 2)
│   └── src/
│       ├── hid.rs          # Haptics engine — HID output reports, 60fps frame loop
│       ├── signal.rs       # DSP — low-pass, EWMA, Pacejka, slew, slip crossover
│       ├── forza.rs        # Forza Data Out UDP telemetry parser
│       ├── f123.rs         # F1 23 telemetry parser
│       ├── acc.rs          # Assetto Corsa telemetry (stub)
│       ├── mc.rs           # Minecraft TCP bridge (localhost:27812)
│       ├── license.rs      # License validation against Cloudflare Worker
│       ├── settings.rs     # Persisted settings & presets
│       ├── feels.rs        # Per-weapon feel tuning state
│       ├── xinput.rs       # ViGEmBus virtual Xbox pad (XInput)
│       └── hidhide.rs      # HidHide device cloaking automation
│
├── worker/                 # Cloudflare Worker — license + beta key server
│   └── src/index.ts        # /activate, /validate, /version, /admin/*
│
└── minecraft-mod/          # Fabric mod (Minecraft 1.20.1)
    └── README.md           # Per-item category TCP stream docs
```

---

## Code Quality

```powershell
# Rust
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# Frontend
npx prettier --check src/
```

---

## Testing

- **Unit tests**: `cargo test` in `src-tauri/`
- **Haptic testing**: Plugin a DualSense via USB. Use the Static profile to verify basic trigger output.
- **Telemetry testing**: Launch Forza Horizon/F1 23 with Data Out enabled. Run in dev mode (`npm run dev`) and check the Racing profile responds.
- **License flow**: Set `LOCAL_LICENSE_SERVER=http://localhost:8787` to test against a local worker.

> [!NOTE]
> Most profiles require either real game telemetry or a game running. The **Static** and **Audio Reactive** profiles work standalone for quick testing.

---

## Submitting Changes

### Opening an Issue

Before working on a significant change, open an issue to discuss the approach.

When reporting a bug, include:
- App version (check the badge or `tauri.conf.json`)
- Windows version + build
- DualSense connection type (USB / Bluetooth)
- Game + profile being used
- Steps to reproduce
- Expected vs. actual behavior

### Pull Request Process

1. Keep PRs focused — one logical change per PR.
2. Test with a physical DualSense if your change touches `hid.rs`, `signal.rs`, or any profile.
3. Update docs if your change affects user-facing behavior.
4. Fill in the PR template — describe what your change does and why.

---

## First-Time Contributors

Good areas to start:

- **Documentation** — Fix typos, improve setup guide, add usage examples
- **UI polish** — Style refinements in `styles.css`, improve layout or accessibility
- **New haptic feels** — Tune existing weapon profiles in `feels.rs` or add new weapons
- **Telemetry sources** — Implement stubs (e.g. `acc.rs`) or add new game telemetry parsers
- **Error messages** — Improve CLI output, HID error handling, user-facing diagnostics

---

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
