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

interface SpeechEventPayload {
  duration_ms: number | null;
  lookback_samples: number[] | null;
  lookback_offset_ms: number | null;
}

// DOM elements
let source1Select: HTMLSelectElement | null;
let source2Select: HTMLSelectElement | null;
let monitorToggle: HTMLInputElement | null;
let aecToggle: HTMLInputElement | null;
let modeSelect: HTMLSelectElement | null;
let transcribeToggle: HTMLInputElement | null;
let statusEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let modelWarning: HTMLElement | null;
let modelPathEl: HTMLElement | null;
let downloadModelBtn: HTMLButtonElement | null;
let downloadStatusEl: HTMLElement | null;
let miniWaveformCanvas: HTMLCanvasElement | null;
let closeBtn: HTMLButtonElement | null;

// Recording mode type matching backend
type RecordingMode = "Mixed" | "EchoCancel";

// State
let isMonitoring = false;
let isAecEnabled = false;
let recordingMode: RecordingMode = "Mixed";
let isTranscribing = false;
let inSpeechSegment = false;
let transcribeQueueDepth = 0;
let allDevices: AudioDevice[] = [];
let miniWaveformRenderer: MiniWaveformRenderer | null = null;
let visualizationUnlisten: UnlistenFn | null = null;
let transcriptionCompleteUnlisten: UnlistenFn | null = null;
let transcriptionErrorUnlisten: UnlistenFn | null = null;
let speechStartedUnlisten: UnlistenFn | null = null;
let speechEndedUnlisten: UnlistenFn | null = null;
let recordingSavedUnlisten: UnlistenFn | null = null;
let transcribeQueueUpdateUnlisten: UnlistenFn | null = null;
let transcriptionStartedUnlisten: UnlistenFn | null = null;
let transcriptionFinishedUnlisten: UnlistenFn | null = null;

// CUDA and transcription status
let cudaBuildEnabled = false;
let cudaIndicator: HTMLElement | null = null;
let transcribingIndicator: HTMLElement | null = null;

