# Implementation Plan: Forza Haptics Refinement

---

## REQ-01 — Suspension Impact Low-Pass Filtering

### Affected Modules
`signal`, `forza`

### Affected Files
- `src-tauri/src/signal.rs` — `low_pass()`, constants `SUSP_LPF_HZ`, `HEAVE_LPF_HZ`
- `src-tauri/src/forza.rs` — `apply_packet()`

### Affected Structs
None (AppState fields are the target, no struct changes)

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `signal::low_pass()` — existing, no signature change
- `forza::apply_packet()` — already calls `low_pass()` for heave and suspension

### Affected State Variables
- `AppState.t_filt_heave` — written by `low_pass()` in apply_packet, read by subsequent haptic use
- `AppState.t_heave` — written from filtered value
- `AppState.t_filt_susp_fl`, `t_filt_susp_fr`, `t_filt_susp_rl`, `t_filt_susp_rr` — written by `low_pass()`
- `AppState.t_susp_fl/fr/rl/rr` — written from filtered values
- `AppState.t_bump_left`, `t_bump_right` — computed from filtered suspension deltas
- `AppState.road_phase` — driven in part by surface roughness

### Affected Configuration
- `SUSP_LPF_HZ` (currently 12.0) — in recommended 10-15 Hz range
- `HEAVE_LPF_HZ` (currently 12.0) — in recommended 10-15 Hz range

### Affected Runtime Paths
- Forza receiver thread: `receiver_loop()` → `apply_packet()` → suspension LPF
- Output thread 60 Hz: `process_frame()` reads filtered suspension values

### Affected Threads
- Forza receiver threads (N per port)
- Output thread (consumes filtered values)

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only

### Validation Strategy
1. Play Forza Horizon, drive over severe compression/bump zones
2. Confirm no audible plastic clack from controller chassis
3. Confirm a distinct low-frequency thud is still felt
4. Measure `t_filt_heave` and `t_filt_susp_*` values at runtime to verify LPF is active
5. If clacking persists, lower `SUSP_LPF_HZ` from 12 to 8-10 and test again

### Estimated Difficulty
Trivial (verification only — may require cutoff frequency tuning)

---

## REQ-02 — Adaptive Trigger Slew Rate Limiting

### Affected Modules
`signal`, `hid`

### Affected Files
- `src-tauri/src/signal.rs` — `slew_rate_limit()`, constant `SLEW_MAX_CHANGE`
- `src-tauri/src/hid.rs` — `process_frame()` → `racing_l2()` branch

### Affected Structs
- `AppState` — fields `l2_resist_slew`, `r2_resist_slew`

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `signal::slew_rate_limit(current, target, max_change)` — existing, no change
- `hid::racing_l2()` — already calls `slew_rate_limit()` for L2 resistance value
- `hid::process_frame()` — already applies `slew_rate_limit()` to R2 throttle resistance

### Affected State Variables
- `AppState.l2_resist_slew` — slew-memory for left trigger, written every frame
- `AppState.r2_resist_slew` — slew-memory for right trigger, written every frame
- `AppState.l2_force` — final L2 force after slew limiting
- `AppState.r2_force` — final R2 force after slew limiting

### Affected Configuration
- `SLEW_MAX_CHANGE` (currently 4) — maximum resistance units per 16ms frame

### Affected Runtime Paths
- Output thread 60 Hz: `process_frame()` → `racing_l2()` → L2 slew rate limit → `haptics_report()`
- Output thread 60 Hz: `process_frame()` → R2 throttle branch → R2 slew rate limit → `haptics_report()`

### Affected Threads
- Output thread (haptic synthesis)

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only

### Validation Strategy
1. In Forza, accelerate, then fully release throttle while airborne over a crest
2. Confirm trigger does not audibly snap or grind during fast resistance transitions
3. Log `l2_resist_slew` and `r2_resist_slew` values frame-by-frame to confirm delta never exceeds `SLEW_MAX_CHANGE`
4. Test rapid ABS pump transitions — confirm no gear clatter during fast mode flips

### Estimated Difficulty
Trivial (verification only)

---

## REQ-03 — EWMA Slip Angle Smoothing

### Affected Modules
`signal`, `forza`

### Affected Files
- `src-tauri/src/signal.rs` — `ewma()`, constant `EWMA_ALPHA`
- `src-tauri/src/forza.rs` — `apply_packet()`

### Affected Structs
None (AppState fields only)

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `signal::ewma(raw, prev, alpha)` — existing, no change
- `forza::apply_packet()` — already calls `ewma()` for slip angle and combined slip

### Affected State Variables
- `AppState.t_ewma_slip_angle` — EWMA state for slip angle, written every valid packet
- `AppState.t_ewma_combined` — EWMA state for combined slip, written every valid packet
- `AppState.t_slip_angle` — output = EWMA state, consumed by `compute_rumble()`
- `AppState.t_slip_combined` — output = EWMA state, consumed by `compute_rumble()` and `racing_l2()`

### Affected Configuration
- `EWMA_ALPHA` (currently 0.1) — smoothing factor

### Affected Runtime Paths
- Forza receiver thread: `apply_packet()` → `ewma()` for slip fields
- Output thread 60 Hz: `process_frame()` → `compute_rumble()` reads EWMA-smoothed slip values

### Affected Threads
- Forza receiver threads
- Output thread

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only

### Validation Strategy
1. Drive a high-horsepower RWD car in Forza at the limit of adhesion
2. Confirm slip vibration intensity changes smoothly frame-to-frame
3. Log `t_ewma_slip_angle` and `t_ewma_combined` to confirm no stair-step patterns
4. If stutter persists, reduce `EWMA_ALPHA` from 0.1 toward 0.05

### Estimated Difficulty
Trivial (verification only — may require alpha tuning)

---

## REQ-04 — Dynamic Load Gating for Airborne False Positives

### Affected Modules
`signal`, `forza`, `hid`

### Affected Files
- `src-tauri/src/signal.rs` — `grip_multiplier()`, `suspension_load_gate()`
- `src-tauri/src/forza.rs` — `apply_packet()`
- `src-tauri/src/hid.rs` — `compute_rumble()`

