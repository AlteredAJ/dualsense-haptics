use serde::Serialize;
use std::ffi::CString;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// ─── DualSense USB IDs ────────────────────────────────────────────────────────

const SONY_VENDOR:       u16 = 0x054C;
const DUALSENSE_PRODUCT: u16 = 0x0CE6;

// ─── Transport ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Usb,
    Bluetooth,
}

// ─── Edition ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edition {
    Free,
    Full,
}

// ─── Profile ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Static,
}

impl Profile {
    pub fn from_str(_s: &str) -> Self { Self::Static }
    pub fn as_str(self) -> &'static str { "static" }
    pub fn lightbar(self) -> [u8; 3] { [255, 0, 255] }
}

// ─── Output mode ────────────────────────────────────────────────────────────

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

// ─── App state ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub profile:       Profile,
    pub output_mode:   OutputMode,
    pub edition:       Edition,
    // Live values
    pub l2_raw:   u8,
    pub r2_raw:   u8,
    pub l2_force: u8,
    pub r2_force: u8,
    pub lx: u8, pub ly: u8, pub rx: u8, pub ry: u8,
    pub buttons: u16,
    pub connected: bool,
    pub error_msg: String,
    // Shift/button edges for Xbox passthrough
    pub touchpad_btn: bool,
    pub shift_left_pulse:  u8,
    pub shift_right_pulse: u8,
    pub r2_blip_frames:    u8,
    pub prev_downshift: bool,
    pub prev_upshift:   bool,
    // Xbox passthrough
    pub game_rumble_l: u8,
    pub game_rumble_r: u8,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            profile:   Profile::Static,
            #[cfg(windows)]
            output_mode: OutputMode::Xbox,
            #[cfg(not(windows))]
            output_mode: OutputMode::Dualsense,
            edition:   Edition::Free,
            l2_raw: 0, r2_raw: 0, l2_force: 0, r2_force: 0,
            lx: 128, ly: 128, rx: 128, ry: 128, buttons: 0,
            connected: false, error_msg: String::new(),
            touchpad_btn: false,
            shift_left_pulse: 0, shift_right_pulse: 0, r2_blip_frames: 0,
            prev_downshift: false, prev_upshift: false,
            game_rumble_l: 0, game_rumble_r: 0,
        }
    }
}

// ─── Strength ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Strength {
    pub label:          &'static str,
    pub brake_start:    u8,
    pub brake_end:      u8,
    pub throttle_start: u8,
    pub throttle_end:   u8,
}

pub const STRENGTHS: [Strength; 1] = [
    Strength { label: "Light", brake_start: 120, brake_end: 215,
               throttle_start: 30, throttle_end: 108 },
];

// ─── UI snapshot ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct StateSnapshot {
    pub profile:     String,
    pub output_mode: String,
    pub connected:   bool,
    pub error_msg:   String,
    pub l2_raw:      u8,
    pub r2_raw:      u8,
    pub l2_force:    u8,
    pub r2_force:    u8,
    pub lx: u8, pub ly: u8, pub rx: u8, pub ry: u8,
    pub buttons: u16,
}

impl AppState {
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            profile:     self.profile.as_str().to_string(),
            output_mode: self.output_mode.as_str().to_string(),
            connected:   self.connected,
            error_msg:   self.error_msg.clone(),
            l2_raw:  self.l2_raw,  r2_raw:  self.r2_raw,
            l2_force: self.l2_force, r2_force: self.r2_force,
            lx: self.lx, ly: self.ly, rx: self.rx, ry: self.ry,
            buttons: self.buttons,
        }
    }
}

// ─── HID output report builders ──────────────────────────────────────────────

