// `cargo run` futtatja a main-t; ez a Tauri bootstrap.
// A windows feature flag elnyomja a console-window-t az NSIS build során.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lumi_usb_installer_lib::run()
}
