import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { MiniWaveformRenderer, VisualizationPayload } from "./renderers";
import { initTheme, getResolvedTheme, onThemeChange } from "./theme";

// Startup timing - marks the moment the JS module is first evaluated
const JS_MODULE_LOAD_TIME = performance.now();
// Log startup diagnostics to stderr via Tauri command so they appear
// in the same terminal stream as the Rust-side diagnostics.
function startupLog(msg: string) {
  invoke("startup_log", { message: msg });
}
startupLog(`JS module evaluated at ${JS_MODULE_LOAD_TIME.toFixed(0)}ms after page origin`);

interface ModelStatus {
  available: boolean;
  path: string;
}

interface CudaStatus {
  build_enabled: boolean;
  runtime_available: boolean;
  system_info: string;
}

// CaptureStatus matches backend TranscribeStatus
interface CaptureStatus {
  capturing: boolean;
  in_speech: boolean;
  queue_depth: number;
  error: string | null;
  source1_id: string | null;
  source2_id: string | null;
  transcription_mode: TranscriptionMode;
}

// Transcription mode matching backend
type TranscriptionMode = "automatic" | "push_to_talk";

// History entry from the service
interface HistoryEntry {
  id: string;
  text: string;
  timestamp: string;
  wav_path: string | null;
}

// Enriched transcription result payload
interface TranscriptionCompletePayload {
  id: string | null;
  text: string;
  timestamp: string | null;
  audio_path: string | null;
}

// DOM elements
let historyContainer: HTMLElement | null;
let modelWarning: HTMLElement | null;
let modelPathEl: HTMLElement | null;
let downloadModelBtn: HTMLButtonElement | null;
let downloadStatusEl: HTMLElement | null;
let miniWaveformCanvas: HTMLCanvasElement | null;
let closeBtn: HTMLButtonElement | null;

// State
let isCapturing = false;

// Event listeners
let visualizationUnlisten: UnlistenFn | null = null;
let transcriptionCompleteUnlisten: UnlistenFn | null = null;
let transcriptionErrorUnlisten: UnlistenFn | null = null;
let captureStateChangedUnlisten: UnlistenFn | null = null;
let historyEntryDeletedUnlisten: UnlistenFn | null = null;

let miniWaveformRenderer: MiniWaveformRenderer | null = null;

// CUDA indicator
let cudaIndicator: HTMLElement | null = null;

async function checkModelStatus() {
  try {
    const status = await invoke<ModelStatus>("check_model_status");

    if (!status.available && modelWarning && modelPathEl) {
      modelWarning.classList.remove("hidden");
      modelPathEl.textContent = `Model location: ${status.path}`;
    } else if (status.available && modelWarning) {
      modelWarning.classList.add("hidden");
    }
  } catch (error) {
    console.error("Failed to check model status:", error);
  }
}

async function downloadModel() {
  if (!downloadModelBtn || !downloadStatusEl) return;

  downloadModelBtn.disabled = true;
  downloadStatusEl.textContent = "Downloading model... This may take a few minutes.";
  downloadStatusEl.className = "download-status loading";

  try {
    await invoke("download_model");
    downloadStatusEl.textContent = "Download complete!";
    downloadStatusEl.className = "download-status success";

    // Hide warning after successful download
    setTimeout(() => {
      checkModelStatus();
    }, 1500);
  } catch (error) {
    console.error("Download error:", error);
    downloadStatusEl.textContent = `Download failed: ${error}`;
    downloadStatusEl.className = "download-status error";
    downloadModelBtn.disabled = false;
  }
}

// ============== Event Listeners ==============

