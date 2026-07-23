// Prevents an extra console window on Windows in release builds
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    dualsense_haptics_lib::run()
}
