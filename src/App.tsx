import { useState, useEffect, useRef } from "react";
// CONCEPT: Tauri IPC (Inter-Process Communication)
// `invoke` is an asynchronous function provided by Tauri that bridges the JavaScript frontend
// to Rust functions annotated with `#[tauri::command]`. It returns a standard Promise.
import { invoke } from "@tauri-apps/api/core";
// CONCEPT: Tauri Event System (Push-based, not Polling)
// `listen()` registers a callback that fires whenever the Rust backend calls
// `app_handle.emit("event-name", payload)`.  This is how FFmpeg streams
// progress percentages to the UI in real-time without us having to ask.
import { listen } from "@tauri-apps/api/event";
// CONCEPT: Native Desktop Webview Events
// Desktop apps need real filesystem paths (e.g. "C:\Users\...\file.png"), which browsers hide for security.
// Tauri's webview module emits native drag-and-drop events containing real disk paths.
import { getCurrentWebview } from "@tauri-apps/api/webview";
import "./App.css";

interface AppInfo {
  name: string;
  version: string;
  status: string;
}

interface DroppedFile {
  name: string;
  path: string;
  size?: number;
  extension: string;
}

// Mirror of the Rust `ConversionTarget` struct from registry.rs.
// The field names use snake_case to match Rust's JSON serialisation.
interface ConversionTarget {
  // Rust's serde serialises unit enum variants (no data) as plain JSON strings by default.
  // e.g. FileFormat::Png → "Png",  Engine::ImageCrate → "ImageCrate"
  format: string;
  engine: string;
  available: boolean;
  // Helpers we derive when parsing
  targetExt?: string;
  engineName?: string;
}

