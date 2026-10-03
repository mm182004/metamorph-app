// ============================================================
// ffmpeg.rs — FFmpeg engine: async subprocess with progress parsing
// ============================================================
//
use std::path::Path;
use tauri::Emitter;  // Tauri v2 requires this trait in scope to call .emit()
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Holds the progress state parsed from FFmpeg's `-progress` output.
struct FfmpegProgress {
    /// Total duration of the source media in microseconds.
    total_duration_us: f64,
    /// Last computed percentage (0–100).
    last_percent: f64,
}

impl FfmpegProgress {
    fn new(total_duration_us: f64) -> Self {
        Self {
            total_duration_us,
            last_percent: 0.0,
        }
    }

    /// Parse one `key=value` line and return Some(percentage) when updated.
    fn parse_line(&mut self, line: &str) -> Option<f64> {
        // FFmpeg's `-progress` output contains lines like:
        //   out_time_us=5230000
        //   progress=continue
        //   progress=end
        if let Some(value) = line.strip_prefix("out_time_us=") {
            if let Ok(us) = value.trim().parse::<f64>() {
                if self.total_duration_us > 0.0 {
                    let pct = (us / self.total_duration_us * 100.0).clamp(0.0, 100.0);
                    self.last_percent = pct;
                    return Some(pct);
                }
            }
        }
        if line.starts_with("progress=end") {
            self.last_percent = 100.0;
            return Some(100.0);
        }
        None
    }
}

/// Probe the total duration of a media file using `ffprobe`.
///
async fn probe_duration(ffprobe_path: &str, source_path: &str) -> Result<f64, String> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            source_path,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe exited with code {:?}",
            output.status.code()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let duration_secs: f64 = stdout
        .trim()
        .parse()
        .map_err(|_| format!("Could not parse ffprobe duration: '{}'", stdout.trim()))?;

    // Convert seconds → microseconds to match FFmpeg's `out_time_us`.
    Ok(duration_secs * 1_000_000.0)
}

/// Build the FFmpeg argument list for a given conversion.
///
fn build_ffmpeg_args(
    source_path: &str,
    output_path: &str,
    target_ext: &str,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-y".into(),               // Overwrite output without asking
        "-i".into(),               // Input flag
        source_path.into(),        // Input file
    ];

    // Format-specific encoding flags
    match target_ext {
        // ── Video ────────────────────────────────────────────────
        "mp4"  => args.extend(["-c:v", "libx264", "-preset", "fast", "-crf", "23", "-c:a", "aac"].map(String::from)),
        "mkv"  => args.extend(["-c:v", "libx264", "-preset", "fast", "-crf", "23", "-c:a", "aac"].map(String::from)),
        "mov"  => args.extend(["-c:v", "libx264", "-preset", "fast", "-crf", "23", "-c:a", "aac"].map(String::from)),
        "webm" => args.extend(["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-c:a", "libopus"].map(String::from)),
        "avi"  => args.extend(["-c:v", "mpeg4", "-q:v", "5", "-c:a", "libmp3lame"].map(String::from)),

        // ── Audio (extract / transcode) ──────────────────────────
        "mp3"  => args.extend(["-vn", "-c:a", "libmp3lame", "-q:a", "2"].map(String::from)),
        "wav"  => args.extend(["-vn", "-c:a", "pcm_s16le"].map(String::from)),
        "flac" => args.extend(["-vn", "-c:a", "flac"].map(String::from)),
        "m4a"  => args.extend(["-vn", "-c:a", "aac", "-q:a", "1"].map(String::from)),
        "ogg"  => args.extend(["-vn", "-c:a", "libvorbis", "-q:a", "5"].map(String::from)),

        // Fallback: let FFmpeg auto-detect based on extension
        _      => {}
    }

    // Machine-readable progress output to stderr (pipe:2 = fd 2 = stderr)
    args.extend(["-progress", "pipe:2"].map(String::from));

    // Output file — must be the very last argument
    args.push(output_path.into());

    args
}

/// Run FFmpeg as an async subprocess and stream progress to the Tauri frontend.
///
/// # Arguments
/// - `ffmpeg_path`  — path to the `ffmpeg` binary (or just `"ffmpeg"` if on PATH).
/// - `ffprobe_path` — path to the `ffprobe` binary.
/// - `source_path`  — absolute path to the input file.
/// - `target_ext`   — desired output extension (e.g. `"mp4"`, `"mp3"`).
/// - `output_dir`   — folder where the output file will be written.
/// - `app_handle`   — Tauri's `AppHandle`, used to `emit()` progress events.
///
/// # Returns
/// The absolute path of the converted output file on success.
pub async fn run_ffmpeg(
    ffmpeg_path: &str,
    ffprobe_path: &str,
    source_path: &str,
    target_ext: &str,
    output_dir: &str,
    app_handle: &tauri::AppHandle,
    cancel_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<String, String> {
    // 1. Probe total duration so we can calculate percentages.
    let total_us = probe_duration(ffprobe_path, source_path).await?;

    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Conversion cancelled by user".to_string());
    }

    // 2. Build the output file path.
    let source_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let output_path = Path::new(output_dir)
        .join(format!("{}.{}", source_name, target_ext));
    let output_str = output_path.to_string_lossy().to_string();

    // 3. Build argument list and spawn FFmpeg.
    let args = build_ffmpeg_args(source_path, &output_str, target_ext);

    let mut child = Command::new(ffmpeg_path)
        .args(&args)
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

    // 4. Take ownership of the stderr pipe.
    let stderr = child.stderr.take()
        .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

    // 5. Wrap the raw pipe in a buffered, async line reader.
    let mut reader = BufReader::new(stderr).lines();
    let mut progress = FfmpegProgress::new(total_us);

    // 6. Read lines from stderr as they arrive with periodic cancellation checks.
    loop {
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = child.kill().await;
            if Path::new(&output_str).exists() {
                let _ = std::fs::remove_file(&output_str);
            }
            return Err("Conversion cancelled by user".to_string());
        }

        tokio::select! {
            line_result = reader.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        if let Some(pct) = progress.parse_line(&line) {
                            let _ = app_handle.emit("conversion-progress", crate::ProgressPayload {
                                percentage: pct,
                                message: format!("Processing audio/video... {:.1}%", pct),
                            });
                        }
                    }
                    Ok(None) => break, // EOF reached
                    Err(e) => return Err(format!("Error reading FFmpeg stderr: {}", e)),
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {
                continue;
            }
        }
    }

    // 7. Wait for FFmpeg to fully exit and check the exit code.
    let status = child.wait().await
        .map_err(|e| format!("Failed to wait for FFmpeg: {}", e))?;

    if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
        if Path::new(&output_str).exists() {
            let _ = std::fs::remove_file(&output_str);
        }
        return Err("Conversion cancelled by user".to_string());
    }

    if !status.success() {
        return Err(format!(
            "FFmpeg exited with code {:?}",
            status.code()
        ));
    }

    // 8. Final 100% event in case FFmpeg didn't emit `progress=end`.
    let _ = app_handle.emit("conversion-progress", crate::ProgressPayload {
        percentage: 100.0,
        message: "Finalizing media...".to_string(),
    });

    Ok(output_str)
}
