import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initTheme, setThemeMode, getThemeMode, ThemeMode } from "./theme";

interface AudioDevice {
  id: string;
  name: string;
}

interface CaptureStatus {
  capturing: boolean;
  in_speech: boolean;
  queue_depth: number;
  error: string | null;
  source1_id: string | null;
  source2_id: string | null;
  transcription_mode: string;
}

interface HotkeyCombination {
  keys: string[];
}

interface PttStatus {
  mode: string;
  hotkeys: HotkeyCombination[];
  auto_toggle_hotkeys: HotkeyCombination[];
  auto_mode_active: boolean;
  is_active: boolean;
  available: boolean;
  error: string | null;
}

// Display names for key codes (snake_case serde name -> display)
const KEY_DISPLAY_NAMES: Record<string, string> = {
  // Modifiers
  right_alt: "Right Alt",
  left_alt: "Left Alt",
  right_control: "Right Ctrl",
  left_control: "Left Ctrl",
  right_shift: "Right Shift",
  left_shift: "Left Shift",
  caps_lock: "Caps Lock",
  left_meta: "Left Win",
  right_meta: "Right Win",
  // Function keys
  f1: "F1", f2: "F2", f3: "F3", f4: "F4", f5: "F5", f6: "F6",
  f7: "F7", f8: "F8", f9: "F9", f10: "F10", f11: "F11", f12: "F12",
  f13: "F13", f14: "F14", f15: "F15", f16: "F16", f17: "F17", f18: "F18",
  f19: "F19", f20: "F20", f21: "F21", f22: "F22", f23: "F23", f24: "F24",
  // Letters
  key_a: "A", key_b: "B", key_c: "C", key_d: "D", key_e: "E",
  key_f: "F", key_g: "G", key_h: "H", key_i: "I", key_j: "J",
  key_k: "K", key_l: "L", key_m: "M", key_n: "N", key_o: "O",
  key_p: "P", key_q: "Q", key_r: "R", key_s: "S", key_t: "T",
  key_u: "U", key_v: "V", key_w: "W", key_x: "X", key_y: "Y", key_z: "Z",
  // Digits
  digit0: "0", digit1: "1", digit2: "2", digit3: "3", digit4: "4",
  digit5: "5", digit6: "6", digit7: "7", digit8: "8", digit9: "9",
  // Navigation
  arrow_up: "Up", arrow_down: "Down", arrow_left: "Left", arrow_right: "Right",
  home: "Home", end: "End", page_up: "Page Up", page_down: "Page Down",
  insert: "Insert", delete: "Delete",
  // Special
  escape: "Esc", tab: "Tab", space: "Space", enter: "Enter",
  backspace: "Backspace", print_screen: "Print Screen",
  scroll_lock: "Scroll Lock", pause: "Pause",
  // Punctuation
  minus: "-", equal: "=", bracket_left: "[", bracket_right: "]",
  backslash: "\\", semicolon: ";", quote: "'", backquote: "`",
  comma: ",", period: ".", slash: "/",
  // Numpad
  numpad0: "Num 0", numpad1: "Num 1", numpad2: "Num 2", numpad3: "Num 3",
  numpad4: "Num 4", numpad5: "Num 5", numpad6: "Num 6", numpad7: "Num 7",
  numpad8: "Num 8", numpad9: "Num 9",
  numpad_multiply: "Num *", numpad_add: "Num +", numpad_subtract: "Num -",
  numpad_decimal: "Num .", numpad_divide: "Num /", num_lock: "Num Lock",
};