### Affected Structs
- `AppState` — all `t_*` fields for slip, surface, bump

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `signal::grip_multiplier(heave_accel)` — existing, no change
- `signal::suspension_load_gate(norm_travel, droop, span)` — existing, static inline
- `forza::apply_packet()` — applies `grip_multiplier()` to generate `t_grip_mult`; applies droop gate at end
- `hid::compute_rumble()` — applies per-axle load gates via `load()` closure using `LOAD_GATE_DROOP`/`LOAD_GATE_SPAN`

### Affected State Variables
- `AppState.t_grip_mult` — computed by grip_multiplier, applied to all slip-derived outputs
- `AppState.t_heave` — input to grip_multiplier
- `AppState.t_slip_front/rear/combined/angle` — zeroed when min suspension < 0.05 (droop gate in apply_packet)
- `AppState.t_surface*`, `AppState.t_bump_*` — scaled by haptic_scale (which includes grip_mult)

### Affected Configuration
- `LOAD_GATE_DROOP` (0.05) — threshold below which wheel is considered airborne
- `LOAD_GATE_SPAN` (0.14) — fade-in range for load gate
- `GRAVITY` (9.81) — standard gravity constant

### Affected Runtime Paths
- Forza receiver thread: `apply_packet()` → load gating + grip multiplier
- Output thread 60 Hz: `compute_rumble()` per-axle load gates for wheelspin/lockup/cornering rumble

### Affected Threads
- Forza receiver threads
- Output thread

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only

### Validation Strategy
1. In Forza, drive over a large jump crest
2. Confirm zero haptic vibration during the airborne phase (no false wheelspin buzz)
3. Confirm normal haptic feedback immediately resumes on landing
4. Test with a car that has visible suspension droop (off-road vehicle) — confirm slip feels natural when one wheel lifts on a berm
5. If airborne false positives persist, check that `min_susp < 0.05` gate in `apply_packet()` is being reached

### Estimated Difficulty
Trivial (verification only)

---

## REQ-05 — Hybrid Drivetrain Frequency Crossover

### Affected Modules
`signal`, `hid`, `forza`

### Affected Files
- `src-tauri/src/signal.rs` — `slip_crossover_freq()`, `SLIP_CROSSOVER_RATIO` constant
- `src-tauri/src/hid.rs` — `process_frame()` (R2 throttle wheelspin flutter path)
- `src-tauri/src/forza.rs` — `apply_packet()` (provides `t_slip_rear`)

### Affected Structs
- `AppState` — `t_slip_rear`, `t_slip_rear_frames`

### Affected Enums
None currently — will require a new `DrivetrainType` enum if implementing per-drivetrain tuning

### Affected Traits
None

### Affected Functions
- `signal::slip_crossover_freq(slip_ratio, base_hz, deep_hz)` — existing, currently called with base=8+slip*20 and deep=40. These hardcoded values do not match the report's recommendation of 30-50 Hz deep judder range
- `hid::process_frame()` — the wheelspin flutter branch that calls `slip_crossover_freq()` and sets throttle mode to 0x06

### Affected State Variables
- `AppState.t_slip_rear` — input to crossover decision
- `AppState.t_slip_rear_frames` — deadzone counter (read before flutter activates)

### Affected Configuration
- `SLIP_CROSSOVER_RATIO` — no direct tuning constant exists; the crossover is implicit in `slip_crossover_freq(base_hz, deep_hz)` call arguments

### Affected Runtime Paths
- Output thread 60 Hz: `process_frame()` → R2 throttle section → `if t_slip_rear > 0.20 && t_slip_rear_frames >= 2` → `slip_crossover_freq()` → 0x06 mode with freq/amp

### Affected Threads
- Output thread
- Forza receiver threads (provide input data)

### Implementation Order
2nd — partially implemented; needs parameter tuning and drivetrain-awareness

### Dependencies
- **REQ-15** — drivetrain-specific tuning profiles provide the mechanism to select different crossover parameters per vehicle type
- Without REQ-15, must apply a single tuned crossover globally (may improve hybrid feel but risk degrading mechanical AWD)

### Migration Risks
**Medium.** Changing the crossover deep frequency from 40 Hz to the 30-50 Hz range affects ALL vehicles universally. Without drivetrain-type differentiation, mechanical AWD vehicles that currently feel acceptable could degrade. Recommend making crossover parameters tunable constants first, then wiring them to drivetrain profiles when REQ-15 is implemented.

### Validation Strategy
1. Drive Ferrari SF90 Stradale (or equivalent R-class hybrid) in Forza at the grip limit
2. Confirm traction-loss vibration transitions from high-freq flutter to low-freq deep judder when slip exceeds ~1.0
3. Confirm no mechanical trigger rattle during eTC intervention oscillations
4. Drive Lamborghini Centenario (mechanical AWD) and confirm no regression in slip feel
5. A/B test: disable crossover (both regimes use the same frequency) → confirm harsh buzz returns for hybrid

### Estimated Difficulty
**Medium.** Core DSP function exists; parameter tuning requires iterative feel testing. Global parameter change risks regressing non-hybrid vehicles.

---

## REQ-06 — Slip-Threshold Transient Deadzone

### Affected Modules
`hid`

### Affected Files
- `src-tauri/src/hid.rs` — `process_frame()` slip-duration counters
- `src-tauri/src/signal.rs` — `SLIP_DEADZONE_FRAMES` constant

### Affected Structs
- `AppState` — `t_slip_rear_frames`, `t_slip_front_frames`

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `hid::process_frame()` — slip-duration counter increment/decrement logic in the Racing engine-phase section

### Affected State Variables
- `AppState.t_slip_rear_frames` — u16 counter, incremented when `t_slip_rear > 0.20`, reset to 0 otherwise
- `AppState.t_slip_front_frames` — u16 counter, incremented when `t_slip_front > 0.20`, reset to 0 otherwise

### Affected Configuration
- `SLIP_DEADZONE_FRAMES` (currently 2) — at 60 Hz ≈ 33 ms deadzone (inside recommended 25-30 ms range)

