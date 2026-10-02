// CONCEPT: `mod` declarations
// Rust modules live in separate files, but the compiler only knows about them
// if you declare them here with `mod`. Think of this as "registering" each file
// as part of the project.
pub mod engines;
pub mod formats;
pub mod registry;

use serde::Serialize;

// Pull specific items from our modules into this file's scope.
// `use` is Rust's equivalent of `import` in JS/TypeScript.
use formats::FileFormat;
use registry::{get_conversion_targets, ConversionTarget};

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub status: String,
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        name: "MetaMorph".to_string(),
        version: "0.1.0".to_string(),
        status: "Native Rust Engine Ready".to_string(),
    }
}

#[tauri::command]
fn ping_engine(target_format: &str) -> String {
    format!("Rust engine acknowledged target format: {target_format}")
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

// ─── Phase 3: File Type Detection ───────────────────────────────────────────

#[tauri::command]
fn detect_file_type(path: &str) -> Result<String, String> {
    match infer::get_from_path(path) {
        Ok(Some(kind)) => Ok(kind.extension().to_string()),
        Ok(None) => {
            // Fallback for text-based formats that have no magic bytes.
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            match ext.as_str() {
                "txt" | "md" | "csv" | "html" | "json" => Ok(ext),
                _ => Err("Unknown file format (no magic bytes found)".to_string()),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

// ─── Phase 4: Conversion Registry ───────────────────────────────────────────

/// Returns the list of valid conversion targets for a given source format extension.
///
/// CONCEPT: Chaining Option methods
/// `FileFormat::from_extension(ext)` returns an `Option<FileFormat>`.
/// Instead of writing an `if let Some(...) { ... } else { ... }`, we use
/// `.map(...)` and `.unwrap_or_default()`:
///   - `.map(|fmt| ...)` transforms the `Some` value if it exists.
///   - `.unwrap_or_default()` returns an empty `Vec` if the format was `None`
///     (i.e. unsupported extension) — no panic, no crash.
#[tauri::command]
fn get_targets(source_ext: &str) -> Vec<ConversionTarget> {
    FileFormat::from_extension(source_ext)
        .map(|fmt| get_conversion_targets(&fmt))
        .unwrap_or_default()
}

// ─── Tauri Entry Point ───────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            ping_engine,
            greet,
            detect_file_type,
            get_targets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
