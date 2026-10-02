import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

interface AppInfo {
  name: string;
  version: string;
  status: string;
}

interface ConversionTarget {
  format: string;
  engine: string;
  available: boolean;
  targetExt: string;
  engineName: string;
}

interface QueueItem {
  id: string;
  name: string;
  path: string;
  size?: number;
  extension: string;
  targetFormat: string;
  availableTargets: ConversionTarget[];
  status: "pending" | "converting" | "done" | "error" | "cancelled";
  progress: number;
  progressMessage: string;
  error?: string;
  outputPath?: string;
}

const variantToExt: Record<string, string> = {
  jpeg: "jpg",
  webp: "webp",
  markdown: "md",
  m4a: "m4a",
  flac: "flac",
  webm: "webm",
};

function normalizeTargets(rawTargets: Array<{ format: string; engine: string; available: boolean }>): ConversionTarget[] {
  return rawTargets.map((t) => {
    const lower = t.format.toLowerCase();
    const targetExt = variantToExt[lower] ?? lower;
    const engineFriendly = t.engine === "ImageCrate" ? "Native Image Engine" : t.engine;
    return {
      ...t,
      targetExt,
      engineName: engineFriendly,
    };
  });
}

export function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [files, setFiles] = useState<QueueItem[]>([]);
  const [masterFormat, setMasterFormat] = useState("");
  const [isConverting, setIsConverting] = useState(false);
  const [activeFileId, setActiveFileId] = useState<string | null>(null);

  // Phase 9: Setup State
  const [needsSetup, setNeedsSetup] = useState(false);
  const [needsLibreOffice, setNeedsLibreOffice] = useState(false);
  const [setupProgress, setSetupProgress] = useState(0);
  const [setupMessage, setSetupMessage] = useState("");
  const [isSettingUp, setIsSettingUp] = useState(false);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const activeFileIdRef = useRef<string | null>(null);
  const stopBatchRef = useRef(false);
  const masterFormatRef = useRef("");
  const processingPathsRef = useRef<Set<string>>(new Set());

  // Keep ref in sync with state
  activeFileIdRef.current = activeFileId;
  masterFormatRef.current = masterFormat;

  // 1. Fetch backend status and check dependencies on startup
  useEffect(() => {
    async function fetchBackendInfo() {
      try {
        const info = await invoke<AppInfo>("get_app_info");
        setAppInfo(info);

        const status = await invoke<{ ffmpeg_ready: boolean; pandoc_ready: boolean; libreoffice_ready: boolean }>("check_dependencies");
        if (!status.ffmpeg_ready || !status.pandoc_ready) {
          setNeedsSetup(true);
        }
        if (!status.libreoffice_ready) {
          setNeedsLibreOffice(true);
          setNeedsSetup(true);
        }
      } catch (err) {
        console.error("Failed to connect to backend:", err);
      }
    }
    fetchBackendInfo();
  }, []);

  async function handleSetup() {
    setIsSettingUp(true);
    setSetupMessage("Initializing download...");

    const unlisten = await listen<{ percentage: number; message: string }>("setup-progress", (event) => {
      setSetupProgress(event.payload.percentage);
      setSetupMessage(event.payload.message);
    });

    try {
      await invoke("install_dependencies");
      setNeedsSetup(false);
    } catch (err) {
      console.error("Setup failed:", err);
      setSetupMessage(`Error: ${err}`);
    } finally {
      unlisten();
      setIsSettingUp(false);
    }
  }

  // 2. Set up Native Tauri Drag-and-Drop event listener for multiple files
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    try {
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
              addFiles(paths.map((p) => ({ path: p })));
            }
          }
        })
        .then((unlistenFn) => {
          unlisten = unlistenFn;
        });
    } catch (e) {
      console.warn("Native webview drag-drop unavailable:", e);
    }

    return () => {
      if (unlisten) unlisten();
    };
  }, []); // Run only once on mount

  // Add multiple file paths to the queue with strict deduplication
  async function addFiles(fileList: { path: string; size?: number }[]) {
    // Filter out paths that are already in flight or already in queue
    const toAdd = fileList.filter(
      (file) => !processingPathsRef.current.has(file.path)
    );
    if (toAdd.length === 0) return;

    // Mark as in-flight
    toAdd.forEach((f) => processingPathsRef.current.add(f.path));

    try {
      const newItems: QueueItem[] = await Promise.all(
        toAdd.map(async (file) => {
          const normalized = file.path.replace(/\\/g, "/");
          const name = normalized.split("/").pop() || file.path;
          const id = `${file.path}_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;

          try {
            const detectedExt = await invoke<string>("detect_file_type", { path: file.path });
            const rawTargets = await invoke<any[]>("get_targets", { sourceExt: detectedExt });
            const targets = normalizeTargets(rawTargets);

            const curMaster = masterFormatRef.current;
            const defaultFormat =
              curMaster && targets.some((t) => t.targetExt === curMaster)
                ? curMaster
                : targets.length > 0
                ? targets[0].targetExt
                : "";

            return {
              id,
              name,
              path: file.path,
              size: file.size,
              extension: detectedExt,
              targetFormat: defaultFormat,
              availableTargets: targets,
              status: "pending" as const,
              progress: 0,
              progressMessage: targets.length > 0 ? "Ready to convert" : "No conversion targets available",
            };
          } catch (err) {
            return {
              id,
              name,
              path: file.path,
              size: file.size,
              extension: "unknown",
              targetFormat: "",
              availableTargets: [],
              status: "error" as const,
              progress: 0,
              progressMessage: "Unsupported file type",
              error: String(err),
            };
          }
        })
      );

      // Functional updater with deduplication against current state
      setFiles((prev) => {
        const existingPaths = new Set(prev.map((f) => f.path));
        const unique = newItems.filter((item) => !existingPaths.has(item.path));
        return [...prev, ...unique];
      });
    } finally {
      toAdd.forEach((f) => processingPathsRef.current.delete(f.path));
    }
  }

  // HTML5 Drag-and-drop handlers (Only manage visual drag indicator; native Tauri handles the files)
  function onHtmlDragOver(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(true);
  }

  function onHtmlDragLeave(e: React.DragEvent) {
    e.preventDefault();
    setIsDragging(false);
  }

  function onHtmlDrop(e: React.DragEvent) {
    // Prevent the webview browser from trying to open/navigate to the dropped file
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
  }

  function handleFileInputChange(e: React.ChangeEvent<HTMLInputElement>) {
    if (e.target.files && e.target.files.length > 0) {
      const selected = Array.from(e.target.files).map((f) => ({
        path: (f as any).path || f.name,
        size: f.size,
      }));
      addFiles(selected);
      // Reset input value so same files can be re-selected if removed
      e.target.value = "";
    }
  }

  // Master format change: updates masterFormat and updates any pending file that supports it
  function handleMasterFormatChange(newFormat: string) {
    setMasterFormat(newFormat);
    if (!newFormat) return;

    setFiles((prev) =>
      prev.map((f) => {
        if (f.status === "pending" && f.availableTargets.some((t) => t.targetExt === newFormat)) {
          return { ...f, targetFormat: newFormat };
        }
        return f;
      })
    );
  }

  function handleItemFormatChange(id: string, newFormat: string) {
    setFiles((prev) =>
      prev.map((f) => (f.id === id ? { ...f, targetFormat: newFormat } : f))
    );
  }

  function handleRemoveFile(id: string) {
    setFiles((prev) => prev.filter((f) => f.id !== id));
  }

  function handleClearQueue() {
    if (isConverting) return;
    setFiles([]);
    setMasterFormat("");
  }

  function handleRetryFile(id: string) {
    setFiles((prev) =>
      prev.map((f) =>
        f.id === id
          ? {
              ...f,
              status: "pending",
              progress: 0,
              progressMessage: "Ready to convert",
              error: undefined,
            }
          : f
      )
    );
  }

  // Cancel conversion for a specific file
  async function handleCancelFile(item: QueueItem) {
    if (item.id === activeFileId) {
      try {
        await invoke("cancel_conversion");
      } catch (e) {
        console.error("Failed to send cancel signal:", e);
      }
    } else if (item.status === "pending") {
      setFiles((prev) =>
        prev.map((f) => (f.id === item.id ? { ...f, status: "cancelled", progressMessage: "Cancelled" } : f))
      );
    }
  }

  // Cancel all pending and current conversions
  async function handleStopAll() {
    stopBatchRef.current = true;
    if (activeFileId) {
      try {
        await invoke("cancel_conversion");
      } catch (e) {
        console.error("Failed to cancel active item:", e);
      }
    }
    setFiles((prev) =>
      prev.map((f) => (f.status === "pending" ? { ...f, status: "cancelled", progressMessage: "Cancelled" } : f))
    );
  }

  // Execute batch conversion sequentially
  async function handleConvertBatch() {
    const toProcess = files.filter(
      (f) => f.status === "pending" || f.status === "error" || f.status === "cancelled"
    );
    if (toProcess.length === 0 || isConverting) return;

    setIsConverting(true);
    stopBatchRef.current = false;

    // Listen to real-time progress events from Rust
    const unlisten = await listen<{ percentage: number; message: string }>(
      "conversion-progress",
      (event) => {
        const curId = activeFileIdRef.current;
        if (!curId) return;
        setFiles((prev) =>
          prev.map((f) =>
            f.id === curId
              ? {
                  ...f,
                  progress: Math.round(event.payload.percentage),
                  progressMessage: event.payload.message,
                }
              : f
          )
        );
      }
    );

    try {
      for (const item of toProcess) {
        if (stopBatchRef.current) break;
        if (!item.targetFormat) continue;

        activeFileIdRef.current = item.id;
        setActiveFileId(item.id);

        setFiles((prev) =>
          prev.map((f) =>
            f.id === item.id
              ? { ...f, status: "converting", progress: 0, progressMessage: "Starting conversion..." }
              : f
          )
        );

        try {
          const outputPath = await invoke<string>("convert_file", {
            sourcePath: item.path,
            sourceExt: item.extension,
            targetExt: item.targetFormat,
          });

          setFiles((prev) =>
            prev.map((f) =>
              f.id === item.id
                ? {
                    ...f,
                    status: "done",
                    progress: 100,
                    progressMessage: "Conversion complete",
                    outputPath,
                  }
                : f
            )
          );
        } catch (err: any) {
          const errStr = String(err);
          const wasCancelled = errStr.toLowerCase().includes("cancel");

          setFiles((prev) =>
            prev.map((f) =>
              f.id === item.id
                ? {
                    ...f,
                    status: wasCancelled ? "cancelled" : "error",
                    progressMessage: wasCancelled ? "Cancelled by user" : `Failed: ${errStr}`,
                    error: wasCancelled ? undefined : errStr,
                  }
                : f
            )
          );
        }
      }
    } finally {
      unlisten();
      setIsConverting(false);
      setActiveFileId(null);
      activeFileIdRef.current = null;
    }
  }

  function formatSize(bytes?: number): string {
    if (!bytes) return "Local file";
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    return `${(kb / 1024).toFixed(1)} MB`;
  }

  // Derive master format options (union of all formats across all pending files)
  const masterFormatOptions = Array.from(
    new Set(
      files
        .filter((f) => f.status === "pending" || f.status === "cancelled" || f.status === "error")
        .flatMap((f) => f.availableTargets.map((t) => t.targetExt))
    )
  );

  const pendingCount = files.filter(
    (f) => f.status === "pending" || f.status === "error" || f.status === "cancelled"
  ).length;
  const completedCount = files.filter((f) => f.status === "done").length;

  return (
    <div
      className={`app-container ${isDragging ? "dragging-over" : ""}`}
      onDragOver={onHtmlDragOver}
      onDragLeave={onHtmlDragLeave}
      onDrop={onHtmlDrop}
    >
      {/* First-Run Setup Overlay */}
      {needsSetup && (
        <div className="setup-overlay">
          <div className="setup-card">
            <h2>Additional Setup Required</h2>
            <p>MetaMorph needs FFmpeg and Pandoc to convert video, audio, and documents.</p>
            <p className="setup-subtext">
              These will be downloaded privately to the app's local data folder. No system installation is required.
            </p>

            {needsLibreOffice && (
              <div className="libreoffice-notice-box">
                <h3>LibreOffice Missing</h3>
                <p>
                  For advanced document conversions (such as .docx to .pdf), LibreOffice is required.
                  Because it is an office suite, please{" "}
                  <a
                    href="#"
                    onClick={(e) => {
                      e.preventDefault();
                      openUrl("https://www.libreoffice.org/download/download-libreoffice/");
                    }}
                  >
                    install it manually
                  </a>
                  , then restart the app.
                </p>
                <button
                  className="btn-secondary"
                  style={{ marginTop: "12px", width: "100%" }}
                  onClick={() => {
                    setNeedsLibreOffice(false);
                    invoke<{ ffmpeg_ready: boolean; pandoc_ready: boolean }>("check_dependencies").then(
                      (status) => {
                        if (status.ffmpeg_ready && status.pandoc_ready) {
                          setNeedsSetup(false);
                        }
                      }
                    );
                  }}
                >
                  I'll do it later
                </button>
              </div>
            )}

            {isSettingUp ? (
              <div className="setup-progress-container">
                <div className="progress-track" style={{ height: "10px" }}>
                  <div className="progress-fill" style={{ width: `${setupProgress}%` }}></div>
                </div>
                <p style={{ marginTop: "10px", fontSize: "0.85rem" }}>{setupMessage}</p>
              </div>
            ) : (
              <button
                className="convert-btn setup-btn"
                onClick={handleSetup}
                style={{ width: "100%", marginTop: "20px" }}
              >
                Download & Install Dependencies
              </button>
            )}
          </div>
        </div>
      )}

      {/* Hidden file input for click-to-browse */}
      <input
        type="file"
        multiple
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
          <span>{appInfo ? appInfo.status : "Engine Ready"}</span>
        </div>
      </header>

      {/* Main Workspace */}
      <main className="workspace">
        {/* Drop Zone: Large when empty, compact bar when queue has items */}
        {files.length === 0 ? (
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
              {isDragging ? "Release to drop files" : "Drop files to convert"}
            </h2>
            <p className="dropzone-subtitle">
              Images, videos, audio, and documents · <span className="browse-link">browse files</span>
            </p>
          </div>
        ) : (
          <div
            className={`dropzone-container compact ${isDragging ? "active" : ""}`}
            onClick={() => fileInputRef.current?.click()}
            title="Click or drop to add more files to queue"
          >
            <div className="dropzone-icon">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <line x1="12" y1="5" x2="12" y2="19" />
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
            </div>
            <span className="dropzone-title">Drop more files here or click to browse</span>
          </div>
        )}

        {/* Master Control Bar (Only visible when files exist) */}
        {files.length > 0 && (
          <div className="batch-master-bar">
            <div className="batch-stats">
              <span>{files.length} file{files.length !== 1 ? "s" : ""}</span>
              <span className="batch-badge">
                {completedCount}/{files.length} done
              </span>
            </div>

            <div className="batch-actions">
              {masterFormatOptions.length > 0 && (
                <div className="master-select-wrap">
                  <span className="master-select-label">Convert All To:</span>
                  <select
                    className="format-select"
                    style={{ width: "auto", padding: "6px 12px" }}
                    value={masterFormat}
                    disabled={isConverting}
                    onChange={(e) => handleMasterFormatChange(e.target.value)}
                  >
                    <option value="">Custom per file</option>
                    {masterFormatOptions.map((fmt) => (
                      <option key={fmt} value={fmt}>
                        .{fmt.toUpperCase()}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              {isConverting ? (
                <button className="btn-action-cancel" onClick={handleStopAll} title="Abort current and stop queue">
                  Stop All
                </button>
              ) : (
                <>
                  <button
                    className="btn-secondary"
                    onClick={handleClearQueue}
                    title="Remove all files from list"
                  >
                    Clear All
                  </button>
                  <button
                    className="convert-btn"
                    style={{ marginTop: 0, padding: "8px 20px" }}
                    disabled={pendingCount === 0}
                    onClick={handleConvertBatch}
                  >
                    Convert {pendingCount > 0 ? `${pendingCount} File${pendingCount > 1 ? "s" : ""}` : "All Done"}
                  </button>
                </>
              )}
            </div>
          </div>
        )}

        {/* Queue List of File Cards */}
        {files.length > 0 && (
          <div className="queue-scroll">
            {files.map((item) => {
              const isActive = item.id === activeFileId;
              return (
                <div
                  key={item.id}
                  className={`queue-item ${isActive ? "active" : ""} status-${item.status}`}
                >
                  <div className="queue-item-main">
                    <div className="queue-item-left">
                      <div className="file-icon">{item.extension}</div>
                      <div className="file-details">
                        <div className="file-name" title={item.name}>
                          {item.name}
                        </div>
                        <div className="file-meta" title={item.path}>
                          {formatSize(item.size)} · {item.path}
                        </div>
                      </div>
                    </div>

                    <div className="queue-item-right">
                      {/* Format Selector per item */}
                      <select
                        className="format-select"
                        style={{ width: "auto", minWidth: "110px", padding: "6px 10px", fontSize: "0.82rem" }}
                        value={item.targetFormat}
                        disabled={isConverting || item.status === "done" || item.availableTargets.length === 0}
                        onChange={(e) => handleItemFormatChange(item.id, e.target.value)}
                      >
                        {item.availableTargets.length === 0 ? (
                          <option value="">No formats</option>
                        ) : (
                          item.availableTargets.map((t) => (
                            <option key={t.targetExt} value={t.targetExt}>
                              .{t.targetExt.toUpperCase()}
                            </option>
                          ))
                        )}
                      </select>

                      {/* Status Tag */}
                      <span className={`status-tag ${item.status}`}>
                        {item.status === "converting" ? `${item.progress}%` : item.status}
                      </span>

                      {/* Action Button: Cancel when converting, Retry when error/cancelled, Remove when pending */}
                      {item.status === "converting" ? (
                        <button
                          className="btn-action-cancel"
                          onClick={() => handleCancelFile(item)}
                          title="Cancel this conversion and proceed to next"
                        >
                          Cancel
                        </button>
                      ) : item.status === "error" || item.status === "cancelled" ? (
                        <div style={{ display: "flex", gap: "6px" }}>
                          <button
                            className="btn-action-retry"
                            onClick={() => handleRetryFile(item.id)}
                            title="Retry conversion"
                          >
                            Retry
                          </button>
                          <button
                            className="remove-btn"
                            onClick={() => handleRemoveFile(item.id)}
                            title="Remove from queue"
                          >
                            ✕
                          </button>
                        </div>
                      ) : (
                        <button
                          className="remove-btn"
                          disabled={isConverting}
                          onClick={() => handleRemoveFile(item.id)}
                          title="Remove from queue"
                        >
                          ✕
                        </button>
                      )}
                    </div>
                  </div>

                  {/* Inline Progress Bar for actively converting item */}
                  {item.status === "converting" && (
                    <div className="item-progress-section">
                      <div className="item-progress-header">
                        <span>{item.progressMessage}</span>
                        <span>{item.progress}%</span>
                      </div>
                      <div className="item-progress-track">
                        <div className="item-progress-fill" style={{ width: `${item.progress}%` }}></div>
                      </div>
                    </div>
                  )}

                  {/* Status/Error note for completed, error, or cancelled */}
                  {item.status === "done" && item.outputPath && (
                    <div style={{ fontSize: "0.76rem", color: "var(--success)", textAlign: "left" }}>
                      Saved to: {item.outputPath}
                    </div>
                  )}

                  {item.status === "error" && (
                    <div style={{ fontSize: "0.76rem", color: "var(--danger)", textAlign: "left" }}>
                      {item.progressMessage}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
