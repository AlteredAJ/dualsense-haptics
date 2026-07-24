# v0.4.0 — Racing Haptics Refinement

## New Features

### Aero Brake Stiffness
The brake trigger now firms up progressively at high speed to simulate aerodynamic downforce loading. Requires Forza Data Out telemetry — harmless fallback when telemetry is inactive.

### Rev-Limiter Bounce
A rhythmic thump fires at max RPM, distinct from the existing redline-approach flutter. Works with both Forza telemetry and the simulated engine model.

### Drivetrain-Specific Tuning Profiles
Five presets (Default, Mechanical AWD, Hybrid Electric, RWD, FWD) tune the slip deadzone, flutter frequency range, and crossover deep-judder frequency to match different vehicle archetypes. Selectable via the `set_drivetrain_profile` Tauri command. Defaults to "Default" which preserves prior behavior.

## Changed Behavior

### Throttle Flutter Frequency Crossover
The slip frequency crossover direction has been corrected. Previously, deep slip (>1.0 ratio) produced a higher-frequency vibration (40 Hz) than normal slip (8-28 Hz). This was inverted — it made hybrid-electric drivetrains feel harsh and clattery as their eTC systems produced rapid slip oscillation at high frequency.

**New behavior:** Normal slip produces a light informative flutter (50-80 Hz, rising with slip intensity). Deep slip produces a heavy low-frequency judder (35 Hz) that the trigger motor can track smoothly. This eliminates worm-gear chatter on hybrid vehicles like the Ferrari SF90 Stradale.

## Optimizations
- HID output reports are now delta-checked on USB — unchanged haptic payloads skip transmission, reducing USB bandwidth consumption. Bluetooth is unaffected (always transmits to advance the sequence counter).

## Known Issues
- Drivetrain profile selection has no frontend UI — defaults to "Default." A dropdown will be added in a future update.
- HidHide cloaking may fail if HidHide is not installed — the virtual Xbox pad still works, but the game may see both controllers.
- Forza watchdog connection flag may briefly flicker on stale-feed detection (self-corrects on next packet).

## Notes
- All existing Racing, Gun, Melee, Static, Audio, and Minecraft profiles are unchanged.
- Settings files from v0.3.0 are fully compatible — new fields default to appropriate values.
