// ─── Assetto Corsa telemetry bridge ─────────────────────────────────────────
//
// Assetto Corsa (original, not Competizione) broadcasts telemetry via UDP on
// port 9996 using a request-response handshake model. The client must send a
// handshake datagram, then subscribe to continuous updates. The server replies
// with flat RTCarInfo structs (328 bytes, padding at offset 23-27 after the
// 6-byte bool block) at ~333 Hz.
//
// Wheel order: FL=0, FR=1, RL=2, RR=3 — identical to Forza, no transposition.
// Surface texture is inferred from the tyreDirtyLevel array (0=clean tarmac,
// escalates on grass/gravel/dirt).
//
// Enable in-game: no settings required — the server automatically binds port
// 9996 on session start. The client initiates the stream.

use crate::hid::AppState;
use crate::signal::{
    self, EWMA_ALPHA, HEAVE_LPF_HZ, SUSP_LPF_HZ, TC_HAPTIC_SCALE,
};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const AC_PORT: u16 = 9996;
const AC_SAMPLE_HZ: f32 = 333.0; // AC broadcasts at physics tick rate
const STALE_AFTER: Duration = Duration::from_millis(1500);

// Handshake constants — client sends these to initiate the stream.
const HANDSHAKE_1: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 0];
const SUBSCRIBE_UPDATE: &[u8] = &[1, 0, 0, 0, 0, 0, 0, 0];

#[inline]
fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

pub fn spawn(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>) {
    let state_rx = state.clone();
    let stop_rx = stop.clone();
    thread::spawn(move || receiver_loop(state_rx, stop_rx));
    thread::spawn(move || watchdog_loop(state, stop));
}

fn try_handshake(socket: &UdpSocket) -> bool {
    let addr = format!("127.0.0.1:{AC_PORT}");
    // AC uses two-phase handshake: initial connection then subscribe.
    for (step, payload) in [HANDSHAKE_1, SUBSCRIBE_UPDATE].iter().enumerate() {
        if socket.send_to(payload, &addr).is_err() {
            eprintln!("[ac] handshake step {step} send failed");
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
    true
}

fn receiver_loop(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::Relaxed) { return; }
        let addr = format!("0.0.0.0:{AC_PORT}");
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ac] bind {addr} failed: {e}; retrying in 3s");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
        if !try_handshake(&socket) {
            eprintln!("[ac] handshake failed; retrying in 3s");
            thread::sleep(Duration::from_secs(3));
            continue;
        }
        eprintln!("[ac] listening on {addr}");

        let mut diag_first = true;
        let mut buf = [0u8; 1024];
        // Re-handshake every 5s in case the server dropped us.
        let mut last_handshake = Instant::now();

        loop {
            if stop.load(Ordering::Relaxed) { return; }
            match socket.recv_from(&mut buf) {
                Ok((len, from)) if len >= 328 => {
                    if diag_first {
                        let gear = if len >= 77 { buf[76] as i32 } else { -99 };
                        eprintln!("[ac] first RTCarInfo packet ({len} bytes, gear={gear}) from {from}");
                        diag_first = false;
                    }
                    if let Ok(mut s) = state.lock() {
                        s.t_connected = true;
                        s.t_last_rx = Some(Instant::now());
                        apply_packet(&mut s, &buf, len);
                    }
                    last_handshake = Instant::now();
                }
                Ok((len, _)) => {
                    if diag_first {
                        eprintln!("[ac] runt packet ({len} bytes)");
                        diag_first = false;
                    }
                }
                Err(_) => {}
            }
            // Periodic re-handshake to keep the stream alive
            if last_handshake.elapsed() > Duration::from_secs(5) {
                try_handshake(&socket);
                last_handshake = Instant::now();
            }
        }
    }
}

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

