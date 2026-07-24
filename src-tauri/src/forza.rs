// ─── Forza telemetry bridge ────────────────────────────────────────────────
//
// Forza Motorsport and Forza Horizon both broadcast a "Data Out" UDP packet every
// frame (~60 Hz). Enable it in-game: Settings → HUD/Gameplay → Data Out → On, IP
// 127.0.0.1, Port 5300 (matching FORZA_PORT below). We bind a UDP socket on loopback,
// parse the packet, and write the REAL car state into AppState so the Racing haptics
// run off actual RPM / acceleration / tire slip instead of values inferred from the
// trigger inputs. When no packets arrive (game closed, Data Out off) we mark telemetry
// inactive and the engine falls back to the inferred model.
//
// The "Sled" block (first 232 bytes) is identical across every Forza title, so the
// fields we care about most — RPM, acceleration, per-wheel tire slip, suspension,
// surface rumble — are at fixed offsets everywhere. Gear lives in the "Dash" block,
// whose offset shifted between titles, so we locate it by packet length.

use crate::hid::AppState;
use crate::signal::{
    self, EWMA_ALPHA, HEAVE_LPF_HZ, SUSP_LPF_HZ, TELEM_SAMPLE_HZ, TC_HAPTIC_SCALE,
};
use std::io::Write;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Bind on all interfaces so we catch Forza's packets whether Data Out targets the
/// loopback (127.0.0.1) or the machine's LAN IP.
///
/// We listen on EVERY common Data Out port at once, so the app just works whatever
/// the user has set in-game without them having to match a single hardcoded value.
/// 5300 is the old Forza default; 7000 is what recent setups (and AJ's) use; 20066
/// is the other value the docs mention. Only the port Forza is actually sending to
/// receives anything — the rest sit idle and cost nothing.
pub const FORZA_PORTS: &[u16] = &[5300, 7000, 20066];

/// A feed with no packet newer than this is treated as stopped (game closed / Data
/// Out off), which clears both the connection and live-race flags.
const STALE_AFTER: Duration = Duration::from_millis(1000);

#[inline]
fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}
#[inline]
fn i32_at(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// Gear byte offset within the Dash block, keyed by total packet length.
/// FM7 = 311 (gear @ 307); FH4 = 324, FH5 / FM2023 = 331 (gear @ 319). Sled-only
/// packets (232) have no gear. Unknown lengths return None and we keep using the
/// controller-bumper shift detection instead.
fn gear_offset(len: usize) -> Option<usize> {
    match len {
        311 => Some(307),
        l if l >= 324 => Some(319),
        _ => None,
    }
}

pub fn spawn_bridge(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>) {
    for &port in FORZA_PORTS {
        let state = state.clone();
        let stop = stop.clone();
        thread::spawn(move || receiver_loop(state, stop, port));
    }
    thread::spawn(move || watchdog_loop(state, stop));
}

/// One UDP receiver per candidate port. On a valid packet it stamps `t_last_rx` and
/// applies the data; it never clears the connection flags — that's the watchdog's job,
/// so a quiet port can't stomp a busy one.
fn receiver_loop(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>, port: u16) {
    loop {
        if stop.load(Ordering::Relaxed) { return; }
        let addr = format!("0.0.0.0:{port}");
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[forza] bind {addr} failed: {e}; retrying in 3s");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
        eprintln!("[forza] listening on {addr}");

        let mut buf = [0u8; 1024];
        let mut diag_first = true;
        loop {
            if stop.load(Ordering::Relaxed) { return; }
            match socket.recv_from(&mut buf) {
                Ok((len, from)) if len >= 232 => {
                    let race_on = i32_at(&buf, 0) != 0;
                    if diag_first {
                        eprintln!("[forza] first packet on :{port}: {len} bytes from {from}, IsRaceOn={race_on}");
                        diag_first = false;
                    }
                    if let Ok(mut s) = state.lock() {
                        // Any valid packet = a real, proven connection, even if paused.
                        s.t_connected = true;
                        s.t_last_rx = Some(Instant::now());
                        apply_packet(&mut s, &buf, len, race_on);
                    }
                }
                Ok((len, from)) => {
                    if diag_first {
                        eprintln!("[forza] runt packet on :{port}: {len} bytes from {from} (need >=232)");
                        diag_first = false;
                    }
                }
                // Timeout: do nothing here — the watchdog owns the "feed stopped"
                // decision so this idle port can't clear a flag another port just set.
                Err(_) => {}
            }
        }
    }
}

/// Single owner of the "feed stopped" decision. When no port has produced a packet
/// within STALE_AFTER, both the connection and live-race flags are cleared and the
/// engine falls back to the inferred model. `t_last_rx` lives inside AppState so
/// the staleness check and flag-clear happen under one lock — no TOCTOU window.
fn watchdog_loop(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::Relaxed) { return; }
        thread::sleep(Duration::from_millis(250));
        if let Ok(mut s) = state.lock() {
            let stale = s.t_last_rx
                .map(|t| t.elapsed() > STALE_AFTER)
                .unwrap_or(true);
            if stale && (s.t_connected || s.t_on) {
                s.t_connected = false;
                s.t_on = false;
            }
        }
    }
}

