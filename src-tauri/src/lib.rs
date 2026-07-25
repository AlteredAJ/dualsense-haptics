mod acc;
mod feels;
mod f123;
mod forza;
mod hid;
mod signal;
mod license;
mod mc;
mod obfuscate;
mod settings;
#[cfg(windows)]
mod hidhide;
#[cfg(windows)]
mod xinput;

use hid::{AppState, Edition, GameSource, Profile, STRENGTHS, WEAPONS};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

/// Holds the stop flag for the active telemetry bridge so we can tear it
/// down cleanly when the user switches games.
struct BridgeManager {
    stop: Option<Arc<AtomicBool>>,
}

/// Return type for init_session — includes edition so JS knows what tier was granted.
#[derive(Serialize)]
struct SessionResult {
    ok:      bool,
    edition: String,  // "free" | "full"
    pro:     bool,    // $4 Pro tier — Lab unlocked
    error:   String,
}

/// Lab gate — every Lab-only command checks this so a patched frontend can't
/// reach Live Preview or feel tuning without a Pro ($4) license.
fn require_pro(s: &AppState) -> Result<(), String> {
    if s.pro { Ok(()) } else { Err("pro_required".to_string()) }
}

pub type SharedState = Arc<Mutex<AppState>>;

// ─── License gate ─────────────────────────────────────────────────────────────
// The HID thread is NOT started at app launch. It only starts after Rust
// confirms a valid license. Patching the JS frontend accomplishes nothing
// because haptics never flow until this gate is passed in Rust.

struct LicenseGate {
    hid_started: AtomicBool,
}

impl LicenseGate {
    fn new() -> Self {
        Self { hid_started: AtomicBool::new(false) }
    }

