use serde::Serialize;
use crate::signal::{
    self, AMBIENT_RPM_CLAMP, LATERAL_SLIP_CRITICAL, PNEUMATIC_DECAY,
    SLEW_MAX_CHANGE, SURFACE_STEREO_SCALE,
};
use std::collections::VecDeque;
use std::ffi::CString;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// ─── DualSense USB IDs ────────────────────────────────────────────────────────

const SONY_VENDOR:       u16 = 0x054C;
const DUALSENSE_PRODUCT: u16 = 0x0CE6;

// ─── Transport ────────────────────────────────────────────────────────────────
// The DualSense uses a different HID report format over USB vs Bluetooth, so we
// detect the link at open time (hidapi bus_type) and branch both the input parse
// and the output report builder on it. Over BT the output report carries a 0x31
// report ID, two header bytes, and a trailing CRC-32 the controller validates —
// without the CRC it silently drops the packet, so haptics simply wouldn't fire.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Usb,
    Bluetooth,
}

// ─── Edition ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edition {
    Free,  // all profiles, Light strength only, no ABS, no shift feedback, gun semi only
    Full,  // Full Immersion — everything unlocked
}

// ─── Profile ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Racing,
    Static,
    Gun,
    Melee,
    Audio,
    Minecraft,
}

impl Profile {
    pub fn from_str(s: &str) -> Self {
        match s {
            "static"    => Self::Static,
            "gun"       => Self::Gun,
            "melee"     => Self::Melee,
            "audio"     => Self::Audio,
            "minecraft" => Self::Minecraft,
            _           => Self::Racing,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Racing    => "racing",
            Self::Static    => "static",
            Self::Gun       => "gun",
            Self::Melee     => "melee",
            Self::Audio     => "audio",
            Self::Minecraft => "minecraft",
        }
    }
    pub fn lightbar(self) -> [u8; 3] {
        match self {
            Self::Racing    => [0,   200, 255],
            Self::Static    => [255, 0,   255],
            Self::Gun       => [255, 30,  0  ],
            Self::Melee     => [255, 80,  0  ],
            Self::Audio     => [0,   80,  255],
            Self::Minecraft => [80,  220, 40 ],  // grass green (overridden per-item when connected)
        }
    }
}

// ─── Output mode ────────────────────────────────────────────────────────────
// DualSense: native — the game reads the controller directly (works for titles with
// DualSense support). Xbox: Windows-only — we spin up a virtual Xbox 360 (XInput) pad
// via ViGEmBus and forward the DualSense's inputs into it, so XInput-only games (Forza,
// etc.) detect the controller. Haptics keep driving the real DualSense in both modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Dualsense,
    Xbox,
}

impl OutputMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "xbox" => Self::Xbox,
            _      => Self::Dualsense,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xbox      => "xbox",
            Self::Dualsense => "dualsense",
        }
    }
}

// ── Game source ──────────────────────────────────────────────────────────────
// Selects which simulation engine feeds telemetry into the shared AppState.t_*
// fields. Only one bridge is active at a time.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GameSource {
    None,
    Forza,
    F123,
    Assetto,
}

impl GameSource {
    pub fn from_str(s: &str) -> Self {
        match s {
            "forza"   => Self::Forza,
            "f123"    => Self::F123,
            "assetto" => Self::Assetto,
            _         => Self::None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Forza   => "forza",
            Self::F123    => "f123",
            Self::Assetto => "assetto",
            Self::None    => "none",
        }
    }
}

// ─── Minecraft held-item category ───────────────────────────────────────────
// The Fabric mod sends the category of the currently held item each time it
// changes. The app maps that to a lightbar color (Phase 1 proof of life) and,
// later, to per-item trigger/rumble feels.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McItem {
    Empty,
    Sword,
    Axe,
    Pickaxe,
    Shovel,
    Hoe,
    Bow,
    Crossbow,
    Trident,
    Shield,
    Food,
    Block,
    Other,
}

impl McItem {
    pub fn from_str(s: &str) -> Self {
        match s {
            "sword"    => Self::Sword,
            "axe"      => Self::Axe,
            "pickaxe"  => Self::Pickaxe,
            "shovel"   => Self::Shovel,
            "hoe"      => Self::Hoe,
            "bow"      => Self::Bow,
            "crossbow" => Self::Crossbow,
            "trident"  => Self::Trident,
            "shield"   => Self::Shield,
            "food"     => Self::Food,
            "block"    => Self::Block,
            "other"    => Self::Other,
            _          => Self::Empty,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty    => "empty",
            Self::Sword    => "sword",
            Self::Axe      => "axe",
            Self::Pickaxe  => "pickaxe",
            Self::Shovel   => "shovel",
            Self::Hoe      => "hoe",
            Self::Bow      => "bow",
            Self::Crossbow => "crossbow",
            Self::Trident  => "trident",
            Self::Shield   => "shield",
            Self::Food     => "food",
            Self::Block    => "block",
            Self::Other    => "other",
        }
    }
    /// Lightbar color per item category — the Phase 1 proof of life.
    pub fn lightbar(self) -> [u8; 3] {
        match self {
            Self::Empty    => [80,  220, 40 ],  // grass green — bare hand
            Self::Sword    => [200, 220, 235],  // steel
            Self::Axe      => [180, 110, 40 ],  // wood/iron
            Self::Pickaxe  => [120, 130, 150],  // stone gray
            Self::Shovel   => [150, 110, 70 ],  // dirt brown
            Self::Hoe      => [90,  170, 60 ],  // crops
            Self::Bow      => [210, 160, 70 ],  // bowstring tan
            Self::Crossbow => [170, 120, 60 ],
            Self::Trident  => [40,  180, 200],  // aqua
            Self::Shield   => [120, 120, 140],  // iron plate
            Self::Food     => [230, 90,  60 ],  // appetite red
            Self::Block    => [150, 200, 120],  // generic block
            Self::Other    => [120, 120, 120],  // neutral gray
        }
    }
}

// ─── Gun fire mode ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GunMode {
    Semi,   // one shot per trigger break, recoil thump
    Burst,  // N rounds per break, own fire rate (AN-94 hyperburst → Fortnite Ch2 burst)
    Auto,   // continuous vibration at adjustable fire rate
}

impl GunMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "burst" => Self::Burst,
            "auto"  => Self::Auto,
            _       => Self::Semi,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Semi  => "semi",
            Self::Burst => "burst",
            Self::Auto  => "auto",
        }
    }
}

// ─── Weapon profiles ──────────────────────────────────────────────────────────
// Each weapon is a one-tap gun feel on the Gun page. `pattern` picks the firing
// behaviour; the rest tunes the 0x06 trigger kick + rumble motors. These values
// came straight out of the Trigger Lab presets.

#[derive(Clone, Copy)]
pub struct Weapon {
    pub key:         &'static str,
    pub label:       &'static str,
    pub pattern:     GunMode,
    pub kick_freq:   u8,  // 0x06 vibration frequency for the recoil kick (Semi/Burst)
    pub kick_amp:    u8,  // 0x06 amplitude
    pub rumble_l:    u8,  // left motor (thud) during kick / auto fire
    pub rumble_r:    u8,  // right motor (buzz)
    pub burst_count: u8,  // rounds per burst (Burst only)
    pub rate_hz:     u8,  // Burst: intra-burst rate; Auto: held-vibration frequency
    pub kick_frames: u8,  // Semi: active 0x06 kick duration in frames. Tune so the
                          // trigger completes ~1 oscillation (no double-bounce):
                          // ~1 cycle ≈ 60/kick_freq frames at 60fps.
}

pub const WEAPONS: [Weapon; 9] = [
    Weapon { key: "pistol",  label: "Pistol",  pattern: GunMode::Semi,  kick_freq: 18, kick_amp: 255, rumble_l: 170, rumble_r: 60,  burst_count: 1, rate_hz: 0,  kick_frames: 3 },
    Weapon { key: "revolver",label: "Revolver",pattern: GunMode::Semi,  kick_freq: 12, kick_amp: 255, rumble_l: 220, rumble_r: 80,  burst_count: 1, rate_hz: 0,  kick_frames: 4 },
    Weapon { key: "rifle",   label: "Rifle",   pattern: GunMode::Semi,  kick_freq: 9,  kick_amp: 255, rumble_l: 160, rumble_r: 110, burst_count: 1, rate_hz: 0,  kick_frames: 5 },
    Weapon { key: "burst",   label: "Burst",   pattern: GunMode::Burst, kick_freq: 30, kick_amp: 255, rumble_l: 180, rumble_r: 200, burst_count: 3, rate_hz: 20, kick_frames: 4 },
    Weapon { key: "ar",      label: "AR Auto", pattern: GunMode::Auto,  kick_freq: 0,  kick_amp: 255, rumble_l: 130, rumble_r: 180, burst_count: 1, rate_hz: 10, kick_frames: 5 },
    Weapon { key: "smg",     label: "SMG",     pattern: GunMode::Auto,  kick_freq: 0,  kick_amp: 200, rumble_l: 90,  rumble_r: 210, burst_count: 1, rate_hz: 16, kick_frames: 5 },
    Weapon { key: "lmg",     label: "LMG",     pattern: GunMode::Auto,  kick_freq: 0,  kick_amp: 255, rumble_l: 210, rumble_r: 220, burst_count: 1, rate_hz: 7,  kick_frames: 5 },
    Weapon { key: "shotgun", label: "Shotgun", pattern: GunMode::Semi,  kick_freq: 8,  kick_amp: 255, rumble_l: 255, rumble_r: 160, burst_count: 1, rate_hz: 0,  kick_frames: 6 },
    Weapon { key: "sniper",  label: "Sniper",  pattern: GunMode::Semi,  kick_freq: 20, kick_amp: 255, rumble_l: 255, rumble_r: 120, burst_count: 1, rate_hz: 0,  kick_frames: 3 },
];

/// Resolve a weapon key to its index, defaulting to the first weapon (pistol).
pub fn weapon_index(key: &str) -> usize {
    WEAPONS.iter().position(|w| w.key == key).unwrap_or(0)
}

// ─── Melee weapon profiles ────────────────────────────────────────────────────
// Each melee weapon is a swing feel on the Melee page. R2 builds resting swing
// heft (0x01 resistance up the pull); crossing the swing threshold fires a
// connect kick (0x06 vibration) plus a both-motor impact thump. Weight, speed,
// and bite differ per weapon. Starter set is themed on Dead Island 2 melee.
#[derive(Clone, Copy)]
pub struct MeleeWeapon {
    pub key:           &'static str,
    pub label:         &'static str,
    pub swing_force:   u8,   // 0x01 resting resistance at full R2 pull (heft)
    pub swing_exp:     f32,  // pull→force curve (higher = builds late)
    pub impact_freq:   u8,   // 0x06 connect-kick frequency (low = heavy thud)
    pub impact_force:  u8,   // 0x06 connect-kick amplitude
    pub impact_frames: u8,   // connect-kick duration in frames
    pub rumble_l:      u8,   // low-freq thump on connect
    pub rumble_r:      u8,   // high-freq grain on connect
}

// Dead Island 2 melee roster: fists (unarmed) → light/heavy blades → blunt → two-handed.
// R2 builds the heavy-attack windup heft; releasing at full draw fires the connect kick.
pub const MELEE_WEAPONS: [MeleeWeapon; 10] = [
    MeleeWeapon { key: "fists",    label: "Fists",        swing_force: 40,  swing_exp: 1.2, impact_freq: 36, impact_force: 130, impact_frames: 2, rumble_l: 80,  rumble_r: 150 },
    MeleeWeapon { key: "knife",    label: "Knife",        swing_force: 58,  swing_exp: 1.3, impact_freq: 32, impact_force: 155, impact_frames: 2, rumble_l: 95,  rumble_r: 165 },
    MeleeWeapon { key: "machete",  label: "Machete",      swing_force: 108, swing_exp: 1.55,impact_freq: 21, impact_force: 205, impact_frames: 4, rumble_l: 168, rumble_r: 148 },
    MeleeWeapon { key: "katana",   label: "Katana",       swing_force: 116, swing_exp: 1.45,impact_freq: 26, impact_force: 212, impact_frames: 3, rumble_l: 176, rumble_r: 172 },
    MeleeWeapon { key: "axe",      label: "Axe",          swing_force: 150, swing_exp: 1.70,impact_freq: 14, impact_force: 235, impact_frames: 5, rumble_l: 210, rumble_r: 120 },
    MeleeWeapon { key: "cleaver",  label: "Cleaver",      swing_force: 150, swing_exp: 1.78,impact_freq: 13, impact_force: 240, impact_frames: 5, rumble_l: 218, rumble_r: 120 },
    MeleeWeapon { key: "knuckles", label: "Knuckles",     swing_force: 70,  swing_exp: 1.35,impact_freq: 24, impact_force: 175, impact_frames: 3, rumble_l: 150, rumble_r: 120 },
    MeleeWeapon { key: "bat",      label: "Bat / Pipe",   swing_force: 178, swing_exp: 1.92,impact_freq: 10, impact_force: 255, impact_frames: 6, rumble_l: 242, rumble_r: 82  },
    MeleeWeapon { key: "spear",    label: "Spear",        swing_force: 100, swing_exp: 1.50,impact_freq: 20, impact_force: 195, impact_frames: 4, rumble_l: 150, rumble_r: 130 },
    MeleeWeapon { key: "sledge",   label: "Sledgehammer", swing_force: 216, swing_exp: 2.20,impact_freq: 7,  impact_force: 255, impact_frames: 8, rumble_l: 255, rumble_r: 60  },
];

/// Resolve a melee weapon key to its index, defaulting to the first (knife).
pub fn melee_weapon_index(key: &str) -> usize {
    MELEE_WEAPONS.iter().position(|w| w.key == key).unwrap_or(0)
}

// ─── Strength levels ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Strength {
    pub label:          &'static str,
    pub brake_start:    u8,
    pub brake_end:      u8,
    pub brake_exp:      f32,
    pub throttle_start: u8,
    pub throttle_end:   u8,
    pub throttle_exp:   f32,
    pub shift_force:    u8,
}

pub const STRENGTHS: [Strength; 4] = [
    Strength { label: "Light",  brake_start: 120, brake_end: 215, brake_exp: 1.5, throttle_start: 30, throttle_end: 108, throttle_exp: 1.3, shift_force: 230 },
    Strength { label: "Medium", brake_start: 140, brake_end: 238, brake_exp: 1.7, throttle_start: 45, throttle_end: 140, throttle_exp: 1.4, shift_force: 245 },
    Strength { label: "Hard",   brake_start: 158, brake_end: 255, brake_exp: 1.9, throttle_start: 58, throttle_end: 172, throttle_exp: 1.4, shift_force: 255 },
    Strength { label: "Max",    brake_start: 175, brake_end: 255, brake_exp: 2.3, throttle_start: 75, throttle_end: 200, throttle_exp: 1.6, shift_force: 255 },
];

/// Extra Racing-Lab-only knobs that live outside the Strength preset table.
/// Defaults match the module constants so behavior is unchanged until a custom
/// profile is active. Only consulted when `AppState::racing_custom_active()`.
#[derive(Clone, Copy)]
pub struct RacingTuning {
    pub abs_freq:       u8,  // ABS pump vibration frequency (Hz). Default 8.
    pub abs_delay:      u8,  // frames of sustained full-brake before ABS fires. Default 18 (~300ms).
    pub engine_texture: u8,  // right-motor engine buzz scale at full throttle. Default 22.
    pub feather_end:    u8,  // L2 raw value where the soft feather zone ends → ramp begins. Default 38.
}

impl Default for RacingTuning {
    fn default() -> Self {
        Self { abs_freq: ABS_FREQ, abs_delay: ABS_DELAY_FRAMES, engine_texture: 22, feather_end: L2_LOW_END }
    }
}

pub const PLAYER_LED: [u8; 4] = [0x04, 0x0A, 0x15, 0x1B];

// ─── Haptic constants ─────────────────────────────────────────────────────────

const DEAD_ZONE:              u8 = 12;  // ignore sub-5% trigger noise (force calc + non-racing profiles)
// Pedal "wall": resistance ramps to a firm stop across this zone so the finger meets
// strong pushback near ~220 and is held off the trigger's internal plastic stop — that
// plastic contact is what made the bottomed-out effect clack/buzz loudly. Quieter than
// an active-drive buzz at the floor, and it still leaves travel for the resistance to
// act on (a determined hard stab can still push through to full lock, e.g. for ABS).
const PEDAL_WALL_START:       u8 = 190;
const PEDAL_WALL_END:         u8 = 225;
// Racing-specific hysteresis: arm haptics at ON, only disarm when trigger drops to OFF.
// Prevents the oscillation where resistance force > finger pressure near the dead zone.
const TRIG_HAPTIC_ON:         u8 = 4;   // ~1.5% travel — arm almost the instant the brake moves
const TRIG_HAPTIC_OFF:        u8 = 2;   // ~0.8% travel — disarm
const THROTTLE_FEATHER_END:   u8 = 42;  // ~17% travel — end of throttle feather ramp (0 → throttle_start)
// Dynamic load gate (GT7 telemetry research) — suppresses slip haptics from an
// unloaded/airborne wheel. NormSuspensionTravel is 0.0 at full droop. Slip feel
// fades in from DROOP and reaches full over the next SPAN. Deliberately low so it
// only ever kills the genuinely-airborne case; grounded driving is untouched. To
// disable, set DROOP negative (gate always returns 1.0).
const LOAD_GATE_DROOP:        f32 = 0.05;
const LOAD_GATE_SPAN:         f32 = 0.14;
// Aerodynamic downforce on the brake: at speed the pedal firms up because aero load
// pushes the tyres harder into the tarmac, requiring higher hydraulic pressure.
// Linear ramp from AERO_MIN_SPEED_MS (no boost) to AERO_MAX_SPEED_MS (full boost).
const AERO_MIN_SPEED_MS:      f32 = 10.0;  // m/s (~36 km/h) — below this no aero effect
const AERO_MAX_SPEED_MS:      f32 = 80.0;  // m/s (~288 km/h) — full aero stiffness
const AERO_MAX_BOOST:         f32 = 0.30;  // +30% resistance at full aero speed
// Starting-resistance floors: the brake/throttle feather zones begin at this % of
// their start force the instant the trigger arms past the deadzone, instead of
// ramping up from ~0. Removes the "empty travel" before the racing effects engage.
// Brake sits high so it's stiff and solid from the first mm (a real brake pedal is
// firm immediately — it should be the last thing to feel empty). Throttle sits
// lower so the gas takes less force to push and stays lighter than the brake — they
// shouldn't feel the same. The progressive curve still builds on top above the zone.
const RACING_BRAKE_FLOOR_PCT:    u16 = 90;
const RACING_THROTTLE_FLOOR_PCT: u16 = 70;
const THROTTLE_DAMPER_FLOOR:    u8 = 38; // minimum resistance off-rest — hydraulic preload
// Racing L2 brake curve zones (0-255 raw range)
const L2_LOW_END:             u8 = 38;  // 15% of 255 — end of feather zone
const L2_RAMP_END:            u8 = 242; // 95% of 255 — end of exponential ramp
const L2_FINAL_ZONE:          u8 = 243; // 96% of 255 — ABS zone begins
const ABS_DELAY_FRAMES:       u8 = 18;  // ~300ms at 60fps before ABS fires
const ABS_FREQ:               u8 = 5;   // Hz — pedal-pump rate; low so each shove is distinct, not a buzz
const ENGINE_IDLE_HZ:        f32 = 7.0;  // left-motor pulse rate at just-off-idle throttle — lumpy
const ENGINE_RED_HZ:         f32 = 26.0; // pulse rate at full throttle — fast flutter (smooth high rev)
const REVLIM_BOUNCE_HZ:      f32 = 5.0;  // Hz — ECU cut/restore cadence at the rev limiter
const REVLIM_RPM_THRESHOLD:  f32 = 0.99; // normalised RPM at which the bounce engages
const GUN_AUTO_HZ:            u8 = 13;   // ~800 RPM, M4 full-auto — default auto rate
const GUN_BREAK_POS:          u8 = 110;  // 43% travel — semi trigger click point
const GUN_AUTO_BREAK_POS:     u8 = 60;   // 24% travel — auto: resistance → vibration
const RECOIL_BREAK_THRESHOLD: u8 = 120;  // slightly past GUN_BREAK_POS
const RECOIL_RELEASE_FRAMES:  u8 = 1;  // brief trigger-drop before the kick fires
// Gun fire-rate adjustable bands (Hz). Auto feeds the 0x06 vibration frequency;
// burst spaces individual recoil pulses.
pub const AUTO_HZ_MIN:        u8 = 5;    // ~300 RPM
pub const AUTO_HZ_MAX:        u8 = 20;   // ~1200 RPM
pub const BURST_HZ_MIN:       u8 = 10;   // ~600 RPM — Fortnite Ch2 burst AR
pub const BURST_HZ_MAX:       u8 = 30;   // ~1800 RPM — AN-94 hyperburst
pub const BURST_COUNT_MIN:    u8 = 2;    // AN-94 = 2-round
pub const BURST_COUNT_MAX:    u8 = 5;
// Burst recoil pulse — snappier than semi (6) so high-RPM bursts fit, but long
// enough that the trigger motor physically engages and the slam is felt.
const BURST_SLAM_FRAMES:      u8 = 3;    // ~50ms slam — felt, not a blip
const BURST_PULSE_FRAMES:     u8 = 4;    // 1 release + 3 slam

