import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { MiniWaveformRenderer, VisualizationPayload } from "./renderers";

interface AudioDevice {
  id: string;
  name: string;
}

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

// Key codes for PTT hotkey
type KeyCode = "right_alt" | "left_alt" | "right_control" | "left_control" | 
               "right_shift" | "left_shift" | "caps_lock" | 
               "f13" | "f14" | "f15" | "f16" | "f17" | "f18" | "f19" | "f20";

interface PttStatus {
  mode: TranscriptionMode;
  key: KeyCode;
  is_active: boolean;
  available: boolean;
  error: string | null;
}

// Key code display names
const KEY_CODE_NAMES: Record<KeyCode, string> = {
  right_alt: "Right Alt",
  left_alt: "Left Alt",
  right_control: "Right Control",
  left_control: "Left Control",
  right_shift: "Right Shift",
  left_shift: "Left Shift",
  caps_lock: "Caps Lock",
  f13: "F13",
  f14: "F14",
  f15: "F15",
  f16: "F16",
  f17: "F17",
  f18: "F18",
  f19: "F19",
  f20: "F20",
};

// DOM elements
let source1Select: HTMLSelectElement | null;
let source2Select: HTMLSelectElement | null;
let modeToggle: HTMLInputElement | null;
let pttKeySelect: HTMLSelectElement | null;
let statusEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let modelWarning: HTMLElement | null;
let modelPathEl: HTMLElement | null;
let downloadModelBtn: HTMLButtonElement | null;
let downloadStatusEl: HTMLElement | null;
let miniWaveformCanvas: HTMLCanvasElement | null;
let closeBtn: HTMLButtonElement | null;
let pttIndicator: HTMLElement | null;

// State
let isCapturing = false;
let inSpeechSegment = false;
let transcribeQueueDepth = 0;
let allDevices: AudioDevice[] = [];
let transcriptionMode: TranscriptionMode = "push_to_talk";
let pttKey: KeyCode = "right_alt";
let isPttActive = false;

// Event listeners
let visualizationUnlisten: UnlistenFn | null = null;
let transcriptionCompleteUnlisten: UnlistenFn | null = null;
let transcriptionErrorUnlisten: UnlistenFn | null = null;
let speechStartedUnlisten: UnlistenFn | null = null;
let speechEndedUnlisten: UnlistenFn | null = null;
let captureStateChangedUnlisten: UnlistenFn | null = null;
let pttPressedUnlisten: UnlistenFn | null = null;
let pttReleasedUnlisten: UnlistenFn | null = null;
let transcriptionModeChangedUnlisten: UnlistenFn | null = null;

let miniWaveformRenderer: MiniWaveformRenderer | null = null;

// CUDA indicator
let cudaIndicator: HTMLElement | null = null;

async function loadDevices(currentSource1?: string | null, currentSource2?: string | null) {
  try {
    allDevices = await invoke<AudioDevice[]>("list_all_sources");

    // Populate both source dropdowns
    populateSourceDropdown(source1Select, true);  // Default: select first device
    populateSourceDropdown(source2Select, false); // Default: select "None"

    // If the service already has sources configured, select those instead
    if (currentSource1 && source1Select) {
      const exists = allDevices.some(d => d.id === currentSource1);
      if (exists) {
        source1Select.value = currentSource1;
      }
    }
    if (currentSource2 && source2Select) {
      const exists = allDevices.some(d => d.id === currentSource2);
      if (exists) {
        source2Select.value = currentSource2;
      }
    }

    // Don't auto-call onSourceChange -- the service may already be capturing.
    // Sources are only changed when the user explicitly changes a dropdown.
  } catch (error) {
    console.error("Failed to load devices:", error);
    if (source1Select) {
      source1Select.innerHTML = `<option value="">Error loading devices</option>`;
    }
    if (source2Select) {
      source2Select.innerHTML = `<option value="">Error loading devices</option>`;
    }
    setStatus(`Error: ${error}`, "error");
  }
}

function populateSourceDropdown(select: HTMLSelectElement | null, selectFirstDevice: boolean) {
  if (!select) return;

  select.innerHTML = "";

  // Add "None" option
  const noneOption = document.createElement("option");
  noneOption.value = "";
  noneOption.textContent = "None";
  select.appendChild(noneOption);

  // Add all devices
  allDevices.forEach((device) => {
    const option = document.createElement("option");
    option.value = device.id;
    option.textContent = device.name;
    select.appendChild(option);
  });

  // Select first device for source1, "None" for source2
  if (selectFirstDevice && allDevices.length > 0) {
    select.value = allDevices[0].id;
  } else {
    select.value = "";
  }
}

