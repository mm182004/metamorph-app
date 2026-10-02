// ============================================================
// registry.rs — Conversion registry: source → valid targets
// ============================================================

// CONCEPT: Modules importing from sibling modules
// `use crate::formats::FileFormat` imports the enum we defined in formats.rs.
// `crate::` is Rust's way of referring to "the root of *this* crate (project)".
use crate::engines::Engine;
use crate::formats::FileFormat;

use serde::{Deserialize, Serialize};

/// A single entry in the registry:  one valid target for a given source format.
///
/// CONCEPT: Structs
/// A `struct` groups related data together, like an object in JS/Python.
/// This one pairs a target format with the engine that handles the conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionTarget {
    /// The format we can convert *into*.
    pub format: FileFormat,
    /// Which engine will carry out the conversion.
    pub engine: Engine,
    /// Whether the necessary external binary (ffmpeg, libreoffice, pandoc) has
    /// been confirmed present. Defaults to `true` for native (image crate) conversions.
    pub available: bool,
}

impl ConversionTarget {
    /// Convenience constructor.
    fn new(format: FileFormat, engine: Engine) -> Self {
        // All entries start as "available"; the first-run setup phase (Phase 9)
        // will flip `available = false` for entries whose binary is missing.
        Self { format, engine, available: true }
    }
}

/// Look up all valid conversion targets for a given source format.
///
/// CONCEPT: Returning owned `Vec<T>` vs. borrowing `&[T]`
/// We return a `Vec<ConversionTarget>` (an owned, growable list on the heap).
/// This is the easiest approach because the list is built fresh each call and
/// the caller (a Tauri command) needs to own the data to serialize it as JSON.
///
/// The `match` arms group formats by category, each spelling out exactly which
/// targets are valid and which engine handles them.
pub fn get_conversion_targets(source: &FileFormat) -> Vec<ConversionTarget> {
    match source {
        // ── Images ──────────────────────────────────────────────────────────
        FileFormat::Jpeg => vec![
            ConversionTarget::new(FileFormat::Png,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::WebP,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Bmp,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Tiff,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Gif,      Engine::ImageCrate),
        ],
        FileFormat::Png => vec![
            ConversionTarget::new(FileFormat::Jpeg,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::WebP,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Bmp,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Tiff,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Gif,      Engine::ImageCrate),
        ],
        FileFormat::WebP => vec![
            ConversionTarget::new(FileFormat::Png,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Jpeg,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Bmp,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Tiff,     Engine::ImageCrate),
        ],
        FileFormat::Gif => vec![
            ConversionTarget::new(FileFormat::Png,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Jpeg,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::WebP,     Engine::ImageCrate),
        ],
        FileFormat::Bmp => vec![
            ConversionTarget::new(FileFormat::Png,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Jpeg,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::WebP,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Tiff,     Engine::ImageCrate),
        ],
        FileFormat::Tiff => vec![
            ConversionTarget::new(FileFormat::Png,      Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Jpeg,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::WebP,     Engine::ImageCrate),
            ConversionTarget::new(FileFormat::Bmp,      Engine::ImageCrate),
        ],

        // ── Video ────────────────────────────────────────────────────────────
        FileFormat::Mp4 => vec![
            ConversionTarget::new(FileFormat::Mkv,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mov,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::WebM,     Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Avi,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg), // extract audio
            ConversionTarget::new(FileFormat::Wav,      Engine::Ffmpeg),
        ],
        FileFormat::Mkv => vec![
            ConversionTarget::new(FileFormat::Mp4,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mov,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::WebM,     Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Wav,      Engine::Ffmpeg),
        ],
        FileFormat::Mov => vec![
            ConversionTarget::new(FileFormat::Mp4,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mkv,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::WebM,     Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
        ],
        FileFormat::Avi => vec![
            ConversionTarget::new(FileFormat::Mp4,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mkv,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::WebM,     Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
        ],
        FileFormat::WebM => vec![
            ConversionTarget::new(FileFormat::Mp4,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Mkv,      Engine::Ffmpeg),
        ],

        // ── Audio ────────────────────────────────────────────────────────────
        FileFormat::Mp3 => vec![
            ConversionTarget::new(FileFormat::Wav,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Flac,     Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::M4a,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Ogg,      Engine::Ffmpeg),
        ],
        FileFormat::Wav => vec![
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Flac,     Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::M4a,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Ogg,      Engine::Ffmpeg),
        ],
        FileFormat::Flac => vec![
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Wav,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::M4a,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Ogg,      Engine::Ffmpeg),
        ],
        FileFormat::M4a => vec![
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Wav,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Flac,     Engine::Ffmpeg),
        ],
        FileFormat::Ogg => vec![
            ConversionTarget::new(FileFormat::Mp3,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Wav,      Engine::Ffmpeg),
            ConversionTarget::new(FileFormat::Flac,     Engine::Ffmpeg),
        ],

        // ── Documents ────────────────────────────────────────────────────────
        FileFormat::Docx => vec![
            ConversionTarget::new(FileFormat::Pdf,      Engine::LibreOffice),
            ConversionTarget::new(FileFormat::Odt,      Engine::LibreOffice),
            ConversionTarget::new(FileFormat::Html,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Markdown, Engine::Pandoc),
            ConversionTarget::new(FileFormat::Txt,      Engine::Pandoc),
        ],
        FileFormat::Odt => vec![
            ConversionTarget::new(FileFormat::Pdf,      Engine::LibreOffice),
            ConversionTarget::new(FileFormat::Docx,     Engine::LibreOffice),
            ConversionTarget::new(FileFormat::Html,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Txt,      Engine::Pandoc),
        ],
        FileFormat::Pdf => vec![
            // PDF → text extraction only; reconstruction is lossy and needs Pandoc
            ConversionTarget::new(FileFormat::Txt,      Engine::Pandoc),
            ConversionTarget::new(FileFormat::Html,     Engine::Pandoc),
        ],

        // ── Text / Markup ────────────────────────────────────────────────────
        // This is exactly what you noted: .txt → .md is trivial, but .txt → .pdf
        // or .txt → .docx are *real* document compilations powered by Pandoc.
        FileFormat::Txt => vec![
            ConversionTarget::new(FileFormat::Pdf,      Engine::Pandoc),
            ConversionTarget::new(FileFormat::Docx,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Html,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Markdown, Engine::Pandoc),
            ConversionTarget::new(FileFormat::Odt,      Engine::Pandoc),
        ],
        FileFormat::Markdown => vec![
            ConversionTarget::new(FileFormat::Pdf,      Engine::Pandoc),
            ConversionTarget::new(FileFormat::Docx,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Html,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Odt,      Engine::Pandoc),
            ConversionTarget::new(FileFormat::Txt,      Engine::Pandoc),
        ],
        FileFormat::Html => vec![
            ConversionTarget::new(FileFormat::Pdf,      Engine::Pandoc),
            ConversionTarget::new(FileFormat::Docx,     Engine::Pandoc),
            ConversionTarget::new(FileFormat::Markdown, Engine::Pandoc),
            ConversionTarget::new(FileFormat::Odt,      Engine::Pandoc),
            ConversionTarget::new(FileFormat::Txt,      Engine::Pandoc),
        ],
    }
}
