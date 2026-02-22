import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { initTheme } from "./theme";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AudioDevice {
  id: string;
  name: string;
  source_type: string;
}

interface ModelStatus {
  available: boolean;
  path: string;
}

interface HotkeyCombination {
  keys: string[];
}

interface TranscriptionResult {
  text: string;
}

// ---------------------------------------------------------------------------
// Key mapping (subset from config.ts)
// ---------------------------------------------------------------------------

const KEY_DISPLAY_NAMES: Record<string, string> = {
  right_alt: "Right Alt", left_alt: "Left Alt",
  right_control: "Right Ctrl", left_control: "Left Ctrl",
  right_shift: "Right Shift", left_shift: "Left Shift",
  caps_lock: "Caps Lock", left_meta: "Left Win", right_meta: "Right Win",
  f1: "F1", f2: "F2", f3: "F3", f4: "F4", f5: "F5", f6: "F6",
  f7: "F7", f8: "F8", f9: "F9", f10: "F10", f11: "F11", f12: "F12",
  f13: "F13", f14: "F14", f15: "F15", f16: "F16",
  escape: "Esc", tab: "Tab", space: "Space", enter: "Enter",
};

const BROWSER_CODE_MAP: Record<string, string> = {
  AltRight: "right_alt", AltLeft: "left_alt",
  ControlRight: "right_control", ControlLeft: "left_control",
  ShiftRight: "right_shift", ShiftLeft: "left_shift",
  CapsLock: "caps_lock", MetaLeft: "left_meta", MetaRight: "right_meta",
  F1: "f1", F2: "f2", F3: "f3", F4: "f4", F5: "f5", F6: "f6",
  F7: "f7", F8: "f8", F9: "f9", F10: "f10", F11: "f11", F12: "f12",
  F13: "f13", F14: "f14", F15: "f15", F16: "f16",
  Escape: "escape", Tab: "tab", Space: "space", Enter: "enter",
};

function keyDisplayName(code: string): string {
  return KEY_DISPLAY_NAMES[code] || code;
}