/// Frames between burst rounds at 60 fps for a given rate, accounting for the
/// pulse itself. At 30 Hz the gap is 0 (rounds blur → AN-94 hyperburst feel).
fn burst_gap_frames(hz: u8) -> u8 {
    let period = (60.0 / hz.max(1) as f32).round() as u8;
    period.saturating_sub(BURST_PULSE_FRAMES)
}
const SHIFT_RELEASE_FRAMES:   u8 = 1;
const SHIFT_SLAM_FRAMES:      u8 = 8;
const SHIFT_TOTAL_FRAMES:     u8 = SHIFT_RELEASE_FRAMES + SHIFT_SLAM_FRAMES;
// Upshift is a crisp instant snap — no leading release frame (which made the
// punch feel late), so the slam lands on the very first output frame. Shorter
// than the downshift peel so it reads as a quick flick, not a heavy thud.
const UPSHIFT_SLAM_FRAMES:    u8 = 6;
const R2_BLIP_FRAMES:         u8 = UPSHIFT_SLAM_FRAMES; // upshift R2 blip matches the snap length

// ─── Minecraft (Phase 2) ────────────────────────────────────────────────────
// Transient event pulse lengths (frames @ 60fps) and ambient feel rates.
pub const MC_ATTACK_FRAMES:   u8  = 7;    // sword/axe swing-connect kick
pub const MC_HURT_FRAMES:     u8  = 9;    // damage jolt
pub const MC_RELEASE_FRAMES:  u8  = 6;    // bow/crossbow/trident release twang
const MC_LOW_HEALTH:          f32 = 6.0;  // <= 3 hearts → heartbeat kicks in
const MC_HEART_HZ:            f32 = 1.7;  // resting-ish heartbeat cadence
const MC_STEP_HZ:             f32 = 2.6;  // sprint footfall cadence
const MC_CHEW_HZ:             f32 = 3.0;  // eating/drinking gulp cadence

// ─── App state ────────────────────────────────────────────────────────────────

/// Drivetrain "feel" character for the Racing profile — the deeper knobs behind the
/// simulated-engine sensation, exposed live in the Racing Lab. Defaults reproduce the
/// original hand-tuned constants exactly, so the feel is unchanged until a slider moves.
#[derive(Clone, Copy)]
pub struct DrivetrainFeel {
    pub take_up: u8,  // throttle feather-zone end (raw trigger travel) — the take-up depth
    pub idle_hz: u8,  // engine pulse rate just off idle (lower = chunkier chug)
    pub red_hz:  u8,  // engine pulse rate at redline (the smooth top-end flutter)
    pub weight:  u8,  // drivetrain inertia 0..100 (higher = more rev build/settle lag)
    pub load:    u8,  // driveline load feel 0..100 (engine strain + tip-in lash strength)
}

impl Default for DrivetrainFeel {
    fn default() -> Self {
        // take_up 42, idle 7 Hz, red 26 Hz match THROTTLE_FEATHER_END / ENGINE_*_HZ.
        // weight 40 maps to the original 0.12 attack / 0.18 settle inertia (see
        // drivetrain_inertia below).
        Self { take_up: 42, idle_hz: 7, red_hz: 26, weight: 40, load: 50 }
    }
}

/// Map the 0..100 drivetrain weight to (attack, settle) lerp factors for the rev
/// chase. weight 40 → (0.12, 0.18), the original feel. Higher weight = smaller
/// factors = heavier lag; clamped so revs never freeze entirely.
pub fn drivetrain_inertia(weight: u8) -> (f32, f32) {
    let w = weight as f32;
    let attack = (0.20 - 0.0020 * w).max(0.02);
    let settle = (0.26 - 0.0020 * w).max(0.02);
    (attack, settle)
}

/// DSP parameters that vary per drivetrain architecture. Predefined profiles for
/// common archetypes; the user picks one in the UI and it tunes the slip deadzone,
/// crossover frequencies, and flutter range to match the vehicle's character.
///
/// The `Default` profile (index 0) reproduces the canonical reference defaults
/// from `signal.rs` constants (`SLIP_DEADZONE_FRAMES`, `SLIP_FLUTTER_*`,
/// `SLIP_CROSSOVER_DEEP_HZ`).  Those constants are no longer imported at runtime
/// — this table is the single source of truth for all drivetrain DSP parameters.
#[derive(Clone, Copy)]
pub struct DrivetrainProfile {
    pub label:                 &'static str,
    pub slip_deadzone_frames:  u16,   // min frames before slip flutter fires
    pub slip_flutter_lo_hz:    f32,   // pre-crossover flutter minimum freq
    pub slip_flutter_hi_hz:    f32,   // pre-crossover flutter maximum freq
    pub slip_crossover_deep_hz: f32,  // post-crossover deep-judder freq
}

pub const DRIVETRAIN_PROFILES: [DrivetrainProfile; 5] = [
    DrivetrainProfile {
        label: "Default", slip_deadzone_frames: 2,
        slip_flutter_lo_hz: 50.0, slip_flutter_hi_hz: 80.0,
        slip_crossover_deep_hz: 35.0,
    },
    DrivetrainProfile {
        label: "Mechanical AWD", slip_deadzone_frames: 2,
        slip_flutter_lo_hz: 45.0, slip_flutter_hi_hz: 75.0,
        slip_crossover_deep_hz: 30.0,
    },
    DrivetrainProfile {
        label: "Hybrid Electric", slip_deadzone_frames: 3,
        slip_flutter_lo_hz: 55.0, slip_flutter_hi_hz: 85.0,
        slip_crossover_deep_hz: 28.0,
    },
    DrivetrainProfile {
        label: "RWD", slip_deadzone_frames: 2,
        slip_flutter_lo_hz: 50.0, slip_flutter_hi_hz: 80.0,
        slip_crossover_deep_hz: 32.0,
    },
    DrivetrainProfile {
        label: "FWD", slip_deadzone_frames: 2,
        slip_flutter_lo_hz: 55.0, slip_flutter_hi_hz: 75.0,
        slip_crossover_deep_hz: 35.0,
    },
];

/// Sliding-window auto-detection: classifies drivetrain type from Forza telemetry
/// so the user doesn't need to pick a profile manually.
const AUTO_DETECT_WINDOW: usize = 15; // frames (~250ms at 60Hz)
const AUTO_DETECT_VAR_THRESHOLD: f32 = 0.15; // derivative variance threshold for hybrid
const AUTO_DETECT_DELTA_HYBRID: f32 = 0.3; // front-rear slip delta threshold for RWD bias

/// Motion (gyro + accelerometer) configuration for tilt-steering and gyro aim.
/// All angles in degrees, rates in degrees/second. Tunable live from the Motion panel.
#[derive(Clone)]
pub struct MotionCfg {
    // ── Tilt steering: physically roll the pad like a wheel; drives the virtual
    //    Xbox left-stick X. Self-centering because it reads absolute tilt (accel).
    pub steer_enabled:  bool,
    pub steer_sens:     f32,   // 1.0 = neutral; scales the mapped angle
    pub steer_deadzone: f32,   // degrees of tilt ignored around center
    pub steer_max_deg:  f32,   // tilt angle that equals full lock
    pub steer_invert:   bool,
    pub steer_axis:     u8,    // 0 = roll (left/right tilt), 1 = pitch (forward/back)
    // ── Gyro aim: angular velocity drives the mouse. Independent of output mode.
    pub aim_enabled:    bool,
    pub aim_mode:       u8,    // 0 = always, 1 = hold activation btn, 2 = toggle
    pub aim_sens_x:     f32,
    pub aim_sens_y:     f32,
    pub aim_deadzone:   f32,   // deg/s below which motion is ignored (hand tremor)
    pub aim_invert_y:   bool,
}

impl Default for MotionCfg {
    fn default() -> Self {
        Self {
            steer_enabled:  false,
            steer_sens:     1.0,
            steer_deadzone: 3.0,
            steer_max_deg:  45.0,
            steer_invert:   false,
            steer_axis:     0,
            aim_enabled:    false,
            aim_mode:       0,
            aim_sens_x:     12.0,
            aim_sens_y:     12.0,
            aim_deadzone:   1.5,
            aim_invert_y:   false,
        }
    }
}

pub struct AppState {
    // User settings
    pub profile:       Profile,
    pub output_mode:   OutputMode,  // DualSense (native) vs Xbox (virtual XInput, Windows)
    pub game_source: GameSource, // active telemetry feed (None = simulated engine)
    pub strength_idx:  usize,
    pub gun_weapon:    usize, // index into `weapons` — selected gun feel
    pub melee_weapon:  usize, // index into `melee_weapons` — selected melee feel
    // Runtime feel tables: code defaults (WEAPONS/MELEE_WEAPONS) with feels.json
    // tuning applied. Same length/order/keys as the constants — only values vary.
    pub weapons:       Vec<Weapon>,
    pub melee_weapons: Vec<MeleeWeapon>,
    pub shift_enabled: bool,
    // Live values (read by frontend)
    pub l2_raw:   u8,
    pub r2_raw:   u8,
    pub l2_force: u8,
    pub r2_force: u8,
    // Controller input state (for display)
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub buttons: u16,      // byte8 (face+dpad) in low byte, byte9 (shoulders) in high byte
    pub touchpad_btn: bool, // byte10 bit1
    pub touch0_active: bool,
    pub touch0_x: u16,     // 0-1919
    pub touch0_y: u16,     // 0-1079
    // ── Motion sensors (raw int16 from the DualSense report) ──────────────────
    pub gx: i16, pub gy: i16, pub gz: i16,   // gyro angular velocity (pitch/yaw/roll)
    pub ax: i16, pub ay: i16, pub az: i16,   // accelerometer (gravity + motion)
    pub motion: MotionCfg,
    pub drivetrain: DrivetrainFeel,          // Racing engine/throttle feel character
    pub drivetrain_profile_idx: usize,       // index into DRIVETRAIN_PROFILES
    pub drivetrain_auto: bool,               // true = auto-detect, false = manual
    pub racing_assist_stability: bool,      // throttle firms near traction limit
    pub racing_assist_drift: bool,          // throttle lightens in drift sweet spot
    pub slip_history_rear: VecDeque<f32>,    // rear slip window for auto-detection
    pub slip_history_front: VecDeque<f32>,   // front slip window for auto-detection
    pub aim_toggle_on: bool,                 // latched state for aim_mode == toggle
    // ── Game rumble passthrough (Xbox output) ─────────────────────────────────
    // Raw motor values the game last sent to the virtual pad (written by the
    // ViGEm notification thread), plus the enrichment config and per-frame state.
    pub game_rumble_l: u8,     // large motor (low-freq, strong)
    pub game_rumble_r: u8,     // small motor (high-freq, weak)
    pub pt_enabled:      bool,
    pub pt_intensity:    f32,  // 1.0 = as sent by the game
    pub pt_trigger_kick: bool, // big hits also kick idle triggers
    pub pt_lightbar:     bool, // big hits flash the lightbar
    pub pt_env_l: f32,         // punch envelopes (fast attack, slower release)
    pub pt_env_r: f32,
    pub pt_prev_l: u8,         // spike detection
    pub pt_kick_frames: u8,
    pub pt_lb_frames:   u8,
    pub connected: bool,
    pub error_msg: String,
    // Display
    pub shift_count:    u32,
    pub last_shift_dir: String,
    pub audio_energy:   f32,
    pub smooth_energy:  f32,
    // Audio two-band split (Full) — bass drives the low-freq motor, treble the high-freq
    pub audio_bass:     f32,
    pub audio_treble:   f32,
    pub smooth_bass:    f32,
    pub smooth_treble:  f32,
    // Per-frame counters (written by HID thread only)
    pub shift_left_pulse:    u8,
    pub shift_right_pulse:   u8,
    pub r2_blip_frames:      u8,  // upshift R2 throttle blip
    pub recoil_pulse_frames: u8,
    pub recoil_fired:        bool,
    pub gun_burst_remaining: u8,  // rounds left to fire in the active burst
    pub gun_burst_gap:       u8,  // frames until the next burst round
    pub melee_impact_frames: u8,
    pub melee_impact_fired:  bool,
    pub prev_square: bool,
    pub prev_circle: bool,
    // Shift detection edges (face buttons OR bumpers) for Racing gear feedback.
    pub prev_downshift: bool,
    pub prev_upshift:   bool,
    // Brake-bite kick: a fast brake stab fires a short jolt as the pads grab.
    pub prev_l2_bite:    u8,
    pub brake_bite_frames: u8,
    // ─── Lab live preview ─────────────────────────────────────────────────────
    // When mc_preview is set, the input loop synthesizes Minecraft gameplay state
    // from the controller (R2 = mine/draw/eat, L2 = shield, □ = attack, △ = hurt,
    // ○ = heal) so the real per-item feels can be tested with no mod connected.
    // preview_prev remembers the profile to restore when preview ends.
    pub mc_preview:    bool,
    pub preview_prev:  Option<Profile>,
    pub prev_mc_hit:   bool,
    pub prev_mc_hurt:  bool,
    pub prev_mc_heal:  bool,
    // Edition — set by init_session; enforced in process_frame (not just UI)
    pub edition: Edition,
    // Pro tier ($4) — unlocks the Lab. Set by init_session from the license server.
    // Debug builds default to true so the Lab is usable without a license.
    pub pro: bool,
    // Racing trigger hysteresis — armed above TRIG_HAPTIC_ON, disarmed below TRIG_HAPTIC_OFF
    pub l2_haptic: bool,
    pub r2_haptic: bool,
    // Racing L2 ABS timer — counts frames in the final zone; ABS fires at ABS_DELAY_FRAMES
    pub l2_abs_frames: u8,
    // Free-running phase counter while ABS is pumping — drives the square-wave kick
    pub abs_phase: u8,
    // Simulated-engine phase (0..1) — advanced per frame at a throttle-driven rate so
    // the left-motor RPM rumble pulses faster as you open the throttle.
    pub engine_phase: f32,
    // Free-running road/friction phase (0..1) — advanced every Racing frame at a rate
    // that climbs with tire slip + surface roughness, so the brake friction tremor and
    // surface grain work even under braking when the engine revs (engine_phase) drop.
    pub road_phase: f32,
    // Rev-limiter bounce phase (0..1) — advanced at a fixed cadence (REVLIM_BOUNCE_HZ)
    // when engine RPM is at or near maximum, driving a rhythmic on/off pulse that
    // simulates the ECU cutting and restoring spark at the electronic rev limiter.
    pub revlim_phase: f32,
    // Smoothed engine "RPM" (0..1) — chases throttle with inertia so the rev rate
    // doesn't jitter on light feathering and spins down when you lift.
    pub engine_rpm: f32,
    // Engine revs (0..1) captured at the instant of a gear change, held for the kick
    // window so the shift punch's strength reflects how hard you were revving: a redline
    // shift bangs, a lazy low-rev shift is soft. Overwritten on each new shift.
    pub shift_rpm: f32,
    // Driveline load (0..1): how far the commanded throttle is ahead of current revs,
    // i.e. how hard the engine is pulling to catch up. Surges on throttle stabs (the
    // "power going to the rear" feel) and fades to 0 once the revs match (cruising).
    pub eng_load: f32,
    // Deceleration load (0..1): how far the revs lead the throttle command, i.e. the
    // engine braking / overrun load — the car's momentum driving the driveline back
    // through the engine. Mirrors eng_load for the lift-off / coast-down side.
    pub eng_decel: f32,
    pub eng_prev_throttle: f32, // for tip-in (lash) edge detection
    pub eng_lash_frames: u8,    // driveline take-up thunk countdown on tip-in
    pub burble_frames: u8,      // overrun exhaust-burble crackle countdown on lift-off
    // ── Forza telemetry (written by the UDP bridge; real car state) ───────────
    pub t_on:            bool,  // live RACE data (packets arriving AND IsRaceOn=1)
    pub t_connected:     bool,  // packets physically arriving (real connection proof,
                                // true even while paused/in menus — distinct from t_on)
    pub t_last_rx:       Option<Instant>, // timestamp of last valid packet (watchdog)
    pub t_f123_max_rpm:   f32,           // dynamic max RPM seen for F1 23 normalization
    pub t_rpm:           f32,   // real engine revs, normalized idle→redline (0..1)
    pub t_accel:         f32,   // longitudinal acceleration (m/s²; + accel, − brake)
    pub t_slip_front:    f32,   // front tire slip ratio (lockup / understeer)
    pub t_slip_rear:     f32,   // rear tire slip ratio (wheelspin on RWD)
    pub t_slip_combined: f32,   // overall combined slip (cornering scrub)
    pub t_surface:       f32,   // road surface rumble 0..1 (texture / gravel)
    pub t_kerb:          f32,   // wheel-on-rumble-strip 0..1 (kerbs)
    pub t_speed:         f32,   // m/s
    pub t_gear:          u8,    // current gear (0 = reverse/neutral depending on title)
    pub t_prev_gear:     u8,    // for telemetry-driven shift detection
    pub t_accel_input:   u8,    // game accel pedal input 0-255 (Dash block offset 315)
    pub t_brake_input:   u8,    // game brake pedal input 0-255 (Dash block offset 316)
    // Per-wheel normalized suspension travel (0..1) from the previous packet, kept so the
    // bridge can compute the rate of travel change (= a bump) each frame.
    pub t_susp_fl:       f32,
    pub t_susp_fr:       f32,
    pub t_susp_rl:       f32,
    pub t_susp_rr:       f32,
    // Per-SIDE suspension bump intensity (how fast that side's suspension is moving):
    // left = max(FL,RL) travel delta, right = max(FR,RR). Drives directional road feel —
    // left wheels → left grip motor, right wheels → right grip motor.
    pub t_bump_left:     f32,
    pub t_bump_right:    f32,
    // Signal-processed telemetry (written by forza bridge)
    pub t_heave:           f32,
    pub t_grip_mult:       f32,
    pub t_slip_angle:      f32,
    pub t_tc_active:       bool,
    pub t_surface_fl:      f32,
    pub t_surface_fr:      f32,
    pub t_surface_rl:      f32,
    pub t_surface_rr:      f32,
    pub t_filt_heave:      f32,
    pub t_filt_susp_fl:    f32,
    pub t_filt_susp_fr:    f32,
    pub t_filt_susp_rl:    f32,
    pub t_filt_susp_rr:    f32,
    pub t_ewma_slip_angle: f32,
    pub t_ewma_combined:   f32,
    pub t_slip_rear_frames:  u16,
    pub t_slip_front_frames: u16,
    // Adaptive trigger slew + pneumatic trail (Racing, per-frame)
    pub l2_resist_slew: u8,
    pub r2_resist_slew: u8,
    pub l2_trail_resist: f32,
    // ─── Minecraft bridge state (written by the TCP bridge thread) ───────────
    // mc_item is the category of the currently held item, pushed by the Fabric
    // mod. mc_connected tracks whether the mod is currently piping state.
    pub mc_item:      McItem,
    pub mc_connected: bool,
    // Live gameplay state streamed by the mod (Phase 2).
    pub mc_using:      bool,  // right-click held (drawing bow, eating, etc.)
    pub mc_use_prog:   f32,   // 0..1 progress of the active use (bow draw, chew)
    pub mc_mining:     bool,  // actively breaking a block
    pub mc_blocking:   bool,  // shield raised
    pub mc_sprinting:  bool,
    pub mc_on_ground:  bool,
    pub mc_health:     f32,   // 0..20
    // Transient event pulses (set by the bridge on a rising-edge event, counted
    // down by process_frame).
    pub mc_attack_frames:  u8,
    pub mc_hurt_frames:    u8,
    pub mc_release_frames: u8,
    // Edge-detection + ambient phases (advanced in process_frame).
    pub mc_prev_using:    bool,
    pub mc_prev_use_prog: f32,
    pub mc_mine_phase:    f32,
    pub mc_heart_phase:   f32,
    pub mc_aux_phase:     f32,  // shared sprint-step / chew cadence
    // Audio subprocess handle (for cleanup)
    pub sox_child: Option<std::process::Child>,
    // Audio capture stop flag (Windows WASAPI loopback thread)
    pub audio_stop: Option<Arc<std::sync::atomic::AtomicBool>>,
    // True when the capture thread is streaming real audio into the DualSense's
    // USB haptic channels — the rumble emulation must stand down while this runs.
    pub audio_true_live: Option<Arc<std::sync::atomic::AtomicBool>>,
    // Live haptic EQ/dynamics, shared with the capture thread for lag-free tuning.
    pub audio_tune: Arc<Mutex<AudioTune>>,
    // ─── Trigger Lab (test bench) ───────────────────────────────────────────
    // When test_active is true, process_frame ignores all profile logic and
    // emits a raw trigger-effect report built from these fields. Lets the UI
    // poke any effect mode + params live to feel which ones are worth using.
    pub test_active:       bool,
    pub test_left_mode:    u8,
    pub test_left_params:  [u8; 10],
    pub test_right_mode:   u8,
    pub test_right_params: [u8; 10],
    pub test_rumble_l:     u8,
    pub test_rumble_r:     u8,
    // ─── Racing Lab (personalize) ───────────────────────────────────────────
    // racing_custom holds a user-tuned brake/throttle curve. When racing_lab_active
    // (live preview while the tab is open) or racing_custom_on (saved + applied)
    // is set, the Racing profile uses this instead of the selected Strength preset.
    // Full edition only — Free always falls back to Light.
    pub racing_lab_active: bool,
    pub racing_custom_on:  bool,
    pub racing_custom:     Strength,
    pub racing_tuning:     RacingTuning,
    // Steering FX (Racing, Full) — opt-in toggles. tire_scrub adds cornering grain;
    // throttle_light bleeds throttle resistance when steering hard at high throttle.
    pub tire_scrub_on:     bool,
    pub throttle_light_on: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            profile:             Profile::Racing,
            // Windows: default to Xbox output so XInput-only games (Forza, etc.) see a
            // virtual Xbox pad on launch without needing the UI. Other platforms stay native.
            #[cfg(windows)]
            output_mode:         OutputMode::Xbox,
            #[cfg(not(windows))]
            output_mode:         OutputMode::Dualsense,
            game_source:         GameSource::None,
            strength_idx:        2,
            gun_weapon:          0,            // pistol
            melee_weapon:        0,            // fists
            weapons:             WEAPONS.to_vec(),
            melee_weapons:       MELEE_WEAPONS.to_vec(),
            shift_enabled:       true,
            l2_raw:              0, r2_raw:   0,
            l2_force:            0, r2_force: 0,
            lx: 128, ly: 128, rx: 128, ry: 128, buttons: 0,
            touchpad_btn: false, touch0_active: false, touch0_x: 0, touch0_y: 0,
            gx: 0, gy: 0, gz: 0, ax: 0, ay: 0, az: 0,
            motion: MotionCfg::default(),
            drivetrain: DrivetrainFeel::default(),
            drivetrain_profile_idx: 0,
            drivetrain_auto: false,
            racing_assist_stability: false,
            racing_assist_drift: false,
            slip_history_rear: VecDeque::new(),
            slip_history_front: VecDeque::new(),
            aim_toggle_on: false,
            game_rumble_l: 0, game_rumble_r: 0,
            pt_enabled: true, pt_intensity: 1.0,
            pt_trigger_kick: true, pt_lightbar: true,
            pt_env_l: 0.0, pt_env_r: 0.0, pt_prev_l: 0,
            pt_kick_frames: 0, pt_lb_frames: 0,
            connected:           false,
            error_msg:           String::new(),
            shift_count:         0,
            last_shift_dir:      String::new(),
            audio_energy:        0.0,
            smooth_energy:       0.0,
            audio_bass: 0.0, audio_treble: 0.0,
            smooth_bass: 0.0, smooth_treble: 0.0,
            shift_left_pulse:    0, shift_right_pulse: 0, r2_blip_frames: 0,
            recoil_pulse_frames: 0, recoil_fired:        false,
            gun_burst_remaining: 0, gun_burst_gap:       0,
            melee_impact_frames: 0, melee_impact_fired:  false,
            prev_square:         false, prev_circle: false,
            prev_downshift: false, prev_upshift: false,
            prev_l2_bite: 0, brake_bite_frames: 0,
            mc_preview: false, preview_prev: None,
            prev_mc_hit: false, prev_mc_hurt: false, prev_mc_heal: false,
            edition: if cfg!(debug_assertions) { Edition::Full } else { Edition::Free },
            pro: cfg!(debug_assertions),
            l2_haptic: false, r2_haptic: false,
            l2_abs_frames: 0,
            abs_phase: 0,
            engine_phase: 0.0,
            road_phase: 0.0,
            revlim_phase: 0.0,
            engine_rpm: 0.0,
            shift_rpm: 0.0,
            eng_load: 0.0,
            eng_decel: 0.0,
            eng_prev_throttle: 0.0,
            eng_lash_frames: 0,
            burble_frames: 0,
            t_on: false, t_connected: false, t_last_rx: None, t_f123_max_rpm: 13000.0, t_rpm: 0.0, t_accel: 0.0,
            t_slip_front: 0.0, t_slip_rear: 0.0, t_slip_combined: 0.0,
            t_surface: 0.0, t_kerb: 0.0, t_speed: 0.0, t_gear: 0, t_prev_gear: 0,
            t_accel_input: 0, t_brake_input: 0,
            t_susp_fl: 0.0, t_susp_fr: 0.0, t_susp_rl: 0.0, t_susp_rr: 0.0,
            t_bump_left: 0.0, t_bump_right: 0.0,
            t_heave: 0.0, t_grip_mult: 1.0, t_slip_angle: 0.0, t_tc_active: false,
            t_surface_fl: 0.0, t_surface_fr: 0.0, t_surface_rl: 0.0, t_surface_rr: 0.0,
            t_filt_heave: 0.0,
            t_filt_susp_fl: 0.0, t_filt_susp_fr: 0.0, t_filt_susp_rl: 0.0, t_filt_susp_rr: 0.0,
            t_ewma_slip_angle: 0.0, t_ewma_combined: 0.0,
            t_slip_rear_frames: 0, t_slip_front_frames: 0,
            l2_resist_slew: 0, r2_resist_slew: 0, l2_trail_resist: 0.0,
            mc_item:      McItem::Empty,
            mc_connected: false,
            mc_using: false, mc_use_prog: 0.0, mc_mining: false, mc_blocking: false,
            mc_sprinting: false, mc_on_ground: true, mc_health: 20.0,
            mc_attack_frames: 0, mc_hurt_frames: 0, mc_release_frames: 0,
            mc_prev_using: false, mc_prev_use_prog: 0.0,
            mc_mine_phase: 0.0, mc_heart_phase: 0.0, mc_aux_phase: 0.0,
            sox_child:           None,
            audio_stop:          None,
            audio_true_live:     None,
            audio_tune:          Arc::new(Mutex::new(AudioTune::default())),
            test_active:       false,
            test_left_mode:    0x05, test_left_params:  [0; 10],
            test_right_mode:   0x05, test_right_params: [0; 10],
            test_rumble_l:     0, test_rumble_r: 0,
            racing_lab_active: false,
            racing_custom_on:  false,
            racing_custom:     Strength {
                label: "Custom", brake_start: 158, brake_end: 255, brake_exp: 1.9,
                throttle_start: 58, throttle_end: 172, throttle_exp: 1.4, shift_force: 255,
            },
            racing_tuning:     RacingTuning::default(),
            tire_scrub_on:     false,
            throttle_light_on: false,
        }
    }
}