function getSelectedSources(): { source1Id: string | null; source2Id: string | null } {
  const source1Id = source1Select?.value || null;
  const source2Id = source2Select?.value || null;
  return {
    source1Id: source1Id || null,
    source2Id: source2Id || null,
  };
}

// Handle source selection changes - configures capture automatically
async function onSourceChange() {
  const { source1Id, source2Id } = getSelectedSources();

  try {
    // Set sources - capture starts/stops automatically based on configuration
    await invoke("set_sources", { source1Id, source2Id });
  } catch (error) {
    console.error("Error configuring sources:", error);
    setStatus(`Error: ${error}`, "error");
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

function setStatus(message: string, type: "normal" | "progress" | "warning" | "error" = "normal") {
  if (statusEl) {
    statusEl.textContent = message;
    statusEl.className = "status";
    if (type !== "normal") {
      statusEl.classList.add(type);
    }
  }
}

// Update status based on current state
function updateStatusDisplay() {
  if (!isCapturing) {
    setStatus("Ready - select an audio source to begin");
    return;
  }

  let statusText: string;
  if (inSpeechSegment) {
    statusText = "Recording speech...";
  } else if (transcribeQueueDepth > 0) {
    statusText = `Listening... (${transcribeQueueDepth} pending)`;
  } else {
    const modeText = transcriptionMode === "push_to_talk" 
      ? `PTT Ready (${KEY_CODE_NAMES[pttKey]})`
      : "Auto (VAD)";
    statusText = `Listening... [${modeText}]`;
  }
  setStatus(statusText, "progress");
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

  // Transcription results
  if (!transcriptionCompleteUnlisten) {
    transcriptionCompleteUnlisten = await listen<string>("transcription-complete", (event) => {
      // Always buffer the text immediately (appendTranscription handles this)
      // The display update will be deferred if window is hidden and applied
      // when the window becomes visible again via visibilitychange handler
      appendTranscription(event.payload);
    });
  }

  // Speech events
  if (!speechStartedUnlisten) {
    speechStartedUnlisten = await listen("speech-started", () => {
      console.log("[Speech] Started speaking");
      inSpeechSegment = true;
      updateStatusDisplay();
    });
  }

  if (!speechEndedUnlisten) {
    speechEndedUnlisten = await listen<number>("speech-ended", (event) => {
      console.log(`[Speech] Stopped speaking (duration: ${event.payload}ms)`);
      inSpeechSegment = false;
      updateStatusDisplay();
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
          setStatus(`Error: ${event.payload.error}`, "error");
        } else {
          updateStatusDisplay();
        }

        // Update waveform renderer
        if (isCapturing) {
          miniWaveformRenderer?.clear();
          miniWaveformRenderer?.start();
        } else {
          miniWaveformRenderer?.stop();
          miniWaveformRenderer?.clear();
        }
      }
    );
  }

  // PTT events
  if (!pttPressedUnlisten) {
    pttPressedUnlisten = await listen("ptt-pressed", () => {
      console.log("[PTT] Key pressed");
      isPttActive = true;
      updatePttIndicator();
    });
  }

  if (!pttReleasedUnlisten) {
    pttReleasedUnlisten = await listen("ptt-released", () => {
      console.log("[PTT] Key released");
      isPttActive = false;
      updatePttIndicator();
    });
  }

  // Mode changed
  if (!transcriptionModeChangedUnlisten) {
    transcriptionModeChangedUnlisten = await listen<TranscriptionMode>(
      "transcription-mode-changed",
      (event) => {
        console.log("[Mode] Changed to:", event.payload);
        transcriptionMode = event.payload;
        updatePttIndicator();
        updateStatusDisplay();
      }
    );
  }
}

function cleanupEventListeners() {
  visualizationUnlisten?.();
  visualizationUnlisten = null;
  
  transcriptionCompleteUnlisten?.();
  transcriptionCompleteUnlisten = null;
  
  transcriptionErrorUnlisten?.();
  transcriptionErrorUnlisten = null;
  
  speechStartedUnlisten?.();
  speechStartedUnlisten = null;
  
  speechEndedUnlisten?.();
  speechEndedUnlisten = null;
  
  captureStateChangedUnlisten?.();
  captureStateChangedUnlisten = null;
  
  pttPressedUnlisten?.();
  pttPressedUnlisten = null;
  
  pttReleasedUnlisten?.();
  pttReleasedUnlisten = null;
  
  transcriptionModeChangedUnlisten?.();
  transcriptionModeChangedUnlisten = null;
}

// ============== Transcription Display ==============

let transcriptionBuffer = "";
let resultTextSpan: HTMLSpanElement | null = null;
// Track if display needs refresh when window becomes visible
let transcriptionDisplayDirty = false;

function updateTranscriptionDisplay(): void {
  if (!resultEl) return;

  // If document is hidden, mark as dirty and skip update (will refresh on visibility change)
  if (document.hidden) {
    transcriptionDisplayDirty = true;
    return;
  }

  // On first call, find or create the text span (avoid innerHTML replacement)
  if (!resultTextSpan) {
    resultTextSpan = resultEl.querySelector(".result-text");
    if (!resultTextSpan) {
      // Create structure if missing
      resultEl.innerHTML = '<span class="result-text"><span class="result-cursor"></span></span>';
      resultTextSpan = resultEl.querySelector(".result-text");
    }
  }

  if (resultTextSpan) {
    // Get cursor element
    const cursor = resultTextSpan.querySelector(".result-cursor");
    
    // Update text content directly (preserves cursor element)
    // First, remove all text nodes
    const childNodes = Array.from(resultTextSpan.childNodes);
    for (const node of childNodes) {
      if (node.nodeType === Node.TEXT_NODE) {
        resultTextSpan.removeChild(node);
      }
    }
    
    // Insert new text before cursor
    if (transcriptionBuffer.length > 0 && cursor) {
      const textNode = document.createTextNode(transcriptionBuffer);
      resultTextSpan.insertBefore(textNode, cursor);
    }
  }

  resultEl.scrollTop = resultEl.scrollHeight;
  transcriptionDisplayDirty = false;
}

// Handle visibility change to refresh display when window becomes visible
function handleVisibilityChange(): void {
  if (!document.hidden && transcriptionDisplayDirty) {
    // Use requestAnimationFrame to ensure we're in a paint cycle
    requestAnimationFrame(() => {
      updateTranscriptionDisplay();
    });
  }
}

function appendTranscription(newText: string): void {
  if (!resultEl) return;

  const trimmedText = newText.trim();
  if (!trimmedText) return;

  console.log("[Transcription] Received:", trimmedText);

  if (transcriptionBuffer.length > 0) {
    transcriptionBuffer += " " + trimmedText;
  } else {
    transcriptionBuffer = trimmedText;
  }

  // Truncate to keep buffer manageable
  const maxChars = 2000;
  if (transcriptionBuffer.length > maxChars) {
    const startIndex = transcriptionBuffer.length - maxChars;
    const spaceIndex = transcriptionBuffer.indexOf(" ", startIndex);
    if (spaceIndex !== -1) {
      transcriptionBuffer = transcriptionBuffer.substring(spaceIndex + 1);
    } else {
      transcriptionBuffer = transcriptionBuffer.substring(startIndex);
    }
  }

  updateTranscriptionDisplay();
}

// ============== PTT and Mode Control ==============

async function loadPttStatus() {
  try {
    const status = await invoke<PttStatus>("get_ptt_status");
    transcriptionMode = status.mode;
    pttKey = status.key;
    isPttActive = status.is_active;
    
    console.log(`PTT status: mode=${transcriptionMode}, key=${pttKey}`);
    
    // Update UI
    if (modeToggle) {
      modeToggle.checked = status.mode === "push_to_talk";
    }
    
    if (pttKeySelect) {
      pttKeySelect.value = status.key;
      pttKeySelect.disabled = status.mode !== "push_to_talk";
    }
    
    updatePttIndicator();
    
    if (status.error) {
      console.warn("PTT error:", status.error);
    }
  } catch (error) {
    console.error("Failed to load PTT status:", error);
  }
}

function updatePttIndicator() {
  if (pttIndicator) {
    if (transcriptionMode === "push_to_talk" && isPttActive) {
      pttIndicator.classList.remove("hidden");
      pttIndicator.classList.add("active");
      pttIndicator.title = `PTT Active (${KEY_CODE_NAMES[pttKey]} held)`;
    } else if (transcriptionMode === "push_to_talk") {
      pttIndicator.classList.remove("hidden");
      pttIndicator.classList.remove("active");
      pttIndicator.title = `PTT Ready (press ${KEY_CODE_NAMES[pttKey]} to speak)`;
    } else {
      pttIndicator.classList.add("hidden");
      pttIndicator.classList.remove("active");
    }
  }
}

async function onModeToggleChange() {
  if (!modeToggle) return;
  
  const newMode: TranscriptionMode = modeToggle.checked ? "push_to_talk" : "automatic";
  
  try {
    await invoke("set_transcription_mode", { mode: newMode });
    transcriptionMode = newMode;
    
    // Update key selector state
    if (pttKeySelect) {
      pttKeySelect.disabled = newMode !== "push_to_talk";
    }
    
    updatePttIndicator();
    updateStatusDisplay();
    
    console.log(`Transcription mode set to: ${newMode}`);
  } catch (error) {
    console.error("Set transcription mode error:", error);
    setStatus(`Error: ${error}`, "error");
    modeToggle.checked = transcriptionMode === "push_to_talk";
  }
}

async function onPttKeyChange() {
  if (!pttKeySelect) return;
  
  const newKey = pttKeySelect.value as KeyCode;
  
  try {
    await invoke("set_ptt_key", { key: newKey });
    pttKey = newKey;
    updatePttIndicator();
    updateStatusDisplay();
    console.log(`PTT key set to: ${KEY_CODE_NAMES[newKey]}`);
  } catch (error) {
    console.error("Set PTT key error:", error);
    setStatus(`Error: ${error}`, "error");
    pttKeySelect.value = pttKey;
  }
}

function populatePttKeySelect() {
  if (!pttKeySelect) return;
  
  pttKeySelect.innerHTML = "";
  
  for (const [value, name] of Object.entries(KEY_CODE_NAMES)) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = name;
    pttKeySelect.appendChild(option);
  }
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
  const vizWindow = await WebviewWindow.getByLabel("visualization");
  if (!vizWindow) {
    console.error("Visualization window not found");
    return;
  }

  const isVisible = await vizWindow.isVisible();
  if (isVisible) {
    await vizWindow.setFocus();
  } else {
    await vizWindow.show();
    await vizWindow.setFocus();
  }
}