### Affected Runtime Paths
- Output thread 60 Hz: `process_frame()` → `if t_on { if t_slip_rear > 0.20 { counter++ } else { counter = 0 } }`
- Output thread 60 Hz: `process_frame()` → R2 throttle section → `if t_slip_rear_frames >= SLIP_DEADZONE_FRAMES` before firing flutter

### Affected Threads
- Output thread

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only. Minor risk if `SLIP_DEADZONE_FRAMES` is increased beyond 2: genuine short-duration traction loss events (e.g., quick lift-off oversteer) could be silenced.

### Validation Strategy
1. In Forza, drive a hybrid supercar and stab the throttle hard while cornering
2. Confirm eTC micro-slips (sub-30ms) do not trigger throttle flutter or trigger rattle
3. Confirm sustained power oversteer (slip lasting >30ms) does trigger full flutter
4. Log `t_slip_rear_frames` counter — verify it resets to 0 within 1-2 frames after slip drops below threshold

### Estimated Difficulty
Trivial (verification only)

---

## REQ-07 — Traction Control Intervention Haptic Attenuation

### Affected Modules
`forza`, `signal`

### Affected Files
- `src-tauri/src/forza.rs` — `apply_packet()` (TC detection + scaling)
- `src-tauri/src/signal.rs` — `TC_HAPTIC_SCALE` constant

### Affected Structs
- `AppState` — `t_tc_active`, `t_slip_rear`, `t_accel_input`, `t_accel`

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `forza::apply_packet()` — sets `t_tc_active` based on heuristic: rear slip > 0.35, throttle > 180, longitudinal G < 2.0; then applies `haptic_scale = t_grip_mult * (TC_HAPTIC_SCALE if active else 1.0)` to all slip/surface/bump fields

### Affected State Variables (written)
- `AppState.t_tc_active` — boolean, set per valid packet
- `AppState.t_slip_front`, `t_slip_rear`, `t_slip_combined`, `t_slip_angle` — all multiplied by `haptic_scale`
- `AppState.t_surface`, `t_surface_fl/fr/rl/rr` — all multiplied by `haptic_scale`
- `AppState.t_bump_left`, `t_bump_right` — multiplied by `haptic_scale`

### Affected Configuration
- `TC_HAPTIC_SCALE` (currently 0.5) — matches report recommendation exactly

### Affected Runtime Paths
- Forza receiver thread: `apply_packet()` → TC detection → haptic scaling

### Affected Threads
- Forza receiver threads

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
Low — the TC detection heuristic (`s.t_slip_rear > 0.35 && s.t_accel_input > 180 && s.t_accel < 2.0`) may produce false positives on vehicles without TC (e.g., classic cars with high rear slip from pure mechanical wheelspin) or false negatives on vehicles with very aggressive early-intervention TC. The attenuation at 50% is a global scalar — fine for the report's recommendation.

### Validation Strategy
1. Drive SF90 Stradale in Forza, corner hard enough to trigger eTC intervention
2. Confirm haptic intensity drops perceptibly during intervention
3. Confirm haptic intensity returns to full immediately after intervention ends
4. Drive a non-TC classic car and confirm `t_tc_active` is not spuriously true

### Estimated Difficulty
Trivial (verification only)

---

## REQ-08 — Pacejka-Based Slip-to-Haptic Intensity Mapping

### Affected Modules
`signal`, `hid`

### Affected Files
- `src-tauri/src/signal.rs` — `pacejka_force()`, `pacejka_haptic()`
- `src-tauri/src/hid.rs` — `compute_rumble()`

### Affected Structs
None (AppState fields only)

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `signal::pacejka_force(slip, B, C, D, E)` — existing, no change
- `signal::pacejka_haptic(slip)` — existing, calls `pacejka_force` with B=10, C=1.9, D=1.0, E=0.97
- `hid::compute_rumble()` — calls `pacejka_haptic(t_slip_rear)` for wheelspin rumble and `pacejka_haptic(t_slip_front)` for lockup rumble

### Affected State Variables
- `AppState.t_slip_rear` — input to wheelspin Pacejka mapping
- `AppState.t_slip_front` — input to lockup Pacejka mapping

### Affected Configuration
- Pacejka parameters (B=10, C=1.9, D=1.0, E=0.97) — hardcoded in `pacejka_haptic()`

### Affected Runtime Paths
- Output thread 60 Hz: `compute_rumble()` → wheelspin section → `pacejka_haptic(s.t_slip_rear) * spin * 240.0`
- Output thread 60 Hz: `compute_rumble()` → lockup section → `pacejka_haptic(s.t_slip_front) * lock * 220.0`

### Affected Threads
- Output thread

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only

### Validation Strategy
1. In Forza, progressively increase throttle from standstill to wheelspin
2. Confirm rumble intensity follows a non-linear curve (grows slowly initially, peaks, then possibly declines slightly at deep slip)
3. Compare against a linear-scaled reference to confirm Pacejka shaping is active
4. Confirm micro-slip (~5-20%) produces subtle rumble versus the strong output at deep slip (>100%)

### Estimated Difficulty
Trivial (verification only)

---

## REQ-09 — Pneumatic Trail Collapse Simulation on Brake Trigger

### Affected Modules
`signal`, `hid`

### Affected Files
- `src-tauri/src/signal.rs` — `pneumatic_trail_decay()`, `PNEUMATIC_DECAY` constant
- `src-tauri/src/hid.rs` — `racing_l2()`

### Affected Structs
- `AppState` — `l2_trail_resist`, `t_slip_combined`, `t_slip_front`

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `signal::pneumatic_trail_decay(current, decay)` — existing, exponential decay toward zero
- `hid::racing_l2()` — applies trail decay when `t_slip_combined > 0.75 && t_slip_front > 0.75`

### Affected State Variables
- `AppState.l2_trail_resist` — trailing resistance value, decays during lockup, restored from brake curve otherwise
- `AppState.t_slip_combined` — input gate for decay activation
- `AppState.t_slip_front` — input gate for decay activation

### Affected Configuration
- `PNEUMATIC_DECAY` (currently 0.1) — decay factor per frame

### Affected Runtime Paths
- Output thread 60 Hz: `process_frame()` → `racing_l2()` → pneumatic trail decay branch

