// Runtime-editable feel tables. The built-in WEAPONS / MELEE_WEAPONS constants in
// hid.rs are the defaults; this module lets you override their numeric tuning from
// a plain JSON file (~/.config/dualsense-haptics/feels.json) with no recompile.
//
// Only the numeric feel values are overridable. The weapon set, order, keys, and
// gun firing pattern stay fixed in code — editing JSON never changes which weapons
// exist, just how each one feels.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::hid::{MeleeWeapon, Weapon, MELEE_WEAPONS, WEAPONS};

#[derive(Serialize, Deserialize, Clone)]
pub struct GunTune {
    pub key:         String,
    pub kick_freq:   u8,
    pub kick_amp:    u8,
    pub rumble_l:    u8,
    pub rumble_r:    u8,
    pub burst_count: u8,
    pub rate_hz:     u8,
    pub kick_frames: u8,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MeleeTune {
    pub key:           String,
    pub swing_force:   u8,
    pub swing_exp:     f32,
    pub impact_freq:   u8,
    pub impact_force:  u8,
    pub impact_frames: u8,
    pub rumble_l:      u8,
    pub rumble_r:      u8,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Feels {
    #[serde(default)]
    pub guns:  Vec<GunTune>,
    #[serde(default)]
    pub melee: Vec<MeleeTune>,
}

fn feels_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("dualsense-haptics")
        .join("feels.json")
}

/// A Feels built entirely from the code defaults.
pub fn defaults() -> Feels {
    Feels {
        guns: WEAPONS
            .iter()
            .map(|w| GunTune {
                key:         w.key.to_string(),
                kick_freq:   w.kick_freq,
                kick_amp:    w.kick_amp,
                rumble_l:    w.rumble_l,
                rumble_r:    w.rumble_r,
                burst_count: w.burst_count,
                rate_hz:     w.rate_hz,
                kick_frames: w.kick_frames,
            })
            .collect(),
        melee: MELEE_WEAPONS
            .iter()
            .map(|m| MeleeTune {
                key:           m.key.to_string(),
                swing_force:   m.swing_force,
                swing_exp:     m.swing_exp,
                impact_freq:   m.impact_freq,
                impact_force:  m.impact_force,
                impact_frames: m.impact_frames,
                rumble_l:      m.rumble_l,
                rumble_r:      m.rumble_r,
            })
            .collect(),
    }
}

/// Load feels.json. If it is missing, write the defaults so there is always a file
/// to edit. Any weapon not present in the file keeps its code default.
pub fn load() -> Feels {
    let path = feels_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| defaults()),
        Err(_) => {
            let d = defaults();
            save(&d);
            d
        }
    }
}

pub fn save(feels: &Feels) {
    let path = feels_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(feels) {
        let _ = std::fs::write(&path, json);
    }
}

/// Resolve the runtime gun table: code defaults with JSON tuning applied by key.
pub fn gun_table(feels: &Feels) -> Vec<Weapon> {
    WEAPONS
        .iter()
        .map(|w| {
            let mut w = *w;
            if let Some(t) = feels.guns.iter().find(|t| t.key == w.key) {
                w.kick_freq = t.kick_freq;
                w.kick_amp = t.kick_amp;
                w.rumble_l = t.rumble_l;
                w.rumble_r = t.rumble_r;
                w.burst_count = t.burst_count;
                w.rate_hz = t.rate_hz;
                w.kick_frames = t.kick_frames;
            }
            w
        })
        .collect()
}

/// Resolve the runtime melee table: code defaults with JSON tuning applied by key.
pub fn melee_table(feels: &Feels) -> Vec<MeleeWeapon> {
    MELEE_WEAPONS
        .iter()
        .map(|m| {
            let mut m = *m;
            if let Some(t) = feels.melee.iter().find(|t| t.key == m.key) {
                m.swing_force = t.swing_force;
                m.swing_exp = t.swing_exp;
                m.impact_freq = t.impact_freq;
                m.impact_force = t.impact_force;
                m.impact_frames = t.impact_frames;
                m.rumble_l = t.rumble_l;
                m.rumble_r = t.rumble_r;
            }
            m
        })
        .collect()
}
