// ============================================================
// engines.rs — ConversionEngine trait and engine tag enum
// ============================================================

// CONCEPT: Traits — Rust's version of interfaces
// A `trait` defines a set of methods that a type *must* implement.
// Any struct that implements `ConversionEngine` promises to provide a
// `convert()` method with the exact signature below.
//
// This is the same idea as:
//   • An `interface` in TypeScript / Java
//   • An `abstract base class` in Python
//
// The key advantage: once the trait exists, we can write functions that accept
// *any* type that implements it, without caring which engine it actually is.

use serde::{Deserialize, Serialize};

/// Which back-end engine is responsible for a given conversion.
///
/// We tag every conversion entry with one of these so the UI can show users
/// what software powers each route, and so we can dispatch correctly later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Engine {
    /// Pure-Rust `image` crate — no external binary required.
    ImageCrate,
    /// Calls FFmpeg as an async subprocess (Phase 6).
    Ffmpeg,
    /// Calls LibreOffice in headless mode (Phase 7).
    LibreOffice,
    /// Calls Pandoc as a subprocess (Phase 7).
    Pandoc,
}

impl Engine {
    /// Human-readable name shown in the UI.
    pub fn display_name(&self) -> &'static str {
        match self {
            Engine::ImageCrate  => "Image Crate (native)",
            Engine::Ffmpeg      => "FFmpeg",
            Engine::LibreOffice => "LibreOffice Headless",
            Engine::Pandoc      => "Pandoc",
        }
    }
}

/// The shared interface every conversion engine must satisfy.
///
/// CONCEPT: `async` in traits
/// Async functions in traits require a little extra setup. For now we define the
/// trait with a regular signature — the async machinery (tokio runtime) will be
/// wired in when we implement the trait on concrete structs in later phases.
///
/// CONCEPT: `Box<dyn std::error::Error + Send + Sync>`
/// This is how Rust does "any error type". Breaking it down:
///   - `dyn std::error::Error` → *any* type that implements the `Error` trait.
///   - `Box<...>`              → heap-allocated because the size isn't known at compile time.
///   - `+ Send + Sync`         → the error can be safely shared across async threads.
pub trait ConversionEngine {
    /// Convert the file at `source_path` into `target_format`, writing the
    /// result to a path derived from `output_dir`.
    ///
    /// # Arguments
    /// - `source_path`   — absolute path to the input file on disk.
    /// - `target_format` — the format we want to produce (as an extension string, e.g. "pdf").
    /// - `output_dir`    — folder where the converted file should land.
    ///
    /// # Returns
    /// `Ok(output_path)` — the absolute path of the converted file on success.
    /// `Err(message)`    — a descriptive error string on failure.
    fn convert(
        &self,
        source_path: &str,
        target_format: &str,
        output_dir: &str,
    ) -> Result<String, String>;
}