export function App() {
  // State management
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [selectedFile, setSelectedFile] = useState<DroppedFile | null>(null);
  const [targetFormat, setTargetFormat] = useState("");
  // Now holds full ConversionTarget objects from the Rust registry
  const [availableTargets, setAvailableTargets] = useState<ConversionTarget[]>([]);
  const [progress, setProgress] = useState(0);
  const [statusMessage, setStatusMessage] = useState("Ready — drag and drop a file to begin");
  const [isConverting, setIsConverting] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);

  // 1. Fetch backend status from Rust on startup via invoke()
  useEffect(() => {
    async function fetchBackendInfo() {
      try {
        // Calls the Rust command `get_app_info()` defined in src-tauri/src/lib.rs
        const info = await invoke<AppInfo>("get_app_info");
        setAppInfo(info);
      } catch (err) {
        console.error("Failed to connect to Rust backend:", err);
      }
    }
    fetchBackendInfo();
  }, []);

  // 2. Set up Native Tauri Drag-and-Drop event listener
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    try {
      // Listen to native window drag/drop events from the OS
      getCurrentWebview()
        .onDragDropEvent((event) => {
          if (event.payload.type === "over" || event.payload.type === "enter") {
            setIsDragging(true);
          } else if (event.payload.type === "leave") {
            setIsDragging(false);
          } else if (event.payload.type === "drop") {
            setIsDragging(false);
            const paths = event.payload.paths;
            if (paths && paths.length > 0) {
              handleFilePath(paths[0]);
            }
          }
        })
        .then((unlistenFn) => {
          unlisten = unlistenFn;
        });
    } catch (e) {
      console.warn("Native webview drag-drop unavailable (browser preview mode):", e);
    }

    // Cleanup: In React, always unregister listeners when the component unmounts
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Process a selected file path (from native drop or file input)
  async function handleFilePath(fullPath: string, size?: number) {
    // Extract file name
    const normalized = fullPath.replace(/\\/g, "/");
    const name = normalized.split("/").pop() || fullPath;

    // Show temporary loading state
    setSelectedFile({ name, path: fullPath, size, extension: "..." });
    setStatusMessage("Detecting true file type using magic bytes...");

    try {
      // Step 1: Ask Rust to identify the real format from magic bytes.
      const detectedExt = await invoke<string>("detect_file_type", { path: fullPath });

      setSelectedFile({ name, path: fullPath, size, extension: detectedExt });

      // Step 2: Ask the Rust conversion registry for the list of valid targets.
      // This returns ConversionTarget[] — a richer structure than plain strings.
      const rawTargets = await invoke<ConversionTarget[]>("get_targets", { sourceExt: detectedExt });

      // Step 3: Derive human-readable extensions from Rust enum variant names.
      // Most variant names lowercase directly to the extension (e.g. "Png" → "png"),
      // but a few differ and need an explicit mapping.
      const variantToExt: Record<string, string> = {
        jpeg: "jpg",
        webp: "webp",
        markdown: "md",
        m4a: "m4a",
        flac: "flac",
        webm: "webm",
      };
      const targets = rawTargets.map((t) => {
        const lower = t.format.toLowerCase();
        return {
          ...t,
          targetExt: variantToExt[lower] ?? lower,
          engineName: t.engine,
        };
      });

      setAvailableTargets(targets);
      setTargetFormat(targets.length > 0 ? targets[0].targetExt! : "");
      setProgress(0);
      setStatusMessage(`Detected as .${detectedExt} — ${targets.length} target format(s) available.`);
    } catch (error) {
      console.error("File detection failed:", error);
      setSelectedFile({ name, path: fullPath, size, extension: "Unknown" });
      setAvailableTargets([]);
      setTargetFormat("");
      setStatusMessage(`Error: ${error}`);
    }
  }

  // HTML5 Drag-and-drop fallback handlers
  function onHtmlDragOver(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(true);
  }

  function onHtmlDragLeave(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(false);
  }

  function onHtmlDrop(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const file = e.dataTransfer.files[0];
      // In web fallback, we might not get full system path, so we use file.name
      handleFilePath(file.name, file.size);
    }
  }

  // File browser input handler
  function handleFileInputChange(e: React.ChangeEvent<HTMLInputElement>) {
    if (e.target.files && e.target.files.length > 0) {
      const file = e.target.files[0];
      handleFilePath(file.name, file.size);
    }
  }

  // Clear selected file
  function handleClearFile() {
    setSelectedFile(null);
    setAvailableTargets([]);
    setTargetFormat("");
    setProgress(0);
    setStatusMessage("Ready — drag and drop a file to begin");
  }

  // Handle conversion trigger (calls Rust via invoke)
  async function handleConvert() {
    if (!selectedFile || !targetFormat) return;

    setIsConverting(true);
    setProgress(0);
    setStatusMessage(`Converting to ${targetFormat.toUpperCase()}...`);

    // Subscribe to real-time progress events from Rust (FFmpeg conversions).
    // `listen()` returns an `unlisten` function we call when done to avoid leaks.
    // For image conversions this won't fire (they complete instantly), but it's
    // harmless to have the listener in place for all conversions.
    const unlisten = await listen<number>("conversion-progress", (event) => {
      const pct = Math.round(event.payload);
      setProgress(pct);
      setStatusMessage(`Converting... ${pct}%`);
    });

    try {
      const outputPath = await invoke<string>("convert_file", {
        sourcePath: selectedFile.path,
        sourceExt: selectedFile.extension,
        targetExt: targetFormat,
      });

      setProgress(100);
      setIsConverting(false);
      setStatusMessage(`Success! Saved to: ${outputPath}`);
    } catch (err) {
      console.error("Conversion error:", err);
      setIsConverting(false);
      setStatusMessage(`Error: ${String(err)}`);
    } finally {
      // Always clean up the event listener, whether we succeeded or failed.
      unlisten();
    }
  }

  // Format file size nicely
  function formatSize(bytes?: number): string {
    if (!bytes) return "Local file";
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    return `${(kb / 1024).toFixed(1)} MB`;
  }

  return (
    <div
      className={`app-container ${isDragging ? "dragging-over" : ""}`}
      onDragOver={onHtmlDragOver}
      onDragLeave={onHtmlDragLeave}
      onDrop={onHtmlDrop}
    >
      {/* Hidden file input for click-to-browse */}
      <input
        type="file"
        ref={fileInputRef}
        onChange={handleFileInputChange}
        style={{ display: "none" }}
      />

      {/* App Header */}
      <header className="app-header">
        <div className="brand">
          <div className="logo-badge">M</div>
          <div className="brand-text">
            <h1>MetaMorph</h1>
            <p>Native Desktop File Converter</p>
          </div>
        </div>

        <div className="backend-status">
          <span className={`status-dot ${appInfo ? "active" : ""}`} />
          <span>{appInfo ? appInfo.status : "Connecting to Rust..."}</span>
        </div>
      </header>

      {/* Main Workspace */}
      <main className="workspace">
        {/* Full Drop Zone / File Card */}
        {!selectedFile ? (
          <div
            className={`dropzone-container ${isDragging ? "active" : ""}`}
            onClick={() => fileInputRef.current?.click()}
          >
            <div className="dropzone-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="17 8 12 3 7 8" />
                <line x1="12" y1="3" x2="12" y2="15" />
              </svg>
            </div>
            <h2 className="dropzone-title">
              {isDragging ? "Release to drop file" : "Drop any file to convert"}
            </h2>
            <p className="dropzone-subtitle">
              Images, videos, audio, and documents · <span className="browse-link">browse files</span>
            </p>
          </div>
        ) : (
          <div className="dropzone-container" style={{ cursor: "default" }}>
            <div className="file-card">
              <div className="file-info">
                <div className="file-icon">{selectedFile.extension || "FILE"}</div>
                <div className="file-details">
                  <div className="file-name" title={selectedFile.name}>
                    {selectedFile.name}
                  </div>
                  <div className="file-meta" title={selectedFile.path}>
                    {formatSize(selectedFile.size)} · {selectedFile.path}
                  </div>
                </div>
              </div>
              <button
                className="remove-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  handleClearFile();
                }}
                title="Remove file"
              >
                ✕
              </button>
            </div>
          </div>
        )}

        {/* Controls: Format picker dropdown & Action */}
        <section className="control-panel">
          <div className="select-group">
            <label className="select-label" htmlFor="format-picker">
              Target Format
            </label>
            <select
              id="format-picker"
              className="format-select"
              value={targetFormat}
              disabled={!selectedFile || availableTargets.length === 0 || isConverting}
              onChange={(e) => setTargetFormat(e.target.value)}
            >
              {!selectedFile ? (
                <option value="">Drop a file to reveal available formats...</option>
              ) : availableTargets.length === 0 ? (
                <option value="">No valid target formats available</option>
              ) : (
                availableTargets.map((t) => (
                  <option key={t.targetExt} value={t.targetExt}>
                    .{t.targetExt!.toUpperCase()} — via {t.engineName}
                  </option>
                ))
              )}
            </select>
          </div>

          <button
            className="convert-btn"
            disabled={!selectedFile || !targetFormat || isConverting}
            onClick={handleConvert}
          >
            {isConverting ? "Converting..." : "Convert File"}
          </button>
        </section>

        {/* Progress Bar & Status Display */}
        <section className="progress-card">
          <div className="progress-header">
            <span className="progress-status">
              {isConverting ? "Conversion in progress" : progress === 100 ? "Completed" : "Status"}
            </span>
            <span className="progress-percent">{progress}%</span>
          </div>

          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${progress}%` }} />
          </div>

          <div className="progress-message">{statusMessage}</div>
        </section>
      </main>
    </div>
  );
}

export default App;
