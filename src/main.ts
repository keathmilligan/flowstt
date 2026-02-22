import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { MiniWaveformRenderer, VisualizationPayload } from "./renderers";
import { initTheme, getResolvedTheme, onThemeChange } from "./theme";

import logoLight from "./assets/flowstt-landscape-light.svg";
import logoDark from "./assets/flowstt-landscape.svg";

// Startup timing - marks the moment the JS module is first evaluated
const JS_MODULE_LOAD_TIME = performance.now();
// Log startup diagnostics to stderr via Tauri command so they appear
// in the same terminal stream as the Rust-side diagnostics.
function startupLog(msg: string) {
  invoke("startup_log", { message: msg });
}
startupLog(`JS module evaluated at ${JS_MODULE_LOAD_TIME.toFixed(0)}ms after page origin`);

function isDebugConsoleHotkey(e: KeyboardEvent): boolean {
  const isIKey = e.code === "KeyI" || e.key === "i" || e.key === "I";
  const isCtrlShift = e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;
  const isMetaAlt = e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey;
  return isIKey && (isCtrlShift || isMetaAlt);
}

interface ModelStatus {
  available: boolean;
  path: string;
}

interface HotkeyCombination {
  keys: string[];
}

interface PttStatus {
  hotkeys: HotkeyCombination[];
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
let miniWaveformHelp: HTMLDivElement | null;
let closeBtn: HTMLButtonElement | null;

// State
let isCapturing = false;

// Event listeners
let visualizationUnlisten: UnlistenFn | null = null;
let transcriptionCompleteUnlisten: UnlistenFn | null = null;
let transcriptionErrorUnlisten: UnlistenFn | null = null;
let captureStateChangedUnlisten: UnlistenFn | null = null;
let historyEntryDeletedUnlisten: UnlistenFn | null = null;
let autoModeToggledUnlisten: UnlistenFn | null = null;

let miniWaveformRenderer: MiniWaveformRenderer | null = null;

const KEY_DISPLAY_NAMES: Record<string, string> = {
  right_alt: "Right Alt",
  left_alt: "Left Alt",
  right_control: "Right Ctrl",
  left_control: "Left Ctrl",
  right_shift: "Right Shift",
  left_shift: "Left Shift",
  caps_lock: "Caps Lock",
  left_meta: "Left Win",
  right_meta: "Right Win",
  f1: "F1", f2: "F2", f3: "F3", f4: "F4", f5: "F5", f6: "F6",
  f7: "F7", f8: "F8", f9: "F9", f10: "F10", f11: "F11", f12: "F12",
  f13: "F13", f14: "F14", f15: "F15", f16: "F16", f17: "F17", f18: "F18",
  f19: "F19", f20: "F20", f21: "F21", f22: "F22", f23: "F23", f24: "F24",
  key_a: "A", key_b: "B", key_c: "C", key_d: "D", key_e: "E",
  key_f: "F", key_g: "G", key_h: "H", key_i: "I", key_j: "J",
  key_k: "K", key_l: "L", key_m: "M", key_n: "N", key_o: "O",
  key_p: "P", key_q: "Q", key_r: "R", key_s: "S", key_t: "T",
  key_u: "U", key_v: "V", key_w: "W", key_x: "X", key_y: "Y", key_z: "Z",
  digit0: "0", digit1: "1", digit2: "2", digit3: "3", digit4: "4",
  digit5: "5", digit6: "6", digit7: "7", digit8: "8", digit9: "9",
  arrow_up: "Up", arrow_down: "Down", arrow_left: "Left", arrow_right: "Right",
  home: "Home", end: "End", page_up: "Page Up", page_down: "Page Down",
  insert: "Insert", delete: "Delete",
  escape: "Esc", tab: "Tab", space: "Space", enter: "Enter",
  backspace: "Backspace", print_screen: "Print Screen",
  scroll_lock: "Scroll Lock", pause: "Pause",
  minus: "-", equal: "=", bracket_left: "[", bracket_right: "]",
  backslash: "\\", semicolon: ";", quote: "'", backquote: "`",
  comma: ",", period: ".", slash: "/",
  numpad0: "Num 0", numpad1: "Num 1", numpad2: "Num 2", numpad3: "Num 3",
  numpad4: "Num 4", numpad5: "Num 5", numpad6: "Num 6", numpad7: "Num 7",
  numpad8: "Num 8", numpad9: "Num 9",
  numpad_multiply: "Num *", numpad_add: "Num +", numpad_subtract: "Num -",
  numpad_decimal: "Num .", numpad_divide: "Num /", num_lock: "Num Lock",
};

const MODIFIER_KEYS = new Set([
  "right_alt", "left_alt", "right_control", "left_control",
  "right_shift", "left_shift", "left_meta", "right_meta",
]);

function keyDisplayName(keyCode: string): string {
  return KEY_DISPLAY_NAMES[keyCode] || keyCode;
}

function combinationDisplayName(combo: HotkeyCombination): string {
  const modifiers = combo.keys.filter((k) => MODIFIER_KEYS.has(k));
  const others = combo.keys.filter((k) => !MODIFIER_KEYS.has(k));
  modifiers.sort();
  others.sort();
  return [...modifiers, ...others].map(keyDisplayName).join(" + ");
}

function setMiniWaveformSlotActive(active: boolean) {
  if (miniWaveformCanvas) {
    miniWaveformCanvas.style.display = active ? "block" : "none";
  }
  if (miniWaveformHelp) {
    miniWaveformHelp.style.display = active ? "none" : "flex";
  }
}

async function refreshHotkeyHelpText(): Promise<void> {
  if (!miniWaveformHelp) return;
  try {
    const status = await invoke<PttStatus>("get_ptt_status");
    const combo = status.hotkeys?.[0] || null;
    const label = combo ? combinationDisplayName(combo) : null;
    const text = label || "Unassigned";
    miniWaveformHelp.textContent = text;
    miniWaveformHelp.title = text;
  } catch (error) {
    console.error("Failed to load PTT status:", error);
  }
}

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
        isCapturing = event.payload.capturing;