// ============== Initialization ==============

window.addEventListener("DOMContentLoaded", () => {
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
  source1Select = document.querySelector("#source1-select");
  source2Select = document.querySelector("#source2-select");
  modeToggle = document.querySelector("#mode-toggle");
  pttKeySelect = document.querySelector("#ptt-key-select");
  statusEl = document.querySelector("#status");
  resultEl = document.querySelector("#transcription-result");
  modelWarning = document.querySelector("#model-warning");
  modelPathEl = document.querySelector("#model-path");
  downloadModelBtn = document.querySelector("#download-model-btn");
  downloadStatusEl = document.querySelector("#download-status");
  miniWaveformCanvas = document.querySelector("#mini-waveform");
  closeBtn = document.querySelector("#close-btn");
  pttIndicator = document.querySelector("#ptt-indicator");
  cudaIndicator = document.querySelector("#cuda-indicator");

  // Initialize mini waveform renderer
  if (miniWaveformCanvas) {
    miniWaveformRenderer = new MiniWaveformRenderer(miniWaveformCanvas, 64);
    miniWaveformRenderer.drawIdle();

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
  source1Select?.addEventListener("change", onSourceChange);
  source2Select?.addEventListener("change", onSourceChange);
  modeToggle?.addEventListener("change", onModeToggleChange);
  pttKeySelect?.addEventListener("change", onPttKeyChange);
  downloadModelBtn?.addEventListener("click", downloadModel);
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

  // Handle visibility change to refresh transcription display when window becomes visible
  // This ensures updates that arrived while backgrounded are rendered
  document.addEventListener("visibilitychange", handleVisibilityChange);

  // Initialize app
  initializeApp();
});

async function initializeApp() {
  // Set initial status
  setStatus("Initializing...");
  
  // Populate PTT key dropdown
  populatePttKeySelect();
  
  // Set up event listeners (must be done before connect_events so we
  // catch the synthetic CaptureStateChanged sent on subscribe)
  await setupEventListeners();
  
  // Connect to service event stream (service is already operational)
  try {
    await invoke("connect_events");
    console.log("Connected to service event stream");
  } catch (error) {
    console.error("Failed to connect to service:", error);
    setStatus(`Connection error: ${error}`, "error");
    return;
  }
  
  // Fetch current service status to sync UI with existing state.
  // The service may already be capturing if started independently.
  let currentSource1: string | null = null;
  let currentSource2: string | null = null;
  try {
    const status = await invoke<CaptureStatus>("get_status");
    console.log("Service status:", status);
    
    // Sync local state with service
    isCapturing = status.capturing;
    inSpeechSegment = status.in_speech;
    transcribeQueueDepth = status.queue_depth;
    transcriptionMode = status.transcription_mode;
    currentSource1 = status.source1_id;
    currentSource2 = status.source2_id;
    
    if (status.error) {
      setStatus(`Error: ${status.error}`, "error");
    }
  } catch (error) {
    console.error("Failed to get service status:", error);
  }
  
  // Load devices and set dropdowns to match current service configuration
  await loadDevices(currentSource1, currentSource2);
  checkModelStatus();
  checkCudaStatus();
  loadPttStatus();
  
  // Update status display based on synced state
  updateStatusDisplay();
  
  // If capturing, start waveform renderer
  if (isCapturing) {
    miniWaveformRenderer?.clear();
    miniWaveformRenderer?.start();
  }
}