fn apply_packet(s: &mut AppState, b: &[u8], _len: usize) {
    // ── RTCarInfo flat struct offsets ────────────────────────────────────

    // Speed: float32 m/s at offset 13 (pre-computed by the engine)
    s.t_speed = f32_at(b, 13);

    // Accel/Brake inputs: f32 0..1 → 0..255
    s.t_accel_input = (f32_at(b, 56).clamp(0.0, 1.0) * 255.0) as u8; // gas at 56
    s.t_brake_input = (f32_at(b, 60).clamp(0.0, 1.0) * 255.0) as u8; // brake at 60

    // Engine RPM: float32 → normalize to 0..1 (assume redline ~7500 for AC cars)
    s.t_rpm = (f32_at(b, 68) / 7500.0).clamp(0.0, 1.0);

    // Gear: int32 at offset 76 — 0=neutral, negative=reverse
    let g = i32::from_le_bytes([b[76], b[77], b[78], b[79]]);
    s.t_gear = if g > 0 { g as u8 } else { 0 };

    // Per-wheel slip angle: f32[4] at offset 100 — FL, FR, RL, RR
    let ang_fl = f32_at(b, 100).abs();
    let ang_fr = f32_at(b, 104).abs();
    let ang_rl = f32_at(b, 108).abs();
    let ang_rr = f32_at(b, 112).abs();
    let ang_max = ang_fl.max(ang_fr).max(ang_rl).max(ang_rr);
    s.t_ewma_slip_angle = signal::ewma(ang_max, s.t_ewma_slip_angle, EWMA_ALPHA);
    s.t_slip_angle = s.t_ewma_slip_angle;

    // Per-wheel slip ratio: f32[4] at offset 132 — FL, FR, RL, RR
    let slip_fl = f32_at(b, 132).abs();
    let slip_fr = f32_at(b, 136).abs();
    let slip_rl = f32_at(b, 140).abs();
    let slip_rr = f32_at(b, 144).abs();
    s.t_slip_front = slip_fl.max(slip_fr);
    s.t_slip_rear  = slip_rl.max(slip_rr);

    // Combined slip — EWMA smoothed
    let comb_raw = s.t_slip_front.max(s.t_slip_rear).max(s.t_slip_angle);
    s.t_ewma_combined = signal::ewma(comb_raw, s.t_ewma_combined, EWMA_ALPHA);
    s.t_slip_combined = s.t_ewma_combined;

    // Surface texture proxy: tyreDirtyLevel[4] at offset 228 — escalate on dirt
    let dirty = [f32_at(b, 228), f32_at(b, 232), f32_at(b, 236), f32_at(b, 240)];
    let dirty_max = dirty.iter().copied().fold(0.0f32, f32::max);
    // Map dirty level 0..~100 to surface 0..1
    s.t_surface = (dirty_max / 100.0).clamp(0.0, 1.0);
    s.t_surface_fl = (dirty[0] / 100.0).clamp(0.0, 1.0);
    s.t_surface_fr = (dirty[1] / 100.0).clamp(0.0, 1.0);
    s.t_surface_rl = (dirty[2] / 100.0).clamp(0.0, 1.0);
    s.t_surface_rr = (dirty[3] / 100.0).clamp(0.0, 1.0);

    // Suspension height: f32[4] at offset 292 — FL, FR, RL, RR
    // These are absolute displacement values, not normalized.
    // Use relative deltas for bump detection.
    let raw_susp = [f32_at(b, 292), f32_at(b, 296), f32_at(b, 300), f32_at(b, 304)];
    let sfl = signal::low_pass(raw_susp[0], s.t_filt_susp_fl, SUSP_LPF_HZ, AC_SAMPLE_HZ);
    let sfr = signal::low_pass(raw_susp[1], s.t_filt_susp_fr, SUSP_LPF_HZ, AC_SAMPLE_HZ);
    let srl = signal::low_pass(raw_susp[2], s.t_filt_susp_rl, SUSP_LPF_HZ, AC_SAMPLE_HZ);
    let srr = signal::low_pass(raw_susp[3], s.t_filt_susp_rr, SUSP_LPF_HZ, AC_SAMPLE_HZ);
    s.t_filt_susp_fl = sfl;
    s.t_filt_susp_fr = sfr;
    s.t_filt_susp_rl = srl;
    s.t_filt_susp_rr = srr;
    s.t_susp_fl = sfl;
    s.t_susp_fr = sfr;
    s.t_susp_rl = srl;
    s.t_susp_rr = srr;

    let d = |now: f32, prev: f32| (now - prev).abs().min(0.30);
    s.t_bump_left  = d(sfl, s.t_susp_fl).max(d(srl, s.t_susp_rl));
    s.t_bump_right = d(sfr, s.t_susp_fr).max(d(srr, s.t_susp_rr));

    // Heave: accG_vertical at offset 28 (G-forces)
    let heave_raw = f32_at(b, 28);
    s.t_filt_heave = signal::low_pass(heave_raw, s.t_filt_heave, HEAVE_LPF_HZ, AC_SAMPLE_HZ);
    s.t_heave = s.t_filt_heave;
    s.t_grip_mult = signal::grip_multiplier(s.t_heave);

    // Longitudinal acceleration: accG_frontal at offset 36
    s.t_accel = f32_at(b, 36);

    // Kerb detection: combine high-frequency heave with dirty-level spikes.
    s.t_kerb = if s.t_heave.abs() > 1.5 { s.t_heave.abs() / 3.0 } else { 0.0 };

    // ── TC / scaling ─────────────────────────────────────────────────────
    s.t_tc_active = s.t_slip_rear > 0.35
        && s.t_accel_input > 180
        && s.t_accel < 2.0;

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

    let min_susp = s.t_susp_fl.min(s.t_susp_fr).min(s.t_susp_rl).min(s.t_susp_rr);
    if min_susp < 0.05 {
        s.t_slip_front = 0.0;
        s.t_slip_rear = 0.0;
        s.t_slip_combined = 0.0;
        s.t_slip_angle = 0.0;
    }

    // AC is always "race on" when streaming telemetry
    s.t_on = true;
}
