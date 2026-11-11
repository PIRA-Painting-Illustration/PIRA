// No console window behind the app on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    pira_pose::run_desktop()
}

#[cfg(target_os = "android")]
fn main() {}
