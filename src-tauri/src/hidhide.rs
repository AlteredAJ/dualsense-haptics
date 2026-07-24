// HidHide automation (Windows only).
//
// When Xbox output mode is on, we cloak the real DualSense from all apps except ours, so
// the game sees ONLY the virtual Xbox pad (no double input). This shells out to
// HidHideCLI.exe (installed with HidHide). Best-effort: if HidHide isn't installed or the
// CLI errors, we surface a message but the virtual pad still works (the game may just see
// both controllers). HidHideCLI usually needs the app to run elevated (admin).
//
// NOTE: HidHideCLI's --dev-gaming output format and flag names have shifted across
// versions. If cloaking doesn't take effect, run `HidHideCLI.exe --dev-gaming` yourself
// and adjust `parse_sony_instances` / the flags below. The manual steps in docs/WINDOWS.md
// remain a fallback.

use std::path::{Path, PathBuf};
use std::process::Command;

// Sony Corp. USB vendor ID — every DualSense reports under this.
const SONY_VID: &str = "VID_054C";

// CreateProcess flag: run the child with NO console window. HidHideCLI.exe is a
// console-subsystem app, so without this every call flashes (or, if it blocks,
// leaves stuck) a black cmd window on top of the GUI.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn cli_path() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files\Nefarius Software Solutions e.U\HidHide\x64\HidHideCLI.exe",
        r"C:\Program Files\Nefarius Software Solutions\HidHide\x64\HidHideCLI.exe",
        r"C:\Program Files\Nefarius Software Solutions\HidHide\HidHideCLI.exe",
        r"C:\Program Files (x86)\Nefarius Software Solutions e.U\HidHide\x64\HidHideCLI.exe",
        r"C:\Program Files (x86)\Nefarius Software Solutions\HidHide\HidHideCLI.exe",
    ];
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

fn run(cli: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(cli);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("HidHideCLI launch failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// Pull device instance paths for any Sony device from `--dev-gaming` output. The CLI
/// prints lines like `Device: HID\VID_054C&PID_0CE6\9&abc&0&0000`; we grab the path token.
fn parse_sony_instances(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter(|l| l.to_uppercase().contains(SONY_VID))
        .filter_map(extract_instance)
        .collect()
}

fn extract_instance(line: &str) -> Option<String> {
    let upper = line.to_uppercase();
    // Device instance paths start with a bus prefix; HID\ for the gamepad interface,
    // USB\ for the base container. Prefer HID\.
    let start = upper.find("HID\\").or_else(|| upper.find("USB\\"))?;
    Some(line[start..].trim().to_string())
}

/// Whitelist our process, hide every Sony DualSense, and turn cloaking on.
pub fn enable() -> Result<(), String> {
    let cli = cli_path().ok_or("HidHideCLI.exe not found — install HidHide")?;

    // Whitelist this app so we keep reading the DualSense while it's hidden from games.
    if let Ok(exe) = std::env::current_exe() {
        let _ = run(&cli, &["--app-reg", &exe.to_string_lossy()]);
    }

    let listing = run(&cli, &["--dev-gaming"]).unwrap_or_default();
    let mut hid_any = false;
    for path in parse_sony_instances(&listing) {
        if run(&cli, &["--dev-hide", &path]).is_ok() {
            hid_any = true;
        }
    }

    run(&cli, &["--cloak-on"])?;
    if hid_any {
        Ok(())
    } else {
        Err("cloaking on, but no DualSense found to hide (check it's connected)".into())
    }
}

/// Un-cloak: stop hiding the DualSense and turn cloaking off so other apps see it again.
pub fn disable() -> Result<(), String> {
    let cli = cli_path().ok_or_else(|| {
        eprintln!("[hidhide] CLI not found — cannot disable cloaking; the controller may stay hidden until the system is restarted or HidHide is configured manually");
        "HidHideCLI.exe not found".to_string()
    })?;
    let listing = run(&cli, &["--dev-gaming"]).unwrap_or_default();
    for path in parse_sony_instances(&listing) {
        let _ = run(&cli, &["--dev-unhide", &path]);
    }
    match run(&cli, &["--cloak-off"]) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("[hidhide] cloak-off failed: {e}");
            Err(e)
        }
    }
}
