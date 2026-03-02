import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initTheme } from "./theme";
import appIcon from "./assets/icon-32.png";

// ─── Types ────────────────────────────────────────────────────────────────────

/** Pre-formatted log line emitted by the backend TauriLogLayer.
 *  Identical in format to what tracing_subscriber::fmt writes to the log file,
 *  so history and live events use the same rendering path. */
interface LogLinePayload {
  line: string;
}

// ─── State ────────────────────────────────────────────────────────────────────

let isPinned = true;
let unlistenFn: UnlistenFn | null = null;

/** Elements queued during the 50ms batch window. */
let pendingElements: HTMLElement[] = [];
let batchTimer: number | null = null;

// ─── DOM refs ─────────────────────────────────────────────────────────────────

let outputEl: HTMLDivElement;
let scrollToBottomBtn: HTMLButtonElement;
let downloadBtn: HTMLButtonElement;

// ─── Log line rendering ───────────────────────────────────────────────────────
//
// Both history (from file) and live events (from TauriLogLayer) arrive as the
// same pre-formatted string. We parse each line into its parts and render them
// as separate styled spans so each part can be colored independently.
//
// Line format produced by tracing_subscriber::fmt (and mirrored by TauriLogLayer):
//   2026-03-02T00:27:33.464210Z  INFO flowstt_lib: message text here
//   ^-- timestamp (no spaces) --^  ^lv^ ^-- target --^ ^-- message --^
//
// The level field is right-padded to 5 chars and separated from timestamp by
// two spaces; target is followed by ": ".

// Regex: (timestamp)(whitespace)(LEVEL)(whitespace)(target): (message)
const LOG_LINE_RE = /^(\S+)\s{2}(\w+)\s+(.+?):\s(.*)$/;

function levelClass(level: string): string {
  switch (level.toUpperCase()) {
    case "ERROR":    return "level-error";
    case "WARN":     return "level-warn";
    case "DEBUG":    return "level-debug";
    case "TRACE":    return "level-trace";
    case "CRITICAL": return "level-critical";
    default:         return "level-info";
  }
}

/**
 * Build a row element from a raw log line string.
 * Parses timestamp / level / target / message and renders each as a
 * separate span so they can be colored independently via CSS.
 * Falls back to a single unstyled span if the line doesn't match the format.
 */
function buildRawRow(rawLine: string): HTMLElement | null {
  const trimmed = rawLine.trimEnd();
  if (!trimmed) return null;

  const row = document.createElement("div");

  const m = LOG_LINE_RE.exec(trimmed);
  if (m) {
    const [, ts, level, target, message] = m;
    const cls = levelClass(level);
    row.className = `log-row ${cls}`;

    const tsSpan = document.createElement("span");
    tsSpan.className = "log-ts";
    tsSpan.textContent = ts;

    const lvlSpan = document.createElement("span");
    lvlSpan.className = "log-level";
    lvlSpan.textContent = level;

    const tgtSpan = document.createElement("span");
    tgtSpan.className = "log-target";
    tgtSpan.textContent = target + ":";

    const msgSpan = document.createElement("span");
    msgSpan.className = "log-message";
    msgSpan.textContent = message;

    // Text nodes preserve whitespace in clipboard copies so the pasted text
    // matches the original log file format exactly.
    row.appendChild(tsSpan);
    row.appendChild(document.createTextNode("  "));
    row.appendChild(lvlSpan);
    row.appendChild(document.createTextNode(" "));
    row.appendChild(tgtSpan);
    row.appendChild(document.createTextNode(" "));
    row.appendChild(msgSpan);
  } else {
    // Unparseable line (continuation, blank separator, etc.) — show as-is
    row.className = "log-row level-info";
    const msgSpan = document.createElement("span");
    msgSpan.className = "log-message log-message-raw";
    msgSpan.textContent = trimmed;
    row.appendChild(msgSpan);
  }

  return row;
}

// ─── DOM batch flush ──────────────────────────────────────────────────────────

function enqueueElement(el: HTMLElement) {
  pendingElements.push(el);
  if (batchTimer === null) {
    batchTimer = window.setTimeout(flushPending, 50);
  }
}