impl AppState {
    /// True when the Racing Lab custom curve + tuning should override the presets:
    /// Racing profile, Full edition, and either live-previewing or saved-on.
    pub fn racing_custom_active(&self) -> bool {
        self.profile == Profile::Racing
            && self.edition == Edition::Full
            && (self.racing_lab_active || self.racing_custom_on)
    }

    pub fn reset_pulse_state(&mut self) {
        self.shift_left_pulse      = 0;
        self.shift_right_pulse     = 0;
        self.r2_blip_frames        = 0;
        self.recoil_pulse_frames   = 0;
        self.recoil_fired          = false;
        self.gun_burst_remaining   = 0;
        self.gun_burst_gap         = 0;
        self.melee_impact_frames   = 0;
        self.melee_impact_fired    = false;
        self.l2_abs_frames         = 0;
        self.abs_phase             = 0;
        self.engine_phase          = 0.0;
        self.road_phase            = 0.0;
        self.revlim_phase          = 0.0;
        self.slip_history_rear.clear();
        self.slip_history_front.clear();
        self.engine_rpm            = 0.0;
        self.shift_rpm             = 0.0;
        self.mc_attack_frames      = 0;
        self.mc_hurt_frames        = 0;
        self.mc_release_frames     = 0;
        self.mc_mine_phase         = 0.0;
        self.mc_heart_phase        = 0.0;
        self.mc_aux_phase          = 0.0;
    }
}

// ─── UI snapshot ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct StateSnapshot {
    pub profile:        String,
    pub output_mode:    String,  // "dualsense" | "xbox"
    pub strength_idx:   usize,
    pub strength_label: String,
    pub gun_weapon:     String,  // weapon key, e.g. "pistol" | "ar" | "sniper"
    pub melee_weapon:   String,  // melee weapon key, e.g. "knife" | "sledge"
    pub mc_preview:     bool,    // lab live preview synthesizing MC input
    pub shift_enabled:  bool,
    pub l2_raw:         u8,
    pub r2_raw:         u8,
    pub l2_force:       u8,
    pub r2_force:       u8,
    pub lx:             u8,
    pub ly:             u8,
    pub rx:             u8,
    pub ry:             u8,
    pub buttons:        u16,
    pub touchpad_btn:   bool,
    pub touch0_active:  bool,
    pub touch0_x:       u16,
    pub touch0_y:       u16,
    pub edition:        String,  // "free" | "full"
    pub pro:            bool,    // $4 Pro tier — Lab unlocked
    pub connected:      bool,
    pub error_msg:      String,
    pub shift_count:    u32,
    pub last_shift_dir: String,
    pub audio_pct:      f32,
    // Racing Lab personalization
    pub racing_custom_on:  bool,
    pub racing_lab_active: bool,
    pub rc_brake_start:    u8,
    pub rc_brake_end:      u8,
    pub rc_brake_exp:      f32,
    pub rc_throttle_start: u8,
    pub rc_throttle_end:   u8,
    pub rc_throttle_exp:   f32,
    pub rc_shift_force:    u8,
    pub rc_abs_freq:       u8,
    pub rc_abs_delay:      u8,
    pub rc_engine_texture: u8,
    pub rc_feather_end:    u8,
    // Steering FX toggles
    pub tire_scrub_on:     bool,
    pub throttle_light_on: bool,
    // Drivetrain feel (live engine character)
    pub dt_take_up: u8,
    pub dt_idle_hz: u8,
    pub dt_red_hz:  u8,
    pub dt_weight:  u8,
    pub dt_load:    u8,
    pub dt_profile: usize,   // drivetrain profile index
    pub dt_auto:    bool,    // auto-detect enabled
    pub eng_rpm:    f32, // live engine revs 0..1 (tach)
    pub eng_load:   f32, // live driveline load 0..1 (tach)
    pub telem_on:        bool, // live race data (packets + IsRaceOn)
    pub telem_connected: bool, // packets arriving at all (real connection, even paused)
    pub telem_gear:      u8,   // current gear from telemetry (0 if none)
    // Minecraft bridge (for the UI)
    pub mc_item:      String,
    pub mc_connected: bool,
    pub mc_using:     bool,
    pub mc_mining:    bool,
    pub mc_blocking:  bool,
    pub mc_health:    f32,
    // Motion live readout
    pub motion_tilt:  f32,   // current tilt angle (deg) on the selected steer axis
    pub gyro_yaw:     f32,   // raw yaw rate (for the aim crosshair viz)
    pub gyro_pitch:   f32,   // raw pitch rate
    pub aim_active:   bool,  // toggle-mode aim currently engaged
    // Motion config
    pub steer_enabled:  bool,
    pub steer_sens:     f32,
    pub steer_deadzone: f32,
    pub steer_max_deg:  f32,
    pub steer_invert:   bool,
    pub steer_axis:     u8,
    pub aim_enabled:    bool,
    pub aim_mode:       u8,
    pub aim_sens_x:     f32,
    pub aim_sens_y:     f32,
    pub aim_deadzone:   f32,
    pub aim_invert_y:   bool,
    // Game rumble passthrough
    pub pt_enabled:      bool,
    pub pt_intensity:    f32,
    pub pt_trigger_kick: bool,
    pub pt_lightbar:     bool,
    pub game_rl:         u8,   // live game rumble levels (for the UI meter)
    pub game_rr:         u8,
    pub audio_true:      bool, // Audio profile is streaming TRUE haptics over USB audio
    pub audio_sub:       f32,  // haptic EQ: sub band gain (left actuator)
    pub audio_engine:    f32,  // haptic EQ: engine band gain (right actuator)
    pub audio_gate:      f32,  // haptic EQ: expander gate threshold
    pub audio_sub_level: f32,  // live sub-band level 0..1 (UI meter)
    pub audio_eng_level: f32,  // live engine-band level 0..1 (UI meter)
}

impl AppState {
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            profile:        self.profile.as_str().to_string(),
            output_mode:    self.output_mode.as_str().to_string(),
            strength_idx:   self.strength_idx,
            strength_label: STRENGTHS.get(self.strength_idx).unwrap_or(&STRENGTHS[0]).label.to_string(),
            gun_weapon:     WEAPONS[self.gun_weapon].key.to_string(),
            melee_weapon:   MELEE_WEAPONS[self.melee_weapon].key.to_string(),
            mc_preview:     self.mc_preview,
            shift_enabled:  self.shift_enabled,
            l2_raw:         self.l2_raw,  r2_raw:   self.r2_raw,
            l2_force:       self.l2_force, r2_force: self.r2_force,
            lx: self.lx, ly: self.ly, rx: self.rx, ry: self.ry,
            buttons: self.buttons,
            touchpad_btn:  self.touchpad_btn,
            touch0_active: self.touch0_active,
            touch0_x:      self.touch0_x,
            touch0_y:      self.touch0_y,
            edition:       match self.edition { Edition::Free => "free", Edition::Full => "full" }.to_string(),
            pro:           self.pro,
            connected:     self.connected,
            error_msg:      self.error_msg.clone(),
            shift_count:    self.shift_count,
            last_shift_dir: self.last_shift_dir.clone(),
            audio_pct:      (self.smooth_energy / 0.08_f32).min(1.0),
            racing_custom_on:  self.racing_custom_on,
            racing_lab_active: self.racing_lab_active,
            rc_brake_start:    self.racing_custom.brake_start,
            rc_brake_end:      self.racing_custom.brake_end,
            rc_brake_exp:      self.racing_custom.brake_exp,
            rc_throttle_start: self.racing_custom.throttle_start,
            rc_throttle_end:   self.racing_custom.throttle_end,
            rc_throttle_exp:   self.racing_custom.throttle_exp,
            rc_shift_force:    self.racing_custom.shift_force,
            rc_abs_freq:       self.racing_tuning.abs_freq,
            rc_abs_delay:      self.racing_tuning.abs_delay,
            rc_engine_texture: self.racing_tuning.engine_texture,
            rc_feather_end:    self.racing_tuning.feather_end,
            tire_scrub_on:     self.tire_scrub_on,
            throttle_light_on: self.throttle_light_on,
            dt_take_up: self.drivetrain.take_up,
            dt_idle_hz: self.drivetrain.idle_hz,
            dt_red_hz:  self.drivetrain.red_hz,
            dt_weight:  self.drivetrain.weight,
            dt_load:    self.drivetrain.load,
            dt_profile: self.drivetrain_profile_idx,
            dt_auto:    self.drivetrain_auto,
            eng_rpm:    self.engine_rpm,
            eng_load:   self.eng_load,
            telem_on:        self.t_on,
            telem_connected: self.t_connected,
            telem_gear:      self.t_gear,
            mc_item:      self.mc_item.as_str().to_string(),
            mc_connected: self.mc_connected,
            mc_using:     self.mc_using,
            mc_mining:    self.mc_mining,
            mc_blocking:  self.mc_blocking,
            mc_health:    self.mc_health,
            // Motion — live readout for the panel viz
            motion_tilt:  motion_tilt_deg(self),
            gyro_yaw:     self.gz as f32,
            gyro_pitch:   self.gx as f32,
            aim_active:   self.aim_toggle_on,
            // Motion — config (so the UI reflects current values)
            steer_enabled:  self.motion.steer_enabled,
            steer_sens:     self.motion.steer_sens,
            steer_deadzone: self.motion.steer_deadzone,
            steer_max_deg:  self.motion.steer_max_deg,
            steer_invert:   self.motion.steer_invert,
            steer_axis:     self.motion.steer_axis,
            aim_enabled:    self.motion.aim_enabled,
            aim_mode:       self.motion.aim_mode,
            aim_sens_x:     self.motion.aim_sens_x,
            aim_sens_y:     self.motion.aim_sens_y,
            aim_deadzone:   self.motion.aim_deadzone,
            aim_invert_y:   self.motion.aim_invert_y,
            pt_enabled:      self.pt_enabled,
            pt_intensity:    self.pt_intensity,
            pt_trigger_kick: self.pt_trigger_kick,
            pt_lightbar:     self.pt_lightbar,
            game_rl:         self.game_rumble_l,
            game_rr:         self.game_rumble_r,
            audio_true:      self.audio_true_live.as_ref()
                .map(|f| f.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(false),
            audio_sub:       self.audio_tune.lock().map(|t| t.sub_gain).unwrap_or(1.4),
            audio_engine:    self.audio_tune.lock().map(|t| t.engine_gain).unwrap_or(1.6),
            audio_gate:      self.audio_tune.lock().map(|t| t.gate).unwrap_or(0.012),
            audio_sub_level: (self.smooth_bass   / 0.030_f32).min(1.0),
            audio_eng_level: (self.smooth_treble / 0.022_f32).min(1.0),
        }
    }
}

// ─── HID output report builders ──────────────────────────────────────────────
//
// DualSense USB: write 48 bytes: [0x02, ...47-byte-payload]
//
// Full buffer layout (buf[0]=0x02 report ID, buf[k] = payload[k-1]):
//   buf[1]  = validFlag0  (0x0C = enable both trigger effects)
//   buf[2]  = validFlag1  (0x04 = lightbar, 0x10 = player LEDs)
//   buf[11] = right trigger mode
//   buf[12] = right P0
//   buf[13] = right P1
//   buf[14] = right P2
//   buf[22] = left trigger mode
//   buf[23] = left P0
//   buf[24] = left P1
//   buf[25] = left P2
//   buf[45] = lightbar R
//   buf[46] = lightbar G
//   buf[47] = lightbar B
//   buf[44] = player LED pattern

fn haptics_report(lm: u8, lp0: u8, lp1: u8, rm: u8, rp0: u8, rp1: u8, lp2: u8, rp2: u8) -> [u8; 48] {
    let mut b = [0u8; 48];
    b[0] = 0x02; b[1] = 0x0C;
    b[11] = rm; b[12] = rp0; b[13] = rp1; b[14] = rp2;
    b[22] = lm; b[23] = lp0; b[24] = lp1; b[25] = lp2;
    b
}

fn lightbar_report(r: u8, g: u8, b_: u8) -> [u8; 48] {
    let mut b = [0u8; 48];
    b[0] = 0x02; b[2] = 0x04;
    b[45] = r; b[46] = g; b[47] = b_;
    b
}

fn player_led_report(pattern: u8) -> [u8; 48] {
    let mut b = [0u8; 48];
    b[0] = 0x02; b[2] = 0x10;
    b[44] = pattern;
    b
}

// ─── Bluetooth output report ─────────────────────────────────────────────────
//
// USB output report 0x02 = [0x02, <47-byte common payload>].
// BT  output report 0x31 = [0x31, 0x02, <same 47-byte common payload>, …pad…, CRC32].
//
// The common payload (validFlag0/1, motors, trigger effects, lightbar, LEDs) is
// IDENTICAL on both links — it just sits one byte further in over BT because of
// the extra 0x02 tag. So we reuse every USB builder above and re-wrap the result
// here, then append the CRC-32 the controller requires over Bluetooth.
//
// CRC seed: the DualSense computes CRC-32/IEEE over a leading 0xA2 output-report
// transaction byte followed by the first 74 bytes of the report; the 4-byte LE
// result lives in bytes 74..78. crc32fast is the standard reflected CRC-32 used.
const BT_REPORT_LEN: usize = 78;
const BT_CRC_SEED:   u8    = 0xA2;

fn to_bt_report(usb: &[u8; 48]) -> [u8; BT_REPORT_LEN] {
    // Layout per Linux hid-playstation (the authoritative implementation):
    //   [0] = 0x31, [1] = rolling sequence number in the HIGH nibble, [2] = 0x10
    //   output tag, [3..50] = the 47-byte common payload, then reserved, CRC last.
    // Getting the header wrong makes the pad silently drop every report — the old
    // 2-byte header shifted the whole payload and killed all haptics over BT.
    use std::sync::atomic::{AtomicU8, Ordering};
    static BT_SEQ: AtomicU8 = AtomicU8::new(0);
    let seq = BT_SEQ.fetch_add(1, Ordering::Relaxed) & 0x0F;

    let mut b = [0u8; BT_REPORT_LEN];
    b[0] = 0x31;
    b[1] = seq << 4;
    b[2] = 0x10;                           // DS_OUTPUT_TAG
    b[3..50].copy_from_slice(&usb[1..48]); // 47-byte common payload
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&[BT_CRC_SEED]);
    hasher.update(&b[0..BT_REPORT_LEN - 4]);
    let crc = hasher.finalize();
    b[BT_REPORT_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    b
}

/// Write a 48-byte USB-format report to the device, transparently converting to
/// the 78-byte CRC-tagged BT format when the controller is connected wirelessly.
fn write_report(device: &hidapi::HidDevice, transport: Transport, usb: &[u8; 48])
    -> Result<usize, hidapi::HidError>
{
    match transport {
        Transport::Usb       => device.write(usb),
        Transport::Bluetooth => device.write(&to_bt_report(usb)),
    }
}

// ─── Force helpers ────────────────────────────────────────────────────────────

fn clamp01(v: f32) -> f32 { v.clamp(0.0, 1.0) }

/// Boost a resistance force into a firm wall as the trigger nears the bottom, so the
/// finger meets strong pushback around ~220 and stops short of slamming the trigger
/// into its plastic stop. Below the wall zone the base force is unchanged.
fn pedal_wall(base: u8, raw: u8) -> u8 {
    if raw <= PEDAL_WALL_START {
        return base;
    }
    let span = (PEDAL_WALL_END - PEDAL_WALL_START) as f32;
    let t = ((raw - PEDAL_WALL_START) as f32 / span).clamp(0.0, 1.0);
    let wall = (160.0 + 95.0 * t) as u8; // 160 at the start of the zone → 255 firm wall
    base.max(wall)
}