async function setupEventListeners() {
  // Visualization data
  if (!visualizationUnlisten) {
    visualizationUnlisten = await listen<VisualizationPayload>("visualization-data", (event) => {
      if (miniWaveformRenderer) {
        miniWaveformRenderer.pushSamples(event.payload.waveform);
      }
    });
  }

  // Transcription results (now with enriched payload)
  if (!transcriptionCompleteUnlisten) {
    transcriptionCompleteUnlisten = await listen<TranscriptionCompletePayload>("transcription-complete", (event) => {
      const payload = event.payload;
      if (payload.id && payload.timestamp) {
        appendHistorySegment({
          id: payload.id,
          text: payload.text,
          timestamp: payload.timestamp,
          wav_path: payload.audio_path,
        });
      }
    });
  }

  // Capture state changes
  if (!captureStateChangedUnlisten) {
    captureStateChangedUnlisten = await listen<{capturing: boolean, error: string | null}>(
      "capture-state-changed", 
      (event) => {
        console.log("[Capture] State changed:", event.payload);
        isCapturing = event.payload.capturing;

        if (event.payload.error) {
          console.error("[Capture] Error:", event.payload.error);
        }

        // Update waveform renderer and visibility
        if (isCapturing) {
          if (miniWaveformCanvas) miniWaveformCanvas.style.display = "block";
          miniWaveformRenderer?.resize();
          miniWaveformRenderer?.clear();
          miniWaveformRenderer?.start();
        } else {
          miniWaveformRenderer?.stop();
          miniWaveformRenderer?.clear();
          if (miniWaveformCanvas) miniWaveformCanvas.style.display = "none";
        }
      }
    );
  }

  // History entry deleted (from another client or cleanup)
  if (!historyEntryDeletedUnlisten) {
    historyEntryDeletedUnlisten = await listen<string>("history-entry-deleted", (event) => {
      removeHistorySegmentFromDOM(event.payload);
    });
  }
}

function cleanupEventListeners() {
  visualizationUnlisten?.();
  visualizationUnlisten = null;
  
  transcriptionCompleteUnlisten?.();
  transcriptionCompleteUnlisten = null;
  
  transcriptionErrorUnlisten?.();
  transcriptionErrorUnlisten = null;
  
  captureStateChangedUnlisten?.();
  captureStateChangedUnlisten = null;
  
  historyEntryDeletedUnlisten?.();
  historyEntryDeletedUnlisten = null;
}

// ============== History Display ==============

// Currently playing audio element (if any)
let currentAudio: HTMLAudioElement | null = null;

/** Format an ISO 8601 timestamp for display */
function formatTimestamp(isoString: string): string {
  try {
    const date = new Date(isoString);
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch {
    return "";
  }
}

/** Create a DOM element for a history segment */
function createSegmentElement(entry: HistoryEntry): HTMLElement {
  const row = document.createElement("div");
  row.className = "history-segment";
  row.dataset.id = entry.id;

  // Timestamp
  const ts = document.createElement("span");
  ts.className = "segment-timestamp";
  ts.textContent = formatTimestamp(entry.timestamp);
  row.appendChild(ts);

  // Text
  const text = document.createElement("span");
  text.className = "segment-text";
  text.textContent = entry.text;
  row.appendChild(text);

  // Actions
  const actions = document.createElement("span");
  actions.className = "segment-actions";

  // Play button (only if WAV exists)
  if (entry.wav_path) {
    const playBtn = document.createElement("button");
    playBtn.className = "segment-btn";
    playBtn.title = "Play audio";
    playBtn.innerHTML = "&#9654;"; // play triangle
    const wavPath = entry.wav_path;
    playBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      playSegmentAudio(wavPath, playBtn);
    });
    actions.appendChild(playBtn);
  }

  // Copy button
  const copyBtn = document.createElement("button");
  copyBtn.className = "segment-btn";
  copyBtn.title = "Copy text";
  copyBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="5" width="9" height="9" rx="1.5"/><path d="M5 11H3.5A1.5 1.5 0 0 1 2 9.5v-7A1.5 1.5 0 0 1 3.5 1h7A1.5 1.5 0 0 1 12 2.5V5"/></svg>';
  copyBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    navigator.clipboard.writeText(entry.text).then(() => {
      copyBtn.classList.add("copy-success");
      setTimeout(() => copyBtn.classList.remove("copy-success"), 1000);
    });
  });
  actions.appendChild(copyBtn);

  // Delete button
  const deleteBtn = document.createElement("button");
  deleteBtn.className = "segment-btn";
  deleteBtn.title = "Delete";
  deleteBtn.innerHTML = "&#10005;"; // X mark
  deleteBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    deleteHistoryEntry(entry.id, row);
  });
  actions.appendChild(deleteBtn);

  row.appendChild(actions);
  return row;
}