### Affected Threads
- Output thread

### Implementation Order
8th (existing implementation — verification-only)

### Dependencies
None—already implemented

### Migration Risks
None—verification only. Minor risk if decay factor is too aggressive (trigger goes limp too quickly) or too gentle (lockup feel is delayed).

### Validation Strategy
1. In Forza, brake hard enough to lock the front tires (slip > 0.75 sustained)
2. Confirm brake trigger resistance smoothly decays to near-zero over several frames
3. Confirm the decay feels like a smooth collapse, not an instantaneous drop
4. Confirm trigger fully disengages (mode 0x05) when trail resistance drops below 8
5. Confirm normal resistance returns immediately when lockup ends

### Estimated Difficulty
Trivial (verification only)

---

## REQ-10 — Aerodynamic Downforce Dynamic Brake Stiffness

### Affected Modules
`hid`, `forza`

### Affected Files
- `src-tauri/src/hid.rs` — `racing_l2()`, `process_frame()`, `compute_rumble()`
- `src-tauri/src/forza.rs` — `apply_packet()` (provides `t_speed`)

### Affected Structs
- `AppState` — `t_speed` (existing, provides vehicle speed in m/s), `l2_force` (existing, written by process_frame)

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `hid::racing_l2()` — must multiply or add speed-derived factor to the final brake resistance
- `hid::process_frame()` — passes context to `racing_l2()`
- `hid::compute_rumble()` — may need speed-awareness for consistency
- `forza::apply_packet()` — already writes `t_speed` (no change needed)

### Affected State Variables
- `AppState.t_speed` — m/s, the input for aero scaling (from Forza telemetry)
- `AppState.l2_force` — final L2 force (output, modified by aero scaling)
- `AppState.l2_resist_slew` — slew limiter memory (must handle smooth speed-driven changes)

### Affected Configuration
- **New constant needed:** Aero scaling coefficient or speed-to-resistance mapping (e.g., resistance increase per 10 m/s)
- **New constant needed:** Maximum aero contribution ceiling to prevent absurd resistance at 400 km/h
- May consider making it a tunable parameter in `RacingTuning` or a new field in `DrivetrainFeel`

### Affected Runtime Paths
- Output thread 60 Hz: `process_frame()` → `racing_l2()` → brake curve with aero scaling → slew rate limit → `haptics_report()`
- Also affects the inferred model path (no Forza telemetry connected): must gracefully degrade — possibly using `engine_rpm` as a weak proxy, or simply disable aero scaling when telemetry is inactive

### Affected Threads
- Output thread
- Forza receiver threads (provide speed data)

### Implementation Order
1st — high-impact feel improvement, no dependencies on other unimplemented items

### Dependencies
- Requires `t_speed` to be populated (Forza telemetry must be active)
- Must have a fallback when no telemetry (`t_on == false`): apply no aero scaling or use a simulated speed proxy
- Should not conflict with existing pneumatic trail decay (they share the L2 resistance path)
- Should not conflict with pedal wall or brake curve zones

### Migration Risks
**Medium.** Adding speed-dependent scaling to the brake resistance curve changes the entire feel of braking. The scaling must be:
- Additive on top of existing brake curve (not a replacement)
- Smooth — no sudden jumps when crossing speed thresholds
- Bounded — must not exceed 255 at any speed
- Must not make the L2 trigger harder to pull at low speed (regression)

The inferred model path (no Forza telemetry) has no `t_speed` — the function must gracefully degrade with `t_on == false`.

### Validation Strategy
1. In Forza, drive at low speed (~30 km/h) and brake — confirm feel is unchanged from current
2. Accelerate to high speed (~250+ km/h) and brake — confirm brake trigger feels progressively firmer
3. Perform a sustained braking event from high speed to low speed — confirm resistance smoothly reduces as speed drops
4. Test with no Forza telemetry (Racing profile, no Data Out) — confirm braking feels normal (no error, no crash)
5. At extreme speeds (350+ km/h), confirm resistance is clamped and does not cause trigger motor stall

### Estimated Difficulty
**Medium.** Requires new scaling function, new constant(s), graceful fallback for no-telemetry path, and extensive feel testing at multiple speed ranges.

---

## REQ-11 — Haptic Hierarchical Layering with Dynamic Dominance

### Affected Modules
`hid`

### Affected Files
- `src-tauri/src/hid.rs` — `compute_rumble()`

### Affected Structs
- `AppState` — multiple `t_*` fields, `engine_rpm`, `engine_phase`, `road_phase`

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `hid::compute_rumble()` — already implements layering logic:
  - `ambient_scale = if lateral_critical { 0.0 } else if t_on { AMBIENT_RPM_CLAMP } else { 1.0 }`
  - `surface_scale = if lateral_critical { 0.0 } else { 1.0 }`
  - Engine rumble multiplied by `ambient_scale`
  - Surface texture gated by `surface_scale > 0.0`
  - Stereophonic road surface routing already exists

### Affected State Variables
- `AppState.t_slip_angle` — checked against `LATERAL_SLIP_CRITICAL` for dominance
- `AppState.engine_rpm` — drives Layer 1 ambient
- `AppState.t_surface_fl/fr/rl/rr` — drives Layer 2 stereophonic texture
- Multiple slip, surface, and bump fields

### Affected Configuration
- `AMBIENT_RPM_CLAMP` (currently 0.05) — 5% ceiling for ambient RPM rumble (matches report recommendation of 5-10%)
- `LATERAL_SLIP_CRITICAL` (currently 0.12) — threshold for Layer 3 dominance

### Affected Runtime Paths
- Output thread 60 Hz: `compute_rumble()` per-frame rumble synthesis

### Affected Threads
- Output thread

### Implementation Order
8th (substantially implemented — may benefit from verification and minor tuning)

### Dependencies
None—already substantially implemented. The layering architecture matches the report's three-layer model. If anything, the `AMBIENT_RPM_CLAMP` could be verified and the `LATERAL_SLIP_CRITICAL` threshold could be calibrated.

