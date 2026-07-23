# Handoff prompt — DualSense haptics: feel tuning + visual redesign

Copy everything below the line into a fresh chat. It is self-contained.

---

You are picking up work on **dualsense-haptics**, a macOS desktop app that drives a
Sony DualSense controller's adaptive triggers, dual rumble motors, and lightbar to
add custom haptic feedback to games that don't natively support it. The app talks to
the controller over USB HID. It is a **Tauri 2** app: Rust backend (the haptics
engine) + a vanilla HTML/CSS/JS frontend (no framework).

Project root: `/Users/ajapaukese/Documents/Projects/dualsense-haptics`
Current version: `0.3.0` (pre-1.0, still iterating).

## Your two jobs

1. **Haptic feel-tuning pass** — make every profile feel more realistic and punchy.
2. **Elevated visual redesign** — keep it lightweight, but raise the design quality
   and add an animated controller SVG that reacts to live input.

Do both. Tackle them independently; they don't depend on each other.

---

## Architecture you need to know

### Backend (Rust) — the part that matters for feel
- `src-tauri/src/hid.rs` — the haptics engine. This is the big one. Contains:
  - The DualSense **output report 0x02** layout. Key bytes:
    - `b[3]` = right motor (high-frequency rumble), `b[4]` = left motor (low-freq).
    - `b[11]` = R2 trigger mode, `b[22]` = L2 trigger mode.
    - Trigger modes: `0x01` rigid resistance, `0x05` off, `0x06` active vibration drive.
    - `b[45/46/47]` = lightbar RGB.
  - Helpers: `haptics_report(...)`, `with_rumble(report, rl, rr)`,
    `lightbar_report(r,g,b)`, `player_led_report(...)`.
  - A **60fps frame loop**. `compute_rumble(s, &st)` reads pre-mutation state and
    returns motor levels; `process_frame(s)` builds the full report (triggers + rumble).
  - `Profile` enum (serde lowercase): `Racing, Static, Gun, Melee, Audio, Minecraft`.
  - `AppState` (the live tuning + input state) behind `Arc<Mutex<AppState>>`.
  - Tuning constants live near the top as `const`s (frame counts, Hz rates, thresholds).
    These are the knobs to turn for feel.
- `src-tauri/src/mc.rs` — Minecraft bridge: a TCP server on `127.0.0.1:27812` that a
  Fabric mod connects to and streams newline-delimited JSON game state (held item, use
  progress, mining, blocking, attack/hurt events, health, sprint, on-ground).
- `src-tauri/src/lib.rs` — Tauri setup, command registration, thread spawns
  (input loop, HID loop, mc bridge).
- The frontend talks to Rust via Tauri `invoke` commands (e.g. `set_steering_fx`,
  profile setters) and reads a `StateSnapshot` struct for live UI.

### Frontend (no framework)
- `src/index.html` — markup. Profile pills, the Racing Lab, Trigger Lab, Minecraft row.
- `src/main.js` — all UI logic. `PROFILES` array, render loop, Tauri invoke calls.
- `src/styles.css` — GitHub-dark monospace aesthetic. Already has a "Lightweight motion
  layer" block (button transitions, bar width transitions, fade/pulse/pop keyframes)
  gated behind `@media (prefers-reduced-motion: reduce)`. Keep that gate on anything
  you add.
- Window is fixed, non-resizable, ~820x540.

### Profiles that exist today
- **Racing**: brake/throttle adaptive-trigger curves, ABS pedal pump, gear-shift clunk
  (upshift = instant slam, downshift = heavier peel), simulated rising-RPM engine
  rumble, optional Steering FX (tire scrub, throttle lightening). Has a "Racing Lab"
  UI for live curve tuning + presets + a custom saved profile.