fn haptics_report(lm: u8, lp0: u8, lp1: u8, rm: u8, rp0: u8, rp1: u8) -> [u8; 48] {
    let mut b = [0u8; 48];
    b[0] = 0x02; b[1] = 0x0C;
    b[22] = lm; b[23] = lp0; b[24] = lp1; b[25] = 0;
    b[11] = rm; b[12] = rp0; b[13] = rp1; b[14] = 0;
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

const BT_REPORT_LEN: usize = 78;
const BT_CRC_SEED:   u8    = 0xA2;

fn to_bt_report(usb: &[u8; 48]) -> [u8; BT_REPORT_LEN] {
    use std::sync::atomic::{AtomicU8, Ordering};
    static BT_SEQ: AtomicU8 = AtomicU8::new(0);
    let seq = BT_SEQ.fetch_add(1, Ordering::Relaxed) & 0x0F;

    let mut b = [0u8; BT_REPORT_LEN];
    b[0] = 0x31;
    b[1] = seq << 4;
    b[2] = 0x10;
    b[3..50].copy_from_slice(&usb[1..48]);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&[BT_CRC_SEED]);
    hasher.update(&b[0..BT_REPORT_LEN - 4]);
    let crc = hasher.finalize();
    b[BT_REPORT_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    b
}

fn write_report(device: &hidapi::HidDevice, transport: Transport, usb: &[u8; 48])
    -> Result<usize, hidapi::HidError>
{
    match transport {
        Transport::Usb       => device.write(usb),
        Transport::Bluetooth => device.write(&to_bt_report(usb)),
    }
}

// ─── Frame processor ──────────────────────────────────────────────────────────

fn process_frame(s: &AppState) -> [u8; 48] {
    // Static profile — fixed resistance when both triggers held
    let st = &STRENGTHS[0];
    let lf = if s.l2_raw > 12 { st.brake_end } else { 0 };
    let rf = if s.r2_raw > 12 { st.throttle_end } else { 0 };
    haptics_report(
        if lf > 0 { 0x01 } else { 0x05 }, 0, lf,
        if rf > 0 { 0x01 } else { 0x05 }, 0, rf,
    )
}

// ─── Input thread ─────────────────────────────────────────────────────────────

fn parse_input_report(raw: &[u8], len: usize, state: &Arc<Mutex<AppState>>) {
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
        s.l2_raw = buf[5];
        s.r2_raw = buf[6];
        let face      = buf[8];
        let shoulders = buf[9];
        s.buttons     = (face as u16) | ((shoulders as u16) << 8);
        if len > 10 { s.touchpad_btn = (buf[10] & 0x02) != 0; }
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
        let mut buf = [0u8; 96];
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
            s.connected = false;
            s.error_msg = "DualSense disconnected".to_string();
            s.l2_raw = 0; s.r2_raw = 0;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(windows)]
fn forward_to_xbox(state: &Arc<Mutex<AppState>>, xbridge: &mut Option<crate::xinput::XBridge>) {
    let snap = {
        let mut s = match state.lock() { Ok(s) => s, Err(_) => return };
        if s.output_mode != OutputMode::Xbox {
            if xbridge.is_some() {
                let _ = crate::hidhide::disable();
            }
            *xbridge = None;
            return;
        }
        (s.lx, s.ly, s.rx, s.ry, s.l2_raw, s.r2_raw, s.buttons, s.touchpad_btn)
    };
    if xbridge.is_none() {
        match crate::xinput::XBridge::new() {
            Ok(mut b) => {
                b.start_feedback(state.clone());
                *xbridge = Some(b);
                if let Err(e) = crate::hidhide::enable() {
                    if let Ok(mut s) = state.lock() {
                        s.error_msg = format!("HidHide cloak failed — {e}");
                    }
                }
            }
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.error_msg = format!("Xbox output unavailable — {e}");
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

// ─── Output thread ───────────────────────────────────────────────────────────

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

fn open_dualsense(api: &hidapi::HidApi, path: &CString) -> Result<hidapi::HidDevice, hidapi::HidError> {
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);
    api.open_path(path)
}

fn hid_loop(state: Arc<Mutex<AppState>>, app: AppHandle) {
    loop {
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
        {
            if let Ok(mut s) = state.lock() {
                s.connected = true;
                s.error_msg = String::new();
                let lb = s.profile.lightbar();
                let _ = write_report(&device, transport, &lightbar_report(lb[0], lb[1], lb[2]));
                let _ = write_report(&device, transport, &player_led_report(0x04));
            }
        }
        let mut last_frame = Instant::now();
        let mut last_emit  = Instant::now();
        // Delta cache — only transmit when the report changes
        let mut last_report: Option<[u8; 48]> = None;
        loop {
            let now = Instant::now();
            if now.duration_since(last_frame) >= Duration::from_millis(16) {
                last_frame = now;
                let report = match state.lock() {
                    Err(_) => break,
                    Ok(s) => process_frame(&s),
                };
                let skip = transport == Transport::Usb
                    && last_report.map_or(false, |prev| prev == report);
                if !skip {
                    if write_report(&device, transport, &report).is_err() { break; }
                    last_report = Some(report);
                }
            }
            if now.duration_since(last_emit) >= Duration::from_millis(33) {
                last_emit = now;
                if let Ok(s) = state.lock() {
                    let _ = app.emit("state-update", s.snapshot());
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        thread::sleep(Duration::from_millis(500));
    }
}

pub fn spawn_hid_thread(state: Arc<Mutex<AppState>>, app: AppHandle) {
    let input_state = state.clone();
    let emit_state  = state.clone();
    let emit_app    = app.clone();
    thread::spawn(move || input_loop(input_state));
    thread::spawn(move || hid_loop(state, app));
    thread::spawn(move || loop {
        if let Ok(s) = emit_state.lock() {
            let _ = emit_app.emit("state-update", s.snapshot());
        }
        thread::sleep(Duration::from_millis(33));
    });
}