// Map browser KeyboardEvent.code to our serde key code names
const BROWSER_CODE_MAP: Record<string, string> = {
  // Modifiers
  AltRight: "right_alt", AltLeft: "left_alt",
  ControlRight: "right_control", ControlLeft: "left_control",
  ShiftRight: "right_shift", ShiftLeft: "left_shift",
  CapsLock: "caps_lock",
  MetaLeft: "left_meta", MetaRight: "right_meta",
  // Function keys
  F1: "f1", F2: "f2", F3: "f3", F4: "f4", F5: "f5", F6: "f6",
  F7: "f7", F8: "f8", F9: "f9", F10: "f10", F11: "f11", F12: "f12",
  F13: "f13", F14: "f14", F15: "f15", F16: "f16", F17: "f17", F18: "f18",
  F19: "f19", F20: "f20", F21: "f21", F22: "f22", F23: "f23", F24: "f24",
  // Letters
  KeyA: "key_a", KeyB: "key_b", KeyC: "key_c", KeyD: "key_d", KeyE: "key_e",
  KeyF: "key_f", KeyG: "key_g", KeyH: "key_h", KeyI: "key_i", KeyJ: "key_j",
  KeyK: "key_k", KeyL: "key_l", KeyM: "key_m", KeyN: "key_n", KeyO: "key_o",
  KeyP: "key_p", KeyQ: "key_q", KeyR: "key_r", KeyS: "key_s", KeyT: "key_t",
  KeyU: "key_u", KeyV: "key_v", KeyW: "key_w", KeyX: "key_x", KeyY: "key_y",
  KeyZ: "key_z",
  // Digits
  Digit0: "digit0", Digit1: "digit1", Digit2: "digit2", Digit3: "digit3",
  Digit4: "digit4", Digit5: "digit5", Digit6: "digit6", Digit7: "digit7",
  Digit8: "digit8", Digit9: "digit9",
  // Navigation
  ArrowUp: "arrow_up", ArrowDown: "arrow_down",
  ArrowLeft: "arrow_left", ArrowRight: "arrow_right",
  Home: "home", End: "end", PageUp: "page_up", PageDown: "page_down",
  Insert: "insert", Delete: "delete",
  // Special
  Escape: "escape", Tab: "tab", Space: "space",
  Enter: "enter", Backspace: "backspace",
  PrintScreen: "print_screen", ScrollLock: "scroll_lock", Pause: "pause",
  // Punctuation
  Minus: "minus", Equal: "equal",
  BracketLeft: "bracket_left", BracketRight: "bracket_right",
  Backslash: "backslash", Semicolon: "semicolon", Quote: "quote",
  Backquote: "backquote", Comma: "comma", Period: "period", Slash: "slash",
  // Numpad
  Numpad0: "numpad0", Numpad1: "numpad1", Numpad2: "numpad2", Numpad3: "numpad3",
  Numpad4: "numpad4", Numpad5: "numpad5", Numpad6: "numpad6", Numpad7: "numpad7",
  Numpad8: "numpad8", Numpad9: "numpad9",
  NumpadMultiply: "numpad_multiply", NumpadAdd: "numpad_add",
  NumpadSubtract: "numpad_subtract", NumpadDecimal: "numpad_decimal",
  NumpadDivide: "numpad_divide", NumLock: "num_lock",
};

function isDebugConsoleHotkey(e: KeyboardEvent): boolean {
  const isIKey = e.code === "KeyI" || e.key === "i" || e.key === "I";
  const isCtrlShift = e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;
  const isMetaAlt = e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey;
  return isIKey && (isCtrlShift || isMetaAlt);
}

// Modifier key codes (for display ordering and warnings)
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

function combinationsEqual(a: HotkeyCombination, b: HotkeyCombination): boolean {
  if (a.keys.length !== b.keys.length) return false;
  const setA = new Set(a.keys);
  return b.keys.every((k) => setA.has(k));
}

// DOM elements
let themeSelect: HTMLSelectElement;
let source1Select: HTMLSelectElement;
let source2Select: HTMLSelectElement;
let hotkeyListEl: HTMLDivElement;
let recorderEl: HTMLDivElement;
let recorderStatusEl: HTMLSpanElement;
let warningEl: HTMLDivElement;
let addHotkeyBtn: HTMLButtonElement;
// Toggle hotkey UI - disabled for now
// let toggleHotkeyListEl: HTMLDivElement;
// let toggleRecorderEl: HTMLDivElement;
// let toggleRecorderStatusEl: HTMLSpanElement;
// let toggleWarningEl: HTMLDivElement;
// let setToggleHotkeyBtn: HTMLButtonElement;