### Migration Risks
Low — if `LATERAL_SLIP_CRITICAL` is tuned too low, ambient rumble ducks in light cornering (annoying). If too high, slip cues are masked. Current value of 0.12 is a reasonable starting point.

### Validation Strategy
1. Drive in Forza at steady cruise — confirm gentle engine RPM rumble is present but subtle
2. Enter a hard corner and induce understeer — confirm surface texture and engine rumble duck to near-zero, only slip vibration remains
3. Exit the corner — confirm ambient layers fade back in smoothly
4. Drive on a rough surface (gravel/dirt) — confirm left/right stereo separation produces distinct per-side texture
5. If slip cues feel masked by ambient noise, lower `AMBIENT_RPM_CLAMP` from 0.05 to 0.03

### Estimated Difficulty
Trivial to Low (verification + possible minor constant tuning)

---

## REQ-12 — Rev-Limiter Bounce Algorithm

### Affected Modules
`hid`

### Affected Files
- `src-tauri/src/hid.rs` — `compute_rumble()` (Racing section)

### Affected Structs
- `AppState` — `t_rpm` (normalized 0..1), `engine_phase`

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `hid::compute_rumble()` — the Racing section where real-telemetry road feel is processed; specifically the existing redline upshift cue block

### Affected State Variables
- `AppState.t_rpm` — normalized engine RPM (0..1), used to detect max RPM condition
- `AppState.engine_phase` — may be used or a new dedicated phase counter for the bounce cadence

### Affected Configuration
- **New constant needed:** `REVLIM_BOUNCE_THRESHOLD` — RPM value at which bounce engages (e.g., 0.99 or 0.995)
- **New constant needed:** `REVLIM_BOUNCE_HZ` — bounce cadence frequency (e.g., 4-6 Hz for the violent limiter feel)
- **New constant needed:** `REVLIM_BOUNCE_AMPLITUDE` — intensity of the bounce pulse in rumble units

### Affected Runtime Paths
- Output thread 60 Hz: `compute_rumble()` → after existing redline cue block → new rev-limiter bounce block

The existing redline cue (`t_rpm > 0.85`) produces progressive rising flutter on the right motor. The new bounce should fire ONLY at `t_rpm >= REVLIM_BOUNCE_THRESHOLD` and produce a rhythmic on/off pulse pattern clearly distinguishable from the progressive flutter.

### Affected Threads
- Output thread

### Implementation Order
4th — independent of other items, good feel improvement

### Dependencies
- Requires `t_rpm` from Forza telemetry (`t_on == true`)
- Can also work with simulated engine model `engine_rpm` when no telemetry (must reach ~1.0 at full throttle)
- Must NOT conflict with or mask the existing progressive redline cue — they should layer: progressive flutter from 0.85 to 0.99, then add rhythmic bounce at ≥0.99
- Must NOT conflict with shift detection — if a shift fires right at the bounce threshold, the shift clunk should take priority

### Migration Risks
**Low.** The bounce is additive to existing rumble, not replacing it. Risk is:
- Bounce too subtle → not noticed
- Bounce too strong → masks other cues (shift clunk, slip vibration)
- Bounce cadence too fast → feels like buzz (defeats purpose)

### Validation Strategy
1. In Forza, hold a gear until the engine hits the rev limiter (do not shift)
2. Confirm a distinct rhythmic bounce pattern is felt, clearly different from the progressive redline-approach flutter
3. Confirm the bounce ceases within one frame of RPM dropping below threshold (e.g., after upshift)
4. Drive normally with shifts before redline — confirm bounce never fires spuriously
5. Test with simulated engine model (no Forza telemetry) — confirm `engine_rpm` reaching ~1.0 also triggers bounce

### Estimated Difficulty
**Low.** New rumble block in `compute_rumble()`, 3 new constants, uses existing `t_rpm`/`engine_rpm`. No new structs, no new threads, no persistence changes.

---

## REQ-13 — HID Output State Delta Checking

### Affected Modules
`hid`

### Affected Files
- `src-tauri/src/hid.rs` — `hid_loop()`, `process_frame()`, `write_report()`

### Affected Structs
- `AppState` — source of haptic parameters (no changes needed)

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `hid::hid_loop()` — the 60 Hz output loop where `process_frame()` is called and `write_report()` is invoked
- `hid::process_frame()` — generates the 48-byte report (no signature change needed)
- `hid::write_report()` — no change (still writes when called)

### Affected State Variables
None in AppState. Requires a **new local variable** in `hid_loop()` — a cached copy of the last-sent `[u8; 48]` report.

### Affected Configuration
None

### Affected Runtime Paths
- Output thread 60 Hz: `hid_loop()` → compare new report against cached → if equal, skip `write_report()` → if different, write and update cache
- The comparison must include:
  - Trigger modes and parameters (bytes 11-14 for R2, bytes 22-25 for L2)
  - Rumble motor levels (bytes 3-4)
  - Lightbar RGB (bytes 45-47)
  - Player LED pattern (byte 44)
  - Valid flags (bytes 1-2)
- Bytes that are always zero or static can be excluded from comparison

### Affected Threads
- Output thread only

### Implementation Order
6th — independent, low-impact optimization

### Dependencies
None—completely self-contained

### Migration Risks
**Low.** The delta check is an optimization, not a functional change. Risks:
- **Incomplete comparison:** If any field that affects actuator behavior is omitted from the comparison, the controller may miss an update (stale haptics for one frame). At 60 Hz, one missed frame is imperceptible.
- **Report format divergence:** USB reports are 48 bytes, BT reports are 78 bytes after `to_bt_report()`. The cache must be of USB-format reports (48 bytes) and the comparison happens before the BT conversion.
- **Lightbar/profile changes:** If `lightbar_report()` or `player_led_report()` are written separately from `process_frame()` output, the delta check must not suppress those writes.

### Validation Strategy
1. Run the app with a DualSense connected via USB
2. Leave controller idle (no input changes) for 5 seconds — confirm no HID writes occur (no haptic heartbeat)
3. Press a trigger — confirm haptic output activates within one frame
4. Switch profiles via the frontend — confirm lightbar and haptics update immediately
5. Test Bluetooth connection — confirm BT sequence number + CRC still generated correctly, report still transmitted when needed
6. Long-duration test: play Forza for 30+ minutes — confirm no haptic freeze or stuck states