    /// Try to start the HID thread. No-ops on subsequent calls. Returns true if spawned now.
    fn try_start_hid(&self, state: SharedState, app: tauri::AppHandle) -> bool {
        if self.hid_started.swap(true, Ordering::SeqCst) {
            return false; // already running
        }
        hid::spawn_hid_thread(state, app);
        true
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_state(state: State<SharedState>) -> hid::StateSnapshot {
    state.lock().unwrap().snapshot()
}

/// Snapshot every persisted field and write it to disk.
fn persist(s: &AppState) {
    settings::save(&settings::SavedSettings {
        profile:      Some(s.profile.as_str().to_string()),
        strength_idx: Some(s.strength_idx),
        gun_weapon:   Some(WEAPONS[s.gun_weapon].key.to_string()),
        melee_weapon: Some(hid::MELEE_WEAPONS[s.melee_weapon].key.to_string()),
        racing_custom_on: s.racing_custom_on,
        racing_custom:    Some(settings::RacingCurve {
            brake_start:    s.racing_custom.brake_start,
            brake_end:      s.racing_custom.brake_end,
            brake_exp:      s.racing_custom.brake_exp,
            throttle_start: s.racing_custom.throttle_start,
            throttle_end:   s.racing_custom.throttle_end,
            throttle_exp:   s.racing_custom.throttle_exp,
            shift_force:    s.racing_custom.shift_force,
            abs_freq:       s.racing_tuning.abs_freq,
            abs_delay:      s.racing_tuning.abs_delay,
            engine_texture: s.racing_tuning.engine_texture,
            feather_end:    s.racing_tuning.feather_end,
        }),
        tire_scrub_on:     s.tire_scrub_on,
        throttle_light_on: s.throttle_light_on,
        drivetrain_profile: Some(s.drivetrain_profile_idx),
        drivetrain_auto: s.drivetrain_auto,
        game_source: Some(s.game_source.as_str().to_string()),
        racing_assist_stability: s.racing_assist_stability,
        racing_assist_drift: s.racing_assist_drift,
        motion: Some(settings::MotionSettings {
            steer_enabled:  s.motion.steer_enabled,
            steer_sens:     s.motion.steer_sens,
            steer_deadzone: s.motion.steer_deadzone,
            steer_max_deg:  s.motion.steer_max_deg,
            steer_invert:   s.motion.steer_invert,
            steer_axis:     s.motion.steer_axis,
            aim_enabled:    s.motion.aim_enabled,
            aim_mode:       s.motion.aim_mode,
            aim_sens_x:     s.motion.aim_sens_x,
            aim_sens_y:     s.motion.aim_sens_y,
            aim_deadzone:   s.motion.aim_deadzone,
            aim_invert_y:   s.motion.aim_invert_y,
        }),
        passthrough: Some(settings::PassthroughSettings {
            enabled:      s.pt_enabled,
            intensity:    s.pt_intensity,
            trigger_kick: s.pt_trigger_kick,
            lightbar:     s.pt_lightbar,
        }),
        audio_tune: s.audio_tune.lock().ok().map(|t| settings::AudioTuneSettings {
            sub_gain:    t.sub_gain,
            engine_gain: t.engine_gain,
            gate:        t.gate,
        }),
        drivetrain: Some(settings::DrivetrainSettings {
            take_up: s.drivetrain.take_up,
            idle_hz: s.drivetrain.idle_hz,
            red_hz:  s.drivetrain.red_hz,
            weight:  s.drivetrain.weight,
            load:    s.drivetrain.load,
        }),
    });
}

#[tauri::command]
fn set_profile(state: State<SharedState>, profile: String) {
    let mut s = state.lock().unwrap();
    s.profile = Profile::from_str(&profile);
    persist(&s);
}

/// Switch output between native DualSense and the virtual Xbox (XInput) pad. The Xbox
/// path only does anything on Windows (it needs ViGEmBus); on other platforms this just
/// stores the preference. Returns the resolved mode.
#[tauri::command]
fn set_output_mode(state: State<SharedState>, mode: String) -> String {
    let mut s = state.lock().unwrap();
    s.output_mode = hid::OutputMode::from_str(&mode);
    s.output_mode.as_str().to_string()
}

#[tauri::command]
fn set_strength(state: State<SharedState>, idx: usize) {
    let mut s = state.lock().unwrap();
    if idx < STRENGTHS.len() {
        s.strength_idx = idx;
        persist(&s);
    }
}

/// Select a gun weapon profile by key (e.g. "pistol", "ar", "sniper").
/// Returns the resolved weapon key. Resets any in-flight recoil/burst state.
#[tauri::command]
fn set_gun_weapon(state: State<SharedState>, key: String) -> String {
    let mut s = state.lock().unwrap();
    s.gun_weapon = hid::weapon_index(&key);
    s.recoil_pulse_frames = 0;
    s.recoil_fired        = false;
    s.gun_burst_remaining = 0;
    s.gun_burst_gap       = 0;
    persist(&s);
    WEAPONS[s.gun_weapon].key.to_string()
}

/// Select a melee weapon profile by key (e.g. "knife", "sledge"). Returns the
/// resolved weapon key. Resets any in-flight swing/impact state.
#[tauri::command]
fn set_melee_weapon(state: State<SharedState>, key: String) -> String {
    let mut s = state.lock().unwrap();
    s.melee_weapon       = hid::melee_weapon_index(&key);
    s.melee_impact_frames = 0;
    s.melee_impact_fired  = false;
    persist(&s);
    hid::MELEE_WEAPONS[s.melee_weapon].key.to_string()
}

/// Set the Minecraft held-item category for lab live preview (e.g. "pickaxe").
/// Resets the dynamic action state so the new item starts clean.
#[tauri::command]
fn set_mc_item(state: State<SharedState>, item: String) {
    let mut s = state.lock().unwrap();
    s.mc_item     = hid::McItem::from_str(&item);
    s.mc_mining   = false;
    s.mc_using    = false;
    s.mc_use_prog = 0.0;
    s.mc_blocking = false;
}

/// Lab live preview — route the real engine to `profile` so the controller drives
/// the genuine effect, with no game/mod connected. Remembers and restores the
/// user's selected profile when toggled off. For Minecraft, enables input
/// synthesis (mc_preview) so R2/L2/face buttons drive the per-item feels.
#[tauri::command]
fn set_preview(state: State<SharedState>, active: bool, profile: String) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    require_pro(&s)?;
    if active {
        if s.preview_prev.is_none() {
            s.preview_prev = Some(s.profile);
        }
        let p = Profile::from_str(&profile);
        s.profile     = p;
        s.mc_preview  = p == Profile::Minecraft;
        if s.mc_preview {
            s.mc_mining = false; s.mc_using = false; s.mc_use_prog = 0.0;
            s.mc_blocking = false; s.mc_health = 20.0; s.mc_on_ground = true;
        }
    } else {
        if let Some(p) = s.preview_prev.take() {
            s.profile = p;
        }
        s.mc_preview  = false;
        s.mc_mining   = false; s.mc_using = false; s.mc_use_prog = 0.0; s.mc_blocking = false;
    }
    s.reset_pulse_state();
    Ok(())
}

/// Return the current feel tuning as a JSON object the Lab editor can render.
#[tauri::command]
fn get_feels() -> feels::Feels {
    feels::load()
}

/// Persist edited feel tuning to feels.json and apply it live to the engine.
#[tauri::command]
fn save_feels(state: State<SharedState>, feels: feels::Feels) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    require_pro(&s)?;
    feels::save(&feels);
    s.weapons       = feels::gun_table(&feels);
    s.melee_weapons = feels::melee_table(&feels);
    Ok(())
}

