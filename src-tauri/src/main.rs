#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "desktop")]
fn main() {
    whaleswitch_lib::run();
}

#[cfg(not(feature = "desktop"))]
fn main() {}