- **Gun**: per-weapon firing patterns (semi/burst/auto) with recoil.
- **Melee**: swing/impact feel.
- **Audio**: two-band reactive — bass -> left motor, treble -> right motor.
- **Static**: fixed lightbar, no haptics.
- **Minecraft**: per-item adaptive triggers + rumble driven by the Fabric mod (bow draw
  tension, mining grind textured per tool, sword/axe swing kick, shield brace, eating
  pulse, damage jolt scaled by health, low-health heartbeat, sprint footfalls). Lightbar
  recolors per held item.

---

## Job 1: Haptic feel-tuning pass

Go profile by profile and make each effect feel more like the real thing. The levers:
- Trigger **force** values and **mode** choice (rigid vs. active drive).
- Rumble **envelope** shape (attack/decay), **peak amplitude**, and **frequency split**
  between the two motors.
- **Timing** — frame counts for one-shot effects (a hit should land on frame 1, not
  feel delayed; we already fixed the upshift this way).
- **Phase-driven oscillators** (the `*_phase` fields + Hz constants) for continuous
  textures like mining grind, engine rumble, heartbeat.

Principles:
- Sharp events (hits, shifts, shots) want a fast attack and a short tail. Anything that
  feels "mushy" or "late" is usually a missing instant first frame.
- Continuous textures want the two motors doing different things; a flat single buzz
  reads as cheap.
- Adaptive-trigger resistance should ramp from a feather zone, not snap to a wall
  (Racing throttle/brake already do this — match that quality elsewhere).
- Scale intensity by context where it makes sense (damage by health, mining by tool).

After tuning, the user will feel-test on the actual controller. Leave the tuning
constants clearly named and grouped so they're easy to re-tweak.

## Job 2: Visual redesign

Keep the GitHub-dark monospace identity. Raise quality without going heavy:
- Refine the **type scale** and **spacing rhythm** (consistent vertical spacing,
  clearer hierarchy between section headers, labels, and values).
- Add an **animated controller SVG** that reacts to live input: trigger pull depth,
  motor activity (left/right), and lightbar color should visibly animate in real time
  off the `StateSnapshot` the UI already polls. This is the centerpiece.
- Lightweight only: CSS transitions + small keyframes + SVG attribute updates from JS.
  No animation libraries, no canvas/WebGL, no heavy assets.
- Everything must respect `@media (prefers-reduced-motion: reduce)` — motion off, layout
  and color still correct.

---

## Hard constraints (do not violate)

- **Never use em dashes in any output** (chat replies, code comments, UI copy). Use a
  comma or restructure.
- **After any change to Rust or JS/CSS runtime code, rebuild and relaunch**, and state
  the exact command you ran. The command is:
  ```
  pkill -f "target/debug/dualsense-haptics"; pkill -f "tauri dev"; sleep 1; cd /Users/ajapaukese/Documents/Projects/dualsense-haptics && npm run dev
  ```
- **Build gotcha**: do NOT run `cargo check` against the shared target dir before a
  `tauri dev` build. It poisons the dir and causes a `crate \`tauri\` required to be
  available in rlib format` error. For type-checking use `npm run check`, which isolates
  check artifacts in a separate `CARGO_TARGET_DIR`. If you hit the rlib error anyway:
  `cargo clean -p tauri` then relaunch.
- **Do not wipe user settings.** They live in `~/.config/dualsense-haptics/`, OUTSIDE
  the project. Settings persistence is backward-compatible per-field; preserve that.
- **Do not modify git config.** If you need a one-off git identity, use inline
  `git -c user.name=... -c user.email=...`.

## How to test
- Plug in a DualSense over USB, launch with the command above.
- Console should print `[mc] bridge listening on 127.0.0.1:27812` and
  `[diag] hid_loop started, haptic output open`.
- Switch profiles in the UI and feel the triggers/motors. For Minecraft, the Fabric mod
  in `minecraft-mod/` must be running in a MC 1.20.1 Fabric client to stream state
  (see `minecraft-mod/README.md`).
