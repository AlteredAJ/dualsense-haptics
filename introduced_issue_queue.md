# Introduced Issue Queue

---

## I01 — MEDIUM: Bluetooth sequence counter may stall with delta cache

**Status:** [x] Complete

**Severity:** Medium

**Files:** `src-tauri/src/hid.rs`

**Root Cause:**
R012 added a 48-byte delta cache in `hid_loop()` that skips `write_report()` when the haptic payload is unchanged. Over Bluetooth, `to_bt_report()` increments a static `AtomicU8` sequence counter on every call. When `write_report()` is skipped, the sequence counter does not advance. Some Bluetooth stacks require monotonic sequence progression to maintain connection health; stalled sequence during extended idle periods (same payload for many seconds) could theoretically cause a BT stack timeout.

**Required Fix:**
Restrict the delta-check skip optimization to USB transport only. For Bluetooth, always call `write_report()` to ensure the BT sequence counter advances monotonically.

**Validation:**
1. Verify `cargo check` passes
2. Confirm USB transport still skips unchanged payloads (optimization preserved)
3. Confirm Bluetooth transport always transmits (sequence counter guaranteed to advance)
4. Confirm no behavioral change for USB path

---

**Remaining Issues:** 0
**Completed Issues:** 1