// State
let allDevices: AudioDevice[] = [];
let hotkeys: HotkeyCombination[] = [];
// Toggle hotkey state - disabled for now
// let toggleHotkeys: HotkeyCombination[] = [];
let isRecording = false;
// let isRecordingToggle = false;
/** All keys that have been pressed during this recording (the combination to save). */
let recordedKeys: Set<string> = new Set();
/** Keys physically held right now. */
let currentlyHeldKeys: Set<string> = new Set();
let releaseTimer: number | null = null;
const RELEASE_DEBOUNCE_MS = 200;
const RECORD_TIMEOUT_MS = 5000;
let recordTimeoutTimer: number | null = null;
// Toggle hotkey recording state - disabled for now
// let toggleRecordedKeys: Set<string> = new Set();
// let toggleCurrentlyHeldKeys: Set<string> = new Set();
// let toggleReleaseTimer: number | null = null;
// let toggleRecordTimeoutTimer: number | null = null;

function populateSourceDropdown(
  select: HTMLSelectElement,
  devices: AudioDevice[]
) {
  select.innerHTML = "";

  const noneOption = document.createElement("option");
  noneOption.value = "";
  noneOption.textContent = "None";
  select.appendChild(noneOption);

  devices.forEach((device) => {
    const option = document.createElement("option");
    option.value = device.id;
    option.textContent = device.name;
    select.appendChild(option);
  });
}

function renderHotkeyList() {
  hotkeyListEl.innerHTML = "";

  if (hotkeys.length === 0) {
    const emptyMsg = document.createElement("div");
    emptyMsg.className = "hotkey-empty";
    emptyMsg.textContent = "No hotkeys configured";
    hotkeyListEl.appendChild(emptyMsg);
    return;
  }

  hotkeys.forEach((combo, index) => {
    const item = document.createElement("div");
    item.className = "hotkey-item";

    const label = document.createElement("span");
    label.className = "hotkey-label";
    label.textContent = combinationDisplayName(combo);

    const removeBtn = document.createElement("button");
    removeBtn.className = "hotkey-remove-btn";
    removeBtn.textContent = "\u00d7";
    removeBtn.title = "Remove";
    removeBtn.type = "button";
    removeBtn.addEventListener("click", () => removeHotkey(index));

    item.appendChild(label);
    item.appendChild(removeBtn);
    hotkeyListEl.appendChild(item);
  });
}

async function removeHotkey(index: number) {
  hotkeys.splice(index, 1);
  renderHotkeyList();
  await saveHotkeys();
}

async function saveHotkeys() {
  try {
    await invoke("set_ptt_hotkeys", { hotkeys });
  } catch (error) {
    console.error("Error setting PTT hotkeys:", error);
  }
}

