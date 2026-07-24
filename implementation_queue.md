# Master Implementation Queue

---

## R001 — Verify Suspension Impact Low-Pass Filtering

**Status:** [x] Complete

**Title:** Verify and tune suspension/heave low-pass filtering eliminates LHA casing impact clack during bottom-out events.

**Objective:**
Confirm that `signal::low_pass()` applied to `NormSuspensionTravel` and `AccelerationY` (heave) in `forza::apply_packet()` produces a smooth, damped thud on bottom-out instead of a loud plastic clack. Tune `SUSP_LPF_HZ` and `HEAVE_LPF_HZ` downward if clacking persists.

**Files To Modify:**
- `src-tauri/src/signal.rs` — `SUSP_LPF_HZ`, `HEAVE_LPF_HZ` constants (tuning only)
- No new code required — verification and constant tuning only

**Functions To Modify:**
- `signal::low_pass()` — no code change, verify output
- `forza::apply_packet()` — no code change, verify call sites

**Dependencies:**
None

**Estimated Risk:** Low — tuning constants only; no structural changes

**Validation Steps:**
1. Launch app with DualSense connected, Forza Horizon running with Data Out enabled
2. Drive over severe compression zones (jump landings, sharp dips)
3. Listen for plastic clack from controller during bottom-out
4. If clacking: reduce `SUSP_LPF_HZ` from 12.0 to 8.0, retest
5. If clacking persists: reduce `HEAVE_LPF_HZ` from 12.0 to 8.0, retest
6. Confirm a low-frequency thud is still felt (impact sensation preserved)

**Completion Criteria:**
- No audible plastic clack from controller chassis during any bottom-out event
- Distinct low-frequency thud sensation preserved
- Suspension travel values entering the haptic pipeline have frequency content at or below the configured cutoff

---

## R002 — Verify Adaptive Trigger Slew Rate Limiting

**Status:** [x] Complete

**Title:** Verify slew rate limiter prevents worm gear grinding during rapid trigger resistance transitions.

**Objective:**
Confirm that `signal::slew_rate_limit()` applied to L2 and R2 resistance values prevents the DC motor from violently reversing or disengaging, eliminating gear grind/clack during fast state changes.

**Files To Modify:**
- `src-tauri/src/signal.rs` — `SLEW_MAX_CHANGE` constant (tuning only if needed)
- No new code required

**Functions To Modify:**
- `signal::slew_rate_limit()` — no code change, verify output
- `hid::racing_l2()` — no code change, verify call site
- `hid::process_frame()` — no code change, verify R2 slew call site

**Dependencies:**
None

**Estimated Risk:** Low — tuning constant only

**Validation Steps:**
1. In Forza, accelerate to moderate speed, then fully release throttle while airborne over a crest
2. Listen for gear snap or grind from L2/R2 triggers during rapid resistance changes
3. Enable diagnostic logging of `l2_resist_slew` and `r2_resist_slew` — confirm per-frame delta never exceeds `SLEW_MAX_CHANGE` (4)
4. Test rapid ABS pump transitions — confirm no mechanical clatter during mode flips between 0x01 and 0x06

**Completion Criteria:**
- No audible gear grinding or snapping during any trigger resistance transition
- Per-frame resistance delta never exceeds `SLEW_MAX_CHANGE`
- Slew-limited transitions are imperceptible to human reaction time

---

## R003 — Verify EWMA Slip Angle Smoothing

**Status:** [x] Complete

**Title:** Verify EWMA smoothing eliminates high-frequency stutter in slip-derived haptic output.

**Objective:**
Confirm that `signal::ewma()` applied to `TireSlipAngle` and `TireCombinedSlip` in `forza::apply_packet()` produces smooth, continuous vibration escalation without micro-oscillation artifacts.

**Files To Modify:**
- `src-tauri/src/signal.rs` — `EWMA_ALPHA` constant (tuning only if needed)
- No new code required

