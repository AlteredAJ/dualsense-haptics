# Technical Debt Queue

All issues are pre-existing — none were introduced by the recent implementation phase.

---

## Phase 1 — Panic Fixes

Crash-on-corrupt-settings. Fix first.

---

### P03 — Unchecked array index in `snapshot()`

**ID:** P03
**Severity:** High
**Category:** Correctness

**Description:**
`snapshot()` at `hid.rs:1014` calls `STRENGTHS[self.strength_idx].label` with no bounds check. `STRENGTHS` has 4 elements. A corrupted or hand-edited `settings.json` with `strength_idx >= 4` causes a panic on the UI emitter thread, crashing the app.

**Files:** `src-tauri/src/hid.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P04 — Unchecked array index in `PLAYER_LED`

**ID:** P04
**Severity:** High
**Category:** Correctness

**Description:**
`hid_loop()` at `hid.rs:3146` and `hid_loop()` at `hid.rs:3225` call `PLAYER_LED[s.strength_idx]` with no bounds check. `PLAYER_LED` has 4 elements. Same corrupt-settings vector as P03. Panics on the output thread, crashing haptics.

**Files:** `src-tauri/src/hid.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

## Phase 2 — Protocol / Signal Correctness

Functional bugs that silently degrade haptic quality.

---

### P01 — Test bench rumble uses conflicting protocol bits

**ID:** P01
**Severity:** High
**Category:** Correctness

**Description:**
`test_report()` at `hid.rs:1794` sets `b[1] |= 0x03` (bit0 legacy + bit1 V2). The `with_rumble()` function at line 1362 uses `report[1] |= 0x02; report[39] |= 0x04` following the Linux hid-playstation V2 convention. Mixing legacy bit0 with V2 bits causes firmware >= 2.24 to fall back to the attenuated DS4-era rumble path, silently weakening Trigger Lab test bench rumble.

**Files:** `src-tauri/src/hid.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P02 — TOCTOU race in Forza watchdog connection flag clearing

**ID:** P02
**Severity:** High
**Category:** Concurrency, Correctness

**Description:**
`watchdog_loop()` at `forza.rs:127-141` reads `last_rx` under one `Mutex` lock, releases it, then acquires the `state` lock to clear `t_connected`/`t_on`. Between the two acquisitions, a receiver thread can stamp a fresh timestamp and set `t_connected = true`. The watchdog then overwrites it to `false`. Window is ≤250 ms (watchdog sleep interval). Self-corrects on the next Forza packet (~16 ms later), causing a brief connection-flag flicker.

**Files:** `src-tauri/src/forza.rs`
**Estimated Effort:** S
**Risk of Fixing:** Medium — adding a double-check lock or merging `last_rx` into `AppState` changes the lock acquisition pattern; must verify no deadlock
**Dependencies:** None

---

## Phase 3 — Error Handling Robustness

Graceful degradation when system tools fail.

---

### P05 — `hidhide::enable()` discards CLI listing failure

**ID:** P05
**Severity:** Medium
**Category:** Reliability

**Description:**
`enable()` at `hidhide.rs:82` calls `run(&cli, &["--dev-gaming"]).unwrap_or_default()`. If the CLI returns an error, this silently returns an empty string. `parse_sony_instances()` finds nothing, and the function returns `Err("cloaking on, but no DualSense found to hide (check it's connected)")`. The user sees a misleading "no controller" error when the real problem is HidHideCLI crashing or failing. The actual CLI error is lost.

**Files:** `src-tauri/src/hidhide.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P06 — `settings::load()` silently discards corrupted save files

**ID:** P06
**Severity:** Medium
**Category:** Reliability

**Description:**
`load()` at `settings.rs:114-118` returns `SavedSettings::default()` on any I/O error or JSON parse failure. The user loses all preferences without warning. The corrupted file is silently overwritten on the next `persist()` call with default values. A transient disk error or a hand-edited typo nukes all settings.

**Files:** `src-tauri/src/settings.rs`
**Estimated Effort:** S
**Risk of Fixing:** Low — requires adding backup logic and a log message
**Dependencies:** None

---

## Phase 4 — Concurrency / Session Correctness

Thread lifecycle and session state consistency.

---

### P07 — Forza threads have no shutdown mechanism

**ID:** P07
**Severity:** Medium
**Category:** Concurrency

**Description:**
`spawn_bridge()` at `forza.rs:61-73` creates `FORZA_PORTS.len() + 1` threads (3 receivers + 1 watchdog) that loop forever. No `AtomicBool` stop flag. Process termination kills them — functionally correct for a desktop app. However, if the user switches away from Racing profile or the app needs to clean up before shutdown, there is no path to stop bridge threads while the process is alive.

**Files:** `src-tauri/src/forza.rs`
**Estimated Effort:** S
**Risk of Fixing:** Low — accept an `Arc<AtomicBool>` in `spawn_bridge()` and check it per loop iteration
**Dependencies:** None

---

### P09 — `init_session` starts HID before license check in release

**ID:** P09
**Severity:** Medium
**Category:** Correctness

**Description:**
In the release build path at `lib.rs:277-298`, `gate.try_start_hid()` runs unconditionally before `license::check()`. If the license is valid, `s.edition = Edition::Full` is applied after HID is already running. Full-tier features (ABS, shift feedback, gun burst/auto) are unavailable for the first few frames until the edition is upgraded. The `LicenseGate` struct comment at line 51 says HID "only starts once init_session() validates" which is inaccurate.

**Files:** `src-tauri/src/lib.rs`
**Estimated Effort:** S
**Risk of Fixing:** Medium — reordering HID start after license check changes the startup timing; must ensure HID still starts in Free tier when no key is provided
**Dependencies:** P08 (comment update should match the corrected behavior)