// Toggle hotkey functions - disabled for now
/*
function renderToggleHotkeys() {
  toggleHotkeyListEl.innerHTML = "";

  if (toggleHotkeys.length === 0) {
    const emptyMsg = document.createElement("div");
    emptyMsg.className = "hotkey-empty";
    emptyMsg.textContent = "No toggle hotkeys configured";
    toggleHotkeyListEl.appendChild(emptyMsg);
    return;
  }

  toggleHotkeys.forEach((combo, index) => {
    const item = document.createElement("div");
    item.className = "hotkey-item";

    const label = document.createElement("span");
    label.className = "hotkey-label";
    label.textContent = combinationDisplayName(combo);

    const removeBtn = document.createElement("button");
    removeBtn.className = "hotkey-remove-btn";
    removeBtn.textContent = "\u00d7";
    removeBtn.title = "Remove";
    removeBtn.type = "button";
    removeBtn.addEventListener("click", () => removeToggleHotkey(index));

    item.appendChild(label);
    item.appendChild(removeBtn);
    toggleHotkeyListEl.appendChild(item);
  });
}

async function removeToggleHotkey(index: number) {
  toggleHotkeys.splice(index, 1);
  renderToggleHotkeys();
  await saveToggleHotkeys();
}

async function saveToggleHotkeys() {
  try {
    await invoke("set_auto_toggle_hotkeys", { hotkeys: toggleHotkeys });
  } catch (error) {
    console.error("Error setting toggle hotkeys:", error);
  }
}

function startToggleRecording() {
  if (isRecordingToggle) return;

  isRecordingToggle = true;
  toggleRecordedKeys.clear();
  toggleCurrentlyHeldKeys.clear();
  toggleRecorderEl.classList.remove("hidden");
  toggleRecorderStatusEl.textContent = "Press keys...";
  setToggleHotkeyBtn.classList.add("hidden");
  hideToggleWarning();

  toggleRecordTimeoutTimer = window.setTimeout(() => {
    if (isRecordingToggle && toggleRecordedKeys.size === 0) {
      showToggleWarning("No keys detected. Some keys may be intercepted by the OS.");
    }
  }, RECORD_TIMEOUT_MS);
}

function stopToggleRecording(cancelled: boolean) {
  if (!isRecordingToggle) return;

  isRecordingToggle = false;
  toggleRecorderEl.classList.add("hidden");
  setToggleHotkeyBtn.classList.remove("hidden");

  if (toggleReleaseTimer !== null) {
    clearTimeout(toggleReleaseTimer);
    toggleReleaseTimer = null;
  }
  if (toggleRecordTimeoutTimer !== null) {
    clearTimeout(toggleRecordTimeoutTimer);
    toggleRecordTimeoutTimer = null;
  }

  if (cancelled || toggleRecordedKeys.size === 0) {
    toggleRecordedKeys.clear();
    return;
  }

  const newCombo: HotkeyCombination = { keys: Array.from(toggleRecordedKeys) };
  toggleRecordedKeys.clear();

  // Check for duplicate
  if (toggleHotkeys.some((existing) => combinationsEqual(existing, newCombo))) {
    showToggleWarning("This hotkey combination is already configured.");
    return;
  }

  // Warn about single non-modifier key
  if (
    newCombo.keys.length === 1 &&
    !MODIFIER_KEYS.has(newCombo.keys[0])
  ) {
    showToggleWarning(
      `Warning: "${keyDisplayName(newCombo.keys[0])}" alone may conflict with normal typing.`
    );
  }

  toggleHotkeys.push(newCombo);
  renderToggleHotkeys();
  saveToggleHotkeys();
}

function showToggleWarning(msg: string) {
  toggleWarningEl.textContent = msg;
  toggleWarningEl.classList.remove("hidden");
}

function hideToggleWarning() {
  toggleWarningEl.textContent = "";
  toggleWarningEl.classList.add("hidden");
}

function handleToggleRecordKeyDown(e: KeyboardEvent) {
  if (!isRecordingToggle) return;

  e.preventDefault();
  e.stopPropagation();

  if (e.code === "Escape") {
    stopToggleRecording(true);
    return;
  }

  if (toggleReleaseTimer !== null) {
    clearTimeout(toggleReleaseTimer);
    toggleReleaseTimer = null;
  }

  if (toggleRecordTimeoutTimer !== null) {
    clearTimeout(toggleRecordTimeoutTimer);
    toggleRecordTimeoutTimer = null;
  }
  hideToggleWarning();

  const keyCode = BROWSER_CODE_MAP[e.code];
  if (!keyCode) return;

  toggleCurrentlyHeldKeys.add(keyCode);
  toggleRecordedKeys.add(keyCode);

  const combo: HotkeyCombination = { keys: Array.from(toggleRecordedKeys) };
  toggleRecorderStatusEl.textContent = combinationDisplayName(combo);
}

function handleToggleRecordKeyUp(e: KeyboardEvent) {
  if (!isRecordingToggle) return;

  e.preventDefault();
  e.stopPropagation();

  const keyCode = BROWSER_CODE_MAP[e.code];
  if (keyCode) {
    toggleCurrentlyHeldKeys.delete(keyCode);
  }

  if (toggleCurrentlyHeldKeys.size === 0 && toggleRecordedKeys.size > 0) {
    if (toggleReleaseTimer !== null) {
      clearTimeout(toggleReleaseTimer);
    }
    toggleReleaseTimer = window.setTimeout(() => {
      if (isRecordingToggle) {
        stopToggleRecording(false);
      }
    }, RELEASE_DEBOUNCE_MS);
  }
}
*/