**Functions To Modify:**
- `signal::ewma()` — no code change, verify output
- `forza::apply_packet()` — no code change, verify call sites

**Dependencies:**
None

**Estimated Risk:** Low — tuning constant only

**Validation Steps:**
1. Drive a high-horsepower RWD car in Forza at the limit of adhesion
2. Confirm slip vibration intensity changes smoothly frame-to-frame without audible stepping or buzzing
3. Log `t_ewma_slip_angle` and `t_ewma_combined` values — confirm no stair-step patterns
4. If stutter persists: reduce `EWMA_ALPHA` from 0.1 to 0.05, retest
5. Confirm slip angle ramp from 0 to >1.0 produces smooth proportional haptic output

**Completion Criteria:**
- No audible high-frequency stutter or buzz from slip-derived haptics
- `t_ewma_*` values show smooth transitions without quantization artifacts
- Sustained high-slip events produce continuous, proportional haptic output

---

## R004 — Verify Dynamic Load Gating for Airborne False Positives

**Status:** [x] Complete

**Title:** Verify load gates and grip multiplier silence wheelspin buzz during airborne phases.

**Objective:**
Confirm that `signal::grip_multiplier()`, the suspension droop gate, and per-axle load gates correctly nullify slip-derived haptic output when wheels are unloaded or airborne.

**Files To Modify:**
- `src-tauri/src/hid.rs` — `LOAD_GATE_DROOP`, `LOAD_GATE_SPAN` constants (tuning only if needed)
- `src-tauri/src/signal.rs` — `GRAVITY` constant (tuning only if needed)
- No new code required

**Functions To Modify:**
- `signal::grip_multiplier()` — no code change, verify output
- `forza::apply_packet()` — no code change, verify load gate block
- `hid::compute_rumble()` — no code change, verify per-axle load gate closure

**Dependencies:**
None

**Estimated Risk:** Low — tuning constants only

**Validation Steps:**
1. In Forza, drive over a large jump crest at high speed
2. Confirm zero haptic vibration during the airborne phase — no false wheelspin buzz
3. Confirm normal haptic feedback resumes immediately on landing
4. Drive an off-road vehicle with visible suspension articulation — confirm one-wheel lift on a berm does not produce false slip vibration on that corner
5. If airborne false positives persist: verify `min_susp < 0.05` gate in `apply_packet()` is reached by logging `t_susp_*` values mid-jump

**Completion Criteria:**
- Zero haptic output from slip telemetry when `NormSuspensionTravel` is below `LOAD_GATE_DROOP` for any wheel
- `grip_multiplier()` drives all slip-derived haptic output toward zero during freefall (heave ≈ GRAVITY)
- Grounded driving is completely unaffected by the load gate

---

## R005 — Verify Slip-Threshold Transient Deadzone

**Status:** [x] Complete

**Title:** Verify transient slip deadzone suppresses micro-slip-induced trigger rattle.

**Objective:**
Confirm that `SLIP_DEADZONE_FRAMES = 2` (~33ms) correctly gates sub-frame slip spikes from triggering throttle flutter, while sustained traction loss still engages normally.

**Files To Modify:**
- `src-tauri/src/signal.rs` — `SLIP_DEADZONE_FRAMES` constant (tuning only if needed)
- No new code required

**Functions To Modify:**
- `hid::process_frame()` — no code change, verify slip counter blocks

**Dependencies:**
None

**Estimated Risk:** Low — tuning constant only

**Validation Steps:**
1. Drive a hybrid supercar in Forza, stab throttle hard while cornering
2. Confirm eTC micro-slips (<33ms) do not trigger throttle flutter or trigger rattle
3. Confirm sustained power oversteer (>33ms) does trigger full throttle flutter
4. Log `t_slip_rear_frames` counter — verify it resets to 0 within 1-2 frames of slip dropping below threshold
5. If persistent oversteer is missed: reduce `SLIP_DEADZONE_FRAMES` to 1

