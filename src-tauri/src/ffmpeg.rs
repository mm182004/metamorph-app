// ============================================================
// ffmpeg.rs — FFmpeg engine: async subprocess with progress parsing
// ============================================================
//
// CONCEPT: Async Subprocesses with tokio
// In a desktop app, we must NEVER block the main thread while waiting
// for a long-running process like FFmpeg.  Tauri's `#[tauri::command]`
// functions marked `async` run on tokio's thread pool.  Inside them we
// can use `tokio::process::Command` — an async version of the standard
// library's `std::process::Command` — to spawn FFmpeg and read its
// output line-by-line without freezing the UI.
//
// CONCEPT: Piping stderr
// FFmpeg writes progress information to stderr (not stdout).  It uses
// a special `-progress pipe:2` flag that prints machine-readable
// key=value lines to stderr, including `out_time_us` (microseconds of
// output processed so far).  By comparing that to the total duration,
// we can compute a percentage.
//
// CONCEPT: Tauri's emit()
// `AppHandle::emit("event-name", payload)` pushes a real-time event
// to the React frontend over Tauri's IPC channel.  The frontend can
// listen with `listen("event-name", callback)`.  This is the push-based
// model described in Phase 8 of the project plan.

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
/// CONCEPT: Why a separate function?
/// We need the total duration *before* we start converting so we can
/// calculate percentage progress.  `ffprobe` is a companion binary
/// that ships with every FFmpeg distribution.
///
/// Returns the duration in microseconds, or an error string.
async fn probe_duration(ffprobe_path: &str, source_path: &str) -> Result<f64, String> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            source_path,
        ])
        // CONCEPT: `.output()` collects all of stdout/stderr into memory.
        // We `.await` because this is an async function — tokio will suspend
        // this task and let other work proceed until ffprobe finishes.
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
/// CONCEPT: Ownership of Strings in a Vec
/// We build a `Vec<String>` because each argument is an owned, heap-allocated
/// string.  When we later pass them to `Command::args()`, Rust borrows them
/// automatically.
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
) -> Result<String, String> {
    // 1. Probe total duration so we can calculate percentages.
    let total_us = probe_duration(ffprobe_path, source_path).await?;

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
        // CONCEPT: Stdio piping
        // We redirect stderr to a pipe so we can read it asynchronously.
        // stdout is left inherited (FFmpeg writes nothing useful there).
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;

    // 4. Take ownership of the stderr pipe.
    // CONCEPT: `.take()` and Option
    // `child.stderr` is an `Option<ChildStderr>`.  `.take()` moves the value
    // out and replaces it with `None`, giving us exclusive ownership of the
    // pipe while letting `child` continue running.
    let stderr = child.stderr.take()
        .ok_or_else(|| "Failed to capture FFmpeg stderr".to_string())?;

    // 5. Wrap the raw pipe in a buffered, async line reader.
    // CONCEPT: BufReader
    // Reading a byte at a time from a pipe is slow. `BufReader` batches
    // reads into a buffer.  `AsyncBufReadExt::read_line()` then lets us
    // process the stream one line at a time without blocking.
    let mut reader = BufReader::new(stderr).lines();
    let mut progress = FfmpegProgress::new(total_us);

    // 6. Read lines from stderr as they arrive.
    // CONCEPT: `while let` with async
    // `.next_line().await` suspends this task until a new line is available.
    // When FFmpeg exits, it returns `Ok(None)` and the loop ends.
    while let Ok(Some(line)) = reader.next_line().await {
        if let Some(pct) = progress.parse_line(&line) {
            // CONCEPT: Tauri emit()
            // Push the percentage to the frontend in real-time.
            // The frontend listens with `listen("conversion-progress", ...)`.
            // We intentionally ignore emit errors (e.g. if the window closed).
            let _ = app_handle.emit("conversion-progress", pct);
        }
    }

    // 7. Wait for FFmpeg to fully exit and check the exit code.
    let status = child.wait().await
        .map_err(|e| format!("Failed to wait for FFmpeg: {}", e))?;

    if !status.success() {
        return Err(format!(
            "FFmpeg exited with code {:?}",
            status.code()
        ));
    }

    // 8. Final 100% event in case FFmpeg didn't emit `progress=end`.
    let _ = app_handle.emit("conversion-progress", 100.0_f64);

    Ok(output_str)
}
