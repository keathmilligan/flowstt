import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

interface PttStatus {
  mode: string;
  key: KeyCode;
  is_active: boolean;
  available: boolean;
  error: string | null;
}

type KeyCode =
  | "right_alt"
  | "left_alt"
  | "right_control"
  | "left_control"
  | "right_shift"
  | "left_shift"
  | "caps_lock"
  | "f13"
  | "f14"
  | "f15"
  | "f16"
  | "f17"
  | "f18"
  | "f19"
  | "f20";

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
let source1Select: HTMLSelectElement;
let source2Select: HTMLSelectElement;
let pttKeySelect: HTMLSelectElement;

// State
let allDevices: AudioDevice[] = [];

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

function populatePttKeySelect(select: HTMLSelectElement) {
  select.innerHTML = "";
  for (const [value, name] of Object.entries(KEY_CODE_NAMES)) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = name;
    select.appendChild(option);
  }
}

async function loadState() {
  try {
    // Fetch devices, current status, and PTT status in parallel
    const [devices, status, pttStatus] = await Promise.all([
      invoke<AudioDevice[]>("list_all_sources"),
      invoke<CaptureStatus>("get_status"),
      invoke<PttStatus>("get_ptt_status"),
    ]);

    allDevices = devices;

    // Populate dropdowns
    populateSourceDropdown(source1Select, allDevices);
    populateSourceDropdown(source2Select, allDevices);
    populatePttKeySelect(pttKeySelect);

    // Pre-select current values
    if (status.source1_id) {
      source1Select.value = status.source1_id;
    }
    if (status.source2_id) {
      source2Select.value = status.source2_id;
    }
    pttKeySelect.value = pttStatus.key;
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

async function onPttKeyChange() {
  const key = pttKeySelect.value;
  try {
    await invoke("set_ptt_key", { key });
  } catch (error) {
    console.error("Error setting PTT key:", error);
  }
}

document.addEventListener("DOMContentLoaded", async () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  // Suppress default keyboard behaviour in this decorationless window
  const suppressKeyHandler = (e: KeyboardEvent) => {
    if (e.key === "F4" && e.altKey) return;
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "SELECT" || tag === "INPUT" || tag === "BUTTON") return;
    e.preventDefault();
  };
  document.addEventListener("keydown", suppressKeyHandler);
  document.addEventListener("keyup", suppressKeyHandler);

  // Get DOM elements
  source1Select = document.getElementById("source1-select") as HTMLSelectElement;
  source2Select = document.getElementById("source2-select") as HTMLSelectElement;
  pttKeySelect = document.getElementById("ptt-key-select") as HTMLSelectElement;

  // Close button
  const closeBtn = document.getElementById("close-btn");
  if (closeBtn) {
    closeBtn.addEventListener("click", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      const win = getCurrentWindow();
      await win.destroy();
    });
  }

  // Wire change handlers
  source1Select.addEventListener("change", onSourceChange);
  source2Select.addEventListener("change", onSourceChange);
  pttKeySelect.addEventListener("change", onPttKeyChange);

  // Load current state and populate dropdowns
  await loadState();
});
