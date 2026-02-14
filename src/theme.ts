/**
 * Theme management module for FlowSTT.
 *
 * Reads the persisted theme mode from the backend config, sets the
 * `data-theme` attribute on `<html>`, and handles auto mode by listening
 * to the OS `prefers-color-scheme` media query. Also listens for the
 * Tauri "theme-changed" event so all windows update when the user changes
 * the theme in the config window.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type ThemeMode = "auto" | "light" | "dark";

/** Resolved theme (what is actually applied to the DOM) */
type ResolvedTheme = "light" | "dark";

const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

let currentMode: ThemeMode = "auto";
let themeChangedUnlisten: UnlistenFn | null = null;
const themeListeners: Array<(theme: ResolvedTheme) => void> = [];

/** Resolve the effective theme from the mode and OS preference. */
function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode === "light") return "light";
  if (mode === "dark") return "dark";
  // auto: follow OS
  return mediaQuery.matches ? "dark" : "light";
}

/** Apply the resolved theme to the document and notify listeners. */
function applyTheme(theme: ResolvedTheme): void {
  document.documentElement.setAttribute("data-theme", theme);
  for (const listener of themeListeners) {
    listener(theme);
  }
}

/**
 * Register a callback that fires whenever the resolved theme changes.
 * The callback receives "light" or "dark". Returns an unsubscribe function.
 */
export function onThemeChange(callback: (theme: ResolvedTheme) => void): () => void {
  themeListeners.push(callback);
  return () => {
    const idx = themeListeners.indexOf(callback);
    if (idx >= 0) themeListeners.splice(idx, 1);
  };
}

/** Get the currently resolved theme ("light" or "dark"). */
export function getResolvedTheme(): ResolvedTheme {
  return resolveTheme(currentMode);
}

/** Handle OS color scheme change (only relevant in auto mode). */
function onMediaChange(): void {
  if (currentMode === "auto") {
    applyTheme(resolveTheme("auto"));
  }
}

/**
 * Initialize the theme system. Should be called as early as possible
 * (before first paint) in each window's entry point.
 *
 * 1. Reads the persisted theme mode from the backend config.
 * 2. Applies it immediately.
 * 3. Listens for OS preference changes (for auto mode).
 * 4. Listens for the Tauri "theme-changed" event from other windows.
 */
export async function initTheme(): Promise<void> {
  // Read persisted mode from config
  try {
    currentMode = await invoke<ThemeMode>("get_theme_mode");
  } catch {
    currentMode = "auto";
  }

  // Apply immediately
  applyTheme(resolveTheme(currentMode));

  // Listen for OS preference changes
  mediaQuery.addEventListener("change", onMediaChange);

  // Listen for theme changes from other windows (via Tauri event)
  if (!themeChangedUnlisten) {
    themeChangedUnlisten = await listen<ThemeMode>("theme-changed", (event) => {
      currentMode = event.payload;
      applyTheme(resolveTheme(currentMode));
    });
  }
}

/**
 * Set the theme mode. Persists to config and emits an event so all
 * windows update. Called from the config window UI.
 */
export async function setThemeMode(mode: ThemeMode): Promise<void> {
  currentMode = mode;
  applyTheme(resolveTheme(mode));
  await invoke("set_theme_mode", { mode });
}

/** Get the current theme mode. */
export function getThemeMode(): ThemeMode {
  return currentMode;
}
