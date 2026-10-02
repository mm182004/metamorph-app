// CONCEPT: Attributes and Conditional Compilation
// `#![cfg_attr(...)]` tells the Rust compiler: "If NOT compiling in debug mode (i.e. release mode),
// use the Windows subsystem instead of console subsystem."
// This hides the black command prompt terminal window when the user opens your desktop app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The main entry point of the binary executable.
// It hands execution over to `metamorph_lib::run()` defined in `lib.rs`.
fn main() {
    metamorph_lib::run()
}

