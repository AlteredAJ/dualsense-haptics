# Gemini Research Prompt — Forza Telemetry Extension

---

## 1. Drivetrain Auto-Detection from Forza UDP Data Out

### Context
The app has 5 `DrivetrainProfile` presets (Default, Mechanical AWD, Hybrid Electric, RWD, FWD) that tune DSP parameters per vehicle type. Currently the user must manually select a profile. We want automatic detection from Forza telemetry.

### Available Telemetry Fields
- `t_slip_rear` — max rear tire slip ratio (0..∞)
- `t_slip_front` — max front tire slip ratio (0..∞)
- `t_tc_active` — boolean heuristic: rear slip > 0.35 AND throttle > 180 AND longitudinal G < 2.0 (ECU cutting power)
- `t_speed` — m/s
- `t_rpm` — normalized engine revs 0..1

### Research Questions
1. What multi-frame signal features best distinguish hybrid-electric drivetrains (rapid slip oscillation from eTC intervention) from mechanical AWD (smooth gradual slip)?
2. How should a sliding window (e.g. 60 frames @ 60 Hz = 1 second) compute classification confidence?
3. What is the minimum window size for reliable classification?
4. How to handle the "cold start" problem (first few seconds of driving, insufficient data)?

### Design Constraints
- Must run in `process_frame()` on the haptic output thread (no new threads)
- Must fail gracefully to Default profile if confidence is low
- Must not introduce perceptible latency
- User must be able to override auto-detection with manual selection

---

## 2. F1 23 Data Out Compatibility

### Context
The app currently parses Forza Motorsport/Forza Horizon Data Out UDP packets at fixed offsets from the Sled (232 bytes) and Dash blocks. F1 23 may use a similar or identical format.

### Known Forza Packet Layouts
| Title | Packet Length | Gear Offset | Speed Offset |
|-------|--------------|-------------|--------------|
| FM7 | 311 bytes | 307 | 244 |
| FH4/5/6 | 324+ bytes | 319 | 256 |

### Research Questions
1. Does F1 23 broadcast a UDP Data Out telemetry feed? If so, is the feature enabled the same way (Settings → Telemetry → UDP)?
2. What UDP ports does F1 23 use for Data Out? Are 5300/7000/20066 supported?
3. What is the packet format? Is it the same 232-byte Sled block with identical offsets for RPM, acceleration, slip, suspension, surface rumble?
4. If different, what are the key offsets for: RPM, longitudinal/lateral acceleration, per-wheel tire slip, suspension travel, surface rumble, gear, speed, pedal inputs?
5. Does F1 23 use little-endian byte ordering like Forza?