### Estimated Difficulty
**Low.** Requires a 48-byte cache variable and a comparison before calling `write_report()`. Likely 20-30 lines of additional code. Must handle edge cases: BT transport, independent lightbar writes, first frame after profile switch.

---

## REQ-14 — HID Device Signal Isolation from Game/Steam

### Affected Modules
`hidhide`, `xinput`, `lib`

### Affected Files
- `src-tauri/src/hidhide.rs` — `enable()`, `disable()`, `cli_path()`, `parse_sony_instances()`
- `src-tauri/src/hid.rs` — `forward_to_xbox()` (calls `hidhide::enable/disable`)
- `src-tauri/src/xinput.rs` — `XBridge` (creates virtual pad, subscribes to game rumble)

### Affected Structs
- `XBridge` (xinput.rs) — the virtual Xbox pad
- `AppState` — `error_msg` (receives HidHide error messages)

### Affected Enums
None

### Affected Traits
None

### Affected Functions
- `hidhide::enable()` — whitelists app, hides Sony devices, turns cloaking on
- `hidhide::disable()` — unhides all, turns cloaking off
- `hid::forward_to_xbox()` — calls `hidhide::enable()` on Xbox mode entry, `hidhide::disable()` on exit
- `hidhide::cli_path()` — searches for HidHideCLI.exe at known paths
- `hidhide::parse_sony_instances()` — extracts VID_054C device paths from stdout

### Affected State Variables
- `AppState.error_msg` — populated with error string if enable/disable fails
- `AppState.output_mode` — Xbox mode triggers enable; DualSense mode triggers disable

### Affected Configuration
None (no config constants — relies on installed HidHide CLI paths)

### Affected Runtime Paths
- Input thread: `parse_input_report()` → `forward_to_xbox()` → `hidhide::enable()` on first Xbox frame / `disable()` on mode switch
- Virtual pad: ViGEmBus → game reads Xbox pad → DualSense hidden from game

### Affected Threads
- Input thread (calls `forward_to_xbox()`, which manages HidHide)

### Implementation Order
7th — partially implemented on Windows; no path for other platforms

### Dependencies
- **ViGEmBus must be installed** (pre-requisite for Xbox output mode)
- **HidHide must be installed** with the CLI at one of the known paths
- **Admin elevation may be required** for HidHideCLI to modify the cloaking driver
- The report also mentions kernel-level filter drivers as an alternative — not planned here

### Migration Risks
**Medium.** The current implementation:
- Is Windows-only (no equivalent on macOS/Linux)
- May fail if HidHide is not installed (graceful degradation — error message shown, virtual pad still works, but game may see both controllers)
- May fail if app path changes (must re-whitelist)
- Shells out to a CLI tool — fragile across HidHide versions (CLI output format changes)

### Validation Strategy
1. On Windows with HidHide installed, enable Xbox output mode
2. Confirm `hidhide::enable()` succeeds — no error message in UI
3. Launch Forza Horizon — confirm game detects the virtual Xbox pad
4. Confirm Forza does NOT detect the physical DualSense (no double input)
5. Switch back to DualSense output mode — confirm `hidhide::disable()` succeeds
6. Test with HidHide not installed — confirm graceful degradation (error message, virtual pad still available)
7. Test across HidHide CLI versions — confirm `parse_sony_instances()` correctly extracts device paths

### Estimated Difficulty
**Low** (verification and hardening of existing implementation — no new code architecture needed). Cross-platform extension (macOS/Linux) would be **High** difficulty due to lack of equivalent kernel masking APIs.

---

## REQ-15 — Drivetrain-Specific Tuning Profiles

### Affected Modules
`hid`, `signal`, `forza`, `lib`, `settings`

### Affected Files
- `src-tauri/src/hid.rs` — `AppState`, `process_frame()`, `compute_rumble()`, `racing_l2()`, new `DrivetrainProfile` enum or struct
- `src-tauri/src/signal.rs` — DSP constants that become per-drivetrain-configurable
- `src-tauri/src/forza.rs` — `apply_packet()` (may need additional telemetry parsing for drivetrain heuristics, or no change if user-selects)
- `src-tauri/src/lib.rs` — new Tauri command `set_drivetrain_profile()`, registration in `invoke_handler`
- `src-tauri/src/settings.rs` — `SavedSettings` needs a `drivetrain_profile` field, `RacingCurve` may need per-drivetrain extension

### Affected Structs
- **New struct:** `DrivetrainProfile` — holds per-type tuning parameters:
  - `slip_deadzone_frames: u16` — transient deadzone duration
  - `crossover_base_hz: f32` — base flutter freq before crossover
  - `crossover_deep_hz: f32` — deep judder freq after crossover
  - `ewma_alpha: f32` — slip smoothing factor
  - `tc_attenuation: f32` — TC intervention scale factor
  - `label: &'static str` — display name
- **New enum or const array:** `DRIVETRAIN_PROFILES` — a static table of predefined profiles (e.g., "Mechanical AWD", "Hybrid Electric", "RWD", "FWD", "Default")
- `AppState` — new field `drivetrain_profile_idx: usize` or `drivetrain_profile: DrivetrainType`
- `SavedSettings` — new field `drivetrain_profile: Option<usize>`

### Affected Enums
- **New enum:** `DrivetrainType` — variants for MechanicalAwd, HybridElectric, Rwd, Fwd, Default; or use a simple index into `DRIVETRAIN_PROFILES`

### Affected Traits
None

### Affected Functions
- `hid::process_frame()` — must select active `DrivetrainProfile` based on `AppState.drivetrain_profile_idx` and pass parameters to DSP calls
- `hid::compute_rumble()` — must use per-profile `ewma_alpha` (currently hardcoded via signal constants) — this is tricky since EWMA is applied in `forza.rs::apply_packet()`, not in `compute_rumble()`. Either:
  - Move EWMA application into `compute_rumble()` (architectural change), or
  - Pass the active profile parameters into `apply_packet()` (requires sharing profile index with Forza thread), or
  - Apply per-profile attenuation on top of the existing EWMA-smoothed values in `compute_rumble()` (simplest, least risky)
