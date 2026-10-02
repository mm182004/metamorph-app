// ============================================================
// libreoffice.rs — LibreOffice headless document conversion engine
// ============================================================
//
// CONCEPT: Synchronous subprocess inside an async context
// LibreOffice conversions are fast (typically < 5 seconds) and don't
// produce parseable progress output, so we use the blocking
// `std::process::Command` API — but wrapped in `tokio::task::spawn_blocking`
// so we don't starve tokio's async runtime.
//
// `spawn_blocking` runs the closure on a dedicated OS thread from
// tokio's blocking thread pool, while the async task that called it
// suspends and yields to let other async work proceed.
//
// CONCEPT: `move` closures and ownership
// When we pass a closure to `spawn_blocking`, the closure must be
// `'static` (no borrows from the stack) and `Send` (safe to ship
// across threads). The `move` keyword takes *ownership* of the
// variables we need inside the closure — the values are moved in
// rather than borrowed.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Detect the LibreOffice binary name on the current OS.
///
/// LibreOffice ships as `soffice` on macOS/Linux and `soffice.exe` on
/// Windows. We try `soffice` first (works on all platforms), then a
/// common Windows install path as a fallback.
fn find_libreoffice() -> Option<String> {
    // Try the standard `soffice` command on PATH first
    if Command::new("soffice").arg("--version").output().is_ok() {
        return Some("soffice".to_string());
    }

    // Common Windows Program Files path
    let win_path = r"C:\Program Files\LibreOffice\program\soffice.exe";
    if Path::new(win_path).exists() {
        return Some(win_path.to_string());
    }

    // macOS application path
    let mac_path = "/Applications/LibreOffice.app/Contents/MacOS/soffice";
    if Path::new(mac_path).exists() {
        return Some(mac_path.to_string());
    }

    None
}

/// Map a target extension to LibreOffice's `--convert-to` format string.
///
/// LibreOffice uses its own filter names, not just file extensions.
fn lo_format_string(target_ext: &str) -> Option<&'static str> {
    match target_ext {
        "pdf"  => Some("pdf"),
        "docx" => Some("docx"),
        "odt"  => Some("odt"),
        "html" => Some("html"),
        "txt"  => Some("txt"),
        _      => None,
    }
}

/// Convert a document using LibreOffice headless mode.
///
/// LibreOffice's `--convert-to` flag always writes the output file
/// into the `--outdir` directory with the new extension. We can't
/// control the exact output filename, so we return the path we
/// expect LibreOffice to have written.
///
/// # Arguments
/// - `source_path` — absolute path to the input document.
/// - `target_ext`  — desired output format (e.g. `"pdf"`, `"docx"`).
/// - `output_dir`  — directory where the converted file will land.
pub async fn run_libreoffice(
    source_path: &str,
    target_ext: &str,
    output_dir: &str,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    use tauri::Emitter;

    let _ = app_handle.emit("conversion-progress", crate::ProgressPayload {
        percentage: 10.0,
        message: "Starting LibreOffice engine...".to_string(),
    });

    // 1. Locate the LibreOffice binary
    let soffice = find_libreoffice()
        .ok_or_else(|| "LibreOffice not found. Please install LibreOffice.".to_string())?;

    // 2. Look up the LibreOffice format string
    let lo_format = lo_format_string(target_ext)
        .ok_or_else(|| format!("LibreOffice does not support target format: {}", target_ext))?;

    let _ = app_handle.emit("conversion-progress", crate::ProgressPayload {
        percentage: 40.0,
        message: format!("Rendering layout as {}...", target_ext.to_uppercase()),
    });

    // 3. Determine the expected output path.
    // LibreOffice always names the output: <source_stem>.<target_ext>
    let source_stem = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let expected_output: PathBuf = Path::new(output_dir)
        .join(format!("{}.{}", source_stem, target_ext));

    // 4. Clone values to move into the blocking closure.
    // CONCEPT: Clone for spawn_blocking
    // `spawn_blocking` requires all captured values to be 'static (owned).
    // Strings are cloneable, so we clone the &str values into owned Strings.
    let soffice = soffice.clone();
    let lo_format = lo_format.to_string();
    let source_path = source_path.to_string();
    let output_dir = output_dir.to_string();
    let expected_output_clone = expected_output.clone();

    // 5. Run LibreOffice on a blocking thread.
    // CONCEPT: tokio::task::spawn_blocking
    // Closures passed here run on a dedicated OS thread so the async
    // runtime's cooperative scheduler isn't blocked.
    tokio::task::spawn_blocking(move || {
        // LibreOffice command:
        //   soffice --headless --convert-to pdf --outdir /path/to/dir /path/to/file.docx
        let output = Command::new(&soffice)
            .args([
                "--headless",
                "--convert-to", &lo_format,
                "--outdir", &output_dir,
                &source_path,
            ])
            .output()
            .map_err(|e| format!("Failed to start LibreOffice: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("LibreOffice failed: {}", stderr.trim()));
        }

        // Verify the expected output file actually exists
        if !expected_output_clone.exists() {
            return Err(format!(
                "LibreOffice ran but output file not found at: {}",
                expected_output_clone.display()
            ));
        }

        Ok(expected_output_clone.to_string_lossy().to_string())
    })
    // CONCEPT: Flattening a nested Result
    // `spawn_blocking` itself returns `Result<Result<String, String>, JoinError>`.
    // `.await?` unwraps the outer JoinError, leaving us with `Result<String, String>`.
    // We `.map_err()` the JoinError into a String first so the types align.
    .await
    .map_err(|e| format!("spawn_blocking failed: {}", e))?
}
