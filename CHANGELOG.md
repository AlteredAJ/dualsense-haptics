# Changelog

All notable changes to this project are documented here. Versions stay in the
0.x range until the first real release, which will be 1.0.0.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.3.0] - Unreleased (in development)

Working version. Not released yet.

### Added
- **$1 / $4 tier system (freemium Pro gate)**: single Gumroad product with two versions —
  Base ($1, all haptic profiles + strengths) and Pro ($4, Base + the Lab). The license
  worker reads the purchased version from Gumroad's `purchase.variants` and returns a
  tamper-proof `pro` flag on both `/activate` and `/validate`; beta keys auto-grant Pro.
  The app gates all Lab commands (`set_preview`, `save_feels`, `reset_feels`, `set_test`,
  `set_racing_lab`) behind a Pro check in Rust, locks the Lab button, and shows a $4
  upsell when a non-Pro user opens it.
- **Lab live preview + runtime feel tuning**: each haptic Lab tab has a Live Preview
  toggle that routes the real engine to that profile so the controller drives the
  genuine effect with no game/mod connected. Gun and Melee tabs gained per-weapon feel
  sliders backed by `feels.json` (in `~/.config/dualsense-haptics/`) — retune any
  weapon and Save with no recompile. Melee now has a real per-weapon table (knife,
  machete, cleaver, bat, katana, sledgehammer) instead of one generic impact.
- **Beta keys (license worker)**: new admin endpoints `/admin/mint-beta`,
  `/admin/revoke`, and `/admin/list-beta` generate `BETA-XXXX-XXXX-XXXX` keys that
  activate the app without a Gumroad purchase. Beta records skip Gumroad verification;
  revoked keys are refused at both activate and validate. Helper CLI at
  `worker/scripts/beta.sh`. See `RELEASE.md` for the full distribution flow.
- **Release packaging**: generated the macOS icon set and wired it into the bundle
  config so `tauri build` produces a proper `.app`/`.dmg`. Documented universal-binary
  build + Gumroad upload + beta-tester onboarding in `RELEASE.md`.
- **Minecraft profile (Phase 2 — per-item feels)**: the mod now streams full gameplay
  state (held item, use progress, mining, blocking, sprint, on-ground, health, plus
  rising-edge attack/hurt events), and the app maps it to adaptive-trigger + rumble
  effects PS5 Minecraft doesn't have:
  - **Bow / crossbow / trident**: right-trigger draw tension that grows with pull, a
    faint string tremble near full draw, and a crisp release twang.
  - **Pickaxe / axe / shovel / hoe**: rhythmic mining grind on the right trigger + motor,
    textured per tool (hard stone bite vs. soft dirt thuds).
  - **Sword / axe**: resting trigger heft plus a swing-connect kick (axe heavier).
  - **Shield**: firm left-trigger brace while blocking.
  - **Food**: springy trigger resistance and a gentle gulp pulse while eating/drinking.
  - **Damage jolt** scaled by how low your health is, a **low-health heartbeat**
    double-thump under 3 hearts, and a subtle **sprint footfall cadence**.
  - UI now shows the live action (Mining / Blocking / Using) next to the held item.
- **Minecraft profile (Phase 1)**: a new profile driven by a Fabric mod bridge. The app
  runs a localhost TCP server on `127.0.0.1:27812`; the mod connects and pushes the
  currently held-item category as newline-delimited JSON. Phase 1 proof of life is the
  lightbar recoloring to match the held item (sword, pickaxe, bow, food, etc.). Per-item
  trigger and rumble feels land in Phase 2. New `Minecraft` profile button, a held-item
  status row in the UI, and the `mc` bridge module on the Rust side.
- **Steering FX (Racing, Full edition)**, two opt-in toggles in the Racing Lab:
  - **Tire scrub**: steering angle and pedal load add high-frequency grain on the
    right motor, so loading the car in a corner feels like the tires scrubbing.
  - **Throttle lightening**: hard steering at high throttle bleeds off throttle
    resistance and adds a right-motor wheelspin judder, mimicking traction loss.
- **Simulated rising-RPM engine rumble (Racing, Full)**: the left motor pulses at a
  rate that climbs with throttle (lumpy near idle, fast flutter toward redline).
  Driven deterministically from the throttle, with rev inertia so the rate spins up
  and down smoothly instead of jittering on light feathering.
- **Audio two-band reactive mode**: bass drives the left motor, treble the right, so
  it reacts to the actual sound instead of a flat single buzz. Kept as the general
  reactive mode for non-driving games (music, shooters).
- `set_steering_fx` command and persisted `tire_scrub_on` / `throttle_light_on`
  settings.

### Changed
- **Throttle trigger now has a feather zone** like the brake: resistance ramps in
  from zero at the deadzone instead of snapping to a wall, removing the clunk and
  the automatic-trigger feel when feathering lightly.
- **ABS pedal pump** uses active trigger drive (0x06) at a low pump rate so each
  shove is a distinct push you can feel, with a soft synced left-motor rumble.
- Version scheme reset to 0.x so 1.0.0 can mark the first real release. The license
  worker's version config (`latest_version` → 0.3.0, `min_version` → 0.1.0) was updated
  to match, so the app no longer prompts to "update" to the old 1.2.0 build.
- **Upshift snap is now instant**: dropped the leading trigger-release frame and shortened
  the slam, so the upshift punch lands on the first frame instead of feeling delayed.
  Downshift keeps the heavier release-then-slam peel.

## [0.2.0] - Archived baseline

Snapshot taken before the 0.3.0 work began. Tagged `v0.2.0` in git and saved as
`dualsense-haptics-v0.2.0-archive.zip`. (Originally labeled 1.2.0 before the version
scheme was reset.)

### Baseline features
- Racing, Gun, Melee, Audio, and Static haptic profiles.
- Racing Lab with brake/throttle curve tuning, presets, live preview, and a custom
  saved profile.
- Trigger Lab for raw effect testing.
- ABS, gear-shift clunk, and engine rumble for the Racing profile.
- Per-weapon gun firing patterns (semi, burst, auto) with recoil.
- Settings persistence with per-field backward-compatible defaults.

[0.3.0]: https://example.invalid/compare/v0.2.0...HEAD
[0.2.0]: https://example.invalid/tree/v0.2.0