**Completion Criteria:**
- Slip events lasting less than configured deadzone frames produce no throttle flutter
- Slip events exceeding deadzone frames engage feedback normally
- No regression in traction-loss detection for any drivetrain type

---

## R006 — Verify Traction Control Intervention Haptic Attenuation

**Status:** [x] Complete

**Title:** Verify TC detection and 50% haptic attenuation during ECU power-cut events.

**Objective:**
Confirm that the TC detection heuristic (`rear_slip > 0.35 && throttle > 180 && accel < 2.0`) correctly identifies traction control intervention and scales all slip-derived haptic output by `TC_HAPTIC_SCALE = 0.5`.

**Files To Modify:**
- `src-tauri/src/signal.rs` — `TC_HAPTIC_SCALE` constant (tuning only if needed)
- `src-tauri/src/forza.rs` — TC detection thresholds in `apply_packet()` (tuning only if needed)
- No new code required

**Functions To Modify:**
- `forza::apply_packet()` — no code change, verify TC detection and scaling block

**Dependencies:**
None

**Estimated Risk:** Low — heuristic threshold tuning only

**Validation Steps:**
1. Drive Ferrari SF90 Stradale (or equivalent hybrid with eTC) in Forza, corner hard enough to trigger TC intervention
2. Confirm haptic intensity drops perceptibly during TC intervention
3. Confirm haptic intensity returns to full immediately after intervention ends
4. Drive a classic non-TC car — confirm `t_tc_active` is not spuriously true
5. Log `t_tc_active` alongside `t_slip_rear`, `t_accel_input`, and `t_accel` to verify detection accuracy

**Completion Criteria:**
- Haptic output from slip telemetry reduced to `TC_HAPTIC_SCALE` factor when TC intervention is detected
- TC detection correctly distinguishes ECU power-cut from normal lift-off deceleration
- No false attenuation during non-TC driving conditions

---

## R007 — Verify Pacejka-Based Slip-to-Haptic Intensity Mapping

**Status:** [x] Complete

**Title:** Verify Pacejka Magic Formula produces non-linear, realistic slip-to-rumble response curves.

**Objective:**
Confirm that `signal::pacejka_haptic()` is applied in `compute_rumble()` for both wheelspin and lockup rumble paths, producing a non-linear intensity curve that mimics real tire force characteristics (build toward peak, post-peak decline).

**Files To Modify:**
- `src-tauri/src/signal.rs` — Pacejka parameters (B, C, D, E) in `pacejka_haptic()` (tuning only if needed)
- No new code required

**Functions To Modify:**
- `signal::pacejka_haptic()` — no code change, verify output shape
- `signal::pacejka_force()` — no code change
- `hid::compute_rumble()` — no code change, verify call sites

**Dependencies:**
None

**Estimated Risk:** Low — parameter tuning only

**Validation Steps:**
1. In Forza, progressively increase throttle from standstill to full wheelspin
2. Confirm rumble intensity follows a non-linear curve (gentle onset, peak, slight post-peak softening) rather than a linear ramp
3. Compare against the raw slip ratio — confirm shaping is active (micro-slip 5-20% produces subtle output; deep slip >100% produces strong but shaped output)
4. Test lockup by braking hard — confirm lockup rumble also follows Pacejka shaping

**Completion Criteria:**
- Slip intensity follows a non-linear Pacejka-shaped curve, not a linear ramp
- Peak haptic intensity corresponds approximately to peak tire force
- Post-peak decline is perceptible at very deep slip (>150%)

---

## R008 — Verify Pneumatic Trail Collapse on Brake Trigger

**Status:** [x] Complete

**Title:** Verify brake trigger resistance smoothly decays during sustained front-tire lockup.