fn racing_forces(s: &AppState) -> (u8, u8) {
    let st = &STRENGTHS[s.strength_idx];
    let tl = clamp01((s.l2_raw as f32 - DEAD_ZONE as f32) / (255.0 - DEAD_ZONE as f32));
    let brake = if s.l2_raw > DEAD_ZONE {
        (st.brake_start as f32 + (st.brake_end as f32 - st.brake_start as f32) * tl.powf(st.brake_exp)).round() as u8
    } else { 0 };
    // Throttle gets a feather zone like the brake: force ramps linearly 0 → throttle_start
    // across the bottom of travel, so engaging the trigger off-rest never snaps from zero
    // resistance to a wall (that snap is the "clunk" when feathering lightly). Past the
    // feather zone it follows the exponential curve.
    let throttle = if s.r2_raw > DEAD_ZONE {
        // Take-up depth is live-tunable (Racing Lab); defaults to THROTTLE_FEATHER_END.
        let fe = s.drivetrain.take_up.max(DEAD_ZONE + 1);
        let base = if s.r2_raw <= fe {
            // Ramp from a lighter floor so the gas resists off rest but stays easier
            // to push than the brake.
            let floor = st.throttle_start as u16 * RACING_THROTTLE_FLOOR_PCT / 100;
            (floor + (st.throttle_start as u16 - floor)
                * (s.r2_raw - DEAD_ZONE) as u16 / (fe - DEAD_ZONE) as u16) as u8
        } else {
            let tr = clamp01((s.r2_raw as f32 - fe as f32) / (255.0 - fe as f32));
            (st.throttle_start as f32
                + (st.throttle_end as f32 - st.throttle_start as f32) * tr.powf(st.throttle_exp)).round() as u8
        };
        // Damper floor: minimum resistance the moment the trigger leaves the
        // deadzone, so the initial push never feels loose or empty. This is the
        // hydraulic preload — like the weight of the pedal linkage itself.
        base.max(THROTTLE_DAMPER_FLOOR) as u8
    } else { 0 };
    (brake, throttle)
}

/// Exponential brake force curve — three zones:
///   0-15%  (raw 0-38):   feather zone, linear 0 → brake_start
///   16-95% (raw 39-242): exponential ramp brake_start → brake_end (shaped by brake_exp)
///   96-100% (raw 243+):  max resistance (ABS zone handled separately)
fn brake_curve(raw: u8, st: &Strength, low_end: u8) -> u8 {
    // Guard: feather zone must end before the ramp's top to avoid a zero-width ramp.
    let low_end = low_end.clamp(1, L2_RAMP_END - 1);
    if raw == 0 { return 0; }
    if raw <= low_end {
        // Ramp from a high floor (not 0) so braking is stiff and solid off rest.
        let floor = st.brake_start as u16 * RACING_BRAKE_FLOOR_PCT / 100;
        (floor + (st.brake_start as u16 - floor) * raw as u16 / low_end as u16) as u8
    } else if raw <= L2_RAMP_END {
        let t = (raw - low_end) as f32 / (L2_RAMP_END - low_end) as f32;
        (st.brake_start as f32
            + (st.brake_end as f32 - st.brake_start as f32) * t.powf(st.brake_exp))
            .round()
            .min(255.0) as u8
    } else {
        st.brake_end
    }
}

/// Full Racing L2 output — brake curve + shift modifiers + ABS delay.
/// Handles hysteresis, ABS timer, and decrement of shift modifier counters.
/// Returns (mode, p0, p1) for use directly in haptics_report.
fn racing_l2(s: &mut AppState, st: &Strength) -> (u8, u8, u8) {
    // Custom profile overrides the global ABS/feather constants when active.
    let custom    = s.racing_custom_active();
    let abs_freq  = if custom { s.racing_tuning.abs_freq.max(1) } else { ABS_FREQ };
    let abs_delay = if custom { s.racing_tuning.abs_delay } else { ABS_DELAY_FRAMES };
    let low_end   = if custom { s.racing_tuning.feather_end } else { L2_LOW_END };

    // Hysteresis — arm/disarm haptic engagement
    if s.l2_raw >= TRIG_HAPTIC_ON  { s.l2_haptic = true;  }
    if s.l2_raw <  TRIG_HAPTIC_OFF { s.l2_haptic = false; s.l2_abs_frames = 0; }

    if !s.l2_haptic {
        return (0x05, 0, 0);
    }

    // Telemetry ABS: real front-wheel slip trips the pump without needing full pedal
    // travel. Fires any time the fronts are actually locking (slip > 0.75) while
    // the brake is armed. Pump rate scales with slip intensity so a mild lock
    // pulses slowly and a full lockup hammers fast.
    let abs_lockup_thresh: f32 = match s.game_source {
        GameSource::F123 | GameSource::Assetto => 0.50,
        _ => 0.75,
    };
    let telem_abs = s.edition == Edition::Full
        && s.t_on
        && s.t_slip_front > abs_lockup_thresh
        && s.l2_haptic;

    // Timer-based ABS: sustained full-brake (no telemetry required). Reset only when
    // neither path is active so a telem_abs event doesn't drain the countdown.
    if s.l2_raw >= L2_FINAL_ZONE {
        if s.l2_abs_frames < abs_delay { s.l2_abs_frames += 1; }
    } else if !telem_abs {
        s.l2_abs_frames = 0;
    }
    if s.edition == Edition::Full && (s.l2_abs_frames >= abs_delay || telem_abs) {
        // ABS pedal pump — use the trigger's ACTIVE drive (mode 0x06) so the motor
        // physically shoves the pedal in and out against your foot. Mode 0x01 only
        // RESISTS, and during hard braking the pedal is already floored with no
        // travel left, so a resistance pump felt like ~1%. 0x06 actively pushes,
        // so each pump is a real shove. When real slip is available, rate scales
        // with lockup severity so the feel matches how badly the tires are sliding.
        s.abs_phase = s.abs_phase.wrapping_add(1);  // kept advancing for rumble sync
        let freq = if telem_abs {
            let extra = ((s.t_slip_front - 0.75) / 0.75 * 6.0) as u8;
            abs_freq.saturating_add(extra).min(15)
        } else {
            abs_freq
        };
        return (0x06, freq, 255);
    }
    s.abs_phase = 0;

    let mut brake_f = brake_curve(s.l2_raw, st, low_end);
    // Aerodynamic downforce — at speed the pedal firms up because aero load pushes
    // the tyres harder into the tarmac, requiring higher hydraulic pressure. Only
    // active when real telemetry is streaming (harmless fallback otherwise).
    if s.edition == Edition::Full && s.t_on && s.t_speed > AERO_MIN_SPEED_MS {
        let speed_factor = ((s.t_speed - AERO_MIN_SPEED_MS)
            / (AERO_MAX_SPEED_MS - AERO_MIN_SPEED_MS)).clamp(0.0, 1.0);
        let aero_mult = 1.0 + speed_factor * AERO_MAX_BOOST;
        brake_f = ((brake_f as f32) * aero_mult).min(255.0) as u8;
    }
    // Surface friction through the brake: rough surfaces (gravel, dirt, grass) add
    // a resistance tremor so the pedal feels grainy and nervous, not just heavy.
    brake_f = if s.t_on && s.t_surface > 0.08 {
        let texture = (s.t_surface - 0.08) / 0.92;
        let noise   = (s.engine_phase * std::f32::consts::TAU * 2.7).sin();
        (brake_f as f32 + texture * 28.0 * noise).clamp(0.0, 255.0) as u8
    } else {
        brake_f
    };
    // Pneumatic trail collapse — during full lock-up the contact patch loses lateral
    // support; unwind brake resistance smoothly instead of holding a rigid wall.
    if s.t_on && s.t_slip_combined > abs_lockup_thresh && s.t_slip_front > abs_lockup_thresh {
        s.l2_trail_resist = signal::pneumatic_trail_decay(
            s.l2_trail_resist.max(brake_f as f32),
            PNEUMATIC_DECAY,
        );
        let trail = s.l2_trail_resist.round().clamp(0.0, 255.0) as u8;
        if trail < 8 {
            return (0x05, 0, 0);
        }
        return (0x01, 0, trail);
    }
    s.l2_trail_resist = brake_f as f32;
    (0x01, 0, brake_f)
}

// ─── Rumble helpers ───────────────────────────────────────────────────────────

/// Patch rumble motors into an existing trigger report in-place.
/// Left motor = low-freq / strong (thud). Right motor = high-freq / weak (buzz).
/// Sets validFlag0 bits 0+1 only when at least one motor is non-zero so we don't
/// clobber the motors every frame when we don't need to.
fn with_rumble(mut report: [u8; 48], left: u8, right: u8) -> [u8; 48] {
    if left > 0 || right > 0 {
        // Follow the Linux hid-playstation convention for firmware >= 2.24 exactly:
        // HAPTICS_SELECT (flag0 bit1) + COMPATIBLE_VIBRATION2 (flag2 bit2) selects the
        // full-strength rumble emulation. The legacy flag0 bit0 path is deliberately
        // attenuated by the firmware (weak DS4-style rumble) and must NOT be set
        // alongside V2, or the firmware can fall back to the soft path.
        report[1]  |= 0x02;
        report[39] |= 0x04;
        report[3] = right;  // right motor: high-freq, weak
        report[4] = left;   // left motor:  low-freq,  strong
    }
    report
}

/// Game rumble passthrough — expand the two flat motor values the game sends to the
/// virtual Xbox pad into something with shape on the DualSense:
/// - punch envelopes (fast attack, slower release) so hits land hard and decay
/// - a frequency split: the strong motor keeps body on the left and bleeds a
///   little texture into the right, instead of a single flat buzz
/// - spike detection arms a trigger kick + lightbar flash (consumed in process_frame)
/// Returns the enriched (left, right) motor levels to blend with profile rumble.
fn game_rumble_mix(s: &mut AppState) -> (u8, u8) {
    if !s.pt_enabled || s.output_mode != OutputMode::Xbox {
        s.pt_env_l = 0.0;
        s.pt_env_r = 0.0;
        s.pt_kick_frames = 0;
        s.pt_prev_l = 0;
        return (0, 0);
    }
    // Perceptual lift: the DualSense's voice-coil actuators barely register low
    // values that an Xbox pad's ERM motors render fine, so run the game's levels
    // through an aggressive gamma curve plus a hard floor — anything the game
    // sends at all should be clearly feelable on this hardware.
    let lift = |v: u8| -> f32 {
        if v == 0 { 0.0 } else { (255.0 * (v as f32 / 255.0).powf(0.55)).max(34.0) }
    };
    let tl = (lift(s.game_rumble_l) * s.pt_intensity).min(255.0);
    let tr = (lift(s.game_rumble_r) * s.pt_intensity).min(255.0);
    let kl = if tl > s.pt_env_l { 0.65 } else { 0.16 };
    let kr = if tr > s.pt_env_r { 0.65 } else { 0.20 };
    s.pt_env_l += (tl - s.pt_env_l) * kl;
    s.pt_env_r += (tr - s.pt_env_r) * kr;
    // A sharp jump on the strong motor reads as an impact (crash, landing, hit).
    if s.game_rumble_l > s.pt_prev_l.saturating_add(50) {
        if s.pt_trigger_kick { s.pt_kick_frames = 8; }
        if s.pt_lightbar     { s.pt_lb_frames   = 8; }
    }
    s.pt_prev_l = s.game_rumble_l;
    let rl = s.pt_env_l.round().min(255.0) as u8;
    let rr = (s.pt_env_r + s.pt_env_l * 0.25).round().min(255.0) as u8;
    (rl, rr)
}