async function loadDevices() {
  try {
    // Load all available sources
    allDevices = await invoke<AudioDevice[]>("list_all_sources");

    // Populate both source dropdowns
    populateSourceDropdown(source1Select, true);  // Has "None" option, select first device
    populateSourceDropdown(source2Select, false); // Has "None" option, select "None"
    
    // Enable controls if we have at least one device
    const hasDevices = allDevices.length > 0;
    if (transcribeToggle) {
      transcribeToggle.disabled = !hasDevices;
    }
    if (monitorToggle) {
      monitorToggle.disabled = !hasDevices;
    }
    if (aecToggle) {
      aecToggle.disabled = !hasDevices;
    }
    
    // Sync AEC and mode state with backend
    await syncBackendState();
    
    // Update mode selector availability
    updateModeSelector();
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

// Sync frontend state with backend (AEC enabled, recording mode)
async function syncBackendState() {
  try {
    // Sync AEC enabled state
    const backendAecEnabled = await invoke<boolean>("is_aec_enabled");
    isAecEnabled = backendAecEnabled;
    if (aecToggle) {
      aecToggle.checked = backendAecEnabled;
    }
    
    // Sync recording mode
    const backendMode = await invoke<RecordingMode>("get_recording_mode");
    recordingMode = backendMode;
    if (modeSelect) {
      modeSelect.value = backendMode;
    }
  } catch (error) {
    console.error("Failed to sync backend state:", error);
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

function hasAnySourceSelected(): boolean {
  const { source1Id, source2Id } = getSelectedSources();
  return source1Id !== null || source2Id !== null;
}

// Handle source selection changes - reconfigure capture if active
async function onSourceChange() {
  // Always update mode selector when sources change
  updateModeSelector();
  
  if (!isMonitoring && !isTranscribing) {
    // Not active, nothing to do
    return;
  }

  if (!hasAnySourceSelected()) {
    // No sources selected - stop everything
    if (isTranscribing) {
      // Can't transcribe with no sources - stop transcribing
      setStatus("Transcription stopped: no sources selected", "error");
      try {
        await invoke("stop_transcribe_mode");
      } catch (e) {
        console.error("Error stopping transcribe mode:", e);
      }
      isTranscribing = false;
      isMonitoring = false;
      if (transcribeToggle) {
        transcribeToggle.checked = false;
      }
      if (monitorToggle) {
        monitorToggle.checked = false;
        monitorToggle.disabled = false;
      }
      miniWaveformRenderer?.stop();
      miniWaveformRenderer?.clear();
      await cleanupVisualizationListener();
    } else if (isMonitoring) {
      // Stop monitoring
      try {
        await invoke("stop_monitor");
      } catch (e) {
        console.error("Error stopping monitor:", e);
      }
      isMonitoring = false;
      if (monitorToggle) {
        monitorToggle.checked = false;
      }
      miniWaveformRenderer?.stop();
      miniWaveformRenderer?.clear();
      await cleanupVisualizationListener();
      setStatus("");
    }
    return;
  }

  // Reconfigure with new sources
  const { source1Id, source2Id } = getSelectedSources();

  if (isTranscribing) {
    // Restart transcribe mode with new sources
    try {
      await invoke("stop_transcribe_mode");
      await invoke("start_transcribe_mode", { source1Id, source2Id });
      updateStatusForCurrentState();
    } catch (error) {
      console.error("Error reconfiguring transcribe mode:", error);
      setStatus(`Error: ${error}`, "error");
    }
  } else if (isMonitoring) {
    // Restart monitoring with new sources
    try {
      await invoke("start_monitor", { source1Id, source2Id });
      updateStatusForCurrentState();
    } catch (error) {
      console.error("Error reconfiguring monitor:", error);
      setStatus(`Error: ${error}`, "error");
    }
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

async function setupVisualizationListener() {
  if (visualizationUnlisten) return;

  visualizationUnlisten = await listen<VisualizationPayload>("visualization-data", (event) => {
    // Push pre-downsampled waveform data to mini waveform
    if (miniWaveformRenderer) {
      miniWaveformRenderer.pushSamples(event.payload.waveform);
    }
  });
}

async function cleanupVisualizationListener() {
  if (visualizationUnlisten) {
    visualizationUnlisten();
    visualizationUnlisten = null;
  }
}

// Transcription text buffer - stores accumulated text
let transcriptionBuffer = "";

// Update display with text and cursor
function updateTranscriptionDisplay(): void {
  if (!resultEl) return;
  
  // Clear and rebuild content with cursor
  resultEl.innerHTML = "";
  
  // Create wrapper for text content (allows flex bottom-align while keeping inline flow)
  const textWrapper = document.createElement("span");
  textWrapper.className = "result-text";
  
  // Add text
  if (transcriptionBuffer.length > 0) {
    textWrapper.appendChild(document.createTextNode(transcriptionBuffer));
  }
  
  // Add blinking block cursor inline with text
  const cursor = document.createElement("span");
  cursor.className = "result-cursor";
  textWrapper.appendChild(cursor);
  
  resultEl.appendChild(textWrapper);
  
  // Auto-scroll to bottom (ensure newest text is visible)
  resultEl.scrollTop = resultEl.scrollHeight;
}

// Append transcription text and manage buffer
function appendTranscription(newText: string): void {
  if (!resultEl) return;
  
  // Trim the new text and skip if empty
  const trimmedText = newText.trim();
  if (!trimmedText) return;
  
  // Append with space separator (no line breaks)
  if (transcriptionBuffer.length > 0) {
    transcriptionBuffer += " " + trimmedText;
  } else {
    transcriptionBuffer = trimmedText;
  }
  
  // Truncate to keep buffer from growing indefinitely
  // Keep enough text to fill the panel with overflow for clipping effect
  // ~80 chars per line, ~20 lines = ~1600 chars max
  const maxChars = 2000;
  if (transcriptionBuffer.length > maxChars) {
    // Find a word boundary to truncate at
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

async function setupTranscriptionListeners() {
  if (transcriptionCompleteUnlisten) return;

  transcriptionCompleteUnlisten = await listen<string>("transcription-complete", (event) => {
    appendTranscription(event.payload);
    if (isMonitoring) {
      setStatus("Monitoring...", "progress");
    } else {
      setStatus("Transcription complete");
    }
  });

  transcriptionErrorUnlisten = await listen<string>("transcription-error", (event) => {
    console.error("Transcription error:", event.payload);
    setStatus(`Transcription error: ${event.payload}`, "error");
  });
}



async function setupSpeechEventListeners() {
  if (speechStartedUnlisten) return;

  speechStartedUnlisten = await listen<SpeechEventPayload>("speech-started", (_event) => {
    console.log("[Speech] Started speaking");
    // Track speech segment state for transcribe mode
    if (isTranscribing) {
      inSpeechSegment = true;
      updateStatusForCurrentState();
    }
  });

  speechEndedUnlisten = await listen<SpeechEventPayload>("speech-ended", (event) => {
    const duration = event.payload.duration_ms;
    console.log(`[Speech] Stopped speaking (duration: ${duration}ms)`);
    // Track speech segment state for transcribe mode
    if (isTranscribing) {
      inSpeechSegment = false;
      updateStatusForCurrentState();
    }
  });
}

function cleanupSpeechEventListeners() {
  if (speechStartedUnlisten) {
    speechStartedUnlisten();
    speechStartedUnlisten = null;
  }
  if (speechEndedUnlisten) {
    speechEndedUnlisten();
    speechEndedUnlisten = null;
  }
}

async function setupRecordingSavedListener() {
  if (recordingSavedUnlisten) return;

  recordingSavedUnlisten = await listen<string>("recording-saved", (event) => {
    console.log(`[Recording] Saved to: ${event.payload}`);
    // Show brief notification in status
    if (statusEl) {
      const currentStatus = statusEl.textContent || "";
      if (!currentStatus.includes("Error")) {
        const savedMsg = `Saved: ${event.payload}`;
        // Briefly show saved message, then restore status
        const prevStatus = currentStatus;
        const prevClass = statusEl.className;
        statusEl.textContent = savedMsg;
        statusEl.className = "status";
        setTimeout(() => {
          if (statusEl && statusEl.textContent === savedMsg) {
            statusEl.textContent = prevStatus;
            statusEl.className = prevClass;
          }
        }, 3000);
      }
    }
  });
}

function cleanupRecordingSavedListener() {
  if (recordingSavedUnlisten) {
    recordingSavedUnlisten();
    recordingSavedUnlisten = null;
  }
}

async function setupTranscribeQueueListener() {
  if (transcribeQueueUpdateUnlisten) return;

  transcribeQueueUpdateUnlisten = await listen<number>("transcribe-queue-update", (event) => {
    transcribeQueueDepth = event.payload;
    // Update status to show queue depth if transcribing
    if (isTranscribing) {
      updateStatusForCurrentState();
    }
  });
}

function cleanupTranscribeQueueListener() {
  if (transcribeQueueUpdateUnlisten) {
    transcribeQueueUpdateUnlisten();
    transcribeQueueUpdateUnlisten = null;
  }
}

function cleanupTranscriptionListeners() {
  if (transcriptionCompleteUnlisten) {
    transcriptionCompleteUnlisten();
    transcriptionCompleteUnlisten = null;
  }
  if (transcriptionErrorUnlisten) {
    transcriptionErrorUnlisten();
    transcriptionErrorUnlisten = null;
  }
}

// Check CUDA build status and show indicator if enabled
async function checkCudaStatus() {
  try {
    const status = await invoke<CudaStatus>("get_cuda_status");
    cudaBuildEnabled = status.build_enabled;
    
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
    console.log(`Whisper system info: ${status.system_info}`);
  } catch (error) {
    console.error("Failed to check CUDA status:", error);
  }
}

// Setup listeners for transcription active state
async function setupTranscriptionActiveListeners() {
  transcriptionStartedUnlisten = await listen("transcription-started", () => {
    console.log(`[Transcription] Started (CUDA: ${cudaBuildEnabled})`);
    if (transcribingIndicator) {
      transcribingIndicator.classList.add("active");
    }
  });
  
  transcriptionFinishedUnlisten = await listen("transcription-finished", () => {
    console.log(`[Transcription] Finished (CUDA: ${cudaBuildEnabled})`);
    if (transcribingIndicator) {
      transcribingIndicator.classList.remove("active");
    }
  });
}

function cleanupTranscriptionActiveListeners() {
  if (transcriptionStartedUnlisten) {
    transcriptionStartedUnlisten();
    transcriptionStartedUnlisten = null;
  }
  if (transcriptionFinishedUnlisten) {
    transcriptionFinishedUnlisten();
    transcriptionFinishedUnlisten = null;
  }
}

async function toggleAec() {
  if (!aecToggle) return;

  const newState = aecToggle.checked;
  try {
    await invoke("set_aec_enabled", { enabled: newState });
    isAecEnabled = newState;
    console.log(`Echo cancellation ${isAecEnabled ? "enabled" : "disabled"}`);
  } catch (error) {
    console.error("Toggle AEC error:", error);
    // Revert toggle on error
    aecToggle.checked = !newState;
  }
}

async function onModeChange() {
  if (!modeSelect) return;

  const newMode = modeSelect.value as RecordingMode;
  try {
    await invoke("set_recording_mode", { mode: newMode });
    recordingMode = newMode;
    console.log(`Recording mode set to: ${recordingMode}`);
    
    // Update status if currently monitoring/transcribing
    if (isMonitoring || isTranscribing) {
      updateStatusForCurrentState();
    }
  } catch (error) {
    console.error("Set recording mode error:", error);
    // Revert selector on error
    modeSelect.value = recordingMode;
  }
}

// Update the mode selector availability based on source selection
function updateModeSelector() {
  if (!modeSelect) return;

  const { source1Id, source2Id } = getSelectedSources();
  const hasTwoSources = source1Id !== null && source2Id !== null;
  
  // Enable/disable the mode selector based on source count
  modeSelect.disabled = !hasTwoSources;
  
  // If only one source is selected and mode was EchoCancel, switch to Mixed
  if (!hasTwoSources && recordingMode === "EchoCancel") {
    modeSelect.value = "Mixed";
    recordingMode = "Mixed";
    invoke("set_recording_mode", { mode: "Mixed" }).catch(e => {
      console.error("Failed to reset recording mode:", e);
    });
  }
}

// Update status message based on current state
function updateStatusForCurrentState() {
  const { source1Id, source2Id } = getSelectedSources();
  const hasTwoSources = source1Id !== null && source2Id !== null;
  
  let statusText: string;
  if (isTranscribing) {
    if (inSpeechSegment) {
      statusText = "Recording speech...";
    } else if (transcribeQueueDepth > 0) {
      statusText = `Listening... (${transcribeQueueDepth} pending)`;
    } else {
      statusText = "Listening...";
    }
  } else if (isMonitoring) {
    if (hasTwoSources) {
      statusText = recordingMode === "EchoCancel" 
        ? "Monitoring (Voice Only)..." 
        : "Monitoring (Mixed)...";
    } else {
      statusText = "Monitoring...";
    }
  } else {
    statusText = "";
  }
  setStatus(statusText, statusText ? "progress" : "normal");
}

async function toggleMonitor() {
  if (!monitorToggle) return;

  if (isMonitoring) {
    // Stop monitoring
    try {
      await invoke("stop_monitor");
      isMonitoring = false;
      monitorToggle.checked = false;
      setStatus("");
      
      miniWaveformRenderer?.stop();
      miniWaveformRenderer?.clear();
      await cleanupVisualizationListener();
    } catch (error) {
      console.error("Stop monitor error:", error);
      setStatus(`Error: ${error}`, "error");
      monitorToggle.checked = true; // Revert toggle on error
    }
  } else {
    // Start monitoring
    if (!hasAnySourceSelected()) {
      setStatus("Please select at least one audio source", "error");
      monitorToggle.checked = false;
      return;
    }

    const { source1Id, source2Id } = getSelectedSources();

    try {
      await setupVisualizationListener();
      await invoke("start_monitor", { 
        source1Id,
        source2Id,
      });
      isMonitoring = true;
      monitorToggle.checked = true;
      
      updateStatusForCurrentState();
      
      miniWaveformRenderer?.clear();
      miniWaveformRenderer?.start();
    } catch (error) {
      console.error("Start monitor error:", error);
      setStatus(`Error: ${error}`, "error");
      monitorToggle.checked = false;
      await cleanupVisualizationListener();
    }
  }
}

async function toggleTranscribe() {
  if (!transcribeToggle) return;

  if (isTranscribing) {
    // Stop transcribe mode
    try {
      await invoke("stop_transcribe_mode");
      
      isTranscribing = false;
      isMonitoring = false;
      inSpeechSegment = false;
      transcribeQueueDepth = 0;
      transcribeToggle.checked = false;
      
      // Re-enable monitor toggle
      if (monitorToggle) {
        monitorToggle.disabled = false;
        monitorToggle.checked = false;
      }

      miniWaveformRenderer?.stop();
      miniWaveformRenderer?.clear();
      await cleanupVisualizationListener();
      setStatus("");
    } catch (error) {
      console.error("Stop transcribe mode error:", error);
      setStatus(`Error: ${error}`, "error");
      isTranscribing = false;
      isMonitoring = false;
      inSpeechSegment = false;
      transcribeQueueDepth = 0;
      transcribeToggle.checked = false;
      if (monitorToggle) {
        monitorToggle.disabled = false;
        monitorToggle.checked = false;
      }
      miniWaveformRenderer?.stop();
      miniWaveformRenderer?.clear();
      await cleanupVisualizationListener();
    }
  } else {
    // Start transcribe mode
    if (!hasAnySourceSelected()) {
      setStatus("Please select at least one audio source", "error");
      transcribeToggle.checked = false;
      return;
    }

    const { source1Id, source2Id } = getSelectedSources();

    try {
      await setupVisualizationListener();
      await setupTranscriptionListeners();
      await setupTranscribeQueueListener();
      
      await invoke("start_transcribe_mode", { 
        source1Id,
        source2Id,
      });
      isTranscribing = true;
      isMonitoring = true;
      transcribeToggle.checked = true;
      
      updateStatusForCurrentState();
      
      // Disable monitor toggle during transcribe mode (monitoring is implicit)
      if (monitorToggle) {
        monitorToggle.disabled = true;
        monitorToggle.checked = true;
      }

      if (!miniWaveformRenderer?.active) {
        miniWaveformRenderer?.clear();
      }
      miniWaveformRenderer?.start();
    } catch (error) {
      console.error("Start transcribe mode error:", error);
      setStatus(`Error: ${error}`, "error");
      transcribeToggle.checked = false;
      if (!isMonitoring) {
        await cleanupVisualizationListener();
      }
    }
  }
}

// Close the app - closes all windows
async function closeApp() {
  // Close visualization window if it exists
  const vizWindow = await WebviewWindow.getByLabel("visualization");
  if (vizWindow) {
    await vizWindow.destroy();
  }

  // Close main window (this will exit the app since it's the last window)
  const mainWindow = getCurrentWindow();
  await mainWindow.destroy();
}

// Open the visualization window (or focus if already open)
async function openVisualizationWindow() {
  // Get the pre-configured visualization window
  const vizWindow = await WebviewWindow.getByLabel("visualization");
  if (!vizWindow) {
    console.error("Visualization window not found");
    return;
  }

  // Check if already visible
  const isVisible = await vizWindow.isVisible();
  if (isVisible) {
    // Already visible, just focus it
    await vizWindow.setFocus();
  } else {
    // Show the window
    await vizWindow.show();
    await vizWindow.setFocus();
  }
}

window.addEventListener("DOMContentLoaded", () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  source1Select = document.querySelector("#source1-select");
  source2Select = document.querySelector("#source2-select");
  monitorToggle = document.querySelector("#monitor-toggle");
  aecToggle = document.querySelector("#aec-toggle");
  modeSelect = document.querySelector("#mode-select");
  transcribeToggle = document.querySelector("#transcribe-toggle");
  statusEl = document.querySelector("#status");
  resultEl = document.querySelector("#transcription-result");
  modelWarning = document.querySelector("#model-warning");
  modelPathEl = document.querySelector("#model-path");
  downloadModelBtn = document.querySelector("#download-model-btn");
  downloadStatusEl = document.querySelector("#download-status");
  miniWaveformCanvas = document.querySelector("#mini-waveform");

  // Initialize mini waveform renderer
  if (miniWaveformCanvas) {
    miniWaveformRenderer = new MiniWaveformRenderer(miniWaveformCanvas);
    miniWaveformRenderer.drawIdle();
    
    // Add double-click handler to open visualization window
    miniWaveformCanvas.addEventListener("dblclick", (e) => {
      e.preventDefault();
      e.stopPropagation();
      console.log("Mini waveform double-clicked, opening visualization window...");
      openVisualizationWindow();
    });
  }

  // Handle window resize
  window.addEventListener("resize", () => {
    if (miniWaveformCanvas && miniWaveformRenderer) {
      miniWaveformRenderer.resize();
    }
  });

  // Setup transcription, speech, and recording event listeners early (always on)
  setupTranscriptionListeners();
  setupSpeechEventListeners();
  setupRecordingSavedListener();

  closeBtn = document.querySelector("#close-btn");

  monitorToggle?.addEventListener("change", toggleMonitor);
  aecToggle?.addEventListener("change", toggleAec);
  modeSelect?.addEventListener("change", onModeChange);
  transcribeToggle?.addEventListener("change", toggleTranscribe);
  downloadModelBtn?.addEventListener("click", downloadModel);
  source1Select?.addEventListener("change", onSourceChange);
  source2Select?.addEventListener("change", onSourceChange);
  closeBtn?.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    await closeApp();
  });

  // Cleanup listeners on app close
  window.addEventListener("beforeunload", () => {
    cleanupVisualizationListener();
    cleanupTranscriptionListeners();
    cleanupSpeechEventListeners();
    cleanupRecordingSavedListener();
    cleanupTranscribeQueueListener();
    cleanupTranscriptionActiveListeners();
  });

  // Initialize CUDA and transcription indicators
  cudaIndicator = document.querySelector("#cuda-indicator");
  transcribingIndicator = document.querySelector("#transcribing-indicator");
  
  loadDevices();
  checkModelStatus();
  checkCudaStatus();
  setupTranscriptionActiveListeners();
});
