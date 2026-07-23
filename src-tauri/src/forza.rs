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
use std::io::Write;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
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

pub fn spawn_bridge(state: Arc<Mutex<AppState>>) {
    // Shared "last packet received" clock, written by whichever port is live and read
    // by the watchdog. This is the single source of truth for the connection state, so
    // one idle port timing out can't fight another port that's actively receiving.
    let last_rx: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

    for &port in FORZA_PORTS {
        let state = state.clone();
        let last_rx = last_rx.clone();
        thread::spawn(move || receiver_loop(state, last_rx, port));
    }
    thread::spawn(move || watchdog_loop(state, last_rx));
}

/// One UDP receiver per candidate port. On a valid packet it stamps `last_rx` and
/// applies the data; it never clears the connection flags — that's the watchdog's job,
/// so a quiet port can't stomp a busy one.
fn receiver_loop(state: Arc<Mutex<AppState>>, last_rx: Arc<Mutex<Option<Instant>>>, port: u16) {
    loop {
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
            match socket.recv_from(&mut buf) {
                Ok((len, from)) if len >= 232 => {
                    let race_on = i32_at(&buf, 0) != 0;
                    if diag_first {
                        eprintln!("[forza] first packet on :{port}: {len} bytes from {from}, IsRaceOn={race_on}");
                        diag_first = false;
                    }
                    if let Ok(mut lr) = last_rx.lock() { *lr = Some(Instant::now()); }
                    if let Ok(mut s) = state.lock() {
                        // Any valid packet = a real, proven connection, even if paused.
                        s.t_connected = true;
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
/// engine falls back to the inferred model.
fn watchdog_loop(state: Arc<Mutex<AppState>>, last_rx: Arc<Mutex<Option<Instant>>>) {
    loop {
        thread::sleep(Duration::from_millis(250));
        let stale = match last_rx.lock() {
            Ok(lr) => lr.map(|t| t.elapsed() > STALE_AFTER).unwrap_or(true),
            Err(_) => continue,
        };
        if stale {
            if let Ok(mut s) = state.lock() {
                if s.t_connected || s.t_on {
                    s.t_connected = false;
                    s.t_on = false;
                }
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
    let comb = f32_at(b, 180).abs()
        .max(f32_at(b, 184).abs())
        .max(f32_at(b, 188).abs())
        .max(f32_at(b, 192).abs());
    s.t_slip_combined = comb;                        // overall grip loss / cornering scrub

    // Surface rumble (road texture / gravel) and kerb contact, max across wheels.
    let surf = f32_at(b, 148).max(f32_at(b, 152)).max(f32_at(b, 156)).max(f32_at(b, 160));
    s.t_surface = surf.clamp(0.0, 1.0);
    let kerb = f32_at(b, 116).max(f32_at(b, 120)).max(f32_at(b, 124)).max(f32_at(b, 128));
    s.t_kerb = kerb;

    // Per-wheel normalized suspension travel (FL,FR,RL,RR) → directional bump feel.
    // A bump is a rapid CHANGE in travel (the wheel jouncing over something), so we take
    // the per-wheel delta vs the previous packet and fold it into a per-SIDE intensity:
    // left = max(FL,RL), right = max(FR,RR). Smooth tarmac = tiny deltas; bumps, crests,
    // kerbs and dips = spikes. Clamp the delta so a one-off jump (e.g. on telemetry
    // start) can't fire a giant jolt.
    let sfl = f32_at(b, 68);
    let sfr = f32_at(b, 72);
    let srl = f32_at(b, 76);
    let srr = f32_at(b, 80);
    let d = |now: f32, prev: f32| (now - prev).abs().min(0.30);
    s.t_bump_left  = d(sfl, s.t_susp_fl).max(d(srl, s.t_susp_rl));
    s.t_bump_right = d(sfr, s.t_susp_fr).max(d(srr, s.t_susp_rr));
    s.t_susp_fl = sfl; s.t_susp_fr = sfr; s.t_susp_rl = srl; s.t_susp_rr = srr;

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
}
