// ─── Minecraft bridge ──────────────────────────────────────────────────────
//
// A tiny localhost TCP server. The Fabric mod connects as a client and pushes
// newline-delimited JSON describing the current game state (Phase 1: just the
// held-item category). The app maps that into AppState; the HID thread recolors
// the lightbar to match. App = server, mod = client, so the mod can reconnect
// freely whenever a world is (re)loaded.
//
// Wire format (one JSON object per line):
//   {"item":"sword"}
//   {"item":"pickaxe"}
//   {"item":"empty"}
//
// Future fields (Phase 2): action events (mine, attack, bow_draw, eat, hurt).

use crate::hid::{AppState, McItem, MC_ATTACK_FRAMES, MC_HURT_FRAMES};
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Bound to loopback only — never exposed off the machine.
pub const MC_BRIDGE_ADDR: &str = "127.0.0.1:27812";

#[derive(Debug, Default, Deserialize)]
struct McMessage {
    item:      Option<String>,
    using:     Option<bool>,
    #[serde(rename = "useProg")]
    use_prog:  Option<f32>,
    mining:    Option<bool>,
    blocking:  Option<bool>,
    attack:    Option<bool>,  // rising-edge swing event
    hurt:      Option<bool>,  // rising-edge damage event
    sprinting: Option<bool>,
    #[serde(rename = "onGround")]
    on_ground: Option<bool>,
    health:    Option<f32>,
}

pub fn spawn_bridge(state: Arc<Mutex<AppState>>) {
    thread::spawn(move || bridge_loop(state));
}

fn bridge_loop(state: Arc<Mutex<AppState>>) {
    loop {
        let listener = match TcpListener::bind(MC_BRIDGE_ADDR) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[mc] bind {MC_BRIDGE_ADDR} failed: {e}; retrying in 3s");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        eprintln!("[mc] bridge listening on {MC_BRIDGE_ADDR}");

        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => handle_client(stream, &state),
                Err(e) => eprintln!("[mc] accept error: {e}"),
            }
            // Client dropped — mark disconnected and reset the held item so the
            // lightbar falls back to the Minecraft default.
            if let Ok(mut s) = state.lock() {
                s.mc_connected = false;
                s.mc_item      = McItem::Empty;
                s.mc_using     = false;
                s.mc_use_prog  = 0.0;
                s.mc_mining    = false;
                s.mc_blocking  = false;
                s.mc_sprinting = false;
                s.mc_health    = 20.0;
            }
        }
    }
}

fn handle_client(stream: TcpStream, state: &Arc<Mutex<AppState>>) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    eprintln!("[mc] mod connected ({peer})");
    if let Ok(mut s) = state.lock() {
        s.mc_connected = true;
    }

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break, // connection closed / read error
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<McMessage>(line) {
            Ok(msg) => {
                if let Ok(mut s) = state.lock() {
                    s.mc_connected = true;
                    if let Some(i) = msg.item      { s.mc_item = McItem::from_str(&i); }
                    if let Some(v) = msg.using     { s.mc_using = v; }
                    if let Some(v) = msg.use_prog  { s.mc_use_prog = v.clamp(0.0, 1.0); }
                    if let Some(v) = msg.mining    { s.mc_mining = v; }
                    if let Some(v) = msg.blocking  { s.mc_blocking = v; }
                    if let Some(v) = msg.sprinting { s.mc_sprinting = v; }
                    if let Some(v) = msg.on_ground { s.mc_on_ground = v; }
                    if let Some(v) = msg.health    { s.mc_health = v.clamp(0.0, 20.0); }
                    // Rising-edge events → fire a pulse the frame loop counts down.
                    if msg.attack == Some(true) { s.mc_attack_frames = MC_ATTACK_FRAMES; }
                    if msg.hurt   == Some(true) { s.mc_hurt_frames   = MC_HURT_FRAMES; }
                }
            }
            Err(e) => eprintln!("[mc] bad message {line:?}: {e}"),
        }
    }
    eprintln!("[mc] mod disconnected ({peer})");
}
