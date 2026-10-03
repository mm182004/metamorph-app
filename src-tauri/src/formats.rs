// ============================================================
// formats.rs — FileFormat enum and conversions
// ============================================================

use serde::{Deserialize, Serialize};
use std::fmt;

/// Every file format that MetaMorph can read or write.
///
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
}

impl fmt::Display for FileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.extension())
    }
}
