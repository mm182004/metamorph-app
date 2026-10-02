// CONCEPT: `mod` declarations
// Rust modules live in separate files, but the compiler only knows about them
// if you declare them here with `mod`. Think of this as "registering" each file
// as part of the project.
pub mod engines;
pub mod ffmpeg;
pub mod formats;
pub mod libreoffice;
pub mod pandoc;
pub mod registry;

use serde::Serialize;

// Pull specific items from our modules into this file's scope.
// `use` is Rust's equivalent of `import` in JS/TypeScript.
use formats::FileFormat;
use registry::{get_conversion_targets, ConversionTarget};

// ─── Phase 8: Structured Progress Events ────────────────────────────────────

/// The payload sent to the frontend for real-time progress updates.
///
/// CONCEPT: Structured Events
/// Instead of just sending a raw number (like `50.0`), we send a JSON object.
/// This allows the UI to show both a progress bar and a descriptive message
/// like "Rendering PDF..." or "Encoding audio...".
#[derive(Clone, Serialize)]
pub struct ProgressPayload {
    pub percentage: f64,
    pub message: String,
}

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

// ─── Phase 5 & 6: Conversion Execution ──────────────────────────────────────

/// Executes the conversion by looking up the correct engine in the registry.
///
/// CONCEPT: Tauri's AppHandle
/// When a `#[tauri::command]` function declares a parameter of type `tauri::AppHandle`,
/// Tauri automatically injects it — you don't pass it from JavaScript.
/// We need it here to `emit()` progress events to the frontend in real-time.
#[tauri::command]
async fn convert_file(
    app_handle: tauri::AppHandle,
    source_path: String,
    source_ext: String,
    target_ext: String,
) -> Result<String, String> {
    // 1. Find the target engine from our registry
    let source_fmt = FileFormat::from_extension(&source_ext)
        .ok_or_else(|| format!("Unknown source format: {}", source_ext))?;
    
    let targets = get_conversion_targets(&source_fmt);
    let target = targets.iter()
        .find(|t| t.format.extension() == target_ext)
        .ok_or_else(|| format!("Conversion from {} to {} not supported", source_ext, target_ext))?;

    // 2. Determine the output directory (same as source for now)
    let output_dir = std::path::Path::new(&source_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "".to_string());

    // 3. Dispatch to the right engine based on the registry's tag
    use crate::engines::{ConversionEngine, Engine, ImageCrateEngine};
    
    match target.engine {
        Engine::ImageCrate => {
            let engine = ImageCrateEngine;
            engine.convert(&source_path, &target_ext, &output_dir, &app_handle)
        },
        Engine::Ffmpeg => {
            // Phase 6: Async FFmpeg subprocess with progress streaming.
            // We try "ffmpeg" / "ffprobe" on PATH first.
            // Phase 9 will add logic to check app_data_dir() for portable builds.
            crate::ffmpeg::run_ffmpeg(
                "ffmpeg",
                "ffprobe",
                &source_path,
                &target_ext,
                &output_dir,
                &app_handle,
            ).await
        },
        Engine::LibreOffice => {
            // Phase 7: LibreOffice headless — best for rendered document fidelity
            crate::libreoffice::run_libreoffice(
                &source_path,
                &target_ext,
                &output_dir,
                &app_handle,
            ).await
        },
        Engine::Pandoc => {
            // Phase 7: Pandoc — best for text/markup structural conversions
            // We pass source_ext so Pandoc knows which format it's reading FROM.
            crate::pandoc::run_pandoc(
                &source_path,
                &source_ext,
                &target_ext,
                &output_dir,
                &app_handle,
            ).await
        },
    }
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
            convert_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