/** Append a new segment to the history display and scroll to bottom */
function appendHistorySegment(entry: HistoryEntry): void {
  if (!historyContainer) return;

  // Remove empty state message if present
  const emptyMsg = historyContainer.querySelector(".history-empty");
  if (emptyMsg) emptyMsg.remove();

  const el = createSegmentElement(entry);
  historyContainer.appendChild(el);
  historyContainer.scrollTop = historyContainer.scrollHeight;
}

/** Remove a segment from the DOM by ID */
function removeHistorySegmentFromDOM(id: string): void {
  if (!historyContainer) return;
  const el = historyContainer.querySelector(`[data-id="${id}"]`);
  if (el) el.remove();

  // Show empty state if no more segments
  if (historyContainer.children.length === 0) {
    showEmptyState();
  }
}

/** Show the empty state message */
function showEmptyState(): void {
  if (!historyContainer) return;
  if (historyContainer.querySelector(".history-empty")) return;
  const msg = document.createElement("div");
  msg.className = "history-empty";
  msg.textContent = "No transcriptions yet. Start speaking to begin.";
  historyContainer.appendChild(msg);
}

/** Load full history from the service and render */
async function loadHistory(): Promise<void> {
  if (!historyContainer) return;

  try {
    const entries = await invoke<HistoryEntry[]>("get_history");
    historyContainer.innerHTML = "";

    if (entries.length === 0) {
      showEmptyState();
      return;
    }

    for (const entry of entries) {
      const el = createSegmentElement(entry);
      historyContainer.appendChild(el);
    }

    // Scroll to bottom to show most recent
    historyContainer.scrollTop = historyContainer.scrollHeight;
  } catch (error) {
    console.error("Failed to load history:", error);
  }
}

/** Delete a history entry via the service */
async function deleteHistoryEntry(id: string, rowEl: HTMLElement): Promise<void> {
  try {
    await invoke("delete_history_entry", { id });
    rowEl.remove();
    if (historyContainer && historyContainer.children.length === 0) {
      showEmptyState();
    }
  } catch (error) {
    console.error("Failed to delete history entry:", error);
  }
}

/** Play a WAV file for a segment */
function playSegmentAudio(wavPath: string, btn: HTMLButtonElement): void {
  // Stop any currently playing audio
  if (currentAudio) {
    currentAudio.pause();
    currentAudio = null;
    // Remove playing state from all buttons
    document.querySelectorAll(".segment-btn.playing").forEach(b => b.classList.remove("playing"));
  }

  const assetUrl = convertFileSrc(wavPath);
  const audio = new Audio(assetUrl);
  currentAudio = audio;
  btn.classList.add("playing");

  audio.addEventListener("ended", () => {
    btn.classList.remove("playing");
    currentAudio = null;
  });

  audio.addEventListener("error", () => {
    btn.classList.remove("playing");
    currentAudio = null;
    console.error("Failed to play audio:", wavPath);
  });

  audio.play().catch((e) => {
    btn.classList.remove("playing");
    currentAudio = null;
    console.error("Audio playback error:", e);
  });
}

// ============== CUDA Status ==============

async function checkCudaStatus() {
  try {
    const status = await invoke<CudaStatus>("get_cuda_status");

    if (cudaIndicator) {
      if (status.build_enabled) {
        cudaIndicator.classList.remove("hidden");
        if (status.runtime_available) {
          cudaIndicator.title = `CUDA GPU Acceleration Active\n${status.system_info}`;
          cudaIndicator.classList.add("active");
        } else {
          cudaIndicator.title = `CUDA Built but NOT Active (GPU not detected)\n${status.system_info}`;
          cudaIndicator.classList.add("inactive");
        }
      } else {
        cudaIndicator.classList.add("hidden");
      }
    }

    console.log(`CUDA status: build_enabled=${status.build_enabled}, runtime_available=${status.runtime_available}`);
  } catch (error) {
    console.error("Failed to check CUDA status:", error);
  }
}

