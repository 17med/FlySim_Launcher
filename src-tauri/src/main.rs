// Release builds on Windows would otherwise open a console window next to the launcher.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    flysim_launcher_lib::run()
}