**Objective:**
Confirm that `signal::pneumatic_trail_decay()` applied in `racing_l2()` produces a smooth exponential collapse of brake trigger resistance when `t_slip_combined > 0.75 && t_slip_front > 0.75`, simulating the physical loss of self-aligning torque during a lockup.

**Files To Modify:**
- `src-tauri/src/signal.rs` — `PNEUMATIC_DECAY` constant (tuning only if needed)
- No new code required

**Functions To Modify:**
- `signal::pneumatic_trail_decay()` — no code change
- `hid::racing_l2()` — no code change, verify the trail decay branch

**Dependencies:**
None

**Estimated Risk:** Low — decay constant tuning only

**Validation Steps:**
1. In Forza, brake hard enough to lock the front tires (sustained slip > 0.75)
2. Confirm brake trigger resistance smoothly decays to near-zero over several frames (not an instantaneous drop)
3. Confirm resistance fully disengages (mode 0x05) when trail drops below threshold of 8
4. Confirm normal resistance returns immediately when lockup ends
5. Test at different speeds — confirm decay rate is appropriate at all speeds

**Completion Criteria:**
- Brake trigger resistance smoothly decays during sustained front-tire slip above 0.75
- Decay follows an exponential curve (not a binary drop)
- Trigger fully disengages when trail resistance drops below the configured floor

---

## R009 — Verify Haptic Hierarchical Layering with Dynamic Dominance

**Status:** [x] Complete

**Title:** Verify three-layer haptic hierarchy ducks ambient effects during critical slip.

**Objective:**
Confirm that `compute_rumble()` correctly implements: Layer 1 (ambient engine RPM, clamped to `AMBIENT_RPM_CLAMP`), Layer 2 (stereophonic road surface texture), and Layer 3 (lateral slip dominance that ducks Layers 1+2 when slip exceeds `LATERAL_SLIP_CRITICAL`).

**Files To Modify:**
- `src-tauri/src/hid.rs` — `AMBIENT_RPM_CLAMP`, `LATERAL_SLIP_CRITICAL` constants (tuning only if needed)
- No new code required

**Functions To Modify:**
- `hid::compute_rumble()` — no code change, verify `ambient_scale` and `surface_scale` gating

**Dependencies:**
None

**Estimated Risk:** Low — constant tuning only

**Validation Steps:**
1. Drive at steady cruise — confirm gentle engine RPM rumble is present but subtle (≤5-10%)
2. Enter a hard corner and induce understeer — confirm surface texture and engine rumble duck to near-zero, only slip vibration remains
3. Exit the corner — confirm ambient layers fade back in smoothly
4. Drive on rough surface (gravel/dirt) — confirm left/right stereo separation produces distinct per-side texture
5. If slip cues feel masked by ambient noise: lower `AMBIENT_RPM_CLAMP` from 0.05 to 0.03

**Completion Criteria:**
- When `t_slip_angle > LATERAL_SLIP_CRITICAL`, engine rumble and surface texture duck to near-zero
- Engine RPM rumble never exceeds 5-10% of maximum LHA output during normal driving
- Road surface texture uses proper left/right stereo separation from per-wheel telemetry
- No "haptic mud" or indistinct vibration states during complex racing scenarios

---

## R010 — Implement Aerodynamic Downforce Dynamic Brake Stiffness

**Status:** [x] Complete

**Title:** Scale brake trigger resistance proportionally with vehicle speed to simulate aerodynamic downforce loading.

**Objective:**
Add a speed-to-resistance scaling factor to `racing_l2()` that increases brake trigger firmness at high speed, simulating the additional hydraulic pressure required to slow a vehicle generating aerodynamic downforce. Must gracefully degrade when Forza telemetry is inactive.

**Files To Modify:**
- `src-tauri/src/hid.rs` — `racing_l2()`, possibly `process_frame()`
- `src-tauri/src/forza.rs` — no change (already provides `t_speed`)