        if (event.payload.error) {
          console.error("[Capture] Error:", event.payload.error);
        }

        // Update waveform renderer and visibility
        if (isCapturing) {
          setMiniWaveformSlotActive(true);
          miniWaveformRenderer?.resize();
          miniWaveformRenderer?.clear();
          miniWaveformRenderer?.start();
        } else {
          miniWaveformRenderer?.stop();
          miniWaveformRenderer?.clear();
          setMiniWaveformSlotActive(false);
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

  // Auto mode toggled (via toggle hotkey)
  if (!autoModeToggledUnlisten) {
    autoModeToggledUnlisten = await listen<TranscriptionMode>("auto-mode-toggled", (event) => {
      const mode = event.payload;
      console.log(`[Main] Auto mode toggled to: ${mode}`);
      // The config window will handle updating its own UI via its own listener
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

  autoModeToggledUnlisten?.();
  autoModeToggledUnlisten = null;
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
    shadow: true,
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
    height: 310,
    resizable: false,
    maximizable: false,
    minimizable: false,
    decorations: false,
    transparent: true,
    shadow: true,
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
    shadow: true,
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
    if (isDebugConsoleHotkey(e)) return;
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
  miniWaveformHelp = document.querySelector("#mini-waveform-help");
  closeBtn = document.querySelector("#close-btn");

  // Swap logo image based on theme
  const appLogo = document.querySelector<HTMLImageElement>(".app-logo");
  if (appLogo) {
    const updateLogo = (theme: string) => {
      appLogo.src = theme === "light" ? logoLight : logoDark;
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

  if (miniWaveformHelp) {
    miniWaveformHelp.textContent = "Unassigned";
    miniWaveformHelp.title = "Unassigned";
  }

  setMiniWaveformSlotActive(isCapturing);

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

  // When the window becomes visible (e.g., after the setup wizard completes),
  // re-check model and capture status since they may have changed while hidden.
  document.addEventListener("visibilitychange", async () => {
    if (!document.hidden) {
      checkModelStatus();
      try {
        const status = await invoke<CaptureStatus>("get_status");
        isCapturing = status.capturing;
        setMiniWaveformSlotActive(isCapturing);
        refreshHotkeyHelpText();
      } catch {
        // Ignore - service may not be ready
      }
    }
  });

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
    // Show window even on error so the user can see the error message,
    // but only if the setup wizard is not active.
    try {
      const setupActive = await invoke<boolean>("needs_setup");
      if (!setupActive) {
        const mainWindow = getCurrentWindow();
        await mainWindow.show();
        await mainWindow.setFocus();
      }
    } catch {
      // If we can't check, show anyway
      const mainWindow = getCurrentWindow();
      await mainWindow.show();
      await mainWindow.setFocus();
    }
    return;
  }
  
  // Fetch current service status to sync UI with existing state.
  // The service may already be capturing if started independently.
  try {
    const status = await invoke<CaptureStatus>("get_status");
    startupLog(`get_status done (+${elapsed()})`);
    
    // Sync local state with service
    isCapturing = status.capturing;
    setMiniWaveformSlotActive(isCapturing);

    if (status.error) {
      console.error(`Service error: ${status.error}`);
    }
  } catch (error) {
    startupLog(`get_status FAILED (+${elapsed()}): ${error}`);
  }

  // Check if the setup wizard is active. If so, skip model/status checks
  // (the wizard handles all of that) and wait for setup-complete instead.
  const setupActive = await invoke<boolean>("needs_setup");

  if (!setupActive) {
    checkModelStatus();
    refreshHotkeyHelpText();

    // Load transcription history from service
    await loadHistory();
    startupLog(`loadHistory done (+${elapsed()})`);

    // If capturing, show and start waveform renderer
    if (isCapturing) {
      setMiniWaveformSlotActive(true);
      miniWaveformRenderer?.resize();
      miniWaveformRenderer?.clear();
      miniWaveformRenderer?.start();
    } else {
      setMiniWaveformSlotActive(false);
    }

    // Show the main window
    const mainWindow = getCurrentWindow();
    await mainWindow.show();
    await mainWindow.setFocus();
    startupLog(`window shown - startup complete (+${elapsed()})`);
  } else {
    startupLog(`setup wizard active - main window stays hidden (+${elapsed()})`);

    // When setup completes, the Rust setup() hook shows this window.
    // Re-check everything since the wizard configured the model, device, and mode.
    await listen("setup-complete", async () => {
      startupLog("setup-complete received - refreshing state");
      await checkModelStatus();
      try {
        const status = await invoke<CaptureStatus>("get_status");
        isCapturing = status.capturing;
        setMiniWaveformSlotActive(isCapturing);
        if (isCapturing) {
          miniWaveformRenderer?.resize();
          miniWaveformRenderer?.clear();
          miniWaveformRenderer?.start();
        }
      } catch {
        // Ignore
      }
      refreshHotkeyHelpText();
      await loadHistory();
    });
  }
}