/// Compute per-frame rumble values from the **pre-mutation** AppState so the
/// motors fire on the same frame as their corresponding trigger effect.
fn compute_rumble(s: &AppState, st: &Strength) -> (u8, u8) {
    let (mut rl, mut rr) = (0u8, 0u8);

    match s.profile {
        Profile::Racing => {
            let custom      = s.racing_custom_active();
            let abs_freq    = if custom { s.racing_tuning.abs_freq.max(1) } else { ABS_FREQ };
            let abs_delay   = if custom { s.racing_tuning.abs_delay } else { ABS_DELAY_FRAMES };
            let engine_tex  = if custom { s.racing_tuning.engine_texture } else { 22 };
            // Gear shift clunk — fires during the SLAM phase of the L2 peel
            if s.shift_left_pulse > 0 && s.shift_left_pulse <= SHIFT_SLAM_FRAMES {
                // The shift "kick" is a bassy LOW-freq thud (left motor) — felt in the
                // palms as a solid clunk, not the high-freq buzz that was loud. Front-load
                // it (strongest on the first slam frames, quick decay) so it reads as a
                // crisp clunk rather than a flat thump. Minimal high-freq so it's a thud.
                let is_up = s.r2_blip_frames > 0 && s.shift_right_pulse == 0;
                let prog = s.shift_left_pulse as f32 / SHIFT_SLAM_FRAMES as f32; // 1 fresh → 0 old
                let env  = 0.45 + 0.55 * prog;                                   // sharp attack
                // Floor the clunk force so the shift always lands as a solid bass hit,
                // even on low strength / custom curves with a soft shift_force.
                let force = (st.shift_force as f32).max(215.0);
                rl = (force * env).min(255.0) as u8;                             // strong bass punch
                rr = if is_up {
                    (force * 0.28 * env) as u8                                   // upshift: a touch of snap
                } else {
                    (force * 0.16 * env) as u8                                   // downshift: almost pure thud
                };
            }
            // ABS lockup — a SOFT low-freq (left motor) rumble pulsing in lockstep
            // with the pedal pump, so each shove has body to it instead of feeling
            // hollow. Kept low + at the slow pump rate so it reads as a soft rumble,
            // not the buzzy vibrate from before. `abs_phase` is read pre-mutation so
            // the rumble bump lands on the same frame as the trigger push.
            let abs_active = s.l2_abs_frames >= abs_delay
                || (s.t_on && s.t_slip_front > 0.75 && s.l2_haptic);
            if s.edition == Edition::Full && abs_active {
                let period = (60 / abs_freq.max(1)).clamp(4, 60);
                let push   = (s.abs_phase % period) < (period / 2).max(1);
                rl = rl.max(if push { 95 } else { 30 });
            }
            // Lateral slip dominance — when cornering slip is critical, ambient engine
            // rumble and road texture yield so the limit-handling layer reads clearly.
            let lateral_critical = s.t_on && s.t_slip_angle > LATERAL_SLIP_CRITICAL;
            let ambient_scale = if lateral_critical { 0.0 } else if s.t_on { AMBIENT_RPM_CLAMP } else { 1.0 };
            let surface_scale = if lateral_critical { 0.0 } else { 1.0 };

            // Engine rumble (simulated RPM) — the LOW-freq (left) motor pulses at a rate
            // that climbs with throttle (engine_phase, advanced in process_frame), while
            // the overall intensity also rises with throttle. Lumpy chug near idle →
            // fast smooth flutter at redline. A tremolo, not a hard gate, so it swells
            // between ~55% and 100% of the ceiling instead of cutting out. The
            // "Engine rumble" knob sets that ceiling. A touch of grain on the right
            // motor keeps some high-freq detail.
            if s.edition == Edition::Full && s.engine_rpm > 0.01 {
                let amt   = (s.engine_rpm * engine_tex as f32 * ambient_scale) as f32;
                let wave  = (s.engine_phase * std::f32::consts::TAU).sin() * 0.5 + 0.5; // 0..1
                // Tremolo depth fades in with revs: light throttle = smooth faint rumble
                // (no thumpy clunk on feathering), opening up restores the punchy pulse.
                let depth = (0.45 * (s.engine_rpm / 0.40)).min(0.45);
                let level = (amt * (1.0 - depth + depth * wave)) as u8;
                rl = rl.max(level);
                rr = rr.max((level as u16 * 25 / 100) as u8);
            }
            // Driveline load — a low-freq swell on the left motor that grows with how
            // hard the engine is pulling (eng_load) and with revs, so accelerating
            // under power feels like torque loading to the rear. Fades to nothing once
            // the revs catch the throttle (steady cruise). Scaled by the Load knob.
            if s.edition == Edition::Full && s.drivetrain.load > 0 {
                let load_amt = s.eng_load
                    * (0.35 + 0.65 * s.engine_rpm)
                    * (s.drivetrain.load as f32 * 2.4);
                rl = rl.max(load_amt.min(220.0) as u8);
            }
            // Tip-in lash — a sharp driveline take-up thunk when the throttle snaps on
            // from closed, the moment the slack loads and power hooks to the rear.
            if s.edition == Edition::Full && s.eng_lash_frames > 0 {
                let thunk = (s.drivetrain.load as f32 * 2.0).min(200.0) as u8;
                rl = rl.max(thunk);
                rr = rr.max((thunk as u16 * 40 / 100) as u8);
            }
            // Tire scrub (opt-in) — high-freq grain on the right motor that grows with
            // steering angle × pedal load, so loading the car in a corner (on throttle
            // OR brake) feels like the tires scrubbing. Steering deadzone keeps the
            // straights clean. Left stick X = steering.
            if s.edition == Edition::Full && s.tire_scrub_on {
                let steer = ((s.lx as i16 - 128).abs() as f32 / 128.0).clamp(0.0, 1.0);
                if steer > 0.12 {
                    // Baseline load so even a light pedal still scrubs while cornering;
                    // pedal load adds the extra bite. Boosted ceiling (160) so it reads
                    // clearly even when the engine rumble shares the right motor.
                    let pedal = s.l2_raw.max(s.r2_raw) as f32 / 255.0;
                    let load  = 0.45 + 0.55 * pedal;
                    let scrub = ((steer - 0.12) / 0.88) * load;
                    rr = rr.max((scrub * 160.0) as u8);
                }
            }
            // Throttle-lightening slip judder — when the rears "let go" (hard steer at
            // high throttle), add a right-motor judder so the traction loss reads as
            // wheelspin chatter, not just a quiet drop in trigger resistance.
            if s.edition == Edition::Full && s.throttle_light_on && s.r2_raw > DEAD_ZONE {
                let steer = ((s.lx as i16 - 128).abs() as f32 / 128.0).clamp(0.0, 1.0);
                if steer > 0.30 {
                    let slip = ((steer - 0.30) / 0.70) * (s.r2_raw as f32 / 255.0);
                    rr = rr.max((slip * 130.0) as u8);
                }
            }
            // Engine braking / decel weight — the overrun load (eng_decel) is the car's
            // momentum driving the driveline back through the engine. A low-freq weight
            // swell that's strongest right after a lift and sustains through the coast,
            // firmer while braking (trailing into a corner). Scaled by the Load knob, so
            // the same slider governs both the on-power pull and the off-power weight.
            if s.edition == Edition::Full && s.eng_decel > 0.02 {
                let on_brake = if s.l2_raw > DEAD_ZONE { 1.0 } else { 0.6 };
                let mut w = (s.eng_decel * (s.drivetrain.load as f32 * 2.2) * on_brake).min(185.0);
                // While coasting off-throttle with the revs up, throb the weight at the
                // engine's rotation rate so the engine braking feels like the motor turning
                // over and dragging — body behind the throttle-pedal grumble — instead of a
                // flat swell. Strongest right after a downshift when revs + decel spike.
                if s.r2_raw <= DEAD_ZONE && s.engine_rpm > 0.18 {
                    let wave  = (s.engine_phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                    w *= 0.6 + 0.7 * wave;
                }
                rl = rl.max(w.min(220.0) as u8);
                rr = rr.max((w * 0.3) as u8);
            }
            // Brake-bite — a sharp both-motor kick the instant you stab hard into the
            // brake, like the pads grabbing the disc.
            if s.edition == Edition::Full && s.brake_bite_frames > 0 {
                rl = rl.max(180);
                rr = rr.max(110);
            }
            // Tire-bite grip — feeding throttle through a corner loads the contact patch;
            // a low swell on the left motor that grows with steer × throttle adds the
            // weight of the tires hooking up (pairs with the firmer gas pedal).
            if s.edition == Edition::Full && s.r2_raw > DEAD_ZONE {
                let steer = ((s.lx as i16 - 128).abs() as f32 / 128.0).clamp(0.0, 1.0);
                if steer > 0.15 {
                    let corner = ((steer - 0.15) / 0.85) * (s.r2_raw as f32 / 255.0);
                    rl = rl.max((corner * 95.0) as u8);
                    rr = rr.max((corner * 45.0) as u8);
                }
            }
            // Overrun burble — irregular sharp pops on the high-freq motor (with a little
            // body on the low) when lifting off throttle with revs up, like the exhaust
            // crackling on the overrun. Fades across the burble window.
            if s.edition == Edition::Full && s.burble_frames > 0 {
                let n = (s.burble_frames as u32).wrapping_mul(2654435761) >> 24;
                let fade = s.burble_frames as f32 / 22.0;
                if n > 130 {
                    let pop = n as f32 * fade;
                    rr = rr.max(pop as u8);
                    rl = rl.max((pop * 0.4) as u8);
                }
            }
            // ── Real-telemetry road feel (Forza Data Out) ─────────────────────────
            // These come straight from the car, so they fire on actual events, not
            // inferred ones: wheelspin under power, lockup under braking, cornering
            // scrub, road surface texture, and kerb strikes.
            if s.edition == Edition::Full && s.t_on {
                // Dynamic vertical-load gate (from the GT7 telemetry research). A tire
                // that's unloaded — suspension near full droop, i.e. a wheel hanging in
                // the air over a crest — spins or locks *freely* and transmits almost no
                // force into the chassis. Slip telemetry still spikes, so without this a
                // wheel spinning in mid-air produces a violent FALSE buzz. NormSuspension
                // Travel is 0.0 at full droop (no load) → 1.0 at full compression
                // (loaded), so we fade each axle's slip haptics in from a small droop
                // threshold. This only ATTENUATES the airborne case: a normally grounded
                // wheel (springs preloaded by the car's weight) sits well above the
                // threshold and is completely untouched. Per-axle so one wheel lifting
                // doesn't kill the other side. Tune/disable via LOAD_GATE_* below.
                let load = |susp: f32| -> f32 {
                    ((susp - LOAD_GATE_DROOP) / LOAD_GATE_SPAN).clamp(0.0, 1.0)
                };
                let rear_load   = load(s.t_susp_rl.max(s.t_susp_rr));
                let front_load  = load(s.t_susp_fl.max(s.t_susp_fr));
                let corner_load = load(s.t_susp_fl.max(s.t_susp_fr).max(s.t_susp_rl).max(s.t_susp_rr));

                // Per-game slip threshold for rumble: F1/AC tyres have more grip.
                let rumble_thresh: f32 = match s.game_source {
                    GameSource::F123 | GameSource::Assetto => 0.12,
                    _ => 0.20,
                };
                // Wheelspin — rear tires slipping under power → strong high-freq grain.
                // Pacejka-shaped intensity so micro-slips stay subtle, deep slips punch.
                if s.t_slip_rear > rumble_thresh && s.r2_raw > DEAD_ZONE {
                    let pacejka = signal::pacejka_haptic(s.t_slip_rear);
                    let spin = ((s.t_slip_rear - rumble_thresh) / (1.0 - rumble_thresh)).clamp(0.0, 1.0) * rear_load * pacejka;
                    rr = rr.max((spin * 240.0) as u8);
                    rl = rl.max((spin * 120.0) as u8);
                }
                // Lockup — front tires sliding under braking → heavy coarse judder.
                if s.t_slip_front > rumble_thresh && s.l2_raw > DEAD_ZONE {
                    let pacejka = signal::pacejka_haptic(s.t_slip_front);
                    let lock = ((s.t_slip_front - rumble_thresh) / (1.0 - rumble_thresh)).clamp(0.0, 1.0) * front_load * pacejka;
                    rl = rl.max((lock * 220.0) as u8);
                    rr = rr.max((lock * 160.0) as u8);
                }
                // Cornering scrub — combined slip + lateral angle → tire-howl grain.
                if s.t_slip_combined > 0.25 || s.t_slip_angle > 0.08 {
                    let scrub_slip = ((s.t_slip_combined - 0.25) / 0.75).clamp(0.0, 1.0);
                    let scrub_ang  = ((s.t_slip_angle - 0.08) / 0.20).clamp(0.0, 1.0);
                    let scrub = scrub_slip.max(scrub_ang) * corner_load;
                    rr = rr.max((scrub * 180.0) as u8);
                    rl = rl.max((scrub * 90.0) as u8);
                }
                // Stereophonic road surface texture — left/right voice coils from per-wheel rumble.
                if surface_scale > 0.0 {
                    let left_surf  = s.t_surface_fl.max(s.t_surface_rl);
                    let right_surf = s.t_surface_fr.max(s.t_surface_rr);
                    if left_surf > 0.05 || right_surf > 0.05 {
                        let wave_l = (s.road_phase * std::f32::consts::TAU * 1.3).sin().abs();
                        let wave_r = (s.road_phase * std::f32::consts::TAU * 1.7).sin().abs();
                        rl = rl.max((left_surf * SURFACE_STEREO_SCALE * wave_l * 140.0) as u8);
                        rr = rr.max((right_surf * SURFACE_STEREO_SCALE * wave_r * 140.0) as u8);
                    } else if s.t_surface > 0.05 {
                        rl = rl.max((s.t_surface * surface_scale * 140.0) as u8);
                        rr = rr.max((s.t_surface * surface_scale * 120.0) as u8);
                    }
                }
                // Kerb strike — sharp hard rattle when a wheel hits a rumble strip.
                if s.t_kerb > 0.5 {
                    rl = rl.max(200);
                    rr = rr.max(220);
                }
                // Suspension bumps — DIRECTIONAL road feel. Left wheels (FL,RL) drive the
                // left grip motor, right wheels (FR,RR) the right, so a bump, dip, crest or
                // kerb under one side of the car is felt on that side of the pad. Intensity
                // tracks how fast the suspension is moving, so smooth tarmac stays quiet
                // and rough ground / impacts jolt. Adds onto the road feel above.
                let bump_l = (s.t_bump_left  * 3500.0).min(210.0) as u8;
                let bump_r = (s.t_bump_right * 3500.0).min(210.0) as u8;
                if bump_l > 10 { rl = rl.saturating_add(bump_l); }
                if bump_r > 10 { rr = rr.saturating_add(bump_r); }
                // Redline upshift cue — driven straight off the car's REAL revs (t_rpm),
                // not the simulated engine: a strong rising flutter as you near the redline
                // tells you to upshift by feel. Added on top (not max'd) so the engine
                // rumble can't swallow it, with a little body on the low motor too.
                if s.t_rpm > 0.85 {
                    let over = ((s.t_rpm - 0.85) / 0.15).clamp(0.0, 1.0);
                    rr = rr.saturating_add((110.0 + over * 145.0) as u8); // 110 → 255
                    rl = rl.saturating_add((40.0 + over * 80.0) as u8);   // body on the low motor
                }
                // Rev-limiter bounce — a distinct rhythmic thump at max RPM that
                // signals the ECU cutting spark, distinct from the progressive
                // redline-approach flutter above. Fires off real telemetry RPM or
                // the simulated engine, whichever is driving.
                // Full edition only — Free tier just gets the redline flutter.
                if s.edition == Edition::Full
                    && ((s.t_on && s.t_rpm >= REVLIM_RPM_THRESHOLD)
                        || (!s.t_on && s.engine_rpm >= REVLIM_RPM_THRESHOLD))
                {
                    let pulse = (s.revlim_phase * std::f32::consts::TAU).sin();
                    if pulse > 0.0 {
                        let amp = (pulse * 180.0) as u8;
                        rl = rl.max(amp);
                        rr = rr.max((amp as u16 * 50 / 100) as u8);
                    }
                }
            }
        }

        Profile::Gun => {
            let w = s.weapons[s.gun_weapon];
            // ADS (L2 held) → scale rumble down for a planted, stable feel
            let ads = if s.l2_raw > DEAD_ZONE { 0.55_f32 } else { 1.0_f32 };
            let pattern = if s.edition == Edition::Full { w.pattern } else { GunMode::Semi };

            // Per-shot kick rumble (Semi/Burst) — fires during the slam phase only,
            // matching the active 0x06 kick window so it stays a single thump.
            if s.recoil_pulse_frames > 0 && s.recoil_pulse_frames <= w.kick_frames {
                rl = (w.rumble_l as f32 * ads) as u8;
                rr = (w.rumble_r as f32 * ads) as u8;
            }
            // Sustained auto-fire rumble while the trigger is held past the break.
            if pattern == GunMode::Auto && s.r2_raw >= GUN_AUTO_BREAK_POS {
                rl = rl.max((w.rumble_l as f32 * ads) as u8);
                rr = rr.max((w.rumble_r as f32 * ads) as u8);
            }
        }

        Profile::Melee => {
            // Connect thump — per-weapon both-motor hit while the impact kick plays.
            if s.melee_impact_frames > 0 {
                let w = s.melee_weapons[s.melee_weapon];
                rl = w.rumble_l; rr = w.rumble_r;
            }
        }

        Profile::Audio => {
            // Two-band reactive — bass → left (low-freq) motor for deep body rumble,
            // treble → right (high-freq) motor for grain/detail. Reflects the actual
            // sound, so it adapts to any track/engine instead of a flat single buzz.
            // When TRUE haptics is streaming the waveform into the pad's USB audio
            // channels, the emulation must stay silent — setting the rumble flags
            // would make the firmware seize the actuators away from the audio feed.
            let true_haptics = s.audio_true_live.as_ref()
                .map(|f| f.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(false);
            // smooth_bass now carries the SUB band (kicks → left/low motor) and
            // smooth_treble the ENGINE band (revs → right/high motor). Narrower
            // bands read lower, so the divisors are tighter and the engine band is
            // scaled up to make RPM intensity clearly felt in reactive mode.
            if s.edition == Edition::Full && !true_haptics {
                rl = (s.smooth_bass   / 0.030_f32 * 200.0).min(200.0) as u8;
                rr = (s.smooth_treble / 0.022_f32 * 170.0).min(170.0) as u8;
            }
        }

        Profile::Static => {
            // Strain feel — slight rumble only when both triggers are held simultaneously
            if s.l2_raw > DEAD_ZONE && s.r2_raw > DEAD_ZONE {
                rl = 18; rr = 18;
            }
        }

        Profile::Minecraft => {
            use std::f32::consts::TAU;
            let bowlike = matches!(s.mc_item, McItem::Bow | McItem::Crossbow | McItem::Trident);

            // Hurt jolt — a sharp both-motor hit; harder when health is already low.
            if s.mc_hurt_frames > 0 {
                let lowf = if s.mc_health <= 8.0 { 1.0 } else { 0.75 };
                rl = (220.0 * lowf) as u8;
                rr = (170.0 * lowf) as u8;
            }
            // Attack connect — sword/axe swing landing (axe = heavier).
            if s.mc_attack_frames > 0 && matches!(s.mc_item, McItem::Sword | McItem::Axe) {
                let amp = if s.mc_item == McItem::Axe { 205 } else { 150 };
                rl = rl.max(amp);
                rr = rr.max((amp as u16 * 60 / 100) as u8);
            }
            // Bow/crossbow/trident release twang — quick crisp snap.
            if s.mc_release_frames > 0 {
                rr = rr.max(175);
                rl = rl.max(90);
            }
            // Mining grind — textured right-motor grain at a tool-specific rhythm.
            // Pickaxe/axe on hard material bite harder than a shovel in dirt.
            if s.mc_mining {
                let (peak, base) = match s.mc_item {
                    McItem::Pickaxe => (155.0, 60.0),
                    McItem::Axe     => (150.0, 60.0),
                    McItem::Shovel  => (95.0,  35.0),
                    McItem::Hoe     => (110.0, 40.0),
                    _               => (110.0, 40.0),
                };
                let wave = (s.mc_mine_phase * TAU).sin() * 0.5 + 0.5;
                let g = base + (peak - base) * wave;
                rr = rr.max(g as u8);
                rl = rl.max((g * 0.30) as u8);
            }
            // Bow draw hum — faint string tremble once it's nearly taut.
            if s.mc_using && bowlike && s.mc_use_prog > 0.6 {
                rr = rr.max(((s.mc_use_prog - 0.6) / 0.4 * 40.0) as u8);
            }
            // Eating / drinking — gentle gulp pulse on the low-freq motor.
            if s.mc_using && s.mc_item == McItem::Food {
                let wave = (s.mc_aux_phase * TAU).sin() * 0.5 + 0.5;
                rl = rl.max((25.0 + 35.0 * wave) as u8);
            }
            // Low-health heartbeat — a slow double-thump that grows as health drops.
            if s.mc_health > 0.0 && s.mc_health <= MC_LOW_HEALTH {
                let p = s.mc_heart_phase;
                let thump = |x: f32| (x.rem_euclid(1.0) * TAU).sin().max(0.0).powi(3);
                let beat = thump(p) + 0.7 * thump(p - 0.16);
                let intensity = 45.0 + (1.0 - s.mc_health / MC_LOW_HEALTH) * 75.0;
                rl = rl.max((beat * intensity).min(165.0) as u8);
            }
            // Sprint footfalls — subtle locomotion cadence on the ground.
            if s.mc_sprinting && s.mc_on_ground && !s.mc_using {
                let step = (s.mc_aux_phase * TAU).sin().max(0.0).powi(4);
                rl = rl.max((step * 45.0) as u8);
            }
        }
    }

    (rl, rr)
}

// ─── Trigger Lab raw report ─────────────────────────────────────────────────
// Builds a full 10-byte-per-trigger effect block straight from the test fields.
// Right trigger: mode at b[11], params b[12..22]. Left: mode b[22], params b[23..33].
fn test_report(s: &AppState) -> [u8; 48] {
    let mut b = [0u8; 48];
    b[0] = 0x02; b[1] = 0x0C;                // enable both trigger effects
    b[11] = s.test_right_mode;
    b[12..22].copy_from_slice(&s.test_right_params);
    b[22] = s.test_left_mode;
    b[23..33].copy_from_slice(&s.test_left_params);
    if s.test_rumble_l > 0 || s.test_rumble_r > 0 {
        // V2 rumble convention (same as with_rumble): HAPTICS_SELECT +
        // COMPATIBLE_VIBRATION2 — must NOT set bit0 (legacy DS4 path) or
        // firmware >= 2.24 falls back to the attenuated weak rumble.
        b[1]  |= 0x02;
        b[39] |= 0x04;
        b[3] = s.test_rumble_r;
        b[4] = s.test_rumble_l;
    }
    b
}

// ─── Frame processor ──────────────────────────────────────────────────────────

/// Drivetrain auto-detection from Forza telemetry.  Pushes current slip values
/// into per-axle ring buffers; when the window is full, computes the variance of
/// the rear-slip discrete derivative (detects eTC square-wave oscillation) and
/// the mean front-to-rear slip delta (detects torque bias).  Sets
/// `drivetrain_profile_idx` if confidence is high, otherwise leaves the current
/// selection unchanged.
fn auto_detect_drivetrain(s: &mut AppState) {
    if !s.t_on || !s.drivetrain_auto {
        return;
    }
    // Fill sliding windows
    s.slip_history_rear.push_back(s.t_slip_rear);
    s.slip_history_front.push_back(s.t_slip_front);
    while s.slip_history_rear.len() > AUTO_DETECT_WINDOW {
        s.slip_history_rear.pop_front();
    }
    while s.slip_history_front.len() > AUTO_DETECT_WINDOW {
        s.slip_history_front.pop_front();
    }
    if s.slip_history_rear.len() < AUTO_DETECT_WINDOW
        || s.slip_history_front.len() < AUTO_DETECT_WINDOW
    {
        return; // not enough data yet
    }

    // Feature 1 — variance of the discrete derivative of rear slip.
    // Hybrid eTC produces violent oscillation (square wave) → high variance.
    let rear: Vec<f32> = s.slip_history_rear.iter().copied().collect();
    let mut deriv = 0.0f32;
    let mut deriv_sq = 0.0f32;
    let n = (AUTO_DETECT_WINDOW - 1) as f32;
    for i in 1..AUTO_DETECT_WINDOW {
        let d = rear[i] - rear[i - 1];
        deriv += d;
        deriv_sq += d * d;
    }
    let mean_deriv = deriv / n;
    let var_deriv = (deriv_sq / n) - (mean_deriv * mean_deriv);
    let var_deriv = if var_deriv > 0.0 { var_deriv } else { 0.0 };

    // Feature 2 — mean front-to-rear slip delta.
    let mean_rear: f32 = rear.iter().sum::<f32>() / AUTO_DETECT_WINDOW as f32;
    let front: Vec<f32> = s.slip_history_front.iter().copied().collect();
    let mean_front: f32 = front.iter().sum::<f32>() / AUTO_DETECT_WINDOW as f32;
    let delta = mean_rear - mean_front;

    // Classification
    let idx = if var_deriv > AUTO_DETECT_VAR_THRESHOLD && delta > AUTO_DETECT_DELTA_HYBRID {
        2 // Hybrid Electric
    } else if var_deriv <= AUTO_DETECT_VAR_THRESHOLD && delta.abs() < 0.10 {
        1 // Mechanical AWD
    } else if delta > 0.50 {
        3 // RWD
    } else if delta < -0.30 {
        4 // FWD
    } else {
        0 // Default
    };
    s.drivetrain_profile_idx = idx;
}

fn process_frame(s: &mut AppState) -> [u8; 48] {
    // Trigger Lab override — bypass all profile logic, emit the raw test effect.
    if s.test_active {
        return test_report(s);
    }

    // Racing Lab custom curve (Full only, Racing profile) takes priority over the
    // strength preset; otherwise Free tier is locked to Light, else the user's pick.
    let st = if s.profile == Profile::Racing
                && s.edition == Edition::Full
                && (s.racing_lab_active || s.racing_custom_on) {
        s.racing_custom
    } else if s.edition == Edition::Free {
        STRENGTHS[0]
    } else {
        STRENGTHS[s.strength_idx]
    };

    // Active drivetrain profile — selected by the user to match the vehicle.
    // Index 0 ("Default") reproduces the original feel.  Used by the Racing
    // slip-deadzone gate and frequency-crossover logic.
    let dp = &DRIVETRAIN_PROFILES[s.drivetrain_profile_idx
        .min(DRIVETRAIN_PROFILES.len() - 1)];

    // Advance the simulated-engine phase (Racing, Full) so the left-motor RPM rumble
    // pulses faster as the throttle opens — lumpy near idle, fast flutter at redline.
    // engine_rpm chases the throttle with inertia (so light feathering doesn't make
    // the rev rate jitter) and spins down when you lift.
    if s.profile == Profile::Racing && s.edition == Edition::Full {
        let target = if s.r2_raw > DEAD_ZONE { s.r2_raw as f32 / 255.0 } else { 0.0 };
        // Tip-in lash: throttle snapping on from near-closed loads the driveline as the
        // slack takes up and power hooks to the rear — a brief thunk you can feel.
        if target > 0.15 && s.eng_prev_throttle < 0.05 {
            s.eng_lash_frames = 5;
        }
        // Overrun burble — lifting off the throttle while the revs are up pops and
        // crackles like an exhaust on the overrun.
        if s.eng_prev_throttle - target > 0.25 && s.engine_rpm > 0.30 {
            s.burble_frames = 22;
        }
        s.eng_prev_throttle = target;

        if s.t_on {
            // ── Real Forza telemetry drives the engine ────────────────────────────
            // Run auto-detection first so the dp binding (taken next frame) reflects
            // the classified drivetrain.
            auto_detect_drivetrain(s);
            // Real revs → the pulse pitch and rev-matching are genuine, not inferred.
            s.engine_rpm = s.t_rpm;
            // Real longitudinal G is the true driveline load: forward accel pulls to the
            // rear, deceleration loads engine braking / the brakes. (~9.81 m/s² = 1 g.)
            let g = s.t_accel / 9.81;
            s.eng_load  = (g * 0.85).clamp(0.0, 1.0);
            s.eng_decel = ((-g) * 0.85).clamp(0.0, 1.0);
            // Shifts from the real gear change — exact timing. Down = revs jump (lash),
            // up = brief blip. (Bumper detection is disabled while telemetry is live.)
            if s.shift_enabled && s.t_gear != s.t_prev_gear && s.t_gear > 0 && s.t_prev_gear > 0 {
                // Capture the real revs at the shift so the kick scales with them.
                s.shift_rpm = s.engine_rpm;
                if s.t_gear < s.t_prev_gear {
                    s.shift_left_pulse  = SHIFT_TOTAL_FRAMES;
                    s.shift_right_pulse = SHIFT_TOTAL_FRAMES;
                    s.r2_blip_frames    = 0;
                    s.eng_lash_frames   = 5;
                    s.last_shift_dir    = "▼ Down".into();
                    s.shift_count      += 1;
                } else {
                    s.shift_left_pulse  = UPSHIFT_SLAM_FRAMES;
                    s.shift_right_pulse = 0;
                    s.r2_blip_frames    = R2_BLIP_FRAMES;
                    s.last_shift_dir    = "▲ Up".into();
                    s.shift_count      += 1;
                }
            }
            s.t_prev_gear = s.t_gear;
        } else {
            // ── Inferred model (no telemetry) ─────────────────────────────────────
            // Inertia lag (live "drivetrain weight"): heavier = revs build/settle slower.
            let (k_up, k_down) = drivetrain_inertia(s.drivetrain.weight);
            let k = if target > s.engine_rpm { k_up } else { k_down };
            s.engine_rpm += (target - s.engine_rpm) * k;
            // Driveline load = how far throttle is ahead of the revs (engine straining to
            // catch up). Surges when you ask for more than you've got, fades as revs meet
            // the command. Smoothed a touch so it reads as a swell, not a spike.
            let demand = (target - s.engine_rpm).max(0.0);
            s.eng_load += (demand - s.eng_load) * 0.35;
            // Decel/overrun load — revs leading the throttle command (engine braking).
            let overrun = (s.engine_rpm - target).max(0.0);
            s.eng_decel += (overrun - s.eng_decel) * 0.30;
        }
        if s.engine_rpm < 0.01 {
            s.engine_rpm = 0.0;
            s.engine_phase = 0.0;
            s.revlim_phase = 0.0;
        } else {
            // Pulse rate climbs idle_hz → red_hz with revs (live "idle chug" / redline).
            let idle = s.drivetrain.idle_hz as f32;
            let red  = (s.drivetrain.red_hz as f32).max(idle + 1.0);
            let freq = idle + s.engine_rpm * (red - idle);
            s.engine_phase = (s.engine_phase + freq / 60.0).fract();
            // Rev-limiter bounce phase advances at fixed cadence only when revs are
            // at the ceiling so it drives a rhythmic pulse even if the engine rpm
            // itself is saturated.
            if s.engine_rpm >= REVLIM_RPM_THRESHOLD || (s.t_on && s.t_rpm >= REVLIM_RPM_THRESHOLD)
            {
                s.revlim_phase = (s.revlim_phase + REVLIM_BOUNCE_HZ / 60.0).fract();
            }
        }
        // Free-running road/friction phase: a grind rate that climbs with how hard the
        // tires are working (max slip) and how rough the surface is, so the friction
        // tremor speeds up the more the car is sliding. Always advances (independent of
        // revs) so the brake feels alive even with the throttle shut.
        if s.t_on {
            let work = s.t_slip_front.max(s.t_slip_rear).max(s.t_slip_combined)
                .max(s.t_surface * 1.5);
            let grind_hz = 16.0 + work.clamp(0.0, 2.0) * 14.0; // 16 → ~44 Hz
            s.road_phase = (s.road_phase + grind_hz / 60.0).fract();
            // Slip-duration counters — ignore transient micro-slips (< ~25 ms).
            if s.t_slip_rear > 0.20 {
                s.t_slip_rear_frames = s.t_slip_rear_frames.saturating_add(1);
            } else {
                s.t_slip_rear_frames = 0;
            }
            if s.t_slip_front > 0.20 {
                s.t_slip_front_frames = s.t_slip_front_frames.saturating_add(1);
            } else {
                s.t_slip_front_frames = 0;
            }
        } else {
            s.road_phase = 0.0;
        }
        if s.eng_lash_frames > 0 { s.eng_lash_frames -= 1; }
        if s.brake_bite_frames > 0 { s.brake_bite_frames -= 1; }
        if s.burble_frames > 0 { s.burble_frames -= 1; }
        // Shift pulse counters drive the bassy shift thud in compute_rumble. The
        // triggers are no longer hijacked during a shift (they stay live so the
        // throttle keeps carrying the load when you feather + downshift), so the
        // counters are wound down here instead of in the old trigger-override branch.
        if s.shift_left_pulse  > 0 { s.shift_left_pulse  -= 1; }
        if s.shift_right_pulse > 0 { s.shift_right_pulse -= 1; }
        if s.r2_blip_frames    > 0 { s.r2_blip_frames    -= 1; }
    }

    // Minecraft ambient phases + use-release edge detection (before compute_rumble
    // so the rumble and trigger logic see the same frame).
    if s.profile == Profile::Minecraft {
        let bowlike = matches!(s.mc_item, McItem::Bow | McItem::Crossbow | McItem::Trident);
        // Releasing a charged bow/crossbow/trident → fire the twang one-shot.
        if bowlike && s.mc_prev_using && !s.mc_using && s.mc_prev_use_prog > 0.4 {
            s.mc_release_frames = MC_RELEASE_FRAMES;
        }
        s.mc_prev_using    = s.mc_using;
        s.mc_prev_use_prog = s.mc_use_prog;

        // Mining grind rhythm (faster/harder tools chew quicker).
        if s.mc_mining {
            let hz = match s.mc_item {
                McItem::Pickaxe => 9.0, McItem::Axe => 8.0,
                McItem::Shovel  => 6.0, _ => 7.0,
            };
            s.mc_mine_phase = (s.mc_mine_phase + hz / 60.0).fract();
        } else {
            s.mc_mine_phase = 0.0;
        }
        // Heartbeat only while wounded.
        if s.mc_health > 0.0 && s.mc_health <= MC_LOW_HEALTH {
            s.mc_heart_phase = (s.mc_heart_phase + MC_HEART_HZ / 60.0).fract();
        } else {
            s.mc_heart_phase = 0.0;
        }
        // Shared aux cadence: chew while eating, otherwise sprint footfalls.
        let eating = s.mc_using && s.mc_item == McItem::Food;
        if eating || (s.mc_sprinting && s.mc_on_ground) {
            let hz = if eating { MC_CHEW_HZ } else { MC_STEP_HZ };
            s.mc_aux_phase = (s.mc_aux_phase + hz / 60.0).fract();
        } else {
            s.mc_aux_phase = 0.0;
        }
    }

    // Rumble computed from pre-mutation state — stays in sync with trigger effects
    let (rl, rr) = compute_rumble(s, &st);

    // Trigger / effect report. Shifts no longer hijack the triggers — they're felt as
    // a bassy motor thud (compute_rumble) — so the throttle and brake stay live through
    // a shift and keep carrying their load (e.g. feathering + downshift pushes back).
    let report = {
        match s.profile {
            Profile::Racing => {
                let (lm, lp0, lp1) = racing_l2(s, &st);
                // Brake wall: when not in the ABS pump (active-drive), ramp the brake
                // resistance to a firm wall near the bottom so the pedal pushes back hard
                // around ~220 and stays off the plastic stop — quiet and progressive.
                let lp1 = if lm == 0x01 {
                    let mut f = pedal_wall(lp1, s.l2_raw) as f32;
                    // Decel weight on the brake: engine braking / momentum loads the pedal,
                    // so it firms up under deceleration — you feel the car's weight pushing
                    // into the brakes. Scaled by the Load knob.
                    if s.edition == Edition::Full {
                        f *= 1.0 + 0.40 * s.eng_decel * (s.drivetrain.load as f32 / 50.0);
                    }
                    // Shift clunk on the brake too (felt when downshifting while braking).
                    if s.shift_left_pulse > 0 {
                        f = f.max((st.shift_force as f32).max(215.0) * 0.85);
                    }
                    // Brake friction grind: front tires scrubbing under braking — below the
                    // full-ABS lockup threshold — buzz a tremor into the pedal resistance so
                    // you feel the tires fighting for grip before they let go. Bridges the
                    // gap between a planted brake and the ABS pump (which takes over >0.75).
                    // F1/AC use tighter thresholds since their tyres have more precise grip.
                    let (grind_lo, grind_hi) = match s.game_source {
                        GameSource::F123 | GameSource::Assetto =>
                            (0.10_f32, 0.50_f32),
                        _ =>
                            (0.20_f32, 0.75_f32),
                    };
                    // F1/AC tyres have less audible feedback — boost the grind
                    // amplitude so the pre-lockup warning cuts through clearly.
                    let grind_amp: f32 = match s.game_source {
                        GameSource::F123 | GameSource::Assetto => 70.0,
                        _ => 45.0,
                    };
                    // Speed awareness: at high speed, the window between planted
                    // and locking is narrower (aero downforce), so the warning
                    // should punch harder. Scale grind up to 1.5x at 300+ km/h.
                    let speed_boost = if s.t_on && s.t_speed > 0.0 {
                        1.0 + (s.t_speed / 100.0).clamp(0.0, 1.0) * 0.5
                    } else { 1.0 };
                    if s.edition == Edition::Full && s.t_on && s.l2_haptic
                        && s.t_slip_front > grind_lo && s.t_slip_front <= grind_hi
                    {
                        let grind = ((s.t_slip_front - grind_lo)
                            / (grind_hi - grind_lo)).clamp(0.0, 1.0);
                        let wave  = (s.road_phase * std::f32::consts::TAU).sin();
                        f += grind * grind_amp * speed_boost * wave;
                    }
                    // Trailbraking oversteer: rear starting to slide while braking
                    // hard into a corner — the rear slip pushes back through the
                    // brake pedal so you feel the car rotating before you see it.
                    if s.edition == Edition::Full && s.t_on && s.l2_haptic
                        && s.t_slip_rear > grind_lo && s.t_slip_rear <= grind_hi
                        && s.t_slip_front > grind_lo
                    {
                        let trail = ((s.t_slip_rear - grind_lo)
                            / (grind_hi - grind_lo)).clamp(0.0, 1.0);
                        f += trail * grind_amp * speed_boost * 0.7;
                    }
                    // Surface friction through the brake: rough/gravel surfaces add grain to
                    // the pedal, mirroring the throttle so both feet feel the road texture.
                    if s.edition == Edition::Full && s.t_on && s.t_surface > 0.08 {
                        let texture = (s.t_surface - 0.08) / 0.92;
                        let noise   = (s.road_phase * std::f32::consts::TAU * 1.7).sin();
                        f += texture * 30.0 * noise;
                    }
                    f.clamp(0.0, 255.0) as u8
                } else { lp1 };
                let lp1 = signal::slew_rate_limit(s.l2_resist_slew, lp1, SLEW_MAX_CHANGE);
                s.l2_resist_slew = lp1;
                s.l2_force = lp1;
                // Downshift thunk through the brake (active drive, mode 0x06) — a heavy
                // mechanical clunk felt even while trail-braking or coasting. Only on
                // downshifts (shift_right_pulse), and never while ABS is pumping
                // (lm == 0x06) so real lockup feedback is never masked by it.
                let (lm, lp0, lp1) = if s.edition == Edition::Full
                    && s.shift_right_pulse > 0 && lm != 0x06 {
                    // Downshift thunk scales with the rev-matched revs — a high-rev
                    // downshift slams, a low-rev one is a soft clunk.
                    let rev01 = s.shift_rpm.clamp(0.0, 1.0);
                    let amp = (130.0 + rev01 * 125.0).min(255.0) as u8; // 130 → 255
                    (0x06u8, 10u8, amp)
                } else {
                    (lm, lp0, lp1)
                };
                let (_, mut throttle) = racing_forces(s);
                // Throttle lightening (opt-in) — hard steering at high throttle bleeds
                // off throttle resistance, like the rears breaking traction at the
                // limit. Starts at ~30% lock, up to ~60% lighter at full lock. Pairs
                // with the slip judder in compute_rumble so the trigger going light and
                // the wheelspin chatter land together.
                if s.edition == Edition::Full && s.throttle_light_on && s.r2_raw > DEAD_ZONE {
                    let steer = ((s.lx as i16 - 128).abs() as f32 / 128.0).clamp(0.0, 1.0);
                    if steer > 0.30 {
                        let cut = ((steer - 0.30) / 0.70) * 0.60;
                        throttle = (throttle as f32 * (1.0 - cut)).round() as u8;
                    }
                }
                // Assisted stability: throttle firms up as rear slip rises past 0.40,
                // making it harder to push through wheelspin — like ESP pushing back.
                if s.edition == Edition::Full && s.racing_assist_stability
                    && s.t_on && s.t_slip_rear > 0.40 && s.r2_raw > DEAD_ZONE
                {
                    let factor = 1.0 + ((s.t_slip_rear - 0.40) / 0.60).clamp(0.0, 1.0) * 1.5;
                    throttle = ((throttle as f32) * factor).min(255.0) as u8;
                }
                // Assisted drift: throttle lightens in the drift sweet spot
                // (moderate slip angle + rear wheelspin) so it's easier to hold
                // a slide at angle without the pedal fighting you.
                if s.edition == Edition::Full && s.racing_assist_drift
                    && s.t_on && s.t_slip_rear > 0.30
                    && s.t_slip_angle > 0.15 && s.t_slip_angle < 0.55
                    && s.r2_raw > DEAD_ZONE
                {
                    let depth = 1.0 - ((s.t_slip_angle - 0.15) / 0.25).clamp(0.0, 1.0);
                    let factor = 0.40 + depth * 0.60;
                    throttle = ((throttle as f32) * factor).max(20.0) as u8;
                }
                // Keep the resistance engaged whenever the trigger is off its rest
                // position. The feather ramp makes the force ~0 at the deadzone, so the
                // mode flip there is imperceptible — no snap-on "clunk" when feathering,
                // and no arm/disarm chatter that felt like an automatic trigger.
                let armed = s.r2_raw > DEAD_ZONE;
                s.r2_haptic = armed;
                // Throttle wall: ramp resistance to a firm wall near the bottom so the gas
                // pushes back hard around ~220 and the trigger stays off its plastic stop.
                let rp1 = if armed {
                    let base = pedal_wall(throttle, s.r2_raw);
                    // Transmission/engine felt THROUGH the throttle, in tandem with the
                    // wall: throb the resistance at the engine pulse rate, depth growing
                    // with revs + driveline load. Stays in resistance mode (quiet) — the
                    // wall holds the trigger off the plastic so the throb doesn't clack
                    // the way the old active-drive buzz did.
                    let mut f = if s.edition == Edition::Full && s.engine_rpm > 0.04 {
                        let wave  = (s.engine_phase * std::f32::consts::TAU).sin(); // -1..1
                        let depth = (0.10 + 0.25 * s.engine_rpm + 0.15 * s.eng_load).min(0.45);
                        base as f32 * (1.0 + depth * 0.5 * wave)
                    } else {
                        base as f32
                    };
                    // Differential / tire-bite weight: feeding throttle while steering
                    // loads the tires and the diff, so the gas firms up mid-corner — you
                    // feel where the tires hook up, and the car gains weight. steer ×
                    // throttle = corner load; the pedal stiffens progressively with it.
                    if s.edition == Edition::Full {
                        let steer = ((s.lx as i16 - 128).abs() as f32 / 128.0).clamp(0.0, 1.0);
                        if steer > 0.15 {
                            let corner = ((steer - 0.15) / 0.85) * (s.r2_raw as f32 / 255.0);
                            f *= 1.0 + 0.40 * corner;
                        }
                    }
                    // Overrun load pushes against the THROTTLE while you feather it: on a
                    // downshift the revs blip up above your light throttle (and on a trailing
                    // lift the engine brakes), so eng_decel surges and the gas firms up under
                    // your finger — you feel the driveline weight come up to meet you.
                    if s.edition == Edition::Full {
                        let push = s.eng_decel * (s.drivetrain.load as f32 / 50.0) * 240.0;
                        f += push;
                    }
                    // Shift clunk — a brief firm resistance spike on the throttle during the
                    // shift, layered on the live pedal so you feel a mechanical clunk under
                    // your finger (resistance, no buzz, doesn't hijack the throttle).
                    if s.r2_blip_frames > 0 || s.shift_right_pulse > 0 {
                        f = f.max((st.shift_force as f32).max(215.0) * 0.85);
                    }
                    // Surface friction through the throttle: rough surfaces add a resistance
                    // tremor so the gas feels grainy on gravel/dirt and planted on tarmac.
                    if s.edition == Edition::Full && s.t_on && s.t_surface > 0.08 {
                        let texture = (s.t_surface - 0.08) / 0.92;
                        let noise   = (s.road_phase * std::f32::consts::TAU * 2.3).sin();
                        f += texture * 32.0 * noise;
                    }
                    f.clamp(40.0, 255.0) as u8
                } else { 0u8 };
                s.r2_force = rp1;
                // Gear-shift punch through the throttle (active drive, mode 0x06) — pushes
                // back at ANY pedal position, so the shift is felt even off-throttle or at
                // full gas, unlike the old resistance bump that vanished when the pedal was
                // already floored. Upshift = crisp high-freq snap; downshift = low heavy
                // thunk. Fires off the same pulse counters in both telemetry and bumper
                // modes, and takes priority over the wheelspin flutter below.
                let shift_up   = s.r2_blip_frames > 0 && s.shift_right_pulse == 0;
                let shift_down = s.shift_right_pulse > 0;
                let (rm, rp0, rp1_final) = if s.edition == Edition::Full && (shift_up || shift_down) {
                    // Kick strength scales with the revs captured at the shift: a redline
                    // shift bangs, a low-rev shift is gentle. Upshift = crisp high snap;
                    // downshift = low heavy thunk, both growing with rev01.
                    let rev01 = s.shift_rpm.clamp(0.0, 1.0);
                    if shift_up {
                        let amp  = (120.0 + rev01 * 135.0).min(255.0) as u8; // 120 → 255
                        let freq = (24.0 + rev01 * 10.0) as u8;              // snappier near redline
                        (0x06u8, freq, amp)
                    } else {
                        let amp  = (140.0 + rev01 * 115.0).min(255.0) as u8; // 140 → 255
                        let freq = (9.0 + rev01 * 5.0) as u8;                // heavy thunk, slight rise
                        (0x06u8, freq, amp)
                    }
                } else if armed
                    && s.edition == Edition::Full
                    && s.t_on
                {
                    // Per-game slip sensitivity: F1 and AC cars have higher grip,
                    // so slip ratios are naturally lower. Tighten the threshold and
                    // boost the amplitude so the pedal communicates the limit.
                    let (slip_thresh, slip_span, amp_floor) = match s.game_source {
                        GameSource::F123 | GameSource::Assetto =>
                            (0.12_f32, 0.48_f32, 160.0_f32),
                        _ =>
                            (0.20_f32, 0.80_f32, 120.0_f32),
                    };
                    if s.t_slip_rear > slip_thresh
                        && s.t_slip_rear_frames >= dp.slip_deadzone_frames
                    {
                    // Telemetry wheelspin pedal: Pacejka-shaped amp + frequency crossover
                    // to deep judder when slip ratio exceeds 1.0 (hybrid / AWD spin).
                    // Normal slip: light informative flutter (50-80 Hz). Deep slip:
                    // heavy low-freq thud (35 Hz) that the trigger motor can track
                    // smoothly instead of chattering from eTC square-wave spikes.
                    let slip = ((s.t_slip_rear - slip_thresh) / slip_span).clamp(0.0, 1.0);
                    let pacejka = signal::pacejka_haptic(s.t_slip_rear);
                    let base_hz = dp.slip_flutter_lo_hz + slip * (dp.slip_flutter_hi_hz - dp.slip_flutter_lo_hz);
                    let freq = signal::slip_crossover_freq(s.t_slip_rear, base_hz, dp.slip_crossover_deep_hz) as u8;
                    let amp  = ((amp_floor + slip * (255.0 - amp_floor)) * pacejka).min(255.0) as u8;
                    (0x06u8, freq, amp)
                    } else {
                        (if armed { 0x01u8 } else { 0x05u8 }, 0u8, rp1)
                    }
                } else if s.edition == Edition::Full
                    && s.t_on
                    && s.r2_raw <= DEAD_ZONE
                    && s.engine_rpm > 0.18
                    && s.eng_decel > 0.12
                {
                    // Engine braking (overrun): throttle lifted, revs still up, car slowing
                    // — the engine is dragging the car back through the driveline. Pulse the
                    // gas pedal at the engine's rotation rate so you feel the motor spinning
                    // down under your finger, a sustained grumble (not a buzz). SURGES right
                    // after a downshift, where the rev-match spikes both revs and eng_decel,
                    // then eases as the engine and road speed converge — exactly the
                    // engine-braking sensation. Naturally fades on upshifts (revs drop).
                    let drag = (s.engine_rpm * s.eng_decel).clamp(0.0, 1.0);
                    let freq = (5.0 + s.engine_rpm * 11.0) as u8;       // low engine-rate grumble
                    let amp  = (80.0 + drag * 175.0).min(255.0) as u8;  // grows with revs × decel
                    (0x06u8, freq, amp)
                } else {
                    (if armed { 0x01u8 } else { 0x05u8 }, 0u8, rp1)
                };
                haptics_report(lm, lp0, lp1, rm, rp0, rp1_final, 0, 0)
            }

            Profile::Static => {
                s.l2_force = st.brake_end; s.r2_force = st.throttle_end;
                haptics_report(0x01, 0, st.brake_end, 0x01, 0, st.throttle_end, 0, 0)
            }

            Profile::Gun => {
                let w             = s.weapons[s.gun_weapon];
                let aim_force     = (st.brake_end as f32 * 0.65).round() as u8;
                let trigger_force = (st.brake_end as f32 * 0.88).round() as u8;
                s.l2_force = aim_force;

                // Free tier forces the semi firing pattern (Burst/Auto need Full).
                let pattern = if s.edition == Edition::Full { w.pattern } else { GunMode::Semi };

                match pattern {
                    GunMode::Auto => {
                        if s.r2_raw > DEAD_ZONE {
                            if s.r2_raw < GUN_AUTO_BREAK_POS {
                                // Pulling toward the break point — weapon wall.
                                s.r2_force = trigger_force;
                                haptics_report(0x01, 0, aim_force, 0x02, 0, GUN_AUTO_BREAK_POS, 0, trigger_force)
                            } else {
                                // Held past break — active 0x06 vibration hammer.
                                s.r2_force = w.kick_amp;
                                haptics_report(0x01, 0, aim_force, 0x06, w.rate_hz, w.kick_amp, 0, 0)
                            }
                        } else {
                            s.r2_force = 0;
                            haptics_report(0x01, 0, aim_force, 0x05, 0, 0, 0, 0)
                        }
                    }

                    GunMode::Burst => {
                        // Advance burst scheduling: when no pulse is playing and rounds
                        // remain, count down the gap then fire the next round.
                        if s.gun_burst_remaining > 0 && s.recoil_pulse_frames == 0 {
                            if s.gun_burst_gap > 0 {
                                s.gun_burst_gap -= 1;
                            } else {
                                s.recoil_pulse_frames = BURST_PULSE_FRAMES;
                                s.gun_burst_remaining -= 1;
                                s.gun_burst_gap = burst_gap_frames(w.rate_hz);
                            }
                        }
                        if s.recoil_pulse_frames > 0 {
                            // Active 0x06 vibration kick per round (the felt recoil).
                            let releasing = s.recoil_pulse_frames > BURST_SLAM_FRAMES;
                            s.r2_force = if releasing { 0 } else { w.kick_amp };
                            s.recoil_pulse_frames -= 1;
                            if releasing {
                                haptics_report(0x01, 0, aim_force, 0x05, 0, 0, 0, 0)
                            } else {
                                haptics_report(0x01, 0, aim_force, 0x06, w.kick_freq, w.kick_amp, 0, 0)
                            }
                        } else {
                            s.r2_force = trigger_force;
                            haptics_report(0x01, 0, aim_force, 0x02, 0, GUN_BREAK_POS, 0, trigger_force)
                        }
                    }

                    GunMode::Semi => {
                        if s.recoil_pulse_frames > 0 {
                            // Active 0x06 vibration kick (the felt recoil).
                            let releasing = s.recoil_pulse_frames > w.kick_frames;
                            s.r2_force = if releasing { 0 } else { w.kick_amp };
                            s.recoil_pulse_frames -= 1;
                            if releasing {
                                haptics_report(0x01, 0, aim_force, 0x05, 0, 0, 0, 0)
                            } else {
                                haptics_report(0x01, 0, aim_force, 0x06, w.kick_freq, w.kick_amp, 0, 0)
                            }
                        } else {
                            s.r2_force = trigger_force;
                            haptics_report(0x01, 0, aim_force, 0x02, 0, GUN_BREAK_POS, 0, trigger_force)
                        }
                    }
                }
            }

            Profile::Melee => {
                let w = s.melee_weapons[s.melee_weapon];
                let block_force = (st.brake_end as f32 * 0.88).round() as u8;
                s.l2_force = block_force;
                if s.melee_impact_frames > 0 {
                    // Connect kick — active 0x06 vibration so the hit shoves the trigger,
                    // weighted per weapon (heavy = low freq, big amp).
                    s.r2_force = w.impact_force;
                    s.melee_impact_frames -= 1;
                    haptics_report(0x01, 0, block_force, 0x06, w.impact_freq, w.impact_force, 0, 0)
                } else {
                    // Resting swing heft — resistance builds with the pull, per weapon.
                    let tr = clamp01(s.r2_raw as f32 / 255.0);
                    let swing = if s.r2_raw > DEAD_ZONE {
                        (w.swing_force as f32 * tr.powf(w.swing_exp)).round() as u8
                    } else { 0 };
                    s.r2_force = swing;
                    haptics_report(0x01, 0, block_force,
                        if s.r2_raw > DEAD_ZONE { 0x01 } else { 0x05 }, 0, swing, 0, 0)
                }
            }

            Profile::Audio => {
                s.smooth_energy = s.smooth_energy * 0.85 + s.audio_energy * 0.15;
                s.smooth_bass   = s.smooth_bass   * 0.85 + s.audio_bass   * 0.15;
                s.smooth_treble = s.smooth_treble * 0.80 + s.audio_treble * 0.20; // treble reacts a touch faster
                let norm = clamp01(s.smooth_energy / 0.08);
                let lf = if s.l2_raw > DEAD_ZONE { (norm * st.brake_end as f32).round() as u8 } else { 0 };
                let rf = if s.r2_raw > DEAD_ZONE { (norm * st.throttle_end as f32).round() as u8 } else { 0 };
                s.l2_force = lf; s.r2_force = rf;
                haptics_report(
                    if s.l2_raw > DEAD_ZONE { 0x01 } else { 0x05 }, 0, lf,
                    if s.r2_raw > DEAD_ZONE { 0x01 } else { 0x05 }, 0, rf,
                    0, 0,
                )
            }

            Profile::Minecraft => {
                let item = s.mc_item;
                let bowlike = matches!(item, McItem::Bow | McItem::Crossbow | McItem::Trident);

                // ── Right trigger = primary action (attack / use / mine) ──────────
                let (rm, rp0, rp1) = if bowlike {
                    if s.mc_release_frames > 0 {
                        (0x06u8, 8, 200)                          // release twang
                    } else if s.mc_using {
                        // Draw tension grows with pull progress — taut at full draw.
                        let f = (40.0 + 200.0 * s.mc_use_prog).min(255.0) as u8;
                        (0x01u8, 0, f)
                    } else {
                        (0x05u8, 0, 0)
                    }
                } else if s.mc_mining && matches!(item,
                        McItem::Pickaxe | McItem::Axe | McItem::Shovel | McItem::Hoe) {
                    // Mining grind — active drive pulsing at a tool-specific rate.
                    let (rate, amp) = match item {
                        McItem::Pickaxe => (10u8, 150u8),
                        McItem::Axe     => (9,  150),
                        McItem::Shovel  => (7,  110),
                        _               => (8,  120),
                    };
                    (0x06u8, rate, amp)
                } else if matches!(item, McItem::Sword | McItem::Axe) {
                    if s.mc_attack_frames > 0 {
                        let amp = if item == McItem::Axe { 205 } else { 150 };
                        (0x06u8, if item == McItem::Axe { 6 } else { 10 }, amp)
                    } else {
                        // Resting heft so the weapon trigger has weight.
                        let f = if item == McItem::Axe { 70u8 } else { 45u8 };
                        (0x01u8, 0, f)
                    }
                } else if s.mc_using && item == McItem::Food {
                    (0x01u8, 0, 60)                                // springy eat resistance
                } else {
                    (0x05u8, 0, 0)
                };

                // ── Left trigger = shield brace ──────────────────────────────────
                let (lm, lp0, lp1) = if s.mc_blocking {
                    (0x01u8, 0, 200)
                } else {
                    (0x05u8, 0, 0)
                };

                s.l2_force = lp1; s.r2_force = rp1;

                // Count down transient one-shots (rumble already read them above).
                if s.mc_attack_frames  > 0 { s.mc_attack_frames  -= 1; }
                if s.mc_hurt_frames    > 0 { s.mc_hurt_frames    -= 1; }
                if s.mc_release_frames > 0 { s.mc_release_frames -= 1; }

                haptics_report(lm, lp0, lp1, rm, rp0, rp1, 0, 0)
            }
        }
    };
    let mut report = report;

    // ── Game rumble passthrough (Xbox output) ────────────────────────────────
    // Blend the enriched game rumble with the profile's own (max, not sum, so
    // neither side gets clipped weird), then spend any armed impact extras.
    let (grl, grr) = game_rumble_mix(s);
    let kicking = s.pt_kick_frames > 0;
    if kicking { s.pt_kick_frames -= 1; }
    if s.pt_enabled && s.output_mode == OutputMode::Xbox && s.pt_trigger_kick {
        // The trigger voice coils are the strongest actuators on the pad, so they
        // carry the passthrough: continuous vibration tracking the rumble envelope,
        // overridden by a harder jolt while a spike kick is active. Only applied to
        // triggers the profile left idle (off/none) so profile effects never get
        // clobbered mid-frame.
        let amp = if kicking {
            (215.0 * s.pt_intensity).min(255.0) as u8
        } else if grl > 40 {
            // Track the envelope at reduced gain so sustained rumble reads as a
            // strong texture under the fingers, not a constant jolt.
            ((grl as f32) * 0.8).min(255.0) as u8
        } else {
            0
        };
        if amp > 0 {
            let freq = if kicking { 34 } else { 22 };
            if report[11] == 0x05 || report[11] == 0x00 {
                report[11] = 0x06; report[12] = freq; report[13] = amp;
            }
            if report[22] == 0x05 || report[22] == 0x00 {
                report[22] = 0x06; report[23] = freq; report[24] = amp;
            }
        }
    }
    if s.pt_lb_frames > 0 {
        s.pt_lb_frames -= 1;
        // Flash toward white and decay back; the final frame writes the profile
        // (or held-item) color exactly so the bar lands back where it belongs.
        let base = if s.profile == Profile::Minecraft {
            s.mc_item.lightbar()
        } else {
            s.profile.lightbar()
        };
        let t = s.pt_lb_frames as f32 / 8.0;
        report[2] |= 0x04;
        report[45] = (base[0] as f32 + (255.0 - base[0] as f32) * t) as u8;
        report[46] = (base[1] as f32 + (255.0 - base[1] as f32) * t) as u8;
        report[47] = (base[2] as f32 + (255.0 - base[2] as f32) * t) as u8;
    }

    with_rumble(report, rl.max(grl), rr.max(grr))
}

// ─── Audio subprocess ─────────────────────────────────────────────────────────

/// Two-band audio split for the Audio profile. A cheap one-pole low-pass pulls out
/// the bass (→ left/low-freq motor for body rumble); the remainder is treble
/// (→ right/high-freq motor for grain). No FFT, no model — it reacts to the actual
/// frequency content of whatever is playing, so it works for any car/engine/sound
/// instead of being tuned to one. Lightweight: one multiply-add per sample.
#[derive(Clone, Copy, Default)]
struct AudioBands { overall: f32, bass: f32, treble: f32 }

/// One-pole low-pass coefficient (~140 Hz @ 44.1 kHz) splitting bass from treble.
const AUDIO_LP_A: f32 = 0.02;

/// Live tuning for the true-haptics audio feed. Two bands, separated in the hands:
/// sub (< ~90 Hz, kicks/impacts) drives the LEFT actuator, the engine band
/// (~90-280 Hz, where engine notes live — pitch tracks RPM) drives the RIGHT.
/// Everything above the engine band is cut so speech/music mids don't turn into a
/// constant buzz, and a per-band gate keeps quiet content at zero instead of a
/// compressed drone. Shared with the capture thread; the Lab edits it live.
#[derive(Clone, Copy)]
pub struct AudioTune {
    pub sub_gain:    f32,  // gain on the sub band (left actuator)
    pub engine_gain: f32,  // gain on the engine band (right actuator)
    pub gate:        f32,  // envelope threshold below which a band stays silent
}

impl Default for AudioTune {
    fn default() -> Self {
        Self { sub_gain: 1.4, engine_gain: 1.6, gate: 0.012 }
    }
}

/// Start capturing system audio into `bands`, whichever way this platform does it.
/// macOS: spawn sox reading the BlackHole loopback device. Windows: WASAPI loopback
/// on the default output device (no virtual audio driver needed).
fn start_audio_capture(s: &mut AppState, bands: Arc<Mutex<AudioBands>>) {
    #[cfg(target_os = "macos")]
    {
        s.sox_child = start_sox(bands);
    }
    #[cfg(windows)]
    {
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let live = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let tune = s.audio_tune.clone();
        s.audio_stop = Some(stop.clone());
        s.audio_true_live = Some(live.clone());
        thread::spawn(move || wasapi_loopback(bands, stop, live, tune));
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (s, bands);
    }
}

/// Tear down whichever capture is running. Safe to call when none is.
fn stop_audio_capture(s: &mut AppState) {
    if let Some(mut child) = s.sox_child.take() {
        let _ = child.kill();
    }
    if let Some(stop) = s.audio_stop.take() {
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    s.audio_true_live = None;
}

/// WASAPI loopback capture: cpal opens an *input* stream on the default *output*
/// device, which on Windows taps whatever is playing (game, music, anything).
/// Same band math as the sox path: one-pole low-pass splits bass from treble,
/// per-buffer RMS feeds the shared AudioBands. The cpal stream is not Send, so it
/// lives entirely on this thread; `stop` tears it down on profile switch.
#[cfg(windows)]
fn wasapi_loopback(
    bands_out: Arc<Mutex<AudioBands>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    live: Arc<std::sync::atomic::AtomicBool>,
    tune: Arc<Mutex<AudioTune>>,
) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::collections::VecDeque;
    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        eprintln!("[audio] no default output device");
        return;
    };
    let config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => { eprintln!("[audio] output config failed: {e}"); return; }
    };
    let channels = (config.channels() as usize).max(1);
    let in_rate = config.sample_rate().0;
    let cfg: cpal::StreamConfig = config.into();

    // Ring buffer feeding the true-haptics output stream: per-frame [sub, engine]
    // band pairs (left/right actuator). Capped at ~125 ms to bound latency.
    let ring: Arc<Mutex<VecDeque<[f32; 2]>>> = Arc::new(Mutex::new(VecDeque::new()));
    let ring_cap = (in_rate / 8) as usize;

    // Two-pole coefficients (per-sample one-pole chained twice, 12 dB/oct):
    // sub corner ~90 Hz, engine-band top ~280 Hz, at the capture rate.
    let a_sub = 1.0 - (-2.0 * std::f32::consts::PI *  90.0 / in_rate as f32).exp();
    let a_mid = 1.0 - (-2.0 * std::f32::consts::PI * 280.0 / in_rate as f32).exp();

    let (mut sub1, mut sub2, mut mid1, mut mid2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (mut env_sub, mut env_eng) = (0.0f32, 0.0f32);
    let cb_bands = bands_out.clone();
    let cb_ring = ring.clone();
    let cb_tune = tune.clone();
    let stream = match device.build_input_stream(
        &cfg,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let frames = data.len() / channels;
            if frames == 0 { return; }
            let t = cb_tune.lock().map(|g| *g).unwrap_or_default();
            let (mut o_sum, mut sub_sum, mut eng_sum) = (0.0f32, 0.0f32, 0.0f32);
            let mut hap: Vec<[f32; 2]> = Vec::with_capacity(frames);
            for fr in data.chunks_exact(channels) {
                let sample = fr.iter().sum::<f32>() / channels as f32;
                o_sum += sample * sample;

                // Band split for the actuators. sub = everything under ~90 Hz
                // (kicks, impacts); engine = 90-280 Hz (engine fundamentals — the
                // felt pitch rises with RPM). Above 280 Hz is dropped entirely so
                // speech and music mids never reach the actuators.
                sub1 += a_sub * (sample - sub1);
                sub2 += a_sub * (sub1 - sub2);
                mid1 += a_mid * (sample - mid1);
                mid2 += a_mid * (mid1 - mid2);
                let sub = sub2;
                let eng = mid2 - sub2;
                // Band RMS (pre-gain) drives both the meter and the reactive-rumble
                // fallback, so that path gets the same clean frequency separation.
                sub_sum += sub * sub;
                eng_sum += eng * eng;

                // Per-band expander gate: fast-attack / slow-release envelopes;
                // a band below the gate decays to silence instead of droning, and
                // the squared ramp above it restores real dynamics (no soft-clip
                // compression "hovering at one level").
                let ka = 0.012; let kr = 0.0009;
                let k_s = if sub.abs() > env_sub { ka } else { kr };
                env_sub += k_s * (sub.abs() - env_sub);
                let k_e = if eng.abs() > env_eng { ka } else { kr };
                env_eng += k_e * (eng.abs() - env_eng);
                let g = |env: f32| ((env - t.gate) / t.gate.max(0.001)).clamp(0.0, 1.0);
                let g_s = g(env_sub) * g(env_sub);
                let g_e = g(env_eng) * g(env_eng);

                hap.push([
                    (sub * t.sub_gain    * g_s).clamp(-1.0, 1.0),
                    (eng * t.engine_gain * g_e).clamp(-1.0, 1.0),
                ]);
            }
            let nf = frames as f32;
            // bass channel carries the SUB band, treble channel the ENGINE band —
            // so the existing reactive-rumble mapping (bass→left, treble→right) now
            // reflects the clean split with no extra plumbing.
            let bands = AudioBands {
                overall: (o_sum / nf).sqrt(),
                bass:    (sub_sum / nf).sqrt(),
                treble:  (eng_sum / nf).sqrt(),
            };
            if let Ok(mut e) = cb_bands.lock() { *e = bands; }
            if let Ok(mut rb) = cb_ring.lock() {
                rb.extend(hap);
                while rb.len() > ring_cap {
                    rb.pop_front(); // drop oldest — bound the latency, keep the now
                }
            }
        },
        |e| eprintln!("[audio] stream error: {e}"),
        None,
    ) {
        Ok(st) => st,
        Err(e) => { eprintln!("[audio] loopback open failed: {e}"); return; }
    };
    if let Err(e) = stream.play() {
        eprintln!("[audio] stream start failed: {e}");
        return;
    }
    eprintln!("[audio] WASAPI loopback capture running");

    // ── True haptics: stream the waveform into the DualSense's USB audio device ──
    // Over USB the pad enumerates as a 4-channel output: ch 1/2 = speaker, ch 3/4 =
    // the left/right haptic actuators. Writing the low-passed system audio to 3/4
    // plays it through the actuators the way PS5 games do. Not available over BT.
    let haptic_stream = (|| -> Option<cpal::Stream> {
        let dev = host.output_devices().ok()?.find(|d| {
            d.name().map(|n| {
                let n = n.to_lowercase();
                n.contains("wireless controller") || n.contains("dualsense")
            }).unwrap_or(false)
        })?;
        let name = dev.name().unwrap_or_default();
        let sc = dev.supported_output_configs().ok()?.find(|c| {
            c.channels() == 4 && c.sample_format() == cpal::SampleFormat::F32
        })?;
        let out_rate = 48000u32.clamp(sc.min_sample_rate().0, sc.max_sample_rate().0);
        let out_cfg: cpal::StreamConfig = sc.with_sample_rate(cpal::SampleRate(out_rate)).into();
        let ratio = in_rate as f64 / out_rate as f64;
        let mut frac = 0.0f64;
        let mut cur = [0.0f32; 2];
        let out_ring = ring.clone();
        let st = dev.build_output_stream(
            &out_cfg,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut rb = match out_ring.lock() { Ok(r) => r, Err(_) => return };
                for fr in data.chunks_exact_mut(4) {
                    // Nearest-sample resample from the capture rate to the pad's rate.
                    frac += ratio;
                    while frac >= 1.0 {
                        if let Some(v) = rb.pop_front() {
                            cur = v;
                        } else {
                            cur[0] *= 0.95; cur[1] *= 0.95;
                        }
                        frac -= 1.0;
                    }
                    fr[0] = 0.0;    fr[1] = 0.0;    // speaker stays silent
                    fr[2] = cur[0]; fr[3] = cur[1]; // left = sub, right = engine band
                }
            },
            |e| eprintln!("[audio] haptic stream error: {e}"),
            None,
        ).ok()?;
        st.play().ok()?;
        eprintln!("[audio] TRUE haptics streaming to '{name}' (USB 4ch, {out_rate} Hz)");
        Some(st)
    })();
    live.store(haptic_stream.is_some(), std::sync::atomic::Ordering::SeqCst);
    if haptic_stream.is_none() {
        eprintln!("[audio] no DualSense USB audio device (BT or unplugged) — using reactive rumble");
    }

    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    live.store(false, std::sync::atomic::Ordering::SeqCst);
    drop(haptic_stream);
    drop(stream);
    if let Ok(mut e) = bands_out.lock() { *e = AudioBands::default(); }
}

