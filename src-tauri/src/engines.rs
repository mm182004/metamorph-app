use serde::{Deserialize, Serialize};

/// Which back-end engine is responsible for a given conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Engine {
    /// Pure-Rust `image` crate — no external binary required.
    ImageCrate,
    /// Calls FFmpeg as an async subprocess.
    Ffmpeg,
    /// Calls LibreOffice in headless mode.
    LibreOffice,
    /// Calls Pandoc as a subprocess.
    Pandoc,
}

/// Convert an image using the pure-Rust `image` crate.
pub fn convert_image(
    source_path: &str,
    target_format: &str,
    output_dir: &str,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    use std::path::Path;
    use image::ImageFormat;
    use tauri::Emitter;
    use crate::ProgressPayload;

    let _ = app_handle.emit("conversion-progress", ProgressPayload {
        percentage: 10.0,
        message: format!("Preparing to convert to {}...", target_format.to_uppercase()),
    });

    // 1. Map the target string ("png", "jpg") to the image crate's format enum.
    let output_format = match target_format.to_lowercase().as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png"          => ImageFormat::Png,
        "webp"         => ImageFormat::WebP,
        "gif"          => ImageFormat::Gif,
        "bmp"          => ImageFormat::Bmp,
        "tiff" | "tif" => ImageFormat::Tiff,
        _ => return Err(format!("Unsupported target image format: {}", target_format)),
    };

    // 2. Construct the output file path.
    let source_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");

    let output_filename = format!("{}.{}", source_name, target_format);
    let output_path = Path::new(output_dir).join(output_filename);

    let _ = app_handle.emit("conversion-progress", ProgressPayload {
        percentage: 40.0,
        message: "Decoding source image into memory...".to_string(),
    });

    // 3. Open and decode the source image into memory.
    let img = image::open(source_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;

    let _ = app_handle.emit("conversion-progress", ProgressPayload {
        percentage: 70.0,
        message: format!("Encoding image as {}...", target_format.to_uppercase()),
    });

    // 4. Encode and save the image to the target path.
    img.save_with_format(&output_path, output_format)
        .map_err(|e| format!("Failed to save image: {}", e))?;

    // 5. Success! Return the final absolute path.
    Ok(output_path.to_string_lossy().to_string())
}