// ============== Window Management ==============

async function openVisualizationWindow() {
  // Check if the window already exists (was previously created)
  const existing = await WebviewWindow.getByLabel("visualization");
  if (existing) {
    const isVisible = await existing.isVisible();
    if (isVisible) {
      await existing.setFocus();
    } else {
      await existing.show();
      await existing.setFocus();
    }
    return;
  }

  // Create the window on demand (not pre-created at startup to avoid
  // the cost of initializing a second WebView2 instance during launch)
  const vizWindow = new WebviewWindow("visualization", {
    url: "visualization.html",
    title: "FlowSTT Visualization",
    width: 900,
    height: 700,
    minWidth: 800,
    minHeight: 600,
    resizable: true,
    decorations: false,
    transparent: true,
    shadow: false,
    center: true,
  });

  vizWindow.once("tauri://error", (e) => {
    console.error("Failed to create visualization window:", e.payload);
  });
}

async function openAboutWindow() {
  const existing = await WebviewWindow.getByLabel("about");
  if (existing) {
    const isVisible = await existing.isVisible();
    if (isVisible) {
      await existing.setFocus();
    } else {
      await existing.show();
      await existing.setFocus();
    }
    return;
  }

  const aboutWindow = new WebviewWindow("about", {
    url: "about.html",
    title: "About FlowSTT",
    width: 400,
    height: 280,
    resizable: false,
    maximizable: false,
    minimizable: false,
    decorations: false,
    transparent: true,
    shadow: false,
    skipTaskbar: true,
    center: true,
  });

  aboutWindow.once("tauri://error", (e) => {
    console.error("Failed to create about window:", e.payload);
  });
}

async function openConfigWindow() {
  const existing = await WebviewWindow.getByLabel("config");
  if (existing) {
    const isVisible = await existing.isVisible();
    if (isVisible) {
      await existing.setFocus();
    } else {
      await existing.show();
      await existing.setFocus();
    }
    return;
  }

  const configWindow = new WebviewWindow("config", {
    url: "config.html",
    title: "FlowSTT Settings",
    width: 480,
    height: 460,
    resizable: false,
    maximizable: false,
    minimizable: false,
    decorations: false,
    transparent: true,
    shadow: false,
    skipTaskbar: true,
    center: true,
  });

  configWindow.once("tauri://error", (e) => {
    console.error("Failed to create config window:", e.payload);
  });
}

// ============== Initialization ==============