/// Reset feel tuning back to the built-in code defaults (and rewrite feels.json).
#[tauri::command]
fn reset_feels(state: State<SharedState>) -> Result<feels::Feels, String> {
    let mut s = state.lock().unwrap();
    require_pro(&s)?;
    let d = feels::defaults();
    feels::save(&d);
    s.weapons       = feels::gun_table(&d);
    s.melee_weapons = feels::melee_table(&d);
    Ok(d)
}

#[tauri::command]
fn toggle_shift(state: State<SharedState>) -> bool {
    let mut s = state.lock().unwrap();
    s.shift_enabled = !s.shift_enabled;
    s.shift_enabled
}

/// Session init — determines edition (Free or Full Immersion) and starts the HID thread.
///
/// Free tier: app always runs, features restricted in Rust process_frame.
/// Full Immersion: valid Gumroad license required.
///
/// Debug builds always return Full and start HID immediately from setup().
#[tauri::command]
#[allow(unused_variables)]
fn init_session(
    key: Option<String>,
    gate: State<'_, LicenseGate>,
    app_state: State<'_, SharedState>,
    app: tauri::AppHandle,
) -> SessionResult {
    #[cfg(debug_assertions)] {
        // Debug: Full Immersion + Pro, HID already started in setup()
        return SessionResult { ok: true, edition: "full".into(), pro: true, error: String::new() };
    }

    #[allow(unreachable_code)]
    {
        // Release: always start HID (free tier runs without a key)
        gate.try_start_hid((*app_state).clone(), app);

        // Check license — either validate cached token or activate a new key
        let result = license::check(key);

        if result.ok {
            // Valid license — upgrade to Full Immersion. Lab unlocked only if Pro ($4).
            if let Ok(mut s) = app_state.lock() {
                s.edition = Edition::Full;
                s.pro     = result.pro;
            }
            SessionResult { ok: true, edition: "full".into(), pro: result.pro, error: String::new() }
        } else if result.error.is_empty() {
            // No key / no cache — start in Free tier, no error shown
            SessionResult { ok: true, edition: "free".into(), pro: false, error: String::new() }
        } else {
            // Key provided but invalid, or token expired — show error, stay Free
            SessionResult { ok: false, edition: "free".into(), pro: false, error: result.error }
        }
    }
}

/// Trigger Lab payload from the frontend test bench.
/// `side`: 0 = L2 only, 1 = R2 only, 2 = both. `params` is up to 10 effect bytes.
#[derive(Deserialize)]
struct TestEffect {
    active:   bool,
    side:     u8,
    mode:     u8,
    params:   Vec<u8>,
    rumble_l: u8,
    rumble_r: u8,
}

#[tauri::command]
fn set_test(state: State<SharedState>, effect: TestEffect) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    require_pro(&s)?;
    s.test_active = effect.active;

    let mut p = [0u8; 10];
    for (i, v) in effect.params.iter().take(10).enumerate() {
        p[i] = *v;
    }

    // Apply the effect to the chosen trigger; set the other side to off (0x05).
    match effect.side {
        0 => {
            s.test_left_mode  = effect.mode; s.test_left_params  = p;
            s.test_right_mode = 0x05;        s.test_right_params = [0; 10];
        }
        1 => {
            s.test_right_mode = effect.mode; s.test_right_params = p;
            s.test_left_mode  = 0x05;        s.test_left_params  = [0; 10];
        }
        _ => {
            s.test_left_mode  = effect.mode; s.test_left_params  = p;
            s.test_right_mode = effect.mode; s.test_right_params = p;
        }
    }

    s.test_rumble_l = effect.rumble_l;
    s.test_rumble_r = effect.rumble_r;
    Ok(())
}

