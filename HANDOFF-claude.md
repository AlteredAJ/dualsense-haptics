# Claude Handoff — v0.4.0

## Repo State

Two branches:

| Branch | Remote | Purpose |
|--------|--------|---------|
| `release-free` | ✅ Pushed to GitHub (AlteredAJ/dualsense-haptics) | Public free static edition. 2 commits, zero history of paid features. MIT licensed. |
| `feature/racing-signal-processing` | ❌ LOCAL ONLY — never push | Full paid version with all DSP, telemetry, profiles. 29 commits from initial to current. |

## What's in release-free (public)
- `hid.rs` — 414 lines, Static profile only, no telemetry fields, no DSP
- `lib.rs` — Minimal Tauri shell, edition=Free hardcoded
- `src/` — Stripped HTML/JS frontend
- `xinput.rs` — ViGEmBus virtual pad
- `hidhide.rs` — HidHide cloaking
- No `forza.rs`, `f123.rs`, `acc.rs`, `signal.rs`, `license.rs`, `mc.rs`, `feels.rs`, `obfuscate.rs`

## What's ONLY on feature/racing-signal-processing (never pushed)
All these files exist ONLY locally:

- `src-tauri/src/acc.rs` — Assetto Corsa bridge (port 9996, handshake, RTCarInfo)
- `src-tauri/src/f123.rs` — F1 23 bridge (port 20777, multi-packet header, wheel transpose)
- `src-tauri/src/forza.rs` — Forza bridge (ports 5300/7000/20066, Dash V2)
- `src-tauri/src/signal.rs` — DSP: LPF, EWMA, Pacejka, slew, pneumatic trail
- `src-tauri/src/license.rs` — License validation (Cloudflare Worker)
- `src-tauri/src/mc.rs` — Minecraft TCP bridge
- `src-tauri/src/feels.rs` — Runtime-editable weapon feel tables
- `src-tauri/src/obfuscate.rs` — Binary hardening (XOR strings, integrity check scaffold)
- `src-tauri/src/hid.rs` — 3500+ lines: all profiles, telemetry fields, DSP routing
- `src-tauri/src/lib.rs` — 800+ lines: all Tauri commands, GameSource selector, bridge management

## Key Feature Commits (29 total)

```
ba8f9e0 Gumroad page copy
aa53833 Binary hardening module
42867c7 Bidirectional suspension coupling
a31a3fe Suspension bump jolts through triggers
4e9d3bc Vehicle mass auto-detection
687aef7 Suspension-coupled trigger feedback
e873c63 Lateral load feedback (cornering G-force)
d6c5f4e Forza telemetry rumble gain 1.2x
25afd0c F1/AC rumble gain 1.4x
5b2057b Speed-aware brake lockup warning
f9dc123 Throttle damper floor
ce602ab Per-bridge sample rates (120Hz/333Hz/60Hz)
4388b49 Per-game brake sensitivity (F1 lockup at 0.50)
292427d Per-game slip sensitivity (F1 at 0.12)
bcfa027 Free tier edition gates
3a95d20 Racing main-page controls + Stability+/Drift assists
24c67d7 Frameless window UI
c232c87 GameSource selector (Forza/F1/AC)
441a80a Assetto Corsa bridge
e177d8c F1 23 bridge
1de0520 Drivetrain profile frontend dropdown
38fb062 Drivetrain auto-detection (sliding window)
4a101a5 Initial racing haptics refinement (R001-R015)
```

## Current State

- **Paid code:** Safe on `feature/racing-signal-processing` (local only)
- **Public repo:** Clean `release-free` with 2 commits, no history
- **GitHub Pages:** Landing page at docs/index.html (needs Pages enabled in settings)
- **Release:** v0.4.0 tag, pre-release, needs exe uploaded manually
- **Gumroad:** Product page needs full description pasted (in gumroad_page_copy.md)

## Next for Claude

1. **Finish Gumroad setup** — paste description, set prices ($1 base, $4 pro)
2. **Upload release exe** — built at `src-tauri/target/release/dualsense-haptics.exe`
3. **Enable GitHub Pages** — settings → Pages → release-free /docs
4. **Test free version** — launch exe, verify Static profile works
5. **Continue DSP tuning** — all code on feature branch is ready to iterate