- `forza::apply_packet()` — if EWMA alpha becomes per-profile, this function needs access to the active profile. Currently it takes `&mut AppState` — could read `t_drivetrain_profile_idx` from state, but this creates a dependency on the profile being set before packets arrive. Simplest approach: keep EWMA fixed in `apply_packet()`, apply per-profile shaping in `compute_rumble()`.
- `lib::*` — new Tauri command `set_drivetrain_profile(idx: usize)`, persisted via `settings::save()`
- `settings::load()` / `settings::save()` — new field in `SavedSettings`

### Affected State Variables
- **New:** `AppState.drivetrain_profile_idx: usize`
- **Existing, affected:** `AppState.t_slip_rear_frames` (deadzone check uses `SLIP_DEADZONE_FRAMES`), `AppState.t_ewma_*` (smoothing applied in forza.rs), throttle flutter freq/amp in `process_frame()`

### Affected Configuration
- Current `SLIP_DEADZONE_FRAMES` becomes a per-profile parameter
- Current `EWMA_ALPHA` may become per-profile
- Current `TC_HAPTIC_SCALE` may become per-profile
- Current crossover parameters (hardcoded in `process_frame()` R2 section) become per-profile
- `SLIP_CROSSOVER_RATIO` may become per-profile

### Affected Runtime Paths
- Output thread 60 Hz: `process_frame()` → select profile → use profile params for deadzone/crossover/attenuation
- Output thread 60 Hz: `compute_rumble()` → use profile params for rumble intensity shaping
- Tauri command: frontend calls `set_drivetrain_profile()` → `AppState.drivetrain_profile_idx` updated → `settings::save()` → next frame uses new profile
- Startup: `settings::load()` → `AppState.drivetrain_profile_idx` restored

### Affected Threads
- Output thread (uses profile parameters)
- Tauri command handler threads (write profile index)

### Implementation Order
5th — largest scope, depends on REQ-05 and REQ-06 being tuned per-profile

### Dependencies
- **REQ-05** (frequency crossover) and **REQ-06** (slip deadzone) — these DSP parameters are what differ between profiles
- **REQ-01** (suspension LPF) — could also be per-profile but not essential for initial implementation
- **Settings persistence** — must store profile selection across launches
- **Frontend** — needs a UI control to select drivetrain profile (e.g., dropdown in Racing settings)
- **Tauri IPC** — new command handler

### Migration Risks
**High.** This is the largest-scope new feature. Key risks:
- **No automatic drivetrain detection:** Forza telemetry does not expose drivetrain type. The user must manually select. Wrong profile → degraded experience.
- **DSP parameter proliferation:** Converting hardcoded constants to per-profile lookups touches multiple functions in `hid.rs`, `signal.rs`, and potentially `forza.rs`. Each touch point is a regression risk.
- **EWMA alpha lives in `forza.rs`:** The EWMA smoothing is applied in the Forza receiver thread, not the output thread. Making alpha per-profile requires either passing profile context across thread boundaries or duplicating/restructuring the smoothing.
- **Profile selection at startup:** If the saved profile index is invalid (e.g., profile list changed between versions), must fall back to "Default" gracefully.
- **Forward compatibility:** Adding new profile parameters later (e.g., per-profile `LOAD_GATE_DROOP`) requires versioning the profile struct or using default values.

**Recommended mitigation:** Start with a minimal `DrivetrainProfile` that only parameterizes `slip_deadzone_frames`, `crossover_deep_hz`, and `tc_attenuation`. Apply these in `compute_rumble()` and `process_frame()` only — do not touch `forza.rs` EWMA. Expand later as needed.

### Validation Strategy
1. With "Default" profile selected, confirm all Racing haptics are indistinguishable from current behavior (no regression)
2. Select "Hybrid Electric" profile, drive SF90 Stradale in Forza — confirm reduced trigger clatter, deep judder instead of buzz, TC attenuation active
3. Select "Mechanical AWD" profile, drive Lamborghini Centenario — confirm progressive slip feel, no harsh crossover artifacts
4. Switch profiles while driving — confirm change takes effect immediately (next frame)
5. Restart the app — confirm saved profile is restored
6. Edit `settings.json` to set an invalid profile index — confirm app falls back to "Default" without crashing
7. Drive each drivetrain archetype with its matching AND non-matching profile — confirm each profile produces distinctly different, appropriate feel

### Estimated Difficulty
**High.** Touches 5+ modules, requires new struct, new enum, new Tauri command, new settings field, DSP parameter routing changes, frontend coordination. Approximately 200-400 lines of new/modified code across the stack.

---

## Implementation Order

### Phase 1 — Verification and Tuning (Trivial)

These items are already implemented. Verify they work as described, tune constants if needed.

| Order | ID | Item | Effort |
|-------|----|------|--------|
| 1 | REQ-01 | Suspension LPF verification | Trivial |
| 2 | REQ-02 | Slew rate limiter verification | Trivial |
| 3 | REQ-03 | EWMA slip smoothing verification | Trivial |
| 4 | REQ-04 | Load gating verification | Trivial |
| 5 | REQ-06 | Slip deadzone verification | Trivial |
| 6 | REQ-07 | TC attenuation verification | Trivial |
| 7 | REQ-08 | Pacejka mapping verification | Trivial |
| 8 | REQ-09 | Pneumatic trail collapse verification | Trivial |
| 9 | REQ-11 | Hierarchical layering verification | Trivial to Low |

### Phase 2 — Independent New Features (Low to Medium)

These items have no dependencies on other unimplemented items and can be built independently.

| Order | ID | Item | Effort |
|-------|----|------|--------|
| 10 | REQ-10 | Aero downforce brake stiffness | Medium |
| 11 | REQ-12 | Rev-limiter bounce algorithm | Low |
| 12 | REQ-13 | HID state delta checking | Low |
| 13 | REQ-14 | HID signal isolation hardening | Low |

### Phase 3 — Architecture-Dependent Features (Medium to High)

These items depend on or enable each other and require coordinated implementation.