function startRecording() {
  if (isRecording) return;

  isRecording = true;
  recordedKeys.clear();
  currentlyHeldKeys.clear();
  recorderEl.classList.remove("hidden");
  recorderStatusEl.textContent = "Press keys...";
  addHotkeyBtn.classList.add("hidden");
  hideWarning();

  // Timeout warning if no keys detected
  recordTimeoutTimer = window.setTimeout(() => {
    if (isRecording && recordedKeys.size === 0) {
      showWarning(
        "No keys detected. Some keys may be intercepted by the OS."
      );
    }
  }, RECORD_TIMEOUT_MS);
}

function stopRecording(cancelled: boolean) {
  if (!isRecording) return;

  isRecording = false;
  recorderEl.classList.add("hidden");
  addHotkeyBtn.classList.remove("hidden");

  if (releaseTimer !== null) {
    clearTimeout(releaseTimer);
    releaseTimer = null;
  }
  if (recordTimeoutTimer !== null) {
    clearTimeout(recordTimeoutTimer);
    recordTimeoutTimer = null;
  }

  if (cancelled || recordedKeys.size === 0) {
    recordedKeys.clear();
    return;
  }

  const newCombo: HotkeyCombination = { keys: Array.from(recordedKeys) };
  recordedKeys.clear();

  // Check for duplicate
  if (hotkeys.some((existing) => combinationsEqual(existing, newCombo))) {
    showWarning("This hotkey combination is already configured.");
    return;
  }

  // Warn about single non-modifier key
  if (
    newCombo.keys.length === 1 &&
    !MODIFIER_KEYS.has(newCombo.keys[0])
  ) {
    showWarning(
      `Warning: "${keyDisplayName(newCombo.keys[0])}" alone may conflict with normal typing.`
    );
  }

  hotkeys.push(newCombo);
  renderHotkeyList();
  saveHotkeys();
}

function showWarning(msg: string) {
  warningEl.textContent = msg;
  warningEl.classList.remove("hidden");
}

function hideWarning() {
  warningEl.textContent = "";
  warningEl.classList.add("hidden");
}

function handleRecordKeyDown(e: KeyboardEvent) {
  if (!isRecording) return;

  e.preventDefault();
  e.stopPropagation();

  if (e.code === "Escape") {
    stopRecording(true);
    return;
  }

  if (releaseTimer !== null) {
    clearTimeout(releaseTimer);
    releaseTimer = null;
  }

  if (recordTimeoutTimer !== null) {
    clearTimeout(recordTimeoutTimer);
    recordTimeoutTimer = null;
  }
  hideWarning();

  const keyCode = BROWSER_CODE_MAP[e.code];
  if (!keyCode) return;

  currentlyHeldKeys.add(keyCode);
  recordedKeys.add(keyCode);

  const combo: HotkeyCombination = { keys: Array.from(recordedKeys) };
  recorderStatusEl.textContent = combinationDisplayName(combo);
}

function handleRecordKeyUp(e: KeyboardEvent) {
  if (!isRecording) return;

  e.preventDefault();
  e.stopPropagation();

  const keyCode = BROWSER_CODE_MAP[e.code];
  if (keyCode) {
    currentlyHeldKeys.delete(keyCode);
  }

  if (currentlyHeldKeys.size === 0 && recordedKeys.size > 0) {
    if (releaseTimer !== null) {
      clearTimeout(releaseTimer);
    }
    releaseTimer = window.setTimeout(() => {
      if (isRecording) {
        stopRecording(false);
      }
    }, RELEASE_DEBOUNCE_MS);
  }
}

// Toggle hotkey recording handlers - disabled for now
/*
function handleToggleRecordKeyDown(e: KeyboardEvent) { ... }
function handleToggleRecordKeyUp(e: KeyboardEvent) { ... }
*/

