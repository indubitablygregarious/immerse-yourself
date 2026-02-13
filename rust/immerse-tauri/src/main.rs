//! Tauri entry point for Immerse Yourself.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    immerse_tauri_lib::run();
}