| Order | ID | Item | Effort |
|-------|----|------|--------|
| 14 | REQ-05 | Hybrid frequency crossover tuning | Medium |
| 15 | REQ-15 | Drivetrain-specific tuning profiles | High |

**Rationale:** REQ-05 (crossover tuning) is partially implemented but its full value is unlocked by REQ-15 (drivetrain profiles). Implementing REQ-15 first establishes the framework; REQ-05 becomes a matter of populating profile parameters.

---

## Dependency Graph

```
REQ-10 ─── independent
REQ-12 ─── independent
REQ-13 ─── independent
REQ-14 ─── independent

REQ-05 ─── needs REQ-15 for full value (or applies global tuning without profiles)
REQ-15 ─── benefits from REQ-05 + REQ-06 parameter definitions
          ─── needs new UI (frontend coordination)
          ─── needs settings persistence
          ─── benefits from REQ-01/REQ-02/REQ-03 verification first (baseline stability)

REQ-01-09,11 ─── already implemented (verification only)
```

---

## Migration Risks Summary

| Risk | Severity | Affected Items | Mitigation |
|------|----------|---------------|------------|
| **Aero scaling regresses low-speed braking** | Medium | REQ-10 | Bounded scaling, only active at speed, fallback to current curve at low speed |
| **Aero scaling inoperative without telemetry** | Medium | REQ-10 | Graceful fallback — no scaling when `t_on == false` |
| **Global crossover tuning regresses mechanical AWD feel** | Medium | REQ-05 | Make tunable first; defer per-profile until REQ-15 |
| **Drivetrain profile struct backwards compatibility** | Medium | REQ-15 | Use serde default attributes; fallback to Default profile |
| **Invalid saved profile index** | Low | REQ-15 | Clamp to valid range on load; Default profile = index 0 |
| **HID delta check suppresses valid lightbar updates** | Low | REQ-13 | Exclude independent lightbar writes from delta check |
| **HidHide CLI version incompatibility** | Medium | REQ-14 | Graceful degradation; fallback message to user |
| **Cross-platform signal isolation gap** | Medium | REQ-14 | Document as Windows-only; no macOS/Linux path |

---

## Validation Strategy Summary

### Per-requirement validation: See individual sections above.

### Integration validation (after Phase 3):

1. **Full-Stack Test:** Launch app, connect DualSense, enable Xbox mode + HidHide, launch Forza Horizon, enable Data Out on port 7000, select "Hybrid Electric" drivetrain profile, drive SF90 Stradale at the limit for 10 minutes.
2. **Regression Test:** Toggle every profile (Racing, Static, Gun, Melee, Audio, Minecraft). Confirm all profiles function normally.
3. **Disconnect Test:** Unplug controller mid-race, confirm graceful degradation. Reconnect, confirm haptics resume.
4. **No-Telemetry Test:** Disable Data Out in Forza. Confirm Racing profile falls back to inferred engine model with full aero scaling disabled.
5. **Long-Duration Test:** Leave app running with Forza telemetry active for 60+ minutes. Confirm no memory leaks, no haptic degradation, no buffer overflow.
6. **Bluetooth Test:** Repeat all tests over Bluetooth. Confirm CRC-32 reports are generated correctly, no dropped packets, delta checking works with BT transport.

---

## Cross-Cutting Concerns

### Frontend Coordination Required For:
- **REQ-15:** New dropdown or selector in Racing settings for drivetrain profile selection. Must call the new `set_drivetrain_profile` Tauri command.
- **REQ-10:** Optional — a slider for aero scaling intensity could be exposed in the Racing Lab. Not required for initial implementation but adds tunability.

### No New Threads Required.
All changes are within existing threads: Forza receiver threads, output thread, input thread, Tauri command handlers.

### No New External Crates Required.
All implementation uses existing dependencies: `signal`, `hidapi`, `serde`, `std`.

### No New Files Required.
All changes fit within existing module files. A new `drivetrain.rs` module would be architecturally cleaner for REQ-15 but is optional — the profile struct can live in `hid.rs` alongside `DrivetrainFeel` and `RacingTuning`.

---

## How the Application Operates: End-to-End

At startup, `main.rs` calls `lib::run()`, creating `AppState` with default settings and loading persisted preferences from `settings.json` and feel tuning from `feels.json`. The Tauri shell registers ~30 command handlers and, in debug builds, immediately starts the HID threads. In release builds, the frontend must call `init_session` to validate the license before haptics begin. The Forza UDP bridge spawns receiver threads on ports 5300, 7000, and 20066, plus a watchdog. The Minecraft TCP bridge binds to localhost:27812.

Once the HID threads are active, the input thread opens the DualSense via hidapi and reads reports at ~500 Hz, writing raw sticks, triggers, buttons, gyro, and touchpad data into `AppState`. The output thread opens the device for writing and runs a 60 Hz loop: each frame it locks `AppState`, runs `process_frame()` which synthesizes a haptic report for the active profile (Racing with real or simulated telemetry, Static, Gun with semi/burst/auto patterns, Melee with swing heft, Audio with two-band reactive rumble or true USB audio haptics, or Minecraft with per-item feel), computes per-profile rumble via `compute_rumble()`, blends game rumble passthrough if in Xbox mode, and writes the final 48-byte (USB) or 78-byte (Bluetooth with CRC-32) HID output report to the controller. The UI emitter pushes state snapshots at ~30 Hz.

On Windows in Xbox output mode, the input loop also forwards DualSense inputs to a ViGEmBus virtual Xbox 360 pad, and HidHide cloaks the physical controller from games. Game rumble flows back through ViGEm's notification thread into `AppState` for passthrough enrichment.

Independently, the Forza bridge parses real RPM, acceleration, tire slip, suspension, surface rumble, gear, and pedal data from Forza Data Out packets and writes signal-processed values (LPF, EWMA, grip multiplier, load gates, TC attenuation) into `AppState.t_*` fields. The Racing profile uses this real data instead of the inferred engine model when telemetry is active. The Minecraft bridge writes game state from the Fabric mod into `AppState.mc_*` fields.

The application runs until process termination with no graceful shutdown.
