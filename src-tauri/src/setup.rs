// ============================================================
// setup.rs — First-Run Setup & Portable Binaries
// ============================================================
//
// CONCEPT: The App Data Directory
// Tauri's `AppHandle::path().app_data_dir()` gives us a secure,
// user-specific folder (like `C:\Users\Name\AppData\Roaming\metamorph\`)
// where we can store downloaded binaries without needing Admin rights.
//
// CONCEPT: Streaming Downloads
// We use `reqwest` with the `stream` feature to download large files
// piece-by-piece, emitting progress events to the UI so it doesn't look frozen.

use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use futures_util::StreamExt;
use zip::ZipArchive;
use serde::Serialize;
use crate::ProgressPayload;

#[derive(Serialize)]
pub struct DependencyStatus {
    pub ffmpeg_ready: bool,
    pub pandoc_ready: bool,
    pub libreoffice_ready: bool,
}

// ─── Constants ──────────────────────────────────────────────────────────────

const FFMPEG_URL: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
const PANDOC_URL: &str = "https://github.com/jgm/pandoc/releases/download/3.1.12.3/pandoc-3.1.12.3-windows-x86_64.zip";

// ─── Tauri Commands ─────────────────────────────────────────────────────────

/// Check if dependencies are already installed (either on PATH or in our local folder).
#[tauri::command]
pub fn check_dependencies(app: AppHandle) -> Result<DependencyStatus, String> {
    Ok(DependencyStatus {
        ffmpeg_ready: resolve_binary(&app, "ffmpeg").is_some(),
        pandoc_ready: resolve_binary(&app, "pandoc").is_some(),
        libreoffice_ready: crate::libreoffice::find_libreoffice().is_some(),
    })
}

/// Download and extract missing dependencies.
#[tauri::command]
pub async fn install_dependencies(app: AppHandle) -> Result<(), String> {
    let bin_dir = binaries_dir(&app)?;
    
    // Create the binaries folder if it doesn't exist
    if !bin_dir.exists() {
        fs::create_dir_all(&bin_dir)
            .map_err(|e| format!("Failed to create binaries directory: {}", e))?;
    }

    // 1. Download and Extract FFmpeg if missing
    if resolve_binary(&app, "ffmpeg").is_none() {
        download_and_extract(
            &app,
            FFMPEG_URL,
            &bin_dir,
            "FFmpeg",
            vec!["ffmpeg.exe", "ffprobe.exe"]
        ).await?;
    }

    // 2. Download and Extract Pandoc if missing
    if resolve_binary(&app, "pandoc").is_none() {
        download_and_extract(
            &app,
            PANDOC_URL,
            &bin_dir,
            "Pandoc",
            vec!["pandoc.exe"]
        ).await?;
    }

    let _ = app.emit("setup-progress", ProgressPayload {
        percentage: 100.0,
        message: "All dependencies installed!".to_string(),
    });

    Ok(())
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Gets the absolute path to our app's private `binaries` folder.
fn binaries_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app.path().app_data_dir()
        .map_err(|e| format!("Could not resolve app data dir: {}", e))?;
    Ok(data_dir.join("binaries"))
}

/// Tries to resolve a binary name to an absolute path.
/// First checks our local `binaries` folder, then checks the system PATH.
pub fn resolve_binary(app: &AppHandle, name: &str) -> Option<String> {
    // 1. Check local app data folder first (e.g. `ffmpeg.exe`)
    if let Ok(dir) = binaries_dir(app) {
        let local_exe = dir.join(format!("{}.exe", name));
        if local_exe.exists() {
            return Some(local_exe.to_string_lossy().to_string());
        }
    }

    // 2. Check system PATH using `where` (Windows) or `which` (Unix)
    #[cfg(windows)]
    let cmd = "where";
    #[cfg(not(windows))]
    let cmd = "which";

    if let Ok(output) = std::process::Command::new(cmd).arg(name).output() {
        if output.status.success() {
            return Some(name.to_string()); // Trust the OS PATH
        }
    }

    None
}

/// Downloads a ZIP file to memory, extracts specific files, and saves them.
async fn download_and_extract(
    app: &AppHandle,
    url: &str,
    bin_dir: &Path,
    name: &str,
    files_to_extract: Vec<&str>,
) -> Result<(), String> {
    // Phase 1: Download
    let _ = app.emit("setup-progress", ProgressPayload {
        percentage: 0.0,
        message: format!("Downloading {}...", name),
    });

    let client = reqwest::Client::new();
    let res = client.get(url).send().await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let total_size = res.content_length().unwrap_or(50_000_000) as f64;
    let mut downloaded: f64 = 0.0;
    
    // We download into a memory buffer instead of a file on disk
    // because these zips are small enough (<100MB) for RAM.
    let mut zip_bytes = Vec::new();
    let mut stream = res.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        zip_bytes.extend_from_slice(&chunk);
        
        downloaded += chunk.len() as f64;
        let pct = (downloaded / total_size * 90.0).clamp(0.0, 90.0);
        
        let _ = app.emit("setup-progress", ProgressPayload {
            percentage: pct,
            message: format!("Downloading {}... ({:.1}%)", name, pct),
        });
    }

    // Phase 2: Extraction
    let _ = app.emit("setup-progress", ProgressPayload {
        percentage: 90.0,
        message: format!("Extracting {}...", name),
    });

    // We spawn a blocking thread because zip extraction blocks the thread
    let bin_dir_clone = bin_dir.to_path_buf();
    let files_clone: Vec<String> = files_to_extract.iter().map(|&s| s.to_string()).collect();
    
    tokio::task::spawn_blocking(move || {
        let cursor = Cursor::new(zip_bytes);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| format!("Failed to read zip: {}", e))?;

        // Search every file in the zip archive
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let file_name = file.name().to_string(); // e.g. "ffmpeg-master/bin/ffmpeg.exe"
            
            // Extract the filename without directories
            let base_name = Path::new(&file_name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // If it's one of the files we want, extract it to bin_dir
            if files_clone.iter().any(|f| f == base_name) {
                let outpath = bin_dir_clone.join(base_name);
                let mut outfile = fs::File::create(&outpath)
                    .map_err(|e| format!("Failed to create {}: {}", base_name, e))?;
                
                io::copy(&mut file, &mut outfile)
                    .map_err(|e| format!("Failed to write {}: {}", base_name, e))?;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Spawn blocking failed: {}", e))??;

    Ok(())
}
