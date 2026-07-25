mod hid;
mod settings;
#[cfg(windows)]
mod hidhide;
#[cfg(windows)]
mod xinput;

use hid::{AppState, Edition, OutputMode, Profile};
use std::sync::{Arc, Mutex};
use tauri::Manager;

type SharedState = Arc<Mutex<AppState>>;

#[tauri::command]
fn get_state(state: tauri::State<SharedState>) -> hid::StateSnapshot {
    state.lock().unwrap().snapshot()
}

#[tauri::command]
fn set_profile(state: tauri::State<SharedState>, profile: String) {
    let mut s = state.lock().unwrap();
    if profile == "static" {
        s.profile = Profile::Static;
    }
}

#[tauri::command]
fn set_output_mode(state: tauri::State<SharedState>, mode: String) -> String {
    let mut s = state.lock().unwrap();
    s.output_mode = hid::OutputMode::from_str(&mode);
    s.output_mode.as_str().to_string()
}

#[tauri::command]
fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state: SharedState = Arc::new(Mutex::new(AppState::default()));

    // Restore saved settings
    {
        let saved = settings::load();
        let mut s = app_state.lock().unwrap();
        if let Some(p) = saved.profile {
            s.profile = Profile::from_str(&p);
        }
        if let Some(m) = saved.output_mode {
            s.output_mode = OutputMode::from_str(&m);
        }
    }

    tauri::Builder::default()
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_profile,
            set_output_mode,
            get_version,
        ])
        .setup(move |app| {
            hid::spawn_hid_thread(app_state.clone(), app.handle().clone());
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: 820.0,
                    height: 540.0,
                }));
                let _ = win.center();
                let _ = win.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running Tauri app");
}