#[cfg(target_os = "macos")]
fn start_sox(bands_out: Arc<Mutex<AudioBands>>) -> Option<std::process::Child> {
    let mut child = std::process::Command::new("/opt/homebrew/bin/sox")
        .args(["-t", "coreaudio", "BlackHole 2ch",
               "-t", "raw", "-r", "44100", "-e", "float", "-b", "32", "-c", "1", "-"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let stdout = child.stdout.take()?;
    thread::spawn(move || {
        use std::io::Read;
        let mut reader = std::io::BufReader::new(stdout);
        let mut buf = [0u8; 4096];
        let mut lp = 0.0f32;  // low-pass state — persists across read buffers
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let floats = n / 4;
                    if floats == 0 { continue; }
                    let (mut o_sum, mut b_sum, mut t_sum) = (0.0f32, 0.0f32, 0.0f32);
                    for i in 0..floats {
                        let b = [buf[i*4], buf[i*4+1], buf[i*4+2], buf[i*4+3]];
                        let sample = f32::from_le_bytes(b);
                        lp += AUDIO_LP_A * (sample - lp);  // bass = low-passed signal
                        let treble = sample - lp;          // treble = the rest
                        o_sum += sample * sample;
                        b_sum += lp * lp;
                        t_sum += treble * treble;
                    }
                    let nf = floats as f32;
                    let bands = AudioBands {
                        overall: (o_sum / nf).sqrt(),
                        bass:    (b_sum / nf).sqrt(),
                        treble:  (t_sum / nf).sqrt(),
                    };
                    if let Ok(mut e) = bands_out.lock() { *e = bands; }
                }
            }
        }
    });

    Some(child)
}