/// Parse the fields we use and write them into AppState.
fn apply_packet(s: &mut AppState, b: &[u8], len: usize, race_on: bool) {
    s.t_on = race_on;
    if !race_on {
        return;
    }

    let max_rpm  = f32_at(b, 8);
    let idle_rpm = f32_at(b, 12);
    let cur_rpm  = f32_at(b, 16);
    // Normalize revs to 0..1 between idle and redline.
    s.t_rpm = if max_rpm > idle_rpm {
        ((cur_rpm - idle_rpm) / (max_rpm - idle_rpm)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // AccelerationZ is longitudinal in the car's local frame: + = accelerating,
    // − = braking / engine braking. This is the real driveline load signal.
    s.t_accel = f32_at(b, 28);

    // Per-wheel tire slip ratio (FL,FR,RL,RR) — >~1 means the wheel is spinning or
    // locking relative to the road. Combined slip folds in cornering slip.
    let slip_fl = f32_at(b, 84).abs();
    let slip_fr = f32_at(b, 88).abs();
    let slip_rl = f32_at(b, 92).abs();
    let slip_rr = f32_at(b, 96).abs();
    s.t_slip_front = slip_fl.max(slip_fr);          // lockup / understeer scrub
    s.t_slip_rear  = slip_rl.max(slip_rr);          // wheelspin (drive wheels on RWD)

    // Surface rumble (road texture / gravel) and kerb contact, max across wheels.
    let surf = f32_at(b, 148).max(f32_at(b, 152)).max(f32_at(b, 156)).max(f32_at(b, 160));
    s.t_surface = surf.clamp(0.0, 1.0);
    let kerb = f32_at(b, 116).max(f32_at(b, 120)).max(f32_at(b, 124)).max(f32_at(b, 128));
    s.t_kerb = kerb;

    // Vertical heave (AccelerationY in the car frame) — low-pass filtered for load gating.
    let heave_raw = f32_at(b, 24);
    s.t_filt_heave = signal::low_pass(
        heave_raw,
        s.t_filt_heave,
        HEAVE_LPF_HZ,
        TELEM_SAMPLE_HZ,
    );
    s.t_heave = s.t_filt_heave;
    s.t_grip_mult = signal::grip_multiplier(s.t_heave);

    // Per-wheel normalized suspension travel — low-pass filtered before bump/droop logic
    // so sharp telemetry spikes (bottoming, crest compression) don't clack the actuators.
    let sfl_raw = f32_at(b, 68);
    let sfr_raw = f32_at(b, 72);
    let srl_raw = f32_at(b, 76);
    let srr_raw = f32_at(b, 80);
    let sfl = signal::low_pass(sfl_raw, s.t_filt_susp_fl, SUSP_LPF_HZ, TELEM_SAMPLE_HZ);
    let sfr = signal::low_pass(sfr_raw, s.t_filt_susp_fr, SUSP_LPF_HZ, TELEM_SAMPLE_HZ);
    let srl = signal::low_pass(srl_raw, s.t_filt_susp_rl, SUSP_LPF_HZ, TELEM_SAMPLE_HZ);
    let srr = signal::low_pass(srr_raw, s.t_filt_susp_rr, SUSP_LPF_HZ, TELEM_SAMPLE_HZ);
    s.t_filt_susp_fl = sfl;
    s.t_filt_susp_fr = sfr;
    s.t_filt_susp_rl = srl;
    s.t_filt_susp_rr = srr;

    // Per-wheel delta vs previous filtered travel → directional bump feel.
    let d = |now: f32, prev: f32| (now - prev).abs().min(0.30);
    s.t_bump_left  = d(sfl, s.t_susp_fl).max(d(srl, s.t_susp_rl));
    s.t_bump_right = d(sfr, s.t_susp_fr).max(d(srr, s.t_susp_rr));
    s.t_susp_fl = sfl;
    s.t_susp_fr = sfr;
    s.t_susp_rl = srl;
    s.t_susp_rr = srr;

    // Tire slip angle (FL,FR,RL,RR) — EWMA smoothed for continuous cornering feel.
    let ang_fl = f32_at(b, 132).abs();
    let ang_fr = f32_at(b, 136).abs();
    let ang_rl = f32_at(b, 140).abs();
    let ang_rr = f32_at(b, 144).abs();
    let ang_max = ang_fl.max(ang_fr).max(ang_rl).max(ang_rr);
    s.t_ewma_slip_angle = signal::ewma(ang_max, s.t_ewma_slip_angle, EWMA_ALPHA);
    s.t_slip_angle = s.t_ewma_slip_angle;

    // Combined slip — EWMA on the per-wheel max before it feeds haptics.
    let comb_raw = f32_at(b, 180).abs()
        .max(f32_at(b, 184).abs())
        .max(f32_at(b, 188).abs())
        .max(f32_at(b, 192).abs());
    s.t_ewma_combined = signal::ewma(comb_raw, s.t_ewma_combined, EWMA_ALPHA);
    s.t_slip_combined = s.t_ewma_combined;

    // Per-wheel surface rumble for stereophonic road texture (left/right voice coils).
    s.t_surface_fl = f32_at(b, 148).clamp(0.0, 1.0);
    s.t_surface_fr = f32_at(b, 152).clamp(0.0, 1.0);
    s.t_surface_rl = f32_at(b, 156).clamp(0.0, 1.0);
    s.t_surface_rr = f32_at(b, 160).clamp(0.0, 1.0);

    // Speed (Dash block, m/s). The whole Dash block shifts +12 bytes on Horizon titles
    // (CarGroup/Smashable* are inserted right after the Sled), so Speed sits at 256 on
    // FH4/5/6 (324+ byte packets) but 244 on Forza Motorsport's 311-byte Dash. Reading a
    // flat 244 on Horizon was actually grabbing PositionX, not speed.
    let speed_off = if len >= 324 { 256 } else { 244 };
    if len >= speed_off + 4 {
        s.t_speed = f32_at(b, speed_off);
    }

    // Gear (Dash block, offset by title) for exact shift detection.
    if let Some(off) = gear_offset(len) {
        if off < len {
            let g = b[off];
            // Diagnostic: log every gear change so we can confirm the offset is right.
            // If shifts feel dead, watch this — no lines here means t_gear isn't moving
            // (wrong packet length / offset), so shift detection can't fire.
            if g != s.t_gear {
                eprintln!("[forza] gear {} -> {} (packet {} bytes, gear offset {})",
                    s.t_gear, g, len, off);
            }
            s.t_gear = g;
        }
    } else {
        // Packet length isn't one we map a gear offset to — shifts won't work. Log once.
        static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("[forza] no gear offset for {len}-byte packets — shift feedback disabled");
        }
    }

    // Accel/Brake pedal inputs (Dash block) — the game's received input 0-255.
    // Stable offsets across FH4 / FH5 / FH6 324-byte packets.
    if len >= 317 {
        s.t_accel_input = b[315];
        s.t_brake_input = b[316];
    }

    // Traction-control proxy: high rear slip + throttle but low longitudinal G means
    // the ECU is cutting power (common on hybrids / TC-equipped cars).
    s.t_tc_active = s.t_slip_rear > 0.35
        && s.t_accel_input > 180
        && s.t_accel < 2.0;

    // Apply grip multiplier + TC attenuation to slip-driven telemetry scalars.
    let haptic_scale = s.t_grip_mult * if s.t_tc_active { TC_HAPTIC_SCALE } else { 1.0 };
    s.t_slip_front *= haptic_scale;
    s.t_slip_rear *= haptic_scale;
    s.t_slip_combined *= haptic_scale;
    s.t_slip_angle *= haptic_scale;
    s.t_surface *= haptic_scale;
    s.t_surface_fl *= haptic_scale;
    s.t_surface_fr *= haptic_scale;
    s.t_surface_rl *= haptic_scale;
    s.t_surface_rr *= haptic_scale;
    s.t_bump_left *= haptic_scale;
    s.t_bump_right *= haptic_scale;

    // Suspension droop gate — zero slip/surface feel when all wheels are unloaded.
    let min_susp = s.t_susp_fl.min(s.t_susp_fr).min(s.t_susp_rl).min(s.t_susp_rr);
    if min_susp < 0.05 {
        s.t_slip_front = 0.0;
        s.t_slip_rear = 0.0;
        s.t_slip_combined = 0.0;
        s.t_slip_angle = 0.0;
    }
}