**Functions To Modify:**
- `hid::racing_l2()` — add speed-based scaling to final brake resistance output
- `hid::process_frame()` — pass `t_speed` context if needed (available via `s.t_speed`)
- No signature changes required for external functions

**Dependencies:**
- R001 through R009 (verification baseline ensures stability before adding new scaling)
- `t_speed` populated by Forza telemetry (`t_on == true`) — must fall back gracefully to no scaling when telemetry inactive
- Must not conflict with `racing_l2()` existing brake curve, pedal wall, or pneumatic trail decay

**Estimated Risk:** Medium — adds multiplicative/additive factor to brake resistance; must be bounded and smoothly interpolated to avoid jarring transitions and preserve low-speed feel

**Validation Steps:**
1. In Forza, brake at low speed (~30 km/h) — confirm feel is unchanged from current
2. Accelerate to high speed (~250 km/h) and brake — confirm brake trigger feels progressively firmer
3. Perform sustained braking from high to low speed — confirm resistance smoothly reduces as speed drops
4. Disable Forza Data Out — confirm braking feels normal with no errors (no scaling applied)
5. Test at extreme speeds (350+ km/h) — confirm resistance is clamped at 255 and does not stall the trigger motor

**Completion Criteria:**
- Brake trigger resistance increases smoothly with vehicle speed
- No sudden jumps in resistance at threshold speeds
- Resistance never exceeds 255
- Low-speed braking behavior unchanged (no regression)
- No effect when Forza telemetry is inactive
- Slew rate limiter smoothly handles speed-driven resistance changes

---

## R011 — Implement Rev-Limiter Bounce Algorithm

**Status:** [x] Complete

**Title:** Add distinct rhythmic pulse when engine RPM reaches maximum, simulating electronic rev-limiter bounce.

**Objective:**
Add a new rumble block in `compute_rumble()` that fires a rhythmic on/off pulse pattern when `t_rpm >= REVLIM_BOUNCE_THRESHOLD` (from Forza telemetry) or `engine_rpm` reaches near-maximum (inferred model), producing a clearly distinguishable cue from the progressive redline-approach flutter.

**Files To Modify:**
- `src-tauri/src/hid.rs` — `compute_rumble()`, new constants

**Functions To Modify:**
- `hid::compute_rumble()` — add new rev-limiter bounce block in the Racing section, after the existing redline upshift cue

**Dependencies:**
- R001 through R009 (verification baseline)
- Must layer on top of existing redline cue (progressive flutter from `t_rpm > 0.85`), not replace it
- Must not conflict with telemetry-driven shift detection (shift clunk takes priority)

**Estimated Risk:** Low — additive rumble block; uses existing `t_rpm` and `engine_rpm` fields; no structural changes

**Validation Steps:**
1. In Forza, hold a gear until the engine hits the rev limiter — do not shift
2. Confirm a distinct rhythmic bounce pattern is felt, clearly different from the progressive redline-approach flutter
3. Confirm bounce ceases within one frame of RPM dropping below threshold (e.g., after upshift)
4. Drive normally with shifts before redline — confirm bounce never fires spuriously
5. Test with simulated engine model (no Forza telemetry) — confirm `engine_rpm` reaching near-max also triggers bounce
6. Test that an upshift precisely at the bounce threshold fires the shift clunk, not a stale bounce pulse

**Completion Criteria:**
- Rhythmic pulse fires when engine RPM is at maximum (≥0.99 normalized for telemetry, or `engine_rpm` ≥ 0.99 for inferred)
- Bounce pattern is clearly distinguishable from the progressive redline-approach flutter
- Bounce ceases immediately when RPM drops below threshold
- Never fires spuriously below redline
- Does not suppress or delay shift-detection clunks

---

## R012 — Implement HID Output State Delta Checking

**Status:** [x] Complete

**Title:** Cache last-sent HID report and skip writes when haptic parameters are unchanged.