/// Racing Lab brake/throttle curve + extra tuning from the personalize tab.
#[derive(Deserialize)]
struct RacingCurve {
    brake_start:    u8,
    brake_end:      u8,
    brake_exp:      f32,
    throttle_start: u8,
    throttle_end:   u8,
    throttle_exp:   f32,
    shift_force:    u8,
    abs_freq:       u8,
    abs_delay:      u8,
    engine_texture: u8,
    feather_end:    u8,
}

fn apply_curve(s: &mut AppState, c: &RacingCurve) {
    s.racing_custom = hid::Strength {
        label:          "Custom",
        brake_start:    c.brake_start,
        brake_end:      c.brake_end,
        brake_exp:      c.brake_exp,
        throttle_start: c.throttle_start,
        throttle_end:   c.throttle_end,
        throttle_exp:   c.throttle_exp,
        shift_force:    c.shift_force,
    };
    s.racing_tuning = hid::RacingTuning {
        abs_freq:       c.abs_freq,
        abs_delay:      c.abs_delay,
        engine_texture: c.engine_texture,
        feather_end:    c.feather_end,
    };
}

/// Live Racing Lab preview — set the custom curve and toggle live application
/// (Racing profile feels it immediately). Does not persist; that's save_racing_custom.
#[tauri::command]
fn set_racing_lab(state: State<SharedState>, active: bool, curve: RacingCurve) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    require_pro(&s)?;
    apply_curve(&mut s, &curve);
    s.racing_lab_active = active;
    Ok(())
}

/// Save the current custom curve as the user's Racing profile (persists, applies
/// outside the lab). `enabled = false` reverts Racing to the strength presets.
#[tauri::command]
fn save_racing_custom(state: State<SharedState>, enabled: bool, curve: RacingCurve) -> bool {
    let mut s = state.lock().unwrap();
    apply_curve(&mut s, &curve);
    s.racing_custom_on = enabled;
    persist(&s);
    s.racing_custom_on
}

/// Toggle the Racing steering-FX overlays (tire scrub, throttle lightening).
/// Persists immediately so the choice sticks across launches.
#[tauri::command]
fn set_steering_fx(state: State<SharedState>, tire_scrub: bool, throttle_light: bool) {
    let mut s = state.lock().unwrap();
    s.tire_scrub_on     = tire_scrub;
    s.throttle_light_on = throttle_light;
    persist(&s);
}

#[tauri::command]
fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ─── Motion (tilt steering + gyro aim) ─────────────────────────────────────────

/// Tilt-steering config from the Motion panel. Drives the virtual Xbox left-stick X
/// from the pad's physical tilt (roll or pitch axis), self-centering via the accel.
#[derive(Deserialize)]
struct SteerCfg {
    enabled:  bool,
    sens:     f32,
    deadzone: f32,
    max_deg:  f32,
    invert:   bool,
    axis:     u8,   // 0 = roll, 1 = pitch
}

#[tauri::command]
fn set_motion_steer(state: State<SharedState>, cfg: SteerCfg) {
    let mut s = state.lock().unwrap();
    s.motion.steer_enabled  = cfg.enabled;
    s.motion.steer_sens     = cfg.sens.clamp(0.1, 5.0);
    s.motion.steer_deadzone = cfg.deadzone.clamp(0.0, 30.0);
    s.motion.steer_max_deg  = cfg.max_deg.clamp(5.0, 90.0);
    s.motion.steer_invert   = cfg.invert;
    s.motion.steer_axis     = cfg.axis.min(1);
    persist(&s);
}

/// Gyro-aim config from the Motion panel. Maps angular velocity to relative mouse
/// motion (Windows). `mode`: 0 = always, 1 = hold touchpad, 2 = touchpad toggles.
#[derive(Deserialize)]
struct AimCfg {
    enabled:  bool,
    mode:     u8,
    sens_x:   f32,
    sens_y:   f32,
    deadzone: f32,
    invert_y: bool,
}

/// Game rumble passthrough config — how the rumble a game sends to the virtual
/// Xbox pad is re-expanded into DualSense haptics.
#[derive(Deserialize)]
struct PassthroughCfg {
    enabled:      bool,
    intensity:    f32,
    trigger_kick: bool,
    lightbar:     bool,
}

#[tauri::command]
fn set_rumble_passthrough(state: State<SharedState>, cfg: PassthroughCfg) {
    let mut s = state.lock().unwrap();
    s.pt_enabled      = cfg.enabled;
    s.pt_intensity    = cfg.intensity.clamp(0.1, 2.0);
    s.pt_trigger_kick = cfg.trigger_kick;
    s.pt_lightbar     = cfg.lightbar;
    persist(&s);
}