---

### P10 — `racing_custom_active()` lab flag can leak across sessions

**ID:** P10
**Severity:** Medium
**Category:** Correctness

**Description:**
`racing_lab_active` is set by `set_racing_lab` (Live Preview toggle) and is not persisted in `SavedSettings`. It resets to `false` on restart — safe. However, `racing_custom_on` IS persisted and could remain `true` from a Full-tier session when the user is downgraded to Free. `racing_custom_active()` correctly gates on `self.edition == Edition::Full`, so the curve is not applied. But `racing_lab_active` and `racing_custom_on` use different lifecycle rules (runtime-only vs persisted), creating an inconsistency that could confuse future changes.

**Files:** `src-tauri/src/hid.rs`, `src-tauri/src/lib.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low — clear `racing_lab_active` when edition is downgraded, or unify the two flags
**Dependencies:** None

---

## Phase 5 — Documentation / Code Quality

Harmless but adds friction for future work.

---

### P08 — Stale `LicenseGate` comment contradicts debug-build behavior

**ID:** P08
**Severity:** Medium
**Category:** Maintainability

**Description:**
Comment at `lib.rs:37-39` states "The HID thread is NOT started at app launch. It only starts after Rust confirms a valid license." In debug builds, `spawn_hid_thread` is called directly from `setup()` at line 726 before any license check. The comment is misleading for anyone working in debug mode.

**Files:** `src-tauri/src/lib.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** P09 (should reflect the corrected behavior if P09 is fixed)

---

### P11 — Dead import `std::io::Write` in `forza.rs`

**ID:** P11
**Severity:** Low
**Category:** Style

**Description:**
`use std::io::Write;` at `forza.rs:20` is never used. The file uses `UdpSocket`, `eprintln!`, and byte-slice operations — none require the `Write` trait.

**Files:** `src-tauri/src/forza.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P12 — Dead constant `GUN_AUTO_HZ`

**ID:** P12
**Severity:** Low
**Category:** Maintainability

**Description:**
`const GUN_AUTO_HZ: u8 = 13` at `hid.rs:377` is defined but never referenced in any code path. The Gun profile's auto-fire rate is set per-weapon via `rate_hz` in the `WEAPONS` table.

**Files:** `src-tauri/src/hid.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P13 — `pedal_wall` uses hardcoded force values

**ID:** P13
**Severity:** Low
**Category:** Maintainability

**Description:**
`pedal_wall()` at `hid.rs:1200-1208` computes `let wall = (160.0 + 95.0 * t) as u8` with unnamed magic numbers. The surrounding code uses named constants for every other tuning parameter (`PEDAL_WALL_START`, `PEDAL_WALL_END`, etc.).

**Files:** `src-tauri/src/hid.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P14 — `pacejka_force` applies `.abs()` redundantly

**ID:** P14
**Severity:** Low
**Category:** Maintainability

**Description:**
`pacejka_force()` at `signal.rs:73-77` calls `.sin()` then `.abs()`, producing always-positive output. All callers already pass `slip.abs()` via `pacejka_haptic()`. The inner `.abs()` is redundant. If a future caller passes signed slip to `pacejka_force()` directly, the sign information is silently lost.

**Files:** `src-tauri/src/signal.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low — move `.abs()` into `pacejka_haptic()`, keep `pacejka_force()` pure
**Dependencies:** None

---

### P15 — `slip_crossover_freq` name misleading

**ID:** P15
**Severity:** Low
**Category:** Maintainability

**Description:**
`slip_crossover_freq()` at `signal.rs:87-93` is a hard binary switch: `if slip > RATIO { deep } else { base }`. The name implies a blended crossover transition. The caller pre-computes `base_hz` via interpolation, but the function itself does not blend.

**Files:** `src-tauri/src/signal.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low — rename to `slip_mode_freq`; no behavior change
**Dependencies:** None

---

### P16 — `Forza::WARNED` uses Relaxed ordering for one-shot

**ID:** P16
**Severity:** Low
**Category:** Concurrency

**Description:**
`AtomicBool::swap(true, Ordering::Relaxed)` at `forza.rs:263` for a one-shot diagnostic warning. Two receiver threads could see `false` simultaneously and both print the warning. The window is tiny (first packet arrival) and double-printing a warning is harmless, but `compare_exchange` would guarantee strict one-shot behavior.

**Files:** `src-tauri/src/forza.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low
**Dependencies:** None

---

### P17 — `brake_curve` parameter shadow

**ID:** P17
**Severity:** Low
**Category:** Style

**Description:**
`brake_curve()` at `hid.rs:1244` shadows its `low_end` parameter: `let low_end = low_end.clamp(...)`. Intentional input validation but the shadow creates a local variable with the same name as the parameter.

**Files:** `src-tauri/src/hid.rs`
**Estimated Effort:** XS
**Risk of Fixing:** Low — rename to `let clamped_low_end`
**Dependencies:** None

---

## Summary

| Phase | Issues | Total Effort | Priority |
|-------|--------|-------------|----------|
| 1 — Panic Fixes | P03, P04 | XS | Must-fix |
| 2 — Protocol Correctness | P01, P02 | XS-S | Should-fix |
| 3 — Error Handling | P05, P06 | XS-S | Should-fix |
| 4 — Concurrency/Session | P07, P09, P10 | XS-S | Nice-to-fix |
| 5 — Code Quality | P08, P11-P17 | XS (each) | Cosmetic |

| Severity | Count |
|----------|-------|
| High | 4 |
| Medium | 6 |
| Low | 7 |
| **Total** | **17** |
