// ============================================================
// pandoc.rs — Pandoc universal document conversion engine
// ============================================================
//
// Pandoc is a command-line tool that converts between markup and
// document formats. It reads from many formats (markdown, HTML,
// docx, odt, rst, LaTeX, ...) and writes to many others.
//
// Key difference from LibreOffice:
// - LibreOffice is best for rendered documents (preserves formatting)
// - Pandoc is best for text/markup transformations (understands content)
//
// For PDF output, Pandoc needs a LaTeX engine installed (like MiKTeX
// on Windows or MacTeX on macOS).  As a fallback we can route PDF
// generation through LibreOffice instead.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Detect the Pandoc binary on the current system.
fn find_pandoc() -> Option<String> {
    // Try standard PATH first
    if Command::new("pandoc").arg("--version").output().is_ok() {
        return Some("pandoc".to_string());
    }

    // Common Windows install path (Pandoc's MSI installer default)
    let win_path = r"C:\Program Files\Pandoc\pandoc.exe";
    if Path::new(win_path).exists() {
        return Some(win_path.to_string());
    }

    None
}

/// Map source extension → Pandoc's `-f` (from) format name.
fn pandoc_from_format(ext: &str) -> Option<&'static str> {
    match ext {
        "md"   => Some("markdown"),
        "txt"  => Some("plain"),
        "html" => Some("html"),
        "docx" => Some("docx"),
        "odt"  => Some("odt"),
        "pdf"  => Some("pdf"),   // Pandoc can extract text from PDF
        _      => None,
    }
}

/// Map target extension → Pandoc's `-t` (to) format name.
fn pandoc_to_format(ext: &str) -> Option<&'static str> {
    match ext {
        "md"   => Some("gfm"),        // GitHub Flavored Markdown
        "txt"  => Some("plain"),
        "html" => Some("html5"),
        "docx" => Some("docx"),
        "odt"  => Some("odt"),
        "pdf"  => Some("pdf"),
        _      => None,
    }
}

/// Build Pandoc argument list.
///
/// CONCEPT: Vec<String> for subprocess arguments
/// We collect arguments as an owned `Vec<String>`.  This lets us
/// conditionally push extra flags (like `--pdf-engine`) without
/// awkward if/else chains inside `Command::args()`.
fn build_pandoc_args(
    source_path: &str,
    from_fmt: &str,
    to_fmt: &str,
    output_path: &str,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        source_path.into(),
        "-f".into(), from_fmt.into(),    // input format
        "-t".into(), to_fmt.into(),      // output format
        "-o".into(), output_path.into(), // output file
        "--standalone".into(),           // produce a complete document (not a fragment)
    ];

    // For PDF output, try using wkhtmltopdf or lualatex if available,
    // otherwise fall back to the default engine.
    // The `--pdf-engine` flag is optional; if omitted Pandoc tries to
    // find one itself.
    if to_fmt == "pdf" {
        // Try wkhtmltopdf first (common on Windows, no full LaTeX install needed)
        if Command::new("wkhtmltopdf").arg("--version").output().is_ok() {
            args.extend(["--pdf-engine", "wkhtmltopdf"].map(String::from));
        }
        // Otherwise let Pandoc pick whatever it finds (lualatex, pdflatex, etc.)
    }

    args
}

/// Convert a document using Pandoc.
///
/// # Arguments
/// - `source_path` — absolute path to the source file.
/// - `source_ext`  — source file extension (e.g. `"md"`, `"docx"`).
/// - `target_ext`  — desired output format (e.g. `"pdf"`, `"html"`).
/// - `output_dir`  — directory where the output file will be written.
pub async fn run_pandoc(
    source_path: &str,
    source_ext: &str,
    target_ext: &str,
    output_dir: &str,
) -> Result<String, String> {
    // 1. Locate Pandoc
    let pandoc_bin = find_pandoc()
        .ok_or_else(|| "Pandoc not found. Please install Pandoc (pandoc.org).".to_string())?;

    // 2. Look up format strings
    let from_fmt = pandoc_from_format(source_ext)
        .ok_or_else(|| format!("Pandoc does not support reading format: {}", source_ext))?;
    let to_fmt = pandoc_to_format(target_ext)
        .ok_or_else(|| format!("Pandoc does not support writing format: {}", target_ext))?;

    // 3. Build output path
    let source_stem = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let output_path: PathBuf = Path::new(output_dir)
        .join(format!("{}.{}", source_stem, target_ext));
    let output_str = output_path.to_string_lossy().to_string();

    // 4. Move owned values into the blocking closure
    let pandoc_bin = pandoc_bin.clone();
    let from_fmt = from_fmt.to_string();
    let to_fmt = to_fmt.to_string();
    let source_path = source_path.to_string();
    let output_str_clone = output_str.clone();

    // 5. Run on a blocking thread
    tokio::task::spawn_blocking(move || {
        let args = build_pandoc_args(&source_path, &from_fmt, &to_fmt, &output_str_clone);

        let output = Command::new(&pandoc_bin)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to start Pandoc: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Pandoc failed: {}", stderr.trim()));
        }

        // Confirm the output file was created
        if !Path::new(&output_str_clone).exists() {
            return Err(format!(
                "Pandoc ran but output file not found at: {}",
                output_str_clone
            ));
        }

        Ok(output_str_clone)
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {}", e))?
}
