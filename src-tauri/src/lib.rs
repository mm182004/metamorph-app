use serde::Serialize;

// CONCEPT: Serde Serialization
// `#[derive(Serialize)]` instructs the `serde` crate to generate code at compile time
// that can convert this Rust struct into JSON. Tauri automatically calls this when sending
// data from Rust back across the IPC boundary to React.
#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub status: String,
}

// CONCEPT: #[tauri::command]
// Transforms this function into an asynchronous Tauri IPC command callable by React via `invoke()`.
#[tauri::command]
fn get_app_info() -> AppInfo {
    // CONCEPT: Ownership & Allocation
    // `to_string()` creates a new heap-allocated `String` owned by this `AppInfo` instance.
    AppInfo {
        name: "MetaMorph".to_string(),
        version: "0.1.0".to_string(),
        status: "Native Rust Engine Ready".to_string(),
    }
}

// CONCEPT: Borrowing references (&str)
// `target_format: &str` borrows an immutable reference to the string sent by JavaScript.
// We don't need to take ownership of it because we're only reading it to build our response.
#[tauri::command]
fn ping_engine(target_format: &str) -> String {
    // `format!` macro allocates a new `String` with formatted text.
    format!("Rust engine acknowledged target format: {target_format}")
}

// Legacy command retained for verification
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

// CONCEPT: Result<T, E> Error Handling
// Instead of crashing (panicking) when a file is unreadable or unknown, we return a `Result`.
// `Result<String, String>` means: on success, return the extension as a `String` (the `Ok` variant).
// On failure, return an error message as a `String` (the `Err` variant).
// Tauri automatically converts `Ok` into a resolved Promise and `Err` into a rejected Promise in JavaScript.
#[tauri::command]
fn detect_file_type(path: &str) -> Result<String, String> {
    // `infer::get_from_path` reads the first few bytes (magic bytes) of the file safely.
    // We use the `match` control flow operator to handle the outcome explicitly.
    match infer::get_from_path(path) {
        // If it successfully read the file and found a match:
        Ok(Some(kind)) => Ok(kind.extension().to_string()),
        // If it read the file but didn't recognize the magic bytes:
        Ok(None) => Err("Unknown file format".to_string()),
        // If reading the file from disk failed (e.g., permissions issue or file deleted):
        // `e` is the std::io::Error. We use `.to_string()` to send the error text to the frontend.
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // CONCEPT: Builder Pattern
    // Assembles plugins, handlers, and runtime options with compile-time type checks.
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Register all invokable commands so the React frontend can call them with `invoke("command_name", { ... })`
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            ping_engine,
            greet,
            detect_file_type
        ])
        .run(tauri::generate_context!())
        // CONCEPT: Result Handling
        // .expect() unboxes Ok(()) or panics with an informative message if Tauri fails to initialize.
        .expect("error while running tauri application");
}
