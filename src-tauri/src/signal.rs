// Shared signal-processing helpers for Forza telemetry and Racing haptics.
// Pure functions — state (previous filtered values, slew memory) lives in AppState.

use std::f32::consts::PI;

pub const TELEM_SAMPLE_HZ: f32 = 60.0;
pub const SUSP_LPF_HZ: f32 = 12.0;
pub const HEAVE_LPF_HZ: f32 = 12.0;
pub const EWMA_ALPHA: f32 = 0.1;
pub const GRAVITY: f32 = 9.81;
pub const SLEW_MAX_CHANGE: u8 = 4;
/// Frames at 60 Hz before transient rear slip triggers throttle flutter (~25 ms).
/// ⚠ Canonical reference default — runtime value is owned by
///   `DrivetrainProfile::slip_deadzone_frames` (see hid.rs DRIVETRAIN_PROFILES).
pub const SLIP_DEADZONE_FRAMES: u16 = 2;
pub const SLIP_CROSSOVER_RATIO: f32 = 1.0;
/// Pre-crossover flutter freq range (normal slip — light informative hum).
/// Rises with slip intensity so micro-slips are subtle, deep approach is urgent.
/// ⚠ Canonical reference defaults — runtime values are owned by
///   `DrivetrainProfile::slip_flutter_{lo,hi}_hz` (see hid.rs DRIVETRAIN_PROFILES).
pub const SLIP_FLUTTER_LO_HZ:  f32 = 50.0;
pub const SLIP_FLUTTER_HI_HZ:  f32 = 80.0;
/// Post-crossover deep-judder freq (slip > crossover ratio — heavy low thud).
/// Low enough that the trigger motor tracks the waveform smoothly instead of
/// chattering against the worm-gear from high-freq eTC square-wave spikes.
///
/// ⚠ The crossover direction was intentionally inverted in R014: pre-crossover
///   flutter is now *higher* frequency (50–80 Hz) and post-crossover deep-judder
///   is *lower* (35 Hz).  The old code had the reverse (8–28 pre / 40 post),
///   which made deep slip produce a *faster* vibration that caused worm-gear
///   chatter on hybrid-electric drivetrains.
///
/// ⚠ Canonical reference default — runtime value is owned by
///   `DrivetrainProfile::slip_crossover_deep_hz` (see hid.rs DRIVETRAIN_PROFILES).
pub const SLIP_CROSSOVER_DEEP_HZ: f32 = 35.0;
pub const LATERAL_SLIP_CRITICAL: f32 = 0.12;
pub const AMBIENT_RPM_CLAMP: f32 = 0.05;
pub const PNEUMATIC_DECAY: f32 = 0.1;
pub const TC_HAPTIC_SCALE: f32 = 0.5;
pub const SURFACE_STEREO_SCALE: f32 = 0.25;

/// First-order low-pass filter (exponential smoothing with cutoff-derived alpha).
#[inline]
pub fn low_pass(raw: f32, prev: f32, cutoff_hz: f32, sample_rate: f32) -> f32 {
    let tau = 1.0 / (2.0 * PI * cutoff_hz.max(0.01));
    let alpha = tau / (tau + 1.0 / sample_rate.max(1.0));
    alpha * raw + (1.0 - alpha) * prev
}

/// Exponential weighted moving average.
#[inline]
pub fn ewma(raw: f32, prev: f32, alpha: f32) -> f32 {
    alpha * raw + (1.0 - alpha) * prev
}

/// Vertical-load grip multiplier: higher heave acceleration → less haptic gain.
#[inline]
pub fn grip_multiplier(heave_accel: f32) -> f32 {
    let load_factor = (heave_accel.abs() / GRAVITY).clamp(0.0, 1.0);
    1.0 - load_factor
}

/// Per-axle suspension load gate (0 = full droop / airborne, 1 = loaded).
#[inline]
pub fn suspension_load_gate(norm_travel: f32, droop: f32, span: f32) -> f32 {
    ((norm_travel - droop) / span).clamp(0.0, 1.0)
}

/// Limit per-frame resistance change to prevent trigger motor chatter.
#[inline]
pub fn slew_rate_limit(current: u8, target: u8, max_change: u8) -> u8 {
    let delta = target as i16 - current as i16;
    if delta.abs() > max_change as i16 {
        if delta > 0 {
            current.saturating_add(max_change)
        } else {
            current.saturating_sub(max_change)
        }
    } else {
        target
    }
}

/// Simplified Pacejka Magic Formula (longitudinal/lateral force shape vs slip).
#[inline]
pub fn pacejka_force(slip: f32, b: f32, c: f32, d: f32, e: f32) -> f32 {
    let bx = b * slip;
    let inner = bx - e * (bx - bx.atan());
    (d * (c * inner.atan()).sin()).abs()
}

/// Pacejka-scaled haptic intensity 0..1 from a slip ratio.
#[inline]
pub fn pacejka_haptic(slip: f32) -> f32 {
    pacejka_force(slip.abs(), 10.0, 1.9, 1.0, 0.97).clamp(0.0, 1.0)
}

/// Wheelspin flutter frequency: deep judder above slip crossover, lighter below.
#[inline]
pub fn slip_crossover_freq(slip_ratio: f32, base_hz: f32, deep_hz: f32) -> f32 {
    if slip_ratio > SLIP_CROSSOVER_RATIO {
        deep_hz
    } else {
        base_hz
    }
}

/// Pneumatic trail collapse during lock-up — exponential decay toward off.
#[inline]
pub fn pneumatic_trail_decay(current: f32, decay: f32) -> f32 {
    (current * decay).clamp(0.0, 255.0)
}