// ─── Input thread (non-exclusive HID read) ────────────────────────────────────
//
// GameController only delivers live input to the foreground app, so polling it
// freezes trigger values when you tab into a game. Shared HID reads keep working
// in the background while games still receive their own copy via gamecontrollerd.

fn parse_input_report(raw: &[u8], len: usize, state: &Arc<Mutex<AppState>>) {
    // USB delivers input report 0x01 (64 bytes). Bluetooth delivers the extended
    // report 0x31 (78 bytes) once any output report has been sent — its payload is
    // the same layout shifted one byte in (an extra header byte after the ID). The
    // >= 40 length gate excludes the 10-byte BT "minimal" 0x01 report the pad emits
    // before it's switched into full mode, so we never misparse that as USB.
    let off = match raw[0] {
        0x01 if len >= 40 => 0usize,
        0x31 if len >= 40 => 1usize,
        _ => return,
    };
    let buf = &raw[off..];
    let len = len - off;
    if let Ok(mut s) = state.lock() {
        s.lx = buf[1];
        s.ly = buf[2];
        s.rx = buf[3];
        s.ry = buf[4];

        let new_l2 = buf[5];
        let new_r2 = buf[6];

        if s.profile == Profile::Gun {
            let w = s.weapons[s.gun_weapon];
            // Free tier forces the semi firing pattern (Burst/Auto need Full).
            let pattern = if s.edition == Edition::Full { w.pattern } else { GunMode::Semi };
            let broke = new_r2 > RECOIL_BREAK_THRESHOLD && s.r2_raw <= RECOIL_BREAK_THRESHOLD;
            match pattern {
                GunMode::Semi => {
                    if broke && !s.recoil_fired {
                        s.recoil_fired        = true;
                        s.recoil_pulse_frames = RECOIL_RELEASE_FRAMES + w.kick_frames;
                    }
                }
                GunMode::Burst => {
                    // On break, load the burst and fire the first round immediately.
                    if broke && !s.recoil_fired {
                        s.recoil_fired         = true;
                        s.recoil_pulse_frames  = BURST_PULSE_FRAMES;
                        s.gun_burst_remaining  = w.burst_count.saturating_sub(1);
                        s.gun_burst_gap        = burst_gap_frames(w.rate_hz);
                    }
                }
                GunMode::Auto => {}
            }
            // Reset the per-pull latch (and any in-flight burst) on release.
            if new_r2 < 50 {
                s.recoil_fired        = false;
                s.gun_burst_remaining = 0;
                s.gun_burst_gap       = 0;
            }
        }
        if s.profile == Profile::Melee {
            if new_r2 > 230 && s.r2_raw <= 230 && !s.melee_impact_fired {
                s.melee_impact_fired  = true;
                s.melee_impact_frames = s.melee_weapons[s.melee_weapon].impact_frames;
            }
            if new_r2 < 50 { s.melee_impact_fired = false; }
        }

        // Brake-bite: a fast stab into the brake fires a short kick as the pads grab.
        if s.profile == Profile::Racing && new_l2 > 70 && new_l2 >= s.prev_l2_bite.saturating_add(45) {
            s.brake_bite_frames = 5;
        }
        s.prev_l2_bite = new_l2;

        s.l2_raw = new_l2;
        s.r2_raw = new_r2;

        let face      = buf[8];
        let shoulders = buf[9];
        s.buttons     = (face as u16) | ((shoulders as u16) << 8);

        if len > 10 { s.touchpad_btn = (buf[10] & 0x02) != 0; }

        // ── Motion sensors ────────────────────────────────────────────────────
        // DualSense input report carries gyro (bytes 16-21) then accel (22-27),
        // each as three little-endian int16s. Same offsets for USB and the BT
        // extended report once `off` has aligned the payload.
        if len >= 28 {
            s.gx = i16::from_le_bytes([buf[16], buf[17]]);
            s.gy = i16::from_le_bytes([buf[18], buf[19]]);
            s.gz = i16::from_le_bytes([buf[20], buf[21]]);
            s.ax = i16::from_le_bytes([buf[22], buf[23]]);
            s.ay = i16::from_le_bytes([buf[24], buf[25]]);
            s.az = i16::from_le_bytes([buf[26], buf[27]]);
        }

        // Lab live preview — synthesize Minecraft gameplay state from the controller
        // so the real per-item feels can be tested with no mod connected.
        if s.mc_preview {
            let r2_held = new_r2 > 200;
            let l2_held = new_l2 > 200;
            match s.mc_item {
                McItem::Pickaxe | McItem::Axe | McItem::Shovel | McItem::Hoe => {
                    s.mc_mining = r2_held;
                }
                McItem::Bow | McItem::Crossbow | McItem::Trident => {
                    s.mc_using = r2_held;
                    if r2_held { s.mc_use_prog = (s.mc_use_prog + 1.0 / 45.0).min(1.0); }
                    else       { s.mc_use_prog = 0.0; }
                }
                McItem::Food => { s.mc_using = r2_held; }
                _ => {}
            }
            s.mc_blocking = matches!(s.mc_item, McItem::Shield) && l2_held;
            // □ = attack swing (sword/axe connect)
            let sq_now = (face & 0x10) != 0;
            if sq_now && !s.prev_mc_hit { s.mc_attack_frames = MC_ATTACK_FRAMES; }
            s.prev_mc_hit = sq_now;
            // △ = take damage — also drops health so the low-health heartbeat kicks in
            let tri_now = (face & 0x80) != 0;
            if tri_now && !s.prev_mc_hurt {
                s.mc_hurt_frames = MC_HURT_FRAMES;
                s.mc_health = (s.mc_health - 3.0).max(0.0);
            }
            s.prev_mc_hurt = tri_now;
            // ○ = heal back to full (reset the heartbeat test)
            let ci_now = (face & 0x40) != 0;
            if ci_now && !s.prev_mc_heal { s.mc_health = 20.0; }
            s.prev_mc_heal = ci_now;
        }

        let sq = (face & 0x10) != 0;
        let ci = (face & 0x40) != 0;

        // Shift detection fires on EITHER the face buttons (Square/Circle) or the
        // bumpers (L1 = down, R1 = up) — bumpers are the common paddle-shift mapping
        // in racing games, so downshifts register no matter which the game uses.
        let l1 = (shoulders & 0x01) != 0;
        let r1 = (shoulders & 0x02) != 0;
        let down_now = sq || l1;
        let up_now   = ci || r1;

        if down_now && !s.prev_downshift && !s.t_on && s.shift_enabled && s.profile == Profile::Racing && s.edition == Edition::Full {
            s.shift_left_pulse  = SHIFT_TOTAL_FRAMES;
            s.shift_right_pulse = SHIFT_TOTAL_FRAMES;
            s.r2_blip_frames    = 0;
            // Downshift rev blip — the revs jump as the lower gear spins the engine up
            // (the rev-match), felt through the engine pulse rate and the load swell.
            s.engine_rpm = (s.engine_rpm + 0.22).min(1.0);
            s.shift_rpm = s.engine_rpm;  // post rev-match revs scale the kick
            s.eng_lash_frames = 5;  // driveline shock as the lower gear bites
            s.last_shift_dir    = "▼ Down".into();
            s.shift_count      += 1;
        }
        if up_now && !s.prev_upshift && !s.t_on && s.shift_enabled && s.profile == Profile::Racing && s.edition == Edition::Full {
            // Upshift: slam immediately (no leading release frame) so the snap is instant.
            s.shift_left_pulse  = UPSHIFT_SLAM_FRAMES;
            s.shift_right_pulse = 0;
            s.r2_blip_frames    = R2_BLIP_FRAMES;
            // Upshift drops revs a touch as the higher gear loads the engine down.
            // Capture the pre-drop revs so a redline upshift still kicks hard.
            s.shift_rpm = s.engine_rpm;
            s.engine_rpm = (s.engine_rpm - 0.12).max(0.0);
            s.last_shift_dir    = "▲ Up".into();
            s.shift_count      += 1;
        }
        s.prev_downshift = down_now;
        s.prev_upshift   = up_now;

        if len >= 37 {
            let active = (buf[33] & 0x80) == 0;
            s.touch0_active = active;
            if active {
                s.touch0_x = (buf[34] as u16) | (((buf[35] & 0x0F) as u16) << 8);
                s.touch0_y = ((buf[35] >> 4) as u16) | ((buf[36] as u16) << 4);
            }
        }

        if !s.connected { s.connected = true; s.error_msg.clear(); }
    }
}