window.addEventListener("DOMContentLoaded", () => {
  startupLog(`DOMContentLoaded fired at ${performance.now().toFixed(0)}ms (module loaded at ${JS_MODULE_LOAD_TIME.toFixed(0)}ms)`);

  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  // Suppress all default keyboard behaviour in this decorationless window.
  // WebView2/Chromium has many built-in shortcuts (Ctrl+P print, Ctrl+F find,
  // Alt menu activation, F5 reload, etc.) that are unwanted in a dedicated
  // app window. We block everything except:
  //   - Alt+F4: allowed through so the OS/Tauri can handle close-to-tray
  //   - Form-element interactions: arrow keys, Enter, Space, Tab, and typed
  //     characters are allowed when a <select>, <input>, or <button> has focus
  const suppressKeyHandler = (e: KeyboardEvent) => {
    // Allow Alt+F4 (window close / hide-to-tray)
    if (e.key === "F4" && e.altKey) return;

    // Allow normal interaction with form controls
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "SELECT" || tag === "INPUT" || tag === "BUTTON") {
      // Let the form element handle its own keys (arrows, Enter, Space,
      // Tab, typed characters for <select> search, etc.)
      return;
    }

    e.preventDefault();
  };
  document.addEventListener("keydown", suppressKeyHandler);
  document.addEventListener("keyup", suppressKeyHandler);

  // Get DOM elements
  historyContainer = document.querySelector("#history-container");
  modelWarning = document.querySelector("#model-warning");
  modelPathEl = document.querySelector("#model-path");
  downloadModelBtn = document.querySelector("#download-model-btn");
  downloadStatusEl = document.querySelector("#download-status");
  miniWaveformCanvas = document.querySelector("#mini-waveform");
  closeBtn = document.querySelector("#close-btn");
  cudaIndicator = document.querySelector("#cuda-indicator");

  // Swap logo image based on theme
  const appLogo = document.querySelector<HTMLImageElement>(".app-logo");
  if (appLogo) {
    const updateLogo = (theme: string) => {
      appLogo.src = theme === "light"
        ? "/src/assets/flowstt-landscape-light.svg"
        : "/src/assets/flowstt-landscape.svg";
    };
    updateLogo(getResolvedTheme());
    onThemeChange(updateLogo);
  }

  // Initialize mini waveform renderer
  if (miniWaveformCanvas) {
    miniWaveformRenderer = new MiniWaveformRenderer(miniWaveformCanvas, 64);

    miniWaveformCanvas.addEventListener("dblclick", (e) => {
      e.preventDefault();
      e.stopPropagation();
      openVisualizationWindow();
    });
  }

  // Handle window resize
  window.addEventListener("resize", () => {
    if (miniWaveformCanvas && miniWaveformRenderer) {
      miniWaveformRenderer.resize();
    }
  });

  // Set up event handlers
  downloadModelBtn?.addEventListener("click", downloadModel);
  document.querySelector("#about-btn")?.addEventListener("click", () => openAboutWindow());
  document.querySelector("#config-btn")?.addEventListener("click", () => openConfigWindow());
  closeBtn?.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    // Hide to tray instead of closing
    const mainWindow = getCurrentWindow();
    await mainWindow.hide();
  });

  // Cleanup on close
  window.addEventListener("beforeunload", () => {
    cleanupEventListeners();
  });

  // Handle visibility change - no special handling needed for history segments
  // since DOM updates persist correctly even when the window is hidden

  // Initialize app
  initializeApp();
});

async function initializeApp() {
  const t0 = performance.now();
  const elapsed = () => `${(performance.now() - t0).toFixed(0)}ms`;

  startupLog(`initializeApp started at ${performance.now().toFixed(0)}ms`);

  // Initialize theme before first paint
  await initTheme();
  startupLog(`initTheme done (+${elapsed()})`);

  // Set up event listeners (must be done before connect_events so we
  // catch the synthetic CaptureStateChanged sent on subscribe)
  await setupEventListeners();
  startupLog(`setupEventListeners done (+${elapsed()})`);
  
  // Connect to service event stream (service is already operational)
  try {
    await invoke("connect_events");
    startupLog(`connect_events done (+${elapsed()})`);
  } catch (error) {
    startupLog(`connect_events FAILED (+${elapsed()}): ${error}`);
    console.error(`Connection error: ${error}`);
    // Show window even on error so the user can see the error message
    const mainWindow = getCurrentWindow();
    await mainWindow.show();
    await mainWindow.setFocus();
    return;
  }
  
  // Fetch current service status to sync UI with existing state.
  // The service may already be capturing if started independently.
  try {
    const status = await invoke<CaptureStatus>("get_status");
    startupLog(`get_status done (+${elapsed()})`);
    
    // Sync local state with service
    isCapturing = status.capturing;

    if (status.error) {
      console.error(`Service error: ${status.error}`);
    }
  } catch (error) {
    startupLog(`get_status FAILED (+${elapsed()}): ${error}`);
  }

  checkModelStatus();
  checkCudaStatus();

  // Load transcription history from service
  await loadHistory();
  startupLog(`loadHistory done (+${elapsed()})`);

  // If capturing, show and start waveform renderer
  if (isCapturing) {
    if (miniWaveformCanvas) miniWaveformCanvas.style.display = "block";
    miniWaveformRenderer?.resize();
    miniWaveformRenderer?.clear();
    miniWaveformRenderer?.start();
  }

  // Show the main window now that the UI is fully initialized and connected.
  // The window starts hidden (visible: false in tauri.conf.json) to avoid
  // showing a blank/unresponsive window while waiting for the service connection.
  const mainWindow = getCurrentWindow();
  await mainWindow.show();
  await mainWindow.setFocus();
  startupLog(`window shown - startup complete (+${elapsed()})`);
}