**Objective:**
Add a 48-byte cache in `hid_loop()` storing the last transmitted USB-format report. Before each `write_report()` call, compare the newly generated report against the cache. Skip transmission if all haptic-relevant fields are identical. Update cache when a transmission occurs.

**Files To Modify:**
- `src-tauri/src/hid.rs` — `hid_loop()`

**Functions To Modify:**
- `hid::hid_loop()` — add local cache variable, add comparison logic before `write_report()`

**Dependencies:**
- R001 through R009 (verification baseline)
- Must handle both USB (48-byte) and BT (78-byte) transport — comparison happens on the USB-format report before `to_bt_report()` conversion
- Must NOT suppress independent lightbar or player LED writes that occur outside `process_frame()`

**Estimated Risk:** Low — optimization, not functional change; one missed update at 60 Hz is imperceptible if comparison omits a field

**Validation Steps:**
1. Run app with DualSense connected via USB
2. Leave controller idle (no input changes) for 5+ seconds — confirm no HID writes occur
3. Press a trigger — confirm haptic output activates immediately (one-frame latency max)
4. Switch profiles via frontend — confirm lightbar and haptics update immediately
5. Test Bluetooth connection — confirm BT sequence number and CRC-32 still generated correctly, report transmitted when changed
6. Long-duration test: play Forza for 30+ minutes — confirm no haptic freeze or stuck state

**Completion Criteria:**
- No HID output report transmitted when all haptic-relevant fields match previous frame
- Changed haptic parameters transmitted without added latency
- Comparison includes trigger modes, force values, frequencies, rumble levels, lightbar RGB, player LED pattern
- Independent lightbar/player LED writes are never suppressed
- BT transport functions identically to USB transport

---

## R013 — Harden HID Device Signal Isolation from Game/Steam

**Status:** [x] Complete

**Title:** Harden HidHide automation reliability and error handling for Windows Xbox mode signal isolation.

**Objective:**
Verify and harden the existing `hidhide.rs` implementation: ensure CLI path detection works across HidHide versions, error messages are actionable, and graceful degradation occurs when HidHide is absent. This is Windows-only — document the gap for other platforms.

**Files To Modify:**
- `src-tauri/src/hidhide.rs` — `cli_path()`, `parse_sony_instances()`, error handling
- `src-tauri/src/hid.rs` — `forward_to_xbox()` error message handling

**Functions To Modify:**
- `hidhide::cli_path()` — verify candidate paths, potentially add more
- `hidhide::parse_sony_instances()` — verify parsing across CLI versions
- `hid::forward_to_xbox()` — verify error propagation to `AppState.error_msg`

**Dependencies:**
- Requires ViGEmBus and HidHide installed on a Windows test machine
- Independent of all other tasks

**Estimated Risk:** Medium — HidHide CLI format changes across versions; fragile by nature (shells out to external CLI); Windows-only limitation

**Validation Steps:**
1. On Windows with HidHide installed, enable Xbox output mode — confirm `enable()` succeeds, no error message
2. Launch Forza Horizon — confirm game detects the virtual Xbox pad, not the physical DualSense
3. Switch back to DualSense mode — confirm `disable()` succeeds, physical DualSense reappears in system
4. Uninstall or rename HidHideCLI.exe — confirm graceful degradation (error message shown, virtual pad still works)
5. Test against at least two different HidHide CLI versions — confirm `parse_sony_instances()` correctly extracts `VID_054C` device paths
6. Verify no crash or hang if `--dev-gaming` output format is unrecognized

**Completion Criteria:**
- Physical DualSense hidden from Forza Horizon and Steam Input while in Xbox output mode
- Only the custom haptic engine writes HID output reports to the DualSense
- No garbled or colliding haptic output during simultaneous game play
- Graceful degradation with actionable error message when HidHide is absent
- Non-Windows platforms: documented limitation (no equivalent kernel masking available)