fn input_loop(state: Arc<Mutex<AppState>>) {
    loop {
        let (path, _transport) = match find_dualsense() {
            Some(p) => p,
            None    => { thread::sleep(Duration::from_secs(1)); continue; }
        };
        let api = match hidapi::HidApi::new() {
            Ok(a)  => a,
            Err(_) => { thread::sleep(Duration::from_secs(1)); continue; }
        };
        let device = match open_dualsense(&api, &path) {
            Ok(d)  => d,
            Err(_) => { thread::sleep(Duration::from_secs(2)); continue; }
        };

        let mut buf = [0u8; 96];  // USB report = 64 B, BT extended report = 78 B
        // Xbox-translated output: lazily created virtual XInput pad (Windows only).
        #[cfg(windows)]
        let mut xbridge: Option<crate::xinput::XBridge> = None;
        loop {
            match device.read_timeout(&mut buf, 4) {
                Ok(0)   => {}
                Ok(len) => {
                    parse_input_report(&buf, len, &state);
                    #[cfg(windows)]
                    forward_to_xbox(&state, &mut xbridge);
                }
                Err(_)  => break,
            }
        }

        if let Ok(mut s) = state.lock() {
            s.connected  = false;
            s.error_msg  = "DualSense disconnected — reconnect USB or Bluetooth".to_string();
            s.l2_raw     = 0;
            s.r2_raw     = 0;
            s.reset_pulse_state();
        }
        thread::sleep(Duration::from_millis(500));
    }
}

/// Forward the latest DualSense input to the virtual Xbox pad when output mode is Xbox.
/// Creates the ViGEm bridge on first use and tears it down when the user switches back to
/// DualSense mode (so the virtual pad disappears and the real DualSense is read natively).
#[cfg(windows)]
// ─── Motion math ─────────────────────────────────────────────────────────────
// Raw accel int16s give the gravity direction, so absolute tilt is just atan2 of
// two axes. Gyro int16s are angular velocity; the exact deg/s scale folds into the
// user sensitivity, so aim uses the raw rate directly.

/// Tilt angle (degrees) of the chosen steering axis, from the accelerometer.
/// axis 0 = roll (left/right), axis 1 = pitch (forward/back).
pub fn motion_tilt_deg(s: &AppState) -> f32 {
    let (num, den) = match s.motion.steer_axis {
        1 => (s.ay as f32, s.az as f32),
        _ => (s.ax as f32, s.az as f32),
    };
    num.atan2(den).to_degrees()
}

/// Map the live tilt to a virtual left-stick X byte (128 = center). Returns the
/// physical stick when steering is off, so the snap path can call it unconditionally.
fn motion_steer_lx(s: &AppState) -> u8 {
    if !s.motion.steer_enabled {
        return s.lx;
    }
    let mut a = motion_tilt_deg(s);
    if s.motion.steer_invert {
        a = -a;
    }
    let dz = s.motion.steer_deadzone;
    if a.abs() <= dz {
        return 128; // inside the dead zone — wheel is centered
    }
    let sign = a.signum();
    let mag = a.abs() - dz;
    let span = (s.motion.steer_max_deg - dz).max(1.0);
    let norm = ((mag / span) * s.motion.steer_sens).clamp(0.0, 1.0) * sign;
    (128.0 + norm * 127.0).round().clamp(0.0, 255.0) as u8
}

fn forward_to_xbox(
    state: &Arc<Mutex<AppState>>,
    xbridge: &mut Option<crate::xinput::XBridge>,
) {
    let snap = {
        let mut s = match state.lock() { Ok(s) => s, Err(_) => return };
        if s.output_mode != OutputMode::Xbox {
            // Switched back to DualSense — un-cloak the real controller and drop the
            // virtual pad so the game reads the real controller natively again.
            // Also clear any held game rumble so the motors don't stick on.
            if xbridge.is_some() {
                let _ = crate::hidhide::disable();
                s.game_rumble_l = 0;
                s.game_rumble_r = 0;
            }
            *xbridge = None;
            return;
        }
        // Tilt steering (when enabled) overrides the left-stick X with the mapped angle.
        let lx_out = motion_steer_lx(&s);
        (lx_out, s.ly, s.rx, s.ry, s.l2_raw, s.r2_raw, s.buttons, s.touchpad_btn)
    };

    if xbridge.is_none() {
        match crate::xinput::XBridge::new() {
            Ok(mut b)  => {
                // Subscribe to the game's rumble on the virtual pad (passthrough).
                b.start_feedback(state.clone());
                *xbridge = Some(b);
                // Cloak the real DualSense so the game sees only the virtual Xbox pad.
                if let Err(e) = crate::hidhide::enable() {
                    if let Ok(mut s) = state.lock() {
                        s.error_msg = format!("Xbox pad on, but HidHide cloak failed — {e}");
                    }
                }
            }
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.error_msg = format!("Xbox output unavailable — {e} (is ViGEmBus installed?)");
                }
                return;
            }
        }
    }
    if let Some(b) = xbridge.as_mut() {
        let (lx, ly, rx, ry, l2, r2, buttons, _ps) = snap;
        b.push(lx, ly, rx, ry, l2, r2, buttons, false);
    }
}

// ─── Output thread (hidapi write-only) ───────────────────────────────────────

pub fn spawn_hid_thread(state: Arc<Mutex<AppState>>, app: AppHandle) {
    let input_state = state.clone();
    let emit_state  = state.clone();
    let emit_app    = app.clone();
    #[cfg(windows)]
    {
        let aim_state = state.clone();
        thread::spawn(move || aim_loop(aim_state));
    }
    thread::spawn(move || input_loop(input_state));
    thread::spawn(move || hid_loop(state, app));
    // Dedicated UI emitter — pushes a state snapshot to the frontend at ~30 fps
    // regardless of whether a controller is connected. This keeps the UI live and
    // interactive (profile/strength/output clicks repaint immediately) even with no
    // DualSense plugged in. The haptic loop above also emits while a device is open;
    // the duplicate frame is harmless.
    thread::spawn(move || loop {
        if let Ok(s) = emit_state.lock() {
            let _ = emit_app.emit("state-update", s.snapshot());
        }
        thread::sleep(Duration::from_millis(33));
    });
}

// Rough DualSense gyro scale (raw int16 units per degree/second). The exact value
// varies per pad; the aim deadzone is the only place it matters, and the sensitivity
// sliders absorb the rest, so an approximate constant is fine.
#[cfg(windows)]
const GYRO_RAW_PER_DPS: f32 = 16.0;

#[cfg(windows)]
extern "system" {
    // user32 relative mouse move. dx/dy are treated as signed for MOUSEEVENTF_MOVE.
    fn mouse_event(dw_flags: u32, dx: i32, dy: i32, dw_data: u32, dw_extra: usize);
}

/// Gyro aim — convert the pad's angular velocity into relative mouse motion.
/// Independent of the output mode (it injects mouse events, so it works alongside
/// the virtual Xbox pad or on its own). Activation: always / hold / toggle, gated on
/// the touchpad click so it never fights gameplay buttons.
#[cfg(windows)]
fn aim_loop(state: Arc<Mutex<AppState>>) {
    const MOUSEEVENTF_MOVE: u32 = 0x0001;
    let mut prev_touch = false;
    let (mut acc_x, mut acc_y) = (0.0f32, 0.0f32);
    loop {
        let (dx, dy) = {
            let mut s = match state.lock() {
                Ok(s) => s,
                Err(_) => { thread::sleep(Duration::from_millis(4)); continue; }
            };
            if !s.motion.aim_enabled {
                prev_touch = s.touchpad_btn;
                drop(s);
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            let active = match s.motion.aim_mode {
                1 => s.touchpad_btn,                              // hold to aim
                2 => {                                            // touchpad toggles aim
                    if s.touchpad_btn && !prev_touch { s.aim_toggle_on = !s.aim_toggle_on; }
                    s.aim_toggle_on
                }
                _ => true,                                        // always on
            };
            prev_touch = s.touchpad_btn;
            if !active {
                (0.0, 0.0)
            } else {
                // gz ≈ yaw (turn left/right → mouse X), gx ≈ pitch (tilt → mouse Y).
                let dz = s.motion.aim_deadzone * GYRO_RAW_PER_DPS;
                let gate = |v: f32| if v.abs() < dz { 0.0 } else { v };
                let mx = gate(s.gz as f32) * s.motion.aim_sens_x / 4000.0;
                let mut my = gate(s.gx as f32) * s.motion.aim_sens_y / 4000.0;
                if s.motion.aim_invert_y { my = -my; }
                (mx, my)
            }
        };
        // Carry the sub-pixel remainder so slow pans aren't quantized away.
        acc_x += dx; acc_y += dy;
        let ix = acc_x.trunc() as i32;
        let iy = acc_y.trunc() as i32;
        acc_x -= ix as f32; acc_y -= iy as f32;
        if ix != 0 || iy != 0 {
            unsafe { mouse_event(MOUSEEVENTF_MOVE, ix, iy, 0, 0); }
        }
        thread::sleep(Duration::from_millis(4)); // ~250 Hz
    }
}

/// Open the DualSense for haptic output without seizing the device on macOS,
/// so games keep receiving input through GameController / gamecontrollerd.
fn open_dualsense(api: &hidapi::HidApi, path: &CString) -> Result<hidapi::HidDevice, hidapi::HidError> {
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);
    api.open_path(path)
}

fn find_dualsense() -> Option<(CString, Transport)> {
    let api = hidapi::HidApi::new().ok()?;
    let found = api.device_list()
        .find(|d| {
            d.vendor_id()   == SONY_VENDOR
            && d.product_id() == DUALSENSE_PRODUCT
            && d.usage_page() == 0x01
            && d.usage()      == 0x05
        })
        .map(|d| {
            let transport = match d.bus_type() {
                hidapi::BusType::Bluetooth => Transport::Bluetooth,
                _                          => Transport::Usb,
            };
            (d.path().to_owned(), transport)
        });
    found
}

fn hid_loop(state: Arc<Mutex<AppState>>, app: AppHandle) {
    loop {
        // ── Open DualSense for haptic output ──────────────────────────────────
        let (path, transport) = match find_dualsense() {
            Some(p) => p,
            None    => { thread::sleep(Duration::from_secs(1)); continue; }
        };

        let api = match hidapi::HidApi::new() {
            Ok(a)  => a,
            Err(_) => { thread::sleep(Duration::from_secs(1)); continue; }
        };

        let device = match open_dualsense(&api, &path) {
            Ok(d)  => d,
            Err(_) => { thread::sleep(Duration::from_secs(2)); continue; }
        };

        // Initialize lightbar + player LED on open; mark connected
        {
            if let Ok(mut s) = state.lock() {
                s.connected = true;
                s.error_msg = String::new();
                let lb = s.profile.lightbar();
                let _ = write_report(&device, transport, &lightbar_report(lb[0], lb[1], lb[2]));
                let _ = write_report(&device, transport, &player_led_report(PLAYER_LED[s.strength_idx.min(PLAYER_LED.len() - 1)]));
            }
        }

        let audio_bands: Arc<Mutex<AudioBands>> = Arc::new(Mutex::new(AudioBands::default()));
        let mut last_profile  = Profile::Racing;
        let mut last_strength = usize::MAX;
        let mut last_mc_item  = McItem::Empty;
        {
            if let Ok(s) = state.lock() { last_strength = s.strength_idx; }
        }

        let mut last_frame = Instant::now();
        let mut last_emit  = Instant::now();
        let mut last_diag  = Instant::now();
        // Haptic report delta cache — only write to the device when the frame
        // output actually changes, saving USB/BT bandwidth and controller cycles.
        let mut last_report: Option<[u8; 48]> = None;
        // Change-detection diagnostics: print only when input moves.
        let (mut diag_l2, mut diag_r2) = (0u8, 0u8);
        eprintln!("[diag] hid_loop started, haptic output open ({:?})", transport);

        // ── Write-only haptic loop ────────────────────────────────────────────
        loop {
            let now = Instant::now();

            // ── Diagnostics ──────────────────────────────────────────────────
            // Print on input change (proves values are live), plus a 5 s heartbeat.
            if let Ok(s) = state.lock() {
                if s.l2_raw.abs_diff(diag_l2) > 3 || s.r2_raw.abs_diff(diag_r2) > 3 {
                    let lx_norm = ((s.lx as f32 - 128.0) / 127.5).clamp(-1.0, 1.0);
                    let ly_norm = (-(s.ly as f32 - 128.0) / 127.5).clamp(-1.0, 1.0);
                    eprintln!("[input] l2={} r2={} lx={:.2} ly={:.2}",
                              s.l2_raw, s.r2_raw, lx_norm, ly_norm);
                    diag_l2 = s.l2_raw; diag_r2 = s.r2_raw;
                }
            }
            if now.duration_since(last_diag) >= Duration::from_secs(5) {
                last_diag = now;
                eprintln!("[diag] haptic heartbeat");
            }

            // ── 60 fps effects output ────────────────────────────────────────
            if now.duration_since(last_frame) >= Duration::from_millis(16) {
                last_frame = now;

                let report = match state.lock() {
                    Err(_) => break,
                    Ok(mut s) => {
                        if s.profile != last_profile {
                            s.reset_pulse_state();
                            s.audio_energy  = 0.0; s.smooth_energy = 0.0;
                            s.audio_bass = 0.0; s.audio_treble = 0.0;
                            s.smooth_bass = 0.0; s.smooth_treble = 0.0;
                            if last_profile == Profile::Audio {
                                stop_audio_capture(&mut s);
                                if let Ok(mut e) = audio_bands.lock() { *e = AudioBands::default(); }
                            }
                            if s.profile == Profile::Audio {
                                start_audio_capture(&mut s, audio_bands.clone());
                            }
                            // Minecraft: open with the held-item color, not the profile
                            // default, so a reconnect/profile-switch lands on the right hue.
                            let lb = if s.profile == Profile::Minecraft {
                                last_mc_item = s.mc_item;
                                s.mc_item.lightbar()
                            } else {
                                s.profile.lightbar()
                            };
                            let _ = write_report(&device, transport, &lightbar_report(lb[0], lb[1], lb[2]));
                            last_profile = s.profile;
                        }
                        // Minecraft live lightbar — recolor whenever the held item changes.
                        if s.profile == Profile::Minecraft && s.mc_item != last_mc_item {
                            let lb = s.mc_item.lightbar();
                            let _ = write_report(&device, transport, &lightbar_report(lb[0], lb[1], lb[2]));
                            last_mc_item = s.mc_item;
                        }
                        if s.strength_idx != last_strength {
                            let _ = write_report(&device, transport, &player_led_report(PLAYER_LED[s.strength_idx.min(PLAYER_LED.len() - 1)]));
                            last_strength = s.strength_idx;
                        }
                        if s.profile == Profile::Audio {
                            if let Ok(b) = audio_bands.lock() {
                                s.audio_energy = b.overall;
                                s.audio_bass   = b.bass;
                                s.audio_treble = b.treble;
                            }
                        }
                        process_frame(&mut s)
                    }
                };

                // Delta-check: skip the write when the haptic report hasn't
                // changed since last frame (idle triggers / steady cruise).
                // First frame (None) always transmits so the device receives
                // an initial report.  Profile-switch / strength / Minecraft
                // lightbar writes above are independent and not affected.
                // Only applied over USB — Bluetooth needs every write to
                // advance the rolling sequence counter in the output report
                // header so the stack keeps the connection alive.
                let skip = transport == Transport::Usb
                    && last_report.map_or(false, |prev| prev == report);
                if !skip {
                    if write_report(&device, transport, &report).is_err() { break; }
                    last_report = Some(report);
                }
            }

            // ── Emit state snapshot to frontend at ~30 fps ───────────────────
            if now.duration_since(last_emit) >= Duration::from_millis(33) {
                last_emit = now;
                if let Ok(s) = state.lock() {
                    let _ = app.emit("state-update", s.snapshot());
                }
            }

            thread::sleep(Duration::from_millis(2));  // ~500 Hz input poll
        }

        // Output handle lost (e.g. game grabbed the device briefly) — retry open.
        // Input thread still owns connection state; don't zero trigger values here.
        if let Ok(mut s) = state.lock() {
            stop_audio_capture(&mut s);
        }
        thread::sleep(Duration::from_millis(500));
    }
}
