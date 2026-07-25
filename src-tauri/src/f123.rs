// ─── F1 23 telemetry bridge ─────────────────────────────────────────────────
//
// F1 23 broadcasts telemetry via UDP on port 20777 using a multi-packet format.
// Each datagram starts with a 29-byte PacketHeader; the m_packetId field routes
// the payload: ID 6 = CarTelemetryData (speed, throttle, brake, gear, RPM,
// surface), ID 13 = MotionExData (per-wheel suspension, slip ratio, slip angle).
// The two packet types arrive interleaved (~60 Hz), so we cache the last-known
// state of each and apply a combined update when both are fresh.
//
// Enable in-game: Settings → Telemetry → UDP → On, IP 127.0.0.1, Port 20777.
//
// Wheel order: RL=0, RR=1, FL=2, FR=3 — transposed to Forza's FL-FR-RL-RR
// convention before writing AppState.

use crate::hid::AppState;
use crate::signal::{
    self, EWMA_ALPHA, HEAVE_LPF_HZ, SUSP_LPF_HZ, TC_HAPTIC_SCALE,
};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const F123_PORT: u16 = 20777;
const F123_SAMPLE_HZ: f32 = 120.0; // user-configurable in F1 settings – match it

const STALE_AFTER: Duration = Duration::from_millis(1500);
const PACKET_FORMAT_F123: u16 = 2023;
const PACKET_ID_CAR_TELEMETRY: u8 = 6;
const PACKET_ID_MOTION_EX: u8 = 13;

