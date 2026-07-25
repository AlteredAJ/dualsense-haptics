// Xbox-translated output (Windows only).
//
// Creates a virtual Xbox 360 (XInput) gamepad via ViGEmBus and forwards the real
// DualSense's inputs into it every frame. XInput-only games (Forza Horizon, etc.) only
// detect Xbox controllers, so without this the DualSense is invisible to them. Our
// haptic output still drives the REAL DualSense, so adaptive triggers and rumble keep
// working while the game reads the virtual Xbox pad.
//
// Pair with HidHide (cloak the real DualSense, whitelist this app) so the game sees ONLY
// the virtual pad and doesn't get double input. See docs/WINDOWS.md.
//
// NOTE: vigem-client's `XButtons` constant names can vary by version. If a const name
// fails to compile, check the installed vigem-client docs and adjust the mapping below.

use std::sync::{Arc, Mutex};

use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};

use crate::hid::AppState;

pub struct XBridge {
    target: Xbox360Wired<Client>,
}

impl XBridge {
    /// Connect to ViGEmBus and plug in a virtual Xbox 360 pad. Errors if the ViGEmBus
    /// driver isn't installed.
    pub fn new() -> Result<Self, String> {
        let client = Client::connect().map_err(|e| format!("ViGEmBus connect failed: {e}"))?;
        let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
        target.plugin().map_err(|e| format!("virtual pad plugin failed: {e}"))?;
        target
            .wait_ready()
            .map_err(|e| format!("virtual pad not ready: {e}"))?;
        Ok(Self { target })
    }

    /// Subscribe to the rumble the game sends to the virtual pad and stream it into
    /// shared state, where process_frame re-expands it into DualSense haptics.
    /// The listener thread exits on its own when the virtual pad is unplugged
    /// (poll returns OperationAborted), so it needs no explicit teardown.
    pub fn start_feedback(&mut self, state: Arc<Mutex<AppState>>) {
        match self.target.request_notification() {
            Ok(req) => {
                let _ = req.spawn_thread(move |_, n| {
                    if let Ok(mut s) = state.lock() {
                        s.game_rumble_l = n.large_motor;
                        s.game_rumble_r = n.small_motor;
                    }
                });
            }
            Err(e) => eprintln!("[xbox] rumble feedback unavailable: {e}"),
        }
    }

    /// Map one frame of DualSense input onto the virtual Xbox pad and submit it.
    ///
    /// `buttons` packs the DualSense face+dpad byte in the low 8 bits and the shoulder
    /// byte in the high 8 bits (the same layout AppState stores). Sticks/triggers are the
    /// raw 0..255 DualSense values; `ps` is the PlayStation/Guide button.
    pub fn push(
        &mut self,
        lx: u8, ly: u8, rx: u8, ry: u8,
        l2: u8, r2: u8,
        buttons: u16,
        ps: bool,
    ) {
        let face = (buttons & 0xFF) as u8;
        let shoulders = (buttons >> 8) as u8;

        let mut b: u16 = 0;

        // D-pad hat: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW, 8=neutral.
        match face & 0x0F {
            0 => b |= XButtons::UP,
            1 => b |= XButtons::UP | XButtons::RIGHT,
            2 => b |= XButtons::RIGHT,
            3 => b |= XButtons::DOWN | XButtons::RIGHT,
            4 => b |= XButtons::DOWN,
            5 => b |= XButtons::DOWN | XButtons::LEFT,
            6 => b |= XButtons::LEFT,
            7 => b |= XButtons::UP | XButtons::LEFT,
            _ => {}
        }

        // Face buttons (DualSense → Xbox): Cross→A, Circle→B, Square→X, Triangle→Y.
        if face & 0x20 != 0 { b |= XButtons::A; }
        if face & 0x40 != 0 { b |= XButtons::B; }
        if face & 0x10 != 0 { b |= XButtons::X; }
        if face & 0x80 != 0 { b |= XButtons::Y; }

        // Shoulders / system: L1→LB, R1→RB, Create→Back, Options→Start, L3/R3→thumbs.
        if shoulders & 0x01 != 0 { b |= XButtons::LB; }
        if shoulders & 0x02 != 0 { b |= XButtons::RB; }
        if shoulders & 0x10 != 0 { b |= XButtons::BACK; }
        if shoulders & 0x20 != 0 { b |= XButtons::START; }
        if shoulders & 0x40 != 0 { b |= XButtons::LTHUMB; }
        if shoulders & 0x80 != 0 { b |= XButtons::RTHUMB; }
        if ps { b |= XButtons::GUIDE; }

        // DualSense sticks are 0..255 with 128 center; XInput wants i16 -32768..32767.
        // XInput Y is positive-up, DualSense Y is positive-down, so Y is negated.
        let axis = |v: u8| -> i16 { (((v as i32) - 128) * 257).clamp(-32768, 32767) as i16 };

        let gamepad = XGamepad {
            buttons: XButtons(b),
            left_trigger: l2,
            right_trigger: r2,
            thumb_lx: axis(lx),
            thumb_ly: axis(ly).saturating_neg(),
            thumb_rx: axis(rx),
            thumb_ry: axis(ry).saturating_neg(),
        };

        let _ = self.target.update(&gamepad);
    }
}
