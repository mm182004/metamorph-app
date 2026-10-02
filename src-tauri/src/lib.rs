// #[tauri::command] is a procedural macro that transforms a standard Rust function
// into an invokable command accessible from the web frontend via `@tauri-apps/api/core`.
#[tauri::command]
// CONCEPT: Borrowing vs Ownership
// `name: &str` borrows a string slice (read-only reference) without taking ownership of the memory.
// `-> String` returns an owned, heap-allocated `String` that is serialized to JSON and sent to the frontend.
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // CONCEPT: Builder Pattern
    // Rust frequently uses builder patterns (`tauri::Builder::default()...`) to construct complex structs
    // step-by-step with type safety instead of constructors with dozens of optional arguments.
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Register commands so the frontend can call them with `invoke("greet", { name: "..." })`
        .invoke_handler(tauri::generate_handler![greet])
        // CONCEPT: Result & .expect()
        // `run()` returns a `Result<(), tauri::Error>`. Rust doesn't have exceptions like JS or Python;
        // errors are represented as values (`Result::Ok` or `Result::Err`).
        // `.expect("...")` unpacks the Ok value or halts (panics) if an error occurred during startup.
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