#[inline]
fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
#[inline]
fn f32_at(b: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// Transpose F1 23 wheel order (RL=0,RR=1,FL=2,FR=3) to Forza (FL,FR,RL,RR).
#[inline]
fn wheel_idx(f123_idx: usize) -> usize {
    [2, 3, 0, 1][f123_idx]
}

pub fn spawn(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>) {
    let state_rx = state.clone();
    let stop_rx = stop.clone();
    thread::spawn(move || receiver_loop(state_rx, stop_rx));
    thread::spawn(move || watchdog_loop(state, stop));
}

fn receiver_loop(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>) {
    loop {
        if stop.load(Ordering::Relaxed) { return; }
        let addr = format!("0.0.0.0:{F123_PORT}");
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[f123] bind {addr} failed: {e}; retrying in 3s");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
        eprintln!("[f123] listening on {addr}");

        // Cache last-known CarTelemetryData and MotionExData — they arrive
        // interleaved, so we apply a combined update when both are available.
        let mut car_telemetry_buf: Option<Vec<u8>> = None;
        let mut motion_ex_buf: Option<Vec<u8>> = None;
        let mut diag_first = true;

        let mut buf = [0u8; 2048];
        loop {
            if stop.load(Ordering::Relaxed) { return; }
            match socket.recv_from(&mut buf) {
                Ok((len, from)) if len >= 29 => {
                    let fmt = u16_at(&buf, 0);
                    if fmt != PACKET_FORMAT_F123 {
                        if diag_first {
                            eprintln!("[f123] unexpected packet format {fmt} from {from} (expected {PACKET_FORMAT_F123})");
                            diag_first = false;
                        }
                        continue;
                    }
                    let pkt_id = buf[6];
                    if diag_first {
                        eprintln!("[f123] first packet (id {pkt_id}, {len} bytes) from {from}");
                        diag_first = false;
                    }

                    if let Ok(mut s) = state.lock() {
                        s.t_connected = true;
                        s.t_last_rx = Some(Instant::now());

                        match pkt_id {
                            PACKET_ID_CAR_TELEMETRY => {
                                car_telemetry_buf = Some(buf[..len].to_vec());
                            }
                            PACKET_ID_MOTION_EX => {
                                motion_ex_buf = Some(buf[..len].to_vec());
                            }
                            _ => {} // ignore other packet types
                        }

                        // Apply combined update when both caches are populated
                        if let (Some(ref car), Some(ref mot)) =
                            (&car_telemetry_buf, &motion_ex_buf)
                        {
                            apply_packet(&mut s, car, mot);
                        }
                    }
                }
                Ok((len, from)) => {
                    if diag_first {
                        eprintln!("[f123] runt packet ({len} bytes) from {from}");
                        diag_first = false;
                    }
                }
                Err(_) => {} // timeout
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

fn apply_packet(s: &mut AppState, car: &[u8], mot: &[u8]) {
    // ── CarTelemetryData — extract player car ──────────────────────────────
    let player_idx = car[27] as usize;
    let car_offset = 29 + player_idx.min(21) * 60;

    // Speed: uint16 km/h → m/s
    if car.len() >= car_offset + 2 {
        s.t_speed = u16_at(car, car_offset) as f32 / 3.6;
    }
    // Throttle: f32 0..1 → 0..255
    if car.len() >= car_offset + 6 {
        s.t_accel_input = (f32_at(car, car_offset + 2).clamp(0.0, 1.0) * 255.0) as u8;
    }
    // Brake: f32 0..1 → 0..255
    if car.len() >= car_offset + 14 {
        s.t_brake_input = (f32_at(car, car_offset + 10).clamp(0.0, 1.0) * 255.0) as u8;
    }
    // Gear: int8 — -1=reverse, 0=neutral, 1-8=forward
    if car.len() >= car_offset + 16 {
        let g = car[car_offset + 15] as i8;
        s.t_gear = if g > 0 { g as u8 } else { 0 };
    }
    // Engine RPM: uint16 — track dynamic max for accurate normalization
    if car.len() >= car_offset + 18 {
        let rpm = u16_at(car, car_offset + 16) as f32;
        if rpm > s.t_f123_max_rpm * 0.9 {
            s.t_f123_max_rpm = s.t_f123_max_rpm.max(rpm).min(18000.0);
        }
        s.t_rpm = (rpm / s.t_f123_max_rpm).clamp(0.0, 1.0);
    }
    // Surface type: uint8[4] — map enum 0-11 to 0..1
    if car.len() >= car_offset + 60 {
        let surf_map = |raw: u8| -> f32 {
            match raw {
                0 => 0.0,   // Tarmac
                1 => 0.7,   // Rumble strip
                2 => 0.1,   // Concrete
                3 => 0.3,   // Rock
                4 => 0.5,   // Gravel
                5 => 0.4,   // Mud
                6 => 0.4,   // Sand
                7 => 0.6,   // Grass
                _ => 0.2,
            }
        };
        let sf = [surf_map(car[car_offset + 56]),
                  surf_map(car[car_offset + 57]),
                  surf_map(car[car_offset + 58]),
                  surf_map(car[car_offset + 59])];
        // Transpose RL,RR,FL,FR → FL,FR,RL,RR
        s.t_surface_fl = sf[2];
        s.t_surface_fr = sf[3];
        s.t_surface_rl = sf[0];
        s.t_surface_rr = sf[1];
        s.t_surface = sf.iter().copied().fold(0.0f32, f32::max);
    }

    // ── MotionExData — suspension + slip ───────────────────────────────────
    // Suspension positions: f32[4] at global offset 29, RL-RR-FL-FR
    if mot.len() >= 45 {
        let raw_susp = [f32_at(mot, 29), f32_at(mot, 33), f32_at(mot, 37), f32_at(mot, 41)];
        s.t_filt_susp_rl = signal::low_pass(raw_susp[0], s.t_filt_susp_rl, SUSP_LPF_HZ, F123_SAMPLE_HZ);
        s.t_filt_susp_rr = signal::low_pass(raw_susp[1], s.t_filt_susp_rr, SUSP_LPF_HZ, F123_SAMPLE_HZ);
        s.t_filt_susp_fl = signal::low_pass(raw_susp[2], s.t_filt_susp_fl, SUSP_LPF_HZ, F123_SAMPLE_HZ);
        s.t_filt_susp_fr = signal::low_pass(raw_susp[3], s.t_filt_susp_fr, SUSP_LPF_HZ, F123_SAMPLE_HZ);
        s.t_susp_rl = s.t_filt_susp_rl;
        s.t_susp_rr = s.t_filt_susp_rr;
        s.t_susp_fl = s.t_filt_susp_fl;
        s.t_susp_fr = s.t_filt_susp_fr;

        let d = |now: f32, prev: f32| (now - prev).abs().min(0.30);
        let sfl = s.t_susp_fl; let sfr = s.t_susp_fr;
        let srl = s.t_susp_rl; let srr = s.t_susp_rr;
        s.t_bump_left  = d(sfl, s.t_susp_fl).max(d(srl, s.t_susp_rl));
        s.t_bump_right = d(sfr, s.t_susp_fr).max(d(srr, s.t_susp_rr));
    }

    // Slip ratios: f32[4] at global offset 93, RL-RR-FL-FR
    if mot.len() >= 109 {
        let slip = [f32_at(mot, 93), f32_at(mot, 97), f32_at(mot, 101), f32_at(mot, 105)];
        // Transpose to Forza wheel order
        let slip_fl = slip[2].abs();
        let slip_fr = slip[3].abs();
        let slip_rl = slip[0].abs();
        let slip_rr = slip[1].abs();
        s.t_slip_front = slip_fl.max(slip_fr);
        s.t_slip_rear  = slip_rl.max(slip_rr);
    }

    // Slip angles: f32[4] at global offset 109, RL-RR-FL-FR — EWMA smoothed
    if mot.len() >= 125 {
        let angle = [f32_at(mot, 109).abs(), f32_at(mot, 113).abs(),
                     f32_at(mot, 117).abs(), f32_at(mot, 121).abs()];
        let ang_max = angle.iter().copied().fold(0.0f32, f32::max);
        s.t_ewma_slip_angle = signal::ewma(ang_max, s.t_ewma_slip_angle, EWMA_ALPHA);
        s.t_slip_angle = s.t_ewma_slip_angle;
    }

    // Combined slip from both ratio and angle (F1 doesn't provide a pre-combined field)
    s.t_ewma_combined = signal::ewma(
        s.t_slip_front.max(s.t_slip_rear).max(s.t_slip_angle),
        s.t_ewma_combined,
        EWMA_ALPHA,
    );
    s.t_slip_combined = s.t_ewma_combined;

    // Heave: m_localVelocityY at global offset 165 — longitudinal accel
    if mot.len() >= 173 {
        let vel_y = f32_at(mot, 165); // vertical velocity (heave proxy)
        s.t_filt_heave = signal::low_pass(vel_y, s.t_filt_heave, HEAVE_LPF_HZ, F123_SAMPLE_HZ);
        s.t_heave = s.t_filt_heave;
        s.t_grip_mult = signal::grip_multiplier(s.t_heave);
    }

    // Longitudinal accel: use m_localVelocityZ delta? For F1 we approximate from
    // wheel forces. Use rear longitudinal force as acceleration proxy.
    if mot.len() >= 156 {
        // Average rear longitudinal force → acceleration proxy (Newtons)
        let rear_force = (f32_at(mot, 141) + f32_at(mot, 145)) / 2.0; // RL + RR long force
        s.t_accel = rear_force / 800.0; // approximate G from force (car mass ~800kg)
    }

    // Kerb: F1 doesn't have a kerb field — synthesize from surface + suspension
    let max_surf = s.t_surface_fl.max(s.t_surface_fr).max(s.t_surface_rl).max(s.t_surface_rr);
    s.t_kerb = if max_surf > 0.5 { max_surf } else { 0.0 };

    // ── TC / scaling (same pattern as Forza) ──────────────────────────────
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

    // F1 23 packets are always "race on" when transmitting telemetry
    s.t_on = true;
}