/// Drivetrain feel — the live engine/throttle character knobs from the Racing Lab.
#[derive(Deserialize)]
struct DrivetrainCfg {
    take_up: u8,
    idle_hz: u8,
    red_hz:  u8,
    weight:  u8,
    load:    u8,
}

#[tauri::command]
fn set_drivetrain(state: State<SharedState>, cfg: DrivetrainCfg) {
    let mut s = state.lock().unwrap();
    s.drivetrain.take_up = cfg.take_up.clamp(16, 110);
    s.drivetrain.idle_hz = cfg.idle_hz.clamp(3, 16);
    s.drivetrain.red_hz  = cfg.red_hz.clamp(16, 44);
    s.drivetrain.weight  = cfg.weight.min(100);
    s.drivetrain.load    = cfg.load.min(100);
    persist(&s);
}

#[tauri::command]
fn set_drivetrain_profile(state: State<SharedState>, idx: usize) -> usize {
    let mut s = state.lock().unwrap();
    let resolved = if idx < hid::DRIVETRAIN_PROFILES.len() { idx } else { 0 };
    s.drivetrain_profile_idx = resolved;
    persist(&s);
    resolved
}

#[tauri::command]
fn set_drivetrain_auto(state: State<SharedState>, enabled: bool) -> bool {
    let mut s = state.lock().unwrap();
    s.drivetrain_auto = enabled;
    if !enabled {
        s.slip_history_rear.clear();
        s.slip_history_front.clear();
    }
    persist(&s);
    s.drivetrain_auto
}

/// Haptic EQ for the Audio profile's true-haptics stream: per-band gains and the
/// expander gate. Applied live by the capture thread (shared AudioTune).
#[derive(Deserialize)]
struct AudioTuneCfg {
    sub:    f32,
    engine: f32,
    gate:   f32,
}

#[tauri::command]
fn set_audio_tune(state: State<SharedState>, cfg: AudioTuneCfg) {
    let s = state.lock().unwrap();
    if let Ok(mut t) = s.audio_tune.lock() {
        t.sub_gain    = cfg.sub.clamp(0.0, 4.0);
        t.engine_gain = cfg.engine.clamp(0.0, 4.0);
        t.gate        = cfg.gate.clamp(0.001, 0.2);
    }
    persist(&s);
}

#[tauri::command]
fn set_motion_aim(state: State<SharedState>, cfg: AimCfg) {
    let mut s = state.lock().unwrap();
    s.motion.aim_enabled  = cfg.enabled;
    s.motion.aim_mode     = cfg.mode.min(2);
    s.motion.aim_sens_x   = cfg.sens_x.clamp(0.0, 100.0);
    s.motion.aim_sens_y   = cfg.sens_y.clamp(0.0, 100.0);
    s.motion.aim_deadzone = cfg.deadzone.clamp(0.0, 20.0);
    s.motion.aim_invert_y = cfg.invert_y;
    if !cfg.enabled { s.aim_toggle_on = false; }
    persist(&s);
}

/// Compare two semver strings. Returns true if `a` is strictly greater than `b`.
fn semver_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let mut p = s.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
        (p.next().unwrap_or(0), p.next().unwrap_or(0), p.next().unwrap_or(0))
    };
    parse(a) > parse(b)
}

#[derive(Serialize)]
struct UpdateInfo {
    update_available: bool,
    latest:           String,
}

#[tauri::command]
fn check_update() -> UpdateInfo {
    const CURRENT: &str = env!("CARGO_PKG_VERSION");
    const URL: &str = "https://dualsense-haptics-license.universal-dualsense-haptics.workers.dev/version";

    match ureq::get(URL).call() {
        Ok(resp) => {
            if let Ok(json) = resp.into_json::<serde_json::Value>() {
                let latest = json["latest"].as_str().unwrap_or(CURRENT).to_string();
                let update_available = semver_gt(&latest, CURRENT);
                return UpdateInfo { update_available, latest };
            }
        }
        Err(_) => {}
    }
    // Network error / timeout — silently no-op
    UpdateInfo { update_available: false, latest: CURRENT.to_string() }
}

#[tauri::command]
fn set_window_size(app: tauri::AppHandle, width: f64, height: f64) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize { width, height }));
    }
}

