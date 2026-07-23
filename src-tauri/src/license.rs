use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LICENSE_SERVER: &str =
    "https://dualsense-haptics-license.universal-dualsense-haptics.workers.dev";
const REVALIDATE_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const APP_VERSION:   &str = env!("CARGO_PKG_VERSION"); // pulled from Cargo.toml at compile time

// ─── Machine fingerprint ──────────────────────────────────────────────────────

pub fn machine_id() -> String {
    let hostname = hostname_str();
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let mac = primary_mac();
    let mut hasher = Sha256::new();
    hasher.update(hostname.as_bytes());
    hasher.update(b"|");
    hasher.update(username.as_bytes());
    hasher.update(b"|");
    hasher.update(mac.as_bytes());
    hex::encode(&hasher.finalize()[..20])
}

fn hostname_str() -> String {
    // `hostname.exe` is a console app, so on Windows it flashes a cmd window each
    // launch. Suppress that window WITHOUT changing what the command returns — the
    // machine-id hash is built from this string, so altering it (e.g. swapping in the
    // COMPUTERNAME env var, which can differ in case) would invalidate license keys
    // already bound to this machine.
    let mut cmd = std::process::Command::new("hostname");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Returns the MAC address of the first active physical interface (en0, en1, …, eth0).
/// Falls back to a stable placeholder so the hash is still unique if ifconfig is unavailable.
fn primary_mac() -> String {
    for iface in &["en0", "en1", "en2", "eth0", "wlan0"] {
        if let Ok(out) = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "ifconfig {} 2>/dev/null | grep -o 'ether [0-9a-f:]*' | awk '{{print $2}}'",
                iface
            ))
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && s != "00:00:00:00:00:00" {
                return s;
            }
        }
    }
    "no-mac".to_string()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─── Storage ──────────────────────────────────────────────────────────────────

fn license_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("dualsense-haptics")
        .join("license.json")
}

#[derive(Serialize, Deserialize, Clone)]
struct CachedLicense {
    key:            String,
    #[serde(rename = "machineId")]
    machine_id:     String,
    token:          String,
    #[serde(rename = "activatedAt", default)]
    activated_at:   u64,
    #[serde(rename = "lastValidated", default)]
    last_validated: u64,
    #[serde(default)]
    pro:            bool, // $4 Pro tier (unlocks the Lab); cached so offline launches know tier
}

fn read_license() -> Option<CachedLicense> {
    let text = std::fs::read_to_string(license_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_license(lic: &CachedLicense) {
    let path = license_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(lic) {
        let _ = std::fs::write(&path, json);
    }
}

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ActivateResp {
    activated: Option<bool>,
    token:     Option<String>,
    #[serde(default)]
    pro:       Option<bool>,
    error:     Option<String>,
}

#[derive(Deserialize)]
struct ValidateResp {
    valid: Option<bool>,
    #[serde(default)]
    pro:   Option<bool>,
    error: Option<String>, // server sets this on version mismatch: "Update required — v1.x.x"
}

fn http_post<T: Serialize>(path: &str, body: &T) -> Result<String, String> {
    let url = format!("{}{}", LICENSE_SERVER, path);
    ureq::post(&url)
        .send_json(body)
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct LicenseResult {
    pub ok:    bool,
    pub pro:   bool,
    pub error: String,
}

/// Called on every launch. Returns LicenseResult { ok: true } if licensed.
/// Pass `key = Some("...")` to activate with a new key.
/// NOTE: debug bypasses live in the Tauri command layer, not here — this always runs real checks.
pub fn check(key: Option<String>) -> LicenseResult {
    let mid = machine_id();

    // If a key was provided, try to activate it
    if let Some(k) = key {
        return activate(&k, &mid);
    }

    // Try existing cached license
    if let Some(mut cached) = read_license() {
        if cached.machine_id != mid {
            return LicenseResult {
                ok:    false,
                pro:   false,
                error: "License bound to a different machine. Re-enter your key.".to_string(),
            };
        }

        // Within revalidation window — trust the cache
        if now_ms().saturating_sub(cached.last_validated) < REVALIDATE_MS {
            return LicenseResult { ok: true, pro: cached.pro, error: String::new() };
        }

        // Need to re-validate with server
        match validate_token(&mid, &cached.token) {
            Ok((true, pro)) => {
                cached.last_validated = now_ms();
                cached.pro = pro; // refresh tier (catches Base→Pro upgrades)
                write_license(&cached);
                return LicenseResult { ok: true, pro: cached.pro, error: String::new() };
            }
            Err(msg) if msg.starts_with("Update required") => {
                // Server is forcing a version upgrade — show the message directly
                return LicenseResult { ok: false, pro: false, error: msg };
            }
            _ => {
                return LicenseResult {
                    ok:    false,
                    pro:   false,
                    error: "License needs re-verification. Enter your key to continue.".to_string(),
                };
            }
        }
    }

    // No cache — need a key
    LicenseResult {
        ok:    false,
        pro:   false,
        error: String::new(), // fresh prompt, no error message
    }
}

fn activate(key: &str, mid: &str) -> LicenseResult {
    #[derive(Serialize)]
    struct Req<'a> {
        key:        &'a str,
        #[serde(rename = "machineId")]  machine_id:  &'a str,
        #[serde(rename = "appVersion")] app_version: &'a str,
    }

    let body = Req { key, machine_id: mid, app_version: APP_VERSION };
    let raw = match http_post("/activate", &body) {
        Ok(s) => s,
        Err(e) => return LicenseResult { ok: false, pro: false, error: format!("Network error: {e}") },
    };

    let resp: ActivateResp = match serde_json::from_str(&raw) {
        Ok(r) => r,
        Err(_) => return LicenseResult { ok: false, pro: false, error: "Bad server response".to_string() },
    };

    if resp.activated == Some(true) {
        if let Some(token) = resp.token {
            let pro = resp.pro.unwrap_or(false);
            let lic = CachedLicense {
                key:            key.to_string(),
                machine_id:     mid.to_string(),
                token,
                activated_at:   now_ms(),
                last_validated: now_ms(),
                pro,
            };
            write_license(&lic);
            return LicenseResult { ok: true, pro, error: String::new() };
        }
    }

    LicenseResult {
        ok:    false,
        pro:   false,
        error: resp.error.unwrap_or_else(|| "Activation failed".to_string()),
    }
}

/// Returns (valid, pro) on success.
fn validate_token(mid: &str, token: &str) -> Result<(bool, bool), String> {
    #[derive(Serialize)]
    struct Req<'a> {
        #[serde(rename = "machineId")]  machine_id:  &'a str,
        token:       &'a str,
        #[serde(rename = "appVersion")] app_version: &'a str,
    }

    let raw = http_post("/validate", &Req { machine_id: mid, token, app_version: APP_VERSION })?;
    let resp: ValidateResp = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if let Some(msg) = resp.error {
        return Err(msg); // propagates "Update required — download vX.Y.Z" to the UI
    }
    Ok((resp.valid == Some(true), resp.pro.unwrap_or(false)))
}
