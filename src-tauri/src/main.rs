#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The main entry point of the binary executable.
// It hands execution over to `metamorph_lib::run()` defined in `lib.rs`.
fn main() {
    metamorph_lib::run()
}