function isDebugConsoleHotkey(e: KeyboardEvent): boolean {
  const isIKey = e.code === "KeyI" || e.key === "i" || e.key === "I";
  const isCtrlShift = e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;
  const isMetaAlt = e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey;
  return isIKey && (isCtrlShift || isMetaAlt);
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

// Detect macOS at startup for conditional accessibility step.
// navigator.platform is deprecated but still reliable in Tauri webviews.
const isMacOS = navigator.platform.toLowerCase().includes("mac");

// Ordered list of step IDs that are active in this session.
// "step-accessibility" is inserted only on macOS + PTT mode.
let activeSteps: string[] = [];

function buildActiveSteps(): string[] {
  const steps = ["step-1", "step-2", "step-3"];
  if (isMacOS && transcriptionMode === "push_to_talk") {
    steps.push("step-accessibility");
  }
  steps.push("step-4");
  return steps;
}

// Current position index into activeSteps (0-based).
let currentStepIndex = 0;

let modelDownloaded = false;
let selectedDeviceId: string | null = null;
let selectedSystemDeviceId: string | null = null;
let transcriptionMode: "automatic" | "push_to_talk" = "push_to_talk";
let pttHotkey: HotkeyCombination = { keys: ["right_alt"] };
let isRecordingHotkey = false;
let recordedKeys: Set<string> = new Set();
let currentlyHeldKeys: Set<string> = new Set();
let releaseTimer: number | null = null;

// Accessibility permission polling
let accessibilityPollInterval: number | null = null;

// ---------------------------------------------------------------------------
// DOM refs (filled on DOMContentLoaded)
// ---------------------------------------------------------------------------

let downloadBtn: HTMLButtonElement;
let downloadLabel: HTMLSpanElement;
let progressContainer: HTMLDivElement;
let progressBar: HTMLDivElement;
let progressText: HTMLSpanElement;
let downloadStatusEl: HTMLDivElement;
let deviceListEl: HTMLDivElement;
let levelMeterSection: HTMLDivElement;
let levelMeterFill: HTMLDivElement;
let levelLabel: HTMLSpanElement;
let systemDeviceSelect: HTMLSelectElement;
let hotkeySection: HTMLDivElement;
let hotkeyLabel: HTMLSpanElement;
let changeHotkeyBtn: HTMLButtonElement;
let hotkeyRecorder: HTMLDivElement;
let recorderStatus: HTMLSpanElement;
let testInstructions: HTMLParagraphElement;
let testResult: HTMLDivElement;
let backBtn: HTMLButtonElement;
let nextBtn: HTMLButtonElement;
let skipLink: HTMLAnchorElement;

// Accessibility step DOM refs
let accessibilityStatusEl: HTMLDivElement;
let accessibilityStatusLabel: HTMLSpanElement;
let accessibilityStatusIcon: HTMLSpanElement;
let openAccessibilityBtn: HTMLButtonElement;

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

function renderStepIndicator() {
  const indicator = document.getElementById("step-indicator");
  if (!indicator) return;
  indicator.innerHTML = "";
  activeSteps.forEach((_stepId, i) => {
    const dot = document.createElement("div");
    dot.className = "step-dot";
    dot.dataset.step = String(i + 1);
    dot.classList.toggle("active", i === currentStepIndex);
    dot.classList.toggle("done", i < currentStepIndex);
    indicator.appendChild(dot);
    if (i < activeSteps.length - 1) {
      const line = document.createElement("div");
      line.className = "step-line";
      indicator.appendChild(line);
    }
  });
}

function showStepById(stepId: string) {
  // Clear accessibility polling when leaving that step
  if (activeSteps[currentStepIndex] === "step-accessibility") {
    clearAccessibilityPoll();
  }

  // Hide all steps
  const allStepIds = ["step-1", "step-2", "step-3", "step-accessibility", "step-4"];
  allStepIds.forEach((id) => {
    const el = document.getElementById(id);
    if (el) el.classList.add("hidden");
  });

  const target = document.getElementById(stepId);
  if (target) target.classList.remove("hidden");

  currentStepIndex = activeSteps.indexOf(stepId);
  renderStepIndicator();

  const isLast = currentStepIndex === activeSteps.length - 1;
  const isFirst = currentStepIndex === 0;

  // Back button visibility
  backBtn.classList.toggle("hidden", isFirst);

  // Next button label and state
  if (isLast) {
    nextBtn.textContent = "Finish";
    nextBtn.disabled = false;
  } else {
    nextBtn.textContent = "Next";
    updateNextEnabled();
  }

  // Skip link only on first step
  skipLink.classList.toggle("hidden", !isFirst);

  // Step-specific initialization
  if (stepId === "step-2") initDeviceStep();
  if (stepId === "step-3") initModeStep();
  if (stepId === "step-accessibility") initAccessibilityStep();
  if (stepId === "step-4") initTestStep();
}

function updateNextEnabled() {
  const stepId = activeSteps[currentStepIndex];
  switch (stepId) {
    case "step-1":
      nextBtn.disabled = !modelDownloaded;
      break;
    case "step-2":
      nextBtn.disabled = !selectedDeviceId;
      break;
    case "step-3":
      nextBtn.disabled = false;
      break;
    case "step-accessibility":
      // Disabled until permission confirmed; managed by initAccessibilityStep polling
      break;
    case "step-4":
      nextBtn.disabled = false;
      break;
  }
}

async function handleNext() {
  if (currentStepIndex < activeSteps.length - 1) {
    showStepById(activeSteps[currentStepIndex + 1]);
  } else {
    await completeSetup();
  }
}

function handleBack() {
  if (currentStepIndex > 0) {
    const currentId = activeSteps[currentStepIndex];
    // Stop test capture when leaving step 2
    if (currentId === "step-2") {
      invoke("stop_test_audio_device").catch(() => {});
    }
    // Stop capture and hotkey listening when leaving step 4
    if (currentId === "step-4") {
      invoke("set_sources", { source1Id: null, source2Id: null }).catch(() => {});
    }
    showStepById(activeSteps[currentStepIndex - 1]);
  }
}

// ---------------------------------------------------------------------------
// Step 1: Model Download
// ---------------------------------------------------------------------------

async function checkModelStatus() {
  try {
    const status = await invoke<ModelStatus>("check_model_status");
    if (status.available) {
      modelDownloaded = true;
      downloadLabel.textContent = "Model already downloaded";
      downloadStatusEl.classList.add("success");
      downloadBtn.classList.add("hidden");
      updateNextEnabled();
      return true;
    }
  } catch {
    // Service not connected yet; will download when user clicks
  }
  return false;
}

async function startDownload() {
  downloadBtn.disabled = true;
  downloadBtn.textContent = "Downloading...";
  progressContainer.classList.remove("hidden");
  downloadLabel.textContent = "Downloading model...";

  try {
    await invoke("download_model");
  } catch (err) {
    downloadLabel.textContent = `Download failed: ${err}`;
    downloadStatusEl.classList.add("error");
    downloadBtn.disabled = false;
    downloadBtn.textContent = "Retry Download";
    progressContainer.classList.add("hidden");
  }
}

// ---------------------------------------------------------------------------
// Step 2: Audio Device Selection
// ---------------------------------------------------------------------------

async function initDeviceStep() {
  try {
    const devices = await invoke<AudioDevice[]>("list_all_sources");
    const inputDevices = devices.filter((d) => d.source_type === "input");
    const systemDevices = devices.filter((d) => d.source_type === "system");

    deviceListEl.innerHTML = "";
    if (inputDevices.length === 0) {
      deviceListEl.innerHTML = '<div class="device-loading">No input devices found</div>';
    } else {
      inputDevices.forEach((device) => {
        const label = document.createElement("label");
        label.className = "device-option";

        const radio = document.createElement("input");
        radio.type = "radio";
        radio.name = "device";
        radio.value = device.id;
        radio.checked = device.id === selectedDeviceId;

        const card = document.createElement("div");
        card.className = "device-card";
        card.textContent = device.name;

        label.appendChild(radio);
        label.appendChild(card);
        label.addEventListener("click", () => selectDevice(device.id));
        deviceListEl.appendChild(label);
      });
    }

    // Show level meter if a device is already selected
    levelMeterSection.classList.toggle("hidden", !selectedDeviceId);

    // Populate system device dropdown
    systemDeviceSelect.innerHTML = '<option value="">None</option>';
    systemDevices.forEach((device) => {
      const option = document.createElement("option");
      option.value = device.id;
      option.textContent = device.name;
      systemDeviceSelect.appendChild(option);
    });
    if (selectedSystemDeviceId) {
      systemDeviceSelect.value = selectedSystemDeviceId;
    }

    // If a device was previously selected, restart test capture
    if (selectedDeviceId) {
      try {
        await invoke("test_audio_device", { deviceId: selectedDeviceId });
      } catch {
        // Ignore - device may no longer be available
      }
    }
  } catch (err) {
    deviceListEl.innerHTML = `<div class="device-loading">Error: ${err}</div>`;
  }
}

async function selectDevice(deviceId: string) {
  selectedDeviceId = deviceId;

  // Update radio button state
  document.querySelectorAll<HTMLInputElement>('input[name="device"]').forEach((radio) => {
    radio.checked = radio.value === deviceId;
  });

  // Show level meter
  levelMeterSection.classList.remove("hidden");
  levelMeterFill.style.width = "0%";
  levelLabel.textContent = "Starting...";

  updateNextEnabled();

  // Start test capture for level meter
  try {
    await invoke("test_audio_device", { deviceId });
    levelLabel.textContent = "Speak to test...";
  } catch (err) {
    levelLabel.textContent = `Error: ${err}`;
  }
}

// ---------------------------------------------------------------------------
// Step 3: Mode Selection
// ---------------------------------------------------------------------------

function initModeStep() {
  const modeRadios = document.querySelectorAll<HTMLInputElement>('input[name="mode"]');
  modeRadios.forEach((radio) => {
    radio.checked = radio.value === transcriptionMode;
  });
  hotkeySection.classList.toggle("hidden", transcriptionMode === "automatic");
  hotkeyLabel.textContent = pttHotkey.keys.map(keyDisplayName).join(" + ");
}

function onModeChange(mode: string) {
  transcriptionMode = mode as "automatic" | "push_to_talk";
  hotkeySection.classList.toggle("hidden", mode === "automatic");
  // Rebuild active step list when mode changes (accessibility step is conditional)
  activeSteps = buildActiveSteps();
  // Re-render indicator to reflect new total
  renderStepIndicator();
  updateNextEnabled();
}

// ---------------------------------------------------------------------------
// Hotkey Recording
// ---------------------------------------------------------------------------

function startHotkeyRecording() {
  isRecordingHotkey = true;
  recordedKeys.clear();
  currentlyHeldKeys.clear();
  hotkeyRecorder.classList.remove("hidden");
  changeHotkeyBtn.classList.add("hidden");
  recorderStatus.textContent = "Press a key...";
}

function stopHotkeyRecording(cancelled: boolean) {
  isRecordingHotkey = false;
  hotkeyRecorder.classList.add("hidden");
  changeHotkeyBtn.classList.remove("hidden");

  if (releaseTimer !== null) {
    clearTimeout(releaseTimer);
    releaseTimer = null;
  }

  if (!cancelled && recordedKeys.size > 0) {
    pttHotkey = { keys: Array.from(recordedKeys) };
    hotkeyLabel.textContent = pttHotkey.keys.map(keyDisplayName).join(" + ");
  }
  recordedKeys.clear();
}

function handleRecordKeyDown(e: KeyboardEvent) {
  if (!isRecordingHotkey) return;
  e.preventDefault();
  e.stopPropagation();

  if (e.code === "Escape") { stopHotkeyRecording(true); return; }

  if (releaseTimer !== null) { clearTimeout(releaseTimer); releaseTimer = null; }

  const keyCode = BROWSER_CODE_MAP[e.code];
  if (!keyCode) return;

  currentlyHeldKeys.add(keyCode);
  recordedKeys.add(keyCode);
  recorderStatus.textContent = Array.from(recordedKeys).map(keyDisplayName).join(" + ");
}

function handleRecordKeyUp(e: KeyboardEvent) {
  if (!isRecordingHotkey) return;
  e.preventDefault();
  e.stopPropagation();

  const keyCode = BROWSER_CODE_MAP[e.code];
  if (keyCode) currentlyHeldKeys.delete(keyCode);

  if (currentlyHeldKeys.size === 0 && recordedKeys.size > 0) {
    if (releaseTimer !== null) clearTimeout(releaseTimer);
    releaseTimer = window.setTimeout(() => {
      if (isRecordingHotkey) stopHotkeyRecording(false);
    }, 200);
  }
}

// ---------------------------------------------------------------------------
// Step: Accessibility Permission (macOS + PTT only)
// ---------------------------------------------------------------------------

function clearAccessibilityPoll() {
  if (accessibilityPollInterval !== null) {
    clearInterval(accessibilityPollInterval);
    accessibilityPollInterval = null;
  }
}

async function initAccessibilityStep() {
  // Reset to pending state
  nextBtn.disabled = true;
  accessibilityStatusEl.className = "accessibility-status pending";
  accessibilityStatusIcon.textContent = "⏳";
  accessibilityStatusLabel.textContent = "Waiting for permission...";

  // Check once immediately in case permission was already granted.
  // The check runs in the service process (which is the one that needs the permission).
  const alreadyGranted = await invoke<boolean>("check_accessibility_permission").catch(() => false);
  if (alreadyGranted) {
    onAccessibilityGranted();
    return;
  }

  // Trigger the macOS accessibility prompt for the app process automatically.
  // This calls AXIsProcessTrustedWithOptions with the prompt flag, which shows
  // the system dialog asking the user to grant access to FlowSTT.
  invoke("open_accessibility_settings").catch(() => {});

  // Poll every 500ms until permission is granted
  clearAccessibilityPoll();
  accessibilityPollInterval = window.setInterval(async () => {
    const granted = await invoke<boolean>("check_accessibility_permission").catch(() => false);
    if (granted) {
      clearAccessibilityPoll();
      onAccessibilityGranted();
    }
  }, 500);
}

function onAccessibilityGranted() {
  accessibilityStatusEl.className = "accessibility-status granted";
  accessibilityStatusIcon.textContent = "✓";
  accessibilityStatusLabel.textContent = "Accessibility access granted!";
  nextBtn.disabled = false;
}

// ---------------------------------------------------------------------------
// Step 4: Test Verification
// ---------------------------------------------------------------------------

async function initTestStep() {
  testResult.innerHTML = '<p class="test-placeholder">Transcribed text will appear here...</p>';

  if (transcriptionMode === "push_to_talk") {
    const keyNames = pttHotkey.keys.map(keyDisplayName).join(" + ");
    testInstructions.innerHTML = `Press and hold <strong>${keyNames}</strong>, then speak to verify everything works.`;
  } else {
    testInstructions.innerHTML = "Speak to verify everything works. Transcription starts automatically.";
  }

  // Stop the device test capture before starting real capture
  try {
    await invoke("stop_test_audio_device");
  } catch {
    // Ignore - may not be running
  }

  // Now configure service with selected device and mode, then start capture.
  // This is the first time hotkey/PTT listening is activated -- only on
  // the test page so the user isn't surprised by keypress interception.
  try {
    await invoke("set_transcription_mode", { mode: transcriptionMode });
    if (transcriptionMode === "push_to_talk") {
      await invoke("set_ptt_hotkeys", { hotkeys: [pttHotkey] });
    }
    await invoke("set_sources", {
      source1Id: selectedDeviceId,
      source2Id: selectedSystemDeviceId || null,
    });
  } catch (err) {
    testResult.innerHTML = `<p class="test-placeholder">Setup error: ${err}</p>`;
  }
}

// ---------------------------------------------------------------------------
// Setup Completion
// ---------------------------------------------------------------------------

async function completeSetup() {
  nextBtn.disabled = true;
  nextBtn.textContent = "Finishing...";

  // Stop any test capture
  invoke("stop_test_audio_device").catch(() => {});

  try {
    await invoke("complete_setup", {
      transcriptionMode,
      hotkeys: [pttHotkey],
      source1Id: selectedDeviceId,
      source2Id: selectedSystemDeviceId || null,
    });
  } catch (err) {
    console.error("Failed to complete setup:", err);
    nextBtn.disabled = false;
    nextBtn.textContent = "Finish";
  }
}

async function skipSetup() {
  // Close the entire app -- setup was not completed
  const mainWin = await WebviewWindow.getByLabel("main");
  if (mainWin) await mainWin.destroy();
  const win = getCurrentWindow();
  await win.destroy();
}

// ---------------------------------------------------------------------------
// Event Listeners
// ---------------------------------------------------------------------------

async function setupEventListeners() {
  // Model download progress
  await listen<number>("model-download-progress", (event) => {
    const percent = event.payload;
    progressBar.style.width = `${percent}%`;
    progressText.textContent = `${percent}%`;
    downloadLabel.textContent = `Downloading... ${percent}%`;
  });

  // Model download complete
  await listen<boolean>("model-download-complete", (event) => {
    if (event.payload) {
      modelDownloaded = true;
      downloadLabel.textContent = "Download complete!";
      downloadStatusEl.classList.add("success");
      downloadBtn.classList.add("hidden");
      progressBar.style.width = "100%";
      progressText.textContent = "100%";
      updateNextEnabled();
    } else {
      downloadLabel.textContent = "Download failed";
      downloadStatusEl.classList.add("error");
      downloadBtn.disabled = false;
      downloadBtn.textContent = "Retry Download";
    }
  });

  // Audio level updates (for device test)
  await listen<{ device_id: string; level_db: number }>("audio-level-update", (event) => {
    const { level_db } = event.payload;
    // Map dB range (-96 to 0) to percentage (0 to 100)
    const percent = Math.max(0, Math.min(100, ((level_db + 96) / 96) * 100));
    levelMeterFill.style.width = `${percent}%`;
    if (level_db > -30) {
      levelLabel.textContent = "Good signal!";
    } else if (level_db > -60) {
      levelLabel.textContent = "Quiet - speak louder";
    } else {
      levelLabel.textContent = "Speak to test...";
    }
  });

  // Transcription results (for test step)
  // Use requestAnimationFrame to ensure the DOM update is painted even when
  // the window is not focused -- same pattern used to fix this bug in the
  // main window previously.
  await listen<TranscriptionResult>("transcription-complete", (event) => {
    if (activeSteps[currentStepIndex] === "step-4") {
      const text = event.payload.text;
      if (text && text !== "(No speech detected)") {
        requestAnimationFrame(() => {
          testResult.innerHTML = `<p class="test-success">${text}</p>`;
        });
      }
    }
  });
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

document.addEventListener("DOMContentLoaded", async () => {
  await initTheme();

  // Disable context menu
  document.addEventListener("contextmenu", (e) => e.preventDefault());

  // Suppress all default keyboard behaviour in this decorationless window.
  // On Windows, releasing the Alt key sends WM_SYSKEYUP which triggers the
  // menu bar activation heuristic. Even in a decorationless window, WebView2
  // processes this and enters a state that suspends rendering until a mouse
  // click cancels menu mode. Since the default PTT key is RightAlt, this
  // would freeze the window every time the user releases the talk key.
  // Block everything except Alt+F4 and form element interactions.
  //
  // JavaScript preventDefault() alone cannot prevent the Win32-level menu
  // activation because WM_SYSKEYUP is processed before JS events fire.
  // We also call cancel_menu_mode on keyup, which sends WM_CANCELMODE to
  // the HWND from Rust, cancelling the menu activation state.
  const suppressKeyHandler = (e: KeyboardEvent) => {
    if (isDebugConsoleHotkey(e)) return;
    // Allow Alt+F4 (window close)
    if (e.key === "F4" && e.altKey) return;

    // Allow normal interaction with form controls
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "SELECT" || tag === "INPUT" || tag === "BUTTON") return;

    e.preventDefault();
  };
  document.addEventListener("keydown", suppressKeyHandler);
  document.addEventListener("keyup", (e) => {
    suppressKeyHandler(e);
    // Cancel Win32 menu activation mode that WM_SYSKEYUP may have triggered.
    // This must be done from Rust (Win32 PostMessage) because JS preventDefault
    // runs too late — the menu heuristic fires at the Win32 message level before
    // the browser dispatches the JS event.
    if (e.key === "Alt" || e.code === "AltLeft" || e.code === "AltRight") {
      invoke("cancel_menu_mode").catch(() => {});
    }
  });

  // Get DOM refs
  downloadBtn = document.getElementById("download-btn") as HTMLButtonElement;
  downloadLabel = document.getElementById("download-label") as HTMLSpanElement;
  progressContainer = document.getElementById("progress-container") as HTMLDivElement;
  progressBar = document.getElementById("progress-bar") as HTMLDivElement;
  progressText = document.getElementById("progress-text") as HTMLSpanElement;
  downloadStatusEl = document.getElementById("download-status") as HTMLDivElement;
  deviceListEl = document.getElementById("device-list") as HTMLDivElement;
  levelMeterSection = document.getElementById("level-meter-section") as HTMLDivElement;
  levelMeterFill = document.getElementById("level-meter-fill") as HTMLDivElement;
  levelLabel = document.getElementById("level-label") as HTMLSpanElement;
  systemDeviceSelect = document.getElementById("system-device-select") as HTMLSelectElement;
  hotkeySection = document.getElementById("hotkey-section") as HTMLDivElement;
  hotkeyLabel = document.getElementById("hotkey-label") as HTMLSpanElement;
  changeHotkeyBtn = document.getElementById("change-hotkey-btn") as HTMLButtonElement;
  hotkeyRecorder = document.getElementById("hotkey-recorder") as HTMLDivElement;
  recorderStatus = document.getElementById("recorder-status") as HTMLSpanElement;
  testInstructions = document.getElementById("test-instructions") as HTMLParagraphElement;
  testResult = document.getElementById("test-result") as HTMLDivElement;
  backBtn = document.getElementById("back-btn") as HTMLButtonElement;
  nextBtn = document.getElementById("next-btn") as HTMLButtonElement;
  skipLink = document.getElementById("skip-link") as HTMLAnchorElement;

  // Accessibility step DOM refs
  accessibilityStatusEl = document.getElementById("accessibility-status") as HTMLDivElement;
  accessibilityStatusLabel = document.getElementById("accessibility-status-label") as HTMLSpanElement;
  accessibilityStatusIcon = document.getElementById("accessibility-status-icon") as HTMLSpanElement;
  openAccessibilityBtn = document.getElementById("open-accessibility-btn") as HTMLButtonElement;
  if (openAccessibilityBtn) {
    openAccessibilityBtn.addEventListener("click", () => {
      invoke("open_accessibility_settings").catch(() => {});
    });
  }

  // Close button - exits the entire application since setup is incomplete
  const closeBtn = document.getElementById("close-btn");
  if (closeBtn) {
    closeBtn.addEventListener("click", async () => {
      // Destroy the hidden main window first, then this window.
      // When all windows are gone, Tauri exits the process.
      const mainWin = await WebviewWindow.getByLabel("main");
      if (mainWin) await mainWin.destroy();
      const win = getCurrentWindow();
      await win.destroy();
    });
  }

  // Navigation
  nextBtn.addEventListener("click", handleNext);
  backBtn.addEventListener("click", handleBack);
  skipLink.addEventListener("click", (e) => { e.preventDefault(); skipSetup(); });
  downloadBtn.addEventListener("click", startDownload);

  // Mode selection
  document.querySelectorAll<HTMLInputElement>('input[name="mode"]').forEach((radio) => {
    radio.addEventListener("change", () => onModeChange(radio.value));
  });

  // Hotkey recording
  changeHotkeyBtn.addEventListener("click", startHotkeyRecording);
  document.addEventListener("keydown", handleRecordKeyDown);
  document.addEventListener("keyup", handleRecordKeyUp);

  // System device change
  systemDeviceSelect.addEventListener("change", () => {
    selectedSystemDeviceId = systemDeviceSelect.value || null;
  });

  // Connect to service event stream
  try {
    await invoke("connect_events");
  } catch (err) {
    console.error("Failed to connect events:", err);
  }

  // Set up event listeners for download progress, audio levels, etc.
  await setupEventListeners();

  // Build the initial step list and render the indicator
  activeSteps = buildActiveSteps();
  renderStepIndicator();

  // Check if model already exists (auto-skip step 1)
  const alreadyHasModel = await checkModelStatus();
  if (alreadyHasModel) {
    // Auto-advance to step 2
    showStepById("step-2");
  } else {
    showStepById("step-1");
  }
});