function flushPending() {
  batchTimer = null;
  if (pendingElements.length === 0) return;

  const fragment = document.createDocumentFragment();
  for (const el of pendingElements) {
    fragment.appendChild(el);
  }
  pendingElements = [];
  outputEl.appendChild(fragment);

  if (isPinned) {
    requestAnimationFrame(() => {
      outputEl.scrollTop = outputEl.scrollHeight;
    });
  }
}

// ─── Scroll management ────────────────────────────────────────────────────────

function updatePinState() {
  const threshold = 4;
  const atBottom =
    outputEl.scrollTop + outputEl.clientHeight >= outputEl.scrollHeight - threshold;
  isPinned = atBottom;
  scrollToBottomBtn.classList.toggle("hidden", isPinned);
}

function scrollToBottom() {
  isPinned = true;
  scrollToBottomBtn.classList.add("hidden");
  outputEl.scrollTop = outputEl.scrollHeight;
}

// ─── Download logs ─────────────────────────────────────────────────────────────

async function handleDownload() {
  downloadBtn.disabled = true;
  downloadBtn.textContent = "Downloading...";
  try {
    await invoke("download_logs");
  } catch (err) {
    if (err === "no_logs") {
      downloadBtn.textContent = "No logs available";
    } else {
      downloadBtn.textContent = "Download failed";
      console.error("download_logs error:", err);
    }
    setTimeout(() => {
      downloadBtn.disabled = false;
      downloadBtn.textContent = "Download Logs";
    }, 3000);
    return;
  }
  downloadBtn.disabled = false;
  downloadBtn.textContent = "Download Logs";
}

// ─── Seed historical log lines from the current session file ──────────────────

async function seedHistory() {
  let raw: string;
  try {
    raw = await invoke<string>("get_log_history");
  } catch {
    return; // Non-fatal: just show live lines going forward
  }

  if (!raw) return;

  const fragment = document.createDocumentFragment();
  let count = 0;
  for (const line of raw.split("\n")) {
    const el = buildRawRow(line);
    if (el) {
      fragment.appendChild(el);
      count++;
    }
  }

  if (count > 0) {
    // Visual separator between session history and new live output
    const sep = document.createElement("div");
    sep.className = "log-history-separator";
    fragment.appendChild(sep);
  }

  outputEl.appendChild(fragment);
  // Scroll to bottom immediately after seeding history
  outputEl.scrollTop = outputEl.scrollHeight;
}

// ─── Entry point ──────────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", async () => {
  // Initialize theme before first paint (consistent with other windows)
  await initTheme();

  // Set icon src via import so Vite hashes and bundles it correctly
  const titleIcon = document.getElementById("logs-title-icon") as HTMLImageElement | null;
  if (titleIcon) titleIcon.src = appIcon;

  outputEl = document.getElementById("logs-output") as HTMLDivElement;
  scrollToBottomBtn = document.getElementById("scroll-to-bottom-btn") as HTMLButtonElement;
  downloadBtn = document.getElementById("download-btn") as HTMLButtonElement;

  // Close button — destroy the window (same as about/config windows)
  const closeBtn = document.getElementById("close-btn");
  if (closeBtn) {
    closeBtn.addEventListener("click", async (e) => {
      e.preventDefault();
      e.stopPropagation();
      const win = getCurrentWindow();
      await win.destroy();
    });
  }

  // Suppress default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  outputEl.addEventListener("scroll", updatePinState);
  scrollToBottomBtn.addEventListener("click", () => scrollToBottom());
  downloadBtn.addEventListener("click", () => void handleDownload());

  // 1. Seed with the current session log file contents
  await seedHistory();

  // 2. Subscribe to live "log-line" events from the backend
  unlistenFn = await listen<LogLinePayload>("log-line", (event) => {
    const el = buildRawRow(event.payload.line);
    if (el) enqueueElement(el);
  });

  // Unsubscribe when the window unloads
  window.addEventListener("unload", () => {
    if (unlistenFn) {
      unlistenFn();
      unlistenFn = null;
    }
  });
});
