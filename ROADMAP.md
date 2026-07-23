# Roadmap — possible features

Not committed work. A parking lot of ideas worth doing, roughly ordered by value
vs. effort. Pull from here when picking the next thing to build.

## Near term
- **Master intensity slider**: one global 0-100% multiplier over all rumble/trigger
  output, so the whole app can be dialed down without editing per-profile curves.
- **Per-game auto profile switch**: detect the foreground app/process and switch
  profiles automatically (e.g. Minecraft running -> Minecraft profile).
- **Tray mode + auto-reconnect**: minimize to the menu bar, keep the HID loop alive,
  and re-grab the controller automatically when it sleeps/wakes or reconnects.
- **Strip debug logging**: gate the `eprintln!` diagnostics behind a debug flag or
  remove them before a release build.

## Minecraft profile
- **Fabric / Forge toggle**: ship a Forge build of the bridge mod, or document it,
  for users not on Fabric.
- **More effects**: Elytra gliding (wind buffet on both motors), fall-damage thud
  scaled by distance, bucket fill/empty, fishing rod cast + bite, eating finish pop,
  bow critical (fully-charged) distinct twang, redstone/lever click.
- **Mob proximity / ambient**: subtle cue when a hostile mob is close (optional).

## Feel + visual (bigger pass)
- **Haptic feel-tuning pass**: re-tune every profile's trigger forces, rumble
  envelopes, and timings for realism and punch. See the handoff prompt in
  `HANDOFF-feel-and-visual.md`.
- **Elevated visual redesign**: refined type scale + spacing, an animated controller
  SVG that reacts to live input (triggers, motors, lightbar), tasteful but still
  lightweight. Also in `HANDOFF-feel-and-visual.md`.

## Release
- **Signed/notarized DMG**: package, codesign, and notarize a distributable build.
- **First-run onboarding**: short explainer of profiles + the Minecraft mod setup.