---

## R014 — Tune Hybrid Drivetrain Frequency Crossover

**Status:** [x] Complete

**Title:** Tune `slip_crossover_freq()` parameters and apply global crossover tuning that improves hybrid vehicle feel.

**Objective:**
Adjust the frequency crossover behavior so that when `TireSlipRatio` exceeds the crossover threshold (~1.0), the flutter frequency transitions from a high-frequency squeal to a low-frequency deep judder (30-50 Hz range), reducing mechanical clatter during hybrid eTC intervention events. Apply as a global tuning improvement (per-drivetrain profiles come in R015).

**Files To Modify:**
- `src-tauri/src/hid.rs` — `process_frame()` R2 throttle wheelspin flutter block
- `src-tauri/src/signal.rs` — potentially new constants for crossover base/deep frequencies

**Functions To Modify:**
- `hid::process_frame()` — the block where `slip_crossover_freq()` is called for R2 wheelspin flutter; update `base_hz` and `deep_hz` arguments
- `signal::slip_crossover_freq()` — no code change, just different call-site arguments

**Dependencies:**
- R001 through R009 (verification baseline for stable foundation)
- R005 (slip deadzone verified) — crossover builds on deadzone-gated slip detection
- This task provides the tuned parameters that R015 will wrap into per-drivetrain profiles

**Estimated Risk:** Medium — global tuning change affects all vehicles; must verify mechanical AWD feel does not regress

**Validation Steps:**
1. Drive Ferrari SF90 Stradale (or R-class hybrid) in Forza at the grip limit
2. Confirm traction-loss flutter transitions from high-freq to low-freq deep judder when slip exceeds cross ratio
3. Confirm no mechanical trigger rattle during eTC intervention oscillations
4. Drive Lamborghini Centenario (mechanical AWD) — confirm slip feel is acceptable (no regression from previous behavior)
5. A/B test: temporarily revert crossover to both regimes using the same frequency — confirm harsh buzz returns for hybrid
6. Test across a range of vehicles (FWD, RWD, AWD, hybrid) — confirm all produce acceptable feel

**Completion Criteria:**
- Throttle flutter frequency crosses over to deep judder (30-50 Hz) when slip ratio exceeds the crossover threshold
- Crossover transition is seamless and immediate
- Hybrid-type vehicles no longer produce harsh mechanical rattle during traction events
- Mechanical AWD vehicle feel is not degraded
- Crossover parameters are externalized as tunable constants

---

## R015 — Implement Drivetrain-Specific Tuning Profiles

**Status:** [x] Complete

**Title:** Create selectable drivetrain tuning profiles that adjust DSP parameters per vehicle type.

**Objective:**
Define a `DrivetrainProfile` struct holding per-type DSP parameters (slip deadzone, crossover frequencies, EWMA alpha, TC attenuation factor), create predefined profiles for major drivetrain archetypes (Default, Mechanical AWD, Hybrid Electric, RWD, FWD), add an `AppState` field for active profile selection, add a Tauri command and settings persistence, and route profile parameters into `compute_rumble()` and `process_frame()`.

**Files To Modify:**
- `src-tauri/src/hid.rs` — new `DrivetrainProfile` struct, new profile constant table, `AppState` field addition, `process_frame()`, `compute_rumble()`
- `src-tauri/src/signal.rs` — no change (constants remain as defaults; profiles override at call sites)
- `src-tauri/src/forza.rs` — no change (EWMA stays fixed in apply_packet; per-profile shaping applied downstream in compute_rumble)
- `src-tauri/src/lib.rs` — new `set_drivetrain_profile` Tauri command, persist on change
- `src-tauri/src/settings.rs` — new `drivetrain_profile` field in `SavedSettings`