// ─── App setup ────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Binary integrity check — prevents tampered/cracked binaries from
    // running Full-tier features. Debug builds skip this.
    #[cfg(not(debug_assertions))]
    obfuscate::verify_integrity();

    // Windows: this machine has a Software Restriction Policy that blocks
    // C:\Program Files (x86)\Microsoft\Edge* — which also matches EdgeWebView, so the
    // stock WebView2 runtime can't spawn its browser process and the window renders
    // blank. We ship a copy of the runtime at an allowed path and point WebView2 at it.
    // Only override if that folder exists, otherwise fall back to the system runtime.
    #[cfg(windows)]
    {
        if let Ok(exe) = std::env::current_exe() {
            // exe = <project>\src-tauri\target\debug\dualsense-haptics.exe
            // runtime folder lives at <project>\wv2runtime
            let mut p = exe;
            for _ in 0..4 { p.pop(); } // -> <project>
            let rt = p.join("wv2runtime");
            if rt.join("msedgewebview2.exe").exists() {
                std::env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", &rt);
                eprintln!("[wv2] using bundled runtime at {}", rt.display());
            } else {
                eprintln!("[wv2] bundled runtime not found at {}, using system", rt.display());
            }
        }
    }

    let app_state: SharedState = Arc::new(Mutex::new(AppState::default()));

    // Load saved settings
    {
        let saved = settings::load();
        let mut s = app_state.lock().unwrap();
        // Apply runtime feel tuning from feels.json (writes defaults if missing).
        let f = feels::load();
        s.weapons       = feels::gun_table(&f);
        s.melee_weapons = feels::melee_table(&f);
        if let Some(p) = saved.profile {
            s.profile = Profile::from_str(&p);
        }
        if let Some(idx) = saved.strength_idx {
            if idx < STRENGTHS.len() {
                s.strength_idx = idx;
            }
        }
        if let Some(key) = saved.gun_weapon {
            s.gun_weapon = hid::weapon_index(&key);
        }
        if let Some(key) = saved.melee_weapon {
            s.melee_weapon = hid::melee_weapon_index(&key);
        }
        if let Some(c) = saved.racing_custom {
            s.racing_custom = hid::Strength {
                label:          "Custom",
                brake_start:    c.brake_start,
                brake_end:      c.brake_end,
                brake_exp:      c.brake_exp,
                throttle_start: c.throttle_start,
                throttle_end:   c.throttle_end,
                throttle_exp:   c.throttle_exp,
                shift_force:    c.shift_force,
            };
            // Tuning fields default to the module constants when absent in older saves.
            s.racing_tuning = hid::RacingTuning {
                abs_freq:       c.abs_freq,
                abs_delay:      c.abs_delay,
                engine_texture: c.engine_texture,
                feather_end:    c.feather_end,
            };
        }
        s.racing_custom_on = saved.racing_custom_on;
        s.tire_scrub_on     = saved.tire_scrub_on;
        s.throttle_light_on = saved.throttle_light_on;
        if let Some(idx) = saved.drivetrain_profile {
            if idx < hid::DRIVETRAIN_PROFILES.len() {
                s.drivetrain_profile_idx = idx;
            }
        }
        s.drivetrain_auto = saved.drivetrain_auto;
        if let Some(gs) = saved.game_source {
            s.game_source = GameSource::from_str(&gs);
        }
        s.racing_assist_stability = saved.racing_assist_stability;
        s.racing_assist_drift = saved.racing_assist_drift;
        if let Some(m) = saved.motion {
            s.motion.steer_enabled  = m.steer_enabled;
            s.motion.steer_sens     = m.steer_sens;
            s.motion.steer_deadzone = m.steer_deadzone;
            s.motion.steer_max_deg  = m.steer_max_deg;
            s.motion.steer_invert   = m.steer_invert;
            s.motion.steer_axis     = m.steer_axis;
            s.motion.aim_enabled    = m.aim_enabled;
            s.motion.aim_mode       = m.aim_mode;
            s.motion.aim_sens_x     = m.aim_sens_x;
            s.motion.aim_sens_y     = m.aim_sens_y;
            s.motion.aim_deadzone   = m.aim_deadzone;
            s.motion.aim_invert_y   = m.aim_invert_y;
        }
        if let Some(p) = saved.passthrough {
            s.pt_enabled      = p.enabled;
            s.pt_intensity    = p.intensity;
            s.pt_trigger_kick = p.trigger_kick;
            s.pt_lightbar     = p.lightbar;
        }
        if let Some(d) = saved.drivetrain {
            s.drivetrain.take_up = d.take_up;
            s.drivetrain.idle_hz = d.idle_hz;
            s.drivetrain.red_hz  = d.red_hz;
            s.drivetrain.weight  = d.weight;
            s.drivetrain.load    = d.load;
        }
        if let Some(a) = saved.audio_tune {
            if let Ok(mut t) = s.audio_tune.lock() {
                t.sub_gain    = a.sub_gain;
                t.engine_gain = a.engine_gain;
                t.gate        = a.gate;
            }
    }
}