async function loadState() {
  try {
    const [devices, status, pttStatus] = await Promise.all([
      invoke<AudioDevice[]>("list_all_sources"),
      invoke<CaptureStatus>("get_status"),
      invoke<PttStatus>("get_ptt_status"),
    ]);

    allDevices = devices;

    populateSourceDropdown(source1Select, allDevices);
    populateSourceDropdown(source2Select, allDevices);

    if (status.source1_id) {
      source1Select.value = status.source1_id;
    }
    if (status.source2_id) {
      source2Select.value = status.source2_id;
    }

    hotkeys = pttStatus.hotkeys || [];
    renderHotkeyList();

    // Toggle hotkeys loading - disabled for now
    // toggleHotkeys = pttStatus.auto_toggle_hotkeys || [];
    // renderToggleHotkeys();
  } catch (error) {
    console.error("Failed to load config state:", error);
    source1Select.innerHTML = `<option value="">Error loading devices</option>`;
    source2Select.innerHTML = `<option value="">Error loading devices</option>`;
  }
}

async function onSourceChange() {
  const source1Id = source1Select.value || null;
  const source2Id = source2Select.value || null;
  try {
    await invoke("set_sources", { source1Id, source2Id });
  } catch (error) {
    console.error("Error setting sources:", error);
  }
}

document.addEventListener("DOMContentLoaded", async () => {
  await initTheme();

  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  themeSelect = document.getElementById("theme-select") as HTMLSelectElement;
  source1Select = document.getElementById("source1-select") as HTMLSelectElement;
  source2Select = document.getElementById("source2-select") as HTMLSelectElement;
  hotkeyListEl = document.getElementById("hotkey-list") as HTMLDivElement;
  recorderEl = document.getElementById("hotkey-recorder") as HTMLDivElement;
  recorderStatusEl = document.getElementById("recorder-status") as HTMLSpanElement;
  warningEl = document.getElementById("hotkey-warning") as HTMLDivElement;
  addHotkeyBtn = document.getElementById("add-hotkey-btn") as HTMLButtonElement;
  // Toggle hotkey UI - disabled for now
  // toggleHotkeyListEl = document.getElementById("toggle-hotkey-list") as HTMLDivElement;
  // toggleRecorderEl = document.getElementById("toggle-hotkey-recorder") as HTMLDivElement;
  // toggleRecorderStatusEl = document.getElementById("toggle-recorder-status") as HTMLSpanElement;
  // toggleWarningEl = document.getElementById("toggle-hotkey-warning") as HTMLDivElement;
  // setToggleHotkeyBtn = document.getElementById("set-toggle-hotkey-btn") as HTMLButtonElement;

  const closeBtn = document.getElementById("close-btn");
  if (closeBtn) {
    closeBtn.addEventListener("click", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      const win = getCurrentWindow();
      await win.destroy();
    });
  }

  source1Select.addEventListener("change", onSourceChange);
  source2Select.addEventListener("change", onSourceChange);
  addHotkeyBtn.addEventListener("click", startRecording);
  // Toggle hotkey button - disabled for now
  // setToggleHotkeyBtn.addEventListener("click", startToggleRecording);

  themeSelect.value = getThemeMode();
  themeSelect.addEventListener("change", async () => {
    const mode = themeSelect.value as ThemeMode;
    await setThemeMode(mode);
  });

  document.addEventListener("keydown", (e) => {
    if (isRecording) {
      handleRecordKeyDown(e);
    // Toggle hotkey recording - disabled for now
    // } else if (isRecordingToggle) {
    //   handleToggleRecordKeyDown(e);
    } else {
      if (isDebugConsoleHotkey(e)) return;
      if (e.key === "F4" && e.altKey) return;
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "SELECT" || tag === "INPUT" || tag === "BUTTON") return;
      e.preventDefault();
    }
  });

  document.addEventListener("keyup", (e) => {
    if (isRecording) {
      handleRecordKeyUp(e);
    // Toggle hotkey recording - disabled for now
    // } else if (isRecordingToggle) {
    //   handleToggleRecordKeyUp(e);
    } else {
      if (isDebugConsoleHotkey(e)) return;
      if (e.key === "F4" && e.altKey) return;
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "SELECT" || tag === "INPUT" || tag === "BUTTON") return;
      e.preventDefault();
    }
  });

  await loadState();
});
