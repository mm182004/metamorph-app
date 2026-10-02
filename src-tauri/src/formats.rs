// ============================================================
// formats.rs — FileFormat enum and conversions
// ============================================================

// CONCEPT: Enums with variants
// In Rust, `enum` is far more powerful than in most languages.
// Each variant here represents a distinct file format that MetaMorph
// understands. By listing formats in an enum, the Rust compiler forces us
// to handle every possible case explicitly — no forgotten branches allowed.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Every file format that MetaMorph can read or write.
///
/// CONCEPT: `#[derive(...)]`
/// - `Debug`     → lets us print the value with `{:?}` for debugging.
/// - `Clone`     → allows Rust to copy the value when needed (enums don't copy by default).
/// - `PartialEq` → lets us compare two `FileFormat` values with `==`.
/// - `Serialize` / `Deserialize` → lets serde convert between Rust and JSON so Tauri
///   can send these values to/from the React frontend automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileFormat {
    // ── Images ──────────────────────────────────────────────
    Jpeg,
    Png,
    WebP,
    Gif,
    Bmp,
    Tiff,

    // ── Video ────────────────────────────────────────────────
    Mp4,
    Mkv,
    Mov,
    Avi,
    WebM,

    // ── Audio ────────────────────────────────────────────────
    Mp3,
    M4a,
    Wav,
    Flac,
    Ogg,

    // ── Documents ────────────────────────────────────────────
    Pdf,
    Docx,
    Odt,

    // ── Text / Markup ────────────────────────────────────────
    Txt,
    Markdown,
    Html,
}

impl FileFormat {
    /// Parse a file-extension string (e.g. "jpg", "mp4") into a `FileFormat`.
    ///
    /// CONCEPT: `Option<T>` — the Rust alternative to `null`
    /// Instead of returning `null` on failure (which causes NullPointerExceptions in other
    /// languages), Rust uses `Option<T>`:
    ///   - `Some(value)` → we got a valid match.
    ///   - `None`        → no match — the caller must decide what to do.
    pub fn from_extension(ext: &str) -> Option<Self> {
        // `.to_lowercase()` returns an owned `String`; we borrow it with `.as_str()`
        // so the `match` arm patterns (`"jpg"`, etc.) can compare against it cheaply.
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some(FileFormat::Jpeg),
            "png"          => Some(FileFormat::Png),
            "webp"         => Some(FileFormat::WebP),
            "gif"          => Some(FileFormat::Gif),
            "bmp"          => Some(FileFormat::Bmp),
            "tiff" | "tif" => Some(FileFormat::Tiff),

            "mp4"          => Some(FileFormat::Mp4),
            "mkv"          => Some(FileFormat::Mkv),
            "mov"          => Some(FileFormat::Mov),
            "avi"          => Some(FileFormat::Avi),
            "webm"         => Some(FileFormat::WebM),

            "mp3"          => Some(FileFormat::Mp3),
            "m4a"          => Some(FileFormat::M4a),
            "wav"          => Some(FileFormat::Wav),
            "flac"         => Some(FileFormat::Flac),
            "ogg"          => Some(FileFormat::Ogg),

            "pdf"          => Some(FileFormat::Pdf),
            "docx"         => Some(FileFormat::Docx),
            "odt"          => Some(FileFormat::Odt),

            "txt"          => Some(FileFormat::Txt),
            "md"           => Some(FileFormat::Markdown),
            "html" | "htm" => Some(FileFormat::Html),

            _              => None, // Unknown or unsupported extension
        }
    }

    /// Return the canonical file extension string for this format (without the dot).
    pub fn extension(&self) -> &'static str {
        // CONCEPT: Lifetime annotation `'static`
        // `&'static str` is a string slice that lives for the entire life of the program.
        // String literals like `"jpg"` are compiled into the binary itself and are always valid.
        match self {
            FileFormat::Jpeg     => "jpg",
            FileFormat::Png      => "png",
            FileFormat::WebP     => "webp",
            FileFormat::Gif      => "gif",
            FileFormat::Bmp      => "bmp",
            FileFormat::Tiff     => "tiff",

            FileFormat::Mp4      => "mp4",
            FileFormat::Mkv      => "mkv",
            FileFormat::Mov      => "mov",
            FileFormat::Avi      => "avi",
            FileFormat::WebM     => "webm",

            FileFormat::Mp3      => "mp3",
            FileFormat::M4a      => "m4a",
            FileFormat::Wav      => "wav",
            FileFormat::Flac     => "flac",
            FileFormat::Ogg      => "ogg",

            FileFormat::Pdf      => "pdf",
            FileFormat::Docx     => "docx",
            FileFormat::Odt      => "odt",

            FileFormat::Txt      => "txt",
            FileFormat::Markdown => "md",
            FileFormat::Html     => "html",
        }
    }

    /// Human-readable display name for the UI.
    pub fn display_name(&self) -> &'static str {
        match self {
            FileFormat::Jpeg     => "JPEG Image",
            FileFormat::Png      => "PNG Image",
            FileFormat::WebP     => "WebP Image",
            FileFormat::Gif      => "GIF Image",
            FileFormat::Bmp      => "Bitmap Image",
            FileFormat::Tiff     => "TIFF Image",

            FileFormat::Mp4      => "MP4 Video",
            FileFormat::Mkv      => "Matroska Video",
            FileFormat::Mov      => "QuickTime Video",
            FileFormat::Avi      => "AVI Video",
            FileFormat::WebM     => "WebM Video",

            FileFormat::Mp3      => "MP3 Audio",
            FileFormat::M4a      => "M4A Audio",
            FileFormat::Wav      => "WAV Audio",
            FileFormat::Flac     => "FLAC Audio",
            FileFormat::Ogg      => "OGG Audio",

            FileFormat::Pdf      => "PDF Document",
            FileFormat::Docx     => "Word Document",
            FileFormat::Odt      => "OpenDocument Text",

            FileFormat::Txt      => "Plain Text",
            FileFormat::Markdown => "Markdown",
            FileFormat::Html     => "HTML Document",
        }
    }
}

// CONCEPT: Implementing a standard Trait — `fmt::Display`
// Traits are like interfaces in other languages. By implementing `Display`,
// we make `FileFormat` printable with `{}` (e.g., in `format!` or `println!`).
impl fmt::Display for FileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.extension())
    }
}