**Functions To Modify:**
- `hid::process_frame()` — read active profile parameters, apply to deadzone gate and crossover frequency selection
- `hid::compute_rumble()` — read active profile parameters, apply to rumble intensity and frequency shaping
- `lib::run()` setup — restore saved profile index from settings
- `lib::set_drivetrain_profile()` — new Tauri command
- `settings::load()` / `settings::save()` — new field read/write
- `settings::persist()` in `lib.rs` — include new field

**Dependencies:**
- R014 (crossover parameters tuned) — profile defaults are populated from tuned values
- R005 (slip deadzone verified) — profile uses deadzone as a tunable parameter
- Frontend coordination: new UI control to select drivetrain profile (dropdown in Racing settings)
- Must handle invalid saved index gracefully (fallback to Default)
- Must handle profile struct versioning (new fields default to current constants)

**Estimated Risk:** High — largest scope; touches 5 modules; requires frontend coordination; creates new struct, new state, new command, new settings field; risk of regression if profile routing is incorrectly wired

**Validation Steps:**
1. With "Default" profile selected, confirm all Racing haptics are indistinguishable from current (pre-R015) behavior
2. Select "Hybrid Electric" profile — drive SF90 Stradale — confirm reduced trigger clatter, deep judder on slip, TC attenuation active
3. Select "Mechanical AWD" profile — drive Lamborghini Centenario — confirm progressive slip feel, no harsh crossover artifacts
4. Switch profiles while driving — confirm change takes effect next frame
5. Restart the app — confirm saved profile index is restored correctly
6. Edit `settings.json` to set an invalid profile index — confirm app falls back to "Default" profile without crashing
7. Add a new profile parameter to the struct with a serde default — confirm existing settings files load without error
8. Drive each drivetrain archetype with its matching AND non-matching profile — confirm each produces distinctly appropriate feel

**Completion Criteria:**
- `DrivetrainProfile` struct with at minimum: `slip_deadzone_frames`, `crossover_base_hz`, `crossover_deep_hz`, `tc_attenuation`
- Predefined profiles: Default (current behavior), Mechanical AWD, Hybrid Electric, RWD, FWD
- `AppState.drivetrain_profile_idx` persists across launches
- Tauri command `set_drivetrain_profile` callable from frontend
- Active profile parameters correctly route to `compute_rumble()` and `process_frame()`
- Invalid saved index gracefully falls back to Default
- No regression when Default profile is active

---

## Queue Summary

| ID | Title | Status | Phase | Risk | Dependencies |
|----|-------|--------|-------|------|-------------|
| R001 | Verify Suspension LPF | Complete | 1 | Low | None |
| R002 | Verify Slew Rate Limiting | Complete | 1 | Low | None |
| R003 | Verify EWMA Slip Smoothing | Complete | 1 | Low | None |
| R004 | Verify Load Gating | Complete | 1 | Low | None |
| R005 | Verify Slip Deadzone | Complete | 1 | Low | None |
| R006 | Verify TC Attenuation | Complete | 1 | Low | None |
| R007 | Verify Pacejka Mapping | Complete | 1 | Low | None |
| R008 | Verify Pneumatic Trail Collapse | Complete | 1 | Low | None |
| R009 | Verify Hierarchical Layering | Complete | 1 | Low | None |
| R010 | Aero Downforce Brake Stiffness | Complete | 2 | Medium | R001-R009 baseline |
| R011 | Rev-Limiter Bounce Algorithm | Complete | 2 | Low | R001-R009 baseline |
| R012 | HID State Delta Checking | Complete | 2 | Low | R001-R009 baseline |
| R013 | Harden HID Signal Isolation | Complete | 2 | Medium | None (independent) |
| R014 | Tune Hybrid Frequency Crossover | Complete | 3 | Medium | R001-R009, R005 |
| R015 | Drivetrain-Specific Tuning Profiles | Complete | 3 | High | R014, R005, frontend |

---

**Remaining Tasks:** 0

**Completed Tasks:** 15

**Current Task:** None (all tasks complete)
