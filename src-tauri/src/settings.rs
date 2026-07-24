use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct SavedSettings {
    pub profile:      Option<String>,
    pub strength_idx: Option<usize>,
    #[serde(default)]
    pub gun_weapon:   Option<String>,
    #[serde(default)]
    pub melee_weapon: Option<String>,
    #[serde(default)]
    pub racing_custom_on: bool,
    #[serde(default)]
    pub racing_custom:    Option<RacingCurve>,
    #[serde(default)]
    pub tire_scrub_on:     bool,
    #[serde(default)]
    pub throttle_light_on: bool,
    #[serde(default)]
    pub drivetrain_profile: Option<usize>,
    #[serde(default)]
    pub drivetrain_auto: bool,
    #[serde(default)]
    pub game_source: Option<String>,
    #[serde(default)]
    pub motion:      Option<MotionSettings>,
    #[serde(default)]
    pub passthrough: Option<PassthroughSettings>,
    #[serde(default)]
    pub audio_tune:  Option<AudioTuneSettings>,
    #[serde(default)]
    pub drivetrain:  Option<DrivetrainSettings>,
}

/// Persisted drivetrain feel (Racing engine/throttle character).
#[derive(Serialize, Deserialize, Clone)]
pub struct DrivetrainSettings {
    pub take_up: u8,
    pub idle_hz: u8,
    pub red_hz:  u8,
    pub weight:  u8,
    #[serde(default = "def_dt_load")]
    pub load:    u8,
}

fn def_dt_load() -> u8 { 50 }

/// Persisted haptic EQ for the Audio profile's true-haptics stream.
#[derive(Serialize, Deserialize, Clone)]
pub struct AudioTuneSettings {
    pub sub_gain:    f32,
    pub engine_gain: f32,
    pub gate:        f32,
}

/// Persisted Motion panel config (tilt steering + gyro aim).
#[derive(Serialize, Deserialize, Clone)]
pub struct MotionSettings {
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
}

/// Persisted game-rumble passthrough config.
#[derive(Serialize, Deserialize, Clone)]
pub struct PassthroughSettings {
    pub enabled:      bool,
    pub intensity:    f32,
    pub trigger_kick: bool,
    pub lightbar:     bool,
}

/// Persisted Racing Lab brake/throttle curve + extra tuning knobs.
/// The tuning fields carry per-field defaults so save files written before
/// they existed still deserialize to the original constant values.
#[derive(Serialize, Deserialize, Clone)]
pub struct RacingCurve {
    pub brake_start:    u8,
    pub brake_end:      u8,
    pub brake_exp:      f32,
    pub throttle_start: u8,
    pub throttle_end:   u8,
    pub throttle_exp:   f32,
    pub shift_force:    u8,
    #[serde(default = "def_abs_freq")]
    pub abs_freq:       u8,
    #[serde(default = "def_abs_delay")]
    pub abs_delay:      u8,
    #[serde(default = "def_engine_texture")]
    pub engine_texture: u8,
    #[serde(default = "def_feather_end")]
    pub feather_end:    u8,
}

fn def_abs_freq()       -> u8 { 5 }
fn def_abs_delay()      -> u8 { 18 }
fn def_engine_texture() -> u8 { 22 }
fn def_feather_end()    -> u8 { 38 }

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("dualsense-haptics")
        .join("settings.json")
}

pub fn load() -> SavedSettings {
    let path = settings_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[settings] read {} failed: {e} — using defaults", path.display());
            return SavedSettings::default();
        }
    };
    match serde_json::from_str(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[settings] parse {} failed: {e} — backing up to .bak and using defaults", path.display());
            let bak = path.with_extension("json.bak");
            let _ = std::fs::write(&bak, &text);
            SavedSettings::default()
        }
    }
}

pub fn save(data: &SavedSettings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(&path, json);
    }
}
