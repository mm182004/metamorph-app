pub mod engines;
pub mod ffmpeg;
pub mod formats;
pub mod libreoffice;
pub mod pandoc;
pub mod registry;
pub mod setup;

use serde::Serialize;

use formats::FileFormat;
use registry::{get_conversion_targets, ConversionTarget};


/// The payload sent to the frontend for real-time progress updates.
///
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
        status: "Engine Ready".to_string(),
    }
}


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


/// Returns the list of valid conversion targets for a given source format extension.
///
#[tauri::command]
fn get_targets(source_ext: &str) -> Vec<ConversionTarget> {
    FileFormat::from_extension(source_ext)
        .map(|fmt| get_conversion_targets(&fmt))
        .unwrap_or_default()
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::State;

pub struct CancellationState(pub Arc<AtomicBool>);

#[tauri::command]
fn cancel_conversion(state: State<'_, CancellationState>) {
    state.0.store(true, Ordering::SeqCst);
}


/// Executes the conversion by looking up the correct engine in the registry.
#[tauri::command]
async fn convert_file(
    app_handle: tauri::AppHandle,
    cancel_state: State<'_, CancellationState>,
    source_path: String,
    source_ext: String,
    target_ext: String,
) -> Result<String, String> {
    // Reset cancellation flag at the start of conversion
    cancel_state.0.store(false, Ordering::SeqCst);
    let cancel_flag = cancel_state.0.clone();

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

    if cancel_flag.load(Ordering::SeqCst) {
        return Err("Conversion cancelled by user".to_string());
    }

    // 3. Dispatch to the right engine based on the registry's tag
    use crate::engines::Engine;
    
    match target.engine {
        Engine::ImageCrate => {
            crate::engines::convert_image(&source_path, &target_ext, &output_dir, &app_handle)
        },
        Engine::Ffmpeg => {
            let ffmpeg_bin = crate::setup::resolve_binary(&app_handle, "ffmpeg")
                .ok_or_else(|| "FFmpeg is not installed.".to_string())?;
            let ffprobe_bin = crate::setup::resolve_binary(&app_handle, "ffprobe")
                .ok_or_else(|| "FFprobe is not installed.".to_string())?;

            crate::ffmpeg::run_ffmpeg(
                &ffmpeg_bin,
                &ffprobe_bin,
                &source_path,
                &target_ext,
                &output_dir,
                &app_handle,
                cancel_flag,
            ).await
        },
        Engine::LibreOffice => {
            crate::libreoffice::run_libreoffice(
                &source_path,
                &target_ext,
                &output_dir,
                &app_handle,
            ).await
        },
        Engine::Pandoc => {
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
    let cancel_state = CancellationState(Arc::new(AtomicBool::new(false)));

    tauri::Builder::default()
        .manage(cancel_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            detect_file_type,
            get_targets,
            convert_file,
            cancel_conversion,
            crate::setup::check_dependencies,
            crate::setup::install_dependencies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