/// Spawn the telemetry bridge for the given game source. Returns the stop flag
/// so the caller can tear it down later.
fn spawn_bridge_for(source: GameSource, state: Arc<Mutex<AppState>>) -> Option<Arc<AtomicBool>> {
    match source {
        GameSource::Forza => {
            let stop = Arc::new(AtomicBool::new(false));
            forza::spawn_bridge(state, stop.clone());
            Some(stop)
        }
        GameSource::F123 => {
            let stop = Arc::new(AtomicBool::new(false));
            f123::spawn(state, stop.clone());
            Some(stop)
        }
        GameSource::Assetto => {
            let stop = Arc::new(AtomicBool::new(false));
            acc::spawn(state, stop.clone());
            Some(stop)
        }
        GameSource::None => None,
    }
}

#[tauri::command]
fn set_game_source(
    state: State<SharedState>,
    bridge: State<Arc<Mutex<BridgeManager>>>,
    source: String,
) -> String {
    let gs = GameSource::from_str(&source);
    let mut s = state.lock().unwrap();
    s.game_source = gs;
    persist(&s);
    drop(s);

    // Tear down old bridge, spawn new one.
    let mut bm = bridge.lock().unwrap();
    if let Some(stop) = bm.stop.take() {
        stop.store(true, Ordering::SeqCst);
    }
    bm.stop = spawn_bridge_for(gs, state.inner().clone());

    gs.as_str().to_string()
}

#[tauri::command]
fn set_racing_assist(state: State<SharedState>, stability: bool, drift: bool) {
    let mut s = state.lock().unwrap();
    s.racing_assist_stability = stability;
    s.racing_assist_drift = drift;
    persist(&s);
}

    tauri::Builder::default()
        .manage(app_state.clone())
        .manage(LicenseGate::new())
        .manage(Arc::new(Mutex::new(BridgeManager { stop: None })))
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_profile,
            set_output_mode,
            set_strength,
            set_gun_weapon,
            toggle_shift,
            init_session,
            get_version,
            check_update,
            set_window_size,
            set_test,
            set_racing_lab,
            save_racing_custom,
            set_steering_fx,
            set_melee_weapon,
            set_mc_item,
            set_preview,
            get_feels,
            save_feels,
            reset_feels,
            set_motion_steer,
            set_motion_aim,
            set_rumble_passthrough,
            set_drivetrain,
            set_drivetrain_profile,
            set_drivetrain_auto,
            set_game_source,
            set_racing_assist,
            set_audio_tune,
        ])
        .setup(move |app| {
            // In debug builds, skip the license gate entirely and start HID immediately.
            // In release builds the HID thread only starts once init_session() validates.
            #[cfg(debug_assertions)]
            hid::spawn_hid_thread(app_state.clone(), app.handle().clone());

            // Minecraft bridge — localhost TCP server the Fabric mod connects to.
            // Always runs; harmless when no mod is connected.
            mc::spawn_bridge(app_state.clone());

            // Telemetry bridge — spawn whichever game is selected (or none).
            spawn_bridge_for(app_state.lock().unwrap().game_source, app_state.clone());

            // Explicitly size and show the main window. On Windows the window can
            // get stuck at a 16x16 placeholder if WebView2 initializes slowly.
            match app.get_webview_window("main") {
                Some(win) => {
                    eprintln!("[setup] got main window, sizing to 820x540");
                    let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                        width: 820.0,
                        height: 540.0,
                    }));
                    let _ = win.center();
                    let _ = win.show();
                    let _ = win.set_focus();
                    eprintln!("[setup] window shown");
                }
                None => eprintln!("[setup] ERROR: get_webview_window(\"main\") returned None"),
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running Tauri app");
}
