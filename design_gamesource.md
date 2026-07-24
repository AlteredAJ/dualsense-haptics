# GameSource Architecture — Design Document

## Problem

The app currently hardcodes Forza telemetry offsets, wheel order, and ports. Adding F1 23, Assetto Corsa, or any other sim requires a clean abstraction so the user can switch games without restarting, and so each game's parser can be developed independently without touching shared haptic synthesis code.

## Decision

**Approach A — Single active GameSource.**

One bridge is active at a time. A `GameSource` enum in `AppState` selects which parser feeds telemetry into the shared `AppState.t_*` fields. The bridge for the active game is spawned; switching games tears down the old bridge and spawns the new one. The user can also set "Auto" mode which detects the active game from the first valid packet received on any known port.

**Rationale:** Running simultaneous bridges is unnecessary — a user plays one sim at a time. Single-active-source avoids priority conflicts, reduces thread count, and keeps state ownership clear. The `t_*` fields remain the single internal representation; each bridge module simply translates its game's format into those fields.

## Architecture

```
                 ┌──────────────────────────┐
                 │       AppState            │
                 │  game_source: GameSource  │
                 │  t_rpm, t_slip_rear, ...  │
                 │  wheel_order: WheelOrder  │
                 └──────────┬───────────────┘
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ forza.rs │ │  f123.rs │ │   acc.rs │
        │ 1 port   │ │ 1 port   │ │ 1 port   │
        │ 324-byte │ │ 29B hdr  │ │  ???     │
        │ FL-FR-RL │ │ RL-RR-FL │ │  ???     │
        └──────────┘ └──────────┘ └──────────┘
```

## New Types

### `GameSource` enum (in `hid.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GameSource {
    None,    // Inferred/simulated engine only
    Auto,    // Detect from first valid packet
    Forza,   // Forza Motorsport / Horizon
    F123,    // F1 23 (Codemasters)
    Assetto, // Assetto Corsa
}
```

### `WheelOrder` enum (in `hid.rs`)

```rust
/// Wheel array index convention used by each game's telemetry.
/// Forza:   0=FL, 1=FR, 2=RL, 3=RR
/// F1 23:   0=RL, 1=RR, 2=FL, 3=FR
/// AC:      TBD by Gemini research
#[derive(Debug, Clone, Copy)]
pub enum WheelOrder {
    FlFrRlRr,  // Forza
    RlRrFlFr,  // F1 23
}
```

## State Changes

### `AppState` new fields

```rust
pub game_source: GameSource,          // active telemetry source
pub wheel_order: WheelOrder,          // per-game wheel index convention
```

### Default values

```rust
game_source: GameSource::None,
wheel_order: WheelOrder::FlFrRlRr,   // match Forza (current default)
```

## Bridge Lifecycle

1. **Startup:** No bridge spawned (`GameSource::None`). Only the simulated/inferred engine runs.
2. **User selects a game:** Store `game_source` in AppState, spawn the corresponding bridge thread(s).
3. **Auto mode:** Spawn a lightweight "scout" thread that listens on all known ports. The first game to send a valid packet wins — `game_source` is set and the appropriate bridge takes over. Scout thread exits.
4. **Switch games:** Tear down the active bridge (via `Arc<AtomicBool>` stop flag from P07), spawn the new one.
5. **Deselect / None:** Tear down bridge, clear telemetry flags, fall back to inferred engine.

## Per-Bridge Module Contract

Each bridge module (`forza.rs`, `f123.rs`, `acc.rs`) must export:

```rust
/// Port(s) this game uses for telemetry UDP broadcasts.
pub const PORTS: &[u16];

/// Wheel index convention.
pub const WHEEL_ORDER: WheelOrder;

/// Spawn receiver + watchdog threads. Accepts a stop flag for clean teardown.
pub fn spawn(state: Arc<Mutex<AppState>>, stop: Arc<AtomicBool>);

/// Parse one packet into AppState.t_* fields. Called by the receiver thread.
fn apply_packet(s: &mut AppState, buf: &[u8], len: usize);
```

## Tauri Command

```rust
#[tauri::command]
fn set_game_source(state: State<SharedState>, source: String) -> String
```

Persisted to `settings.json` as `game_source: Option<String>`.

## Assetto Corsa — Unknowns (for Gemini)

- Does AC broadcast UDP telemetry natively?
- What port(s)? What packet format?
- Wheel order convention?
- What physics fields are available (RPM, slip, suspension, speed, gear)?
- Does it use the same offsets as any known format?

## Implementation Order

| Step | Description | Effort |
|------|-------------|--------|
| 1 | Add `GameSource` + `WheelOrder` enums to `hid.rs` | XS |
| 2 | Add `AppState.game_source` + `wheel_order` fields | XS |
| 3 | Add `set_game_source` Tauri command + persistence | S |
| 4 | Refactor `forza::spawn_bridge` → `forza::spawn` matching contract | S |
| 5 | Implement `f123::spawn` + `apply_packet` (from Gemini spec) | M |
| 6 | Implement `acc::spawn` + `apply_packet` (blanks — needs Gemini) | M |
| 7 | Auto-detect scout thread for `GameSource::Auto` | S |
| 8 | Bridge teardown on source switch via stop flag | XS |
