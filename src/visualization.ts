// Visualization window entry point
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  WaveformRenderer,
  SpectrogramRenderer,
  SpeechActivityRenderer,
  VisualizationPayload,
} from "./renderers";

// DOM elements
let waveformCanvas: HTMLCanvasElement | null;
let spectrogramCanvas: HTMLCanvasElement | null;
let speechActivityCanvas: HTMLCanvasElement | null;
let closeBtn: HTMLButtonElement | null;

// Renderers
let waveformRenderer: WaveformRenderer | null = null;
let spectrogramRenderer: SpectrogramRenderer | null = null;
let speechActivityRenderer: SpeechActivityRenderer | null = null;

// Event listeners
let visualizationUnlisten: UnlistenFn | null = null;
let closeRequestedUnlisten: UnlistenFn | null = null;

async function setupVisualizationListener() {
  if (visualizationUnlisten) return;

  visualizationUnlisten = await listen<VisualizationPayload>("visualization-data", (event) => {
    // Push pre-downsampled waveform data
    if (waveformRenderer) {
      waveformRenderer.pushSamples(event.payload.waveform);
    }
    // Push pre-computed spectrogram column when available
    if (spectrogramRenderer && event.payload.spectrogram) {
      spectrogramRenderer.pushColumn(event.payload.spectrogram.colors);
    }
    // Push speech detection metrics when available
    if (speechActivityRenderer && event.payload.speech_metrics) {
      speechActivityRenderer.pushMetrics(event.payload.speech_metrics);
    }
  });
}

function cleanupVisualizationListener() {
  if (visualizationUnlisten) {
    visualizationUnlisten();
    visualizationUnlisten = null;
  }
}

function startRenderers() {
  waveformRenderer?.start();
  spectrogramRenderer?.start();
  speechActivityRenderer?.start();
}

function stopRenderers() {
  waveformRenderer?.stop();
  spectrogramRenderer?.stop();
  speechActivityRenderer?.stop();
}

window.addEventListener("DOMContentLoaded", async () => {
  // Disable default context menu
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
  });

  // Get canvas elements
  waveformCanvas = document.querySelector("#waveform-canvas");
  spectrogramCanvas = document.querySelector("#spectrogram-canvas");
  speechActivityCanvas = document.querySelector("#speech-activity-canvas");
  closeBtn = document.querySelector("#close-btn");

  // Initialize renderers
  if (waveformCanvas) {
    waveformRenderer = new WaveformRenderer(waveformCanvas);
    waveformRenderer.drawIdle();
  }

  if (spectrogramCanvas) {
    spectrogramRenderer = new SpectrogramRenderer(spectrogramCanvas);
    spectrogramRenderer.drawIdle();
  }

  if (speechActivityCanvas) {
    speechActivityRenderer = new SpeechActivityRenderer(speechActivityCanvas);
    speechActivityRenderer.drawIdle();
  }

  // Handle window resize
  window.addEventListener("resize", () => {
    if (waveformCanvas && waveformRenderer) {
      const dpr = window.devicePixelRatio || 1;
      const rect = waveformCanvas.getBoundingClientRect();
      waveformCanvas.width = rect.width * dpr;
      waveformCanvas.height = rect.height * dpr;
      const ctx = waveformCanvas.getContext("2d");
      if (ctx) {
        ctx.scale(dpr, dpr);
      }
    }
    if (spectrogramCanvas && spectrogramRenderer) {
      spectrogramRenderer.resize();
    }
    if (speechActivityCanvas && speechActivityRenderer) {
      speechActivityRenderer.resize();
    }
  });

  // Setup visualization event listener and start renderers
  await setupVisualizationListener();
  startRenderers();

  // Intercept close requests and hide instead of destroying the window
  // This allows the window to be reopened later
  const appWindow = getCurrentWindow();
  closeRequestedUnlisten = await appWindow.onCloseRequested(async (event) => {
    // Prevent the window from being destroyed
    event.preventDefault();
    // Hide it instead
    await appWindow.hide();
  });

  // Close button handler - hide instead of close to allow reopening
  closeBtn?.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    await appWindow.hide();
  });

  // Cleanup on window close (only runs if window is actually destroyed)
  window.addEventListener("beforeunload", () => {
    stopRenderers();
    cleanupVisualizationListener();
    if (closeRequestedUnlisten) {
      closeRequestedUnlisten();
      closeRequestedUnlisten = null;
    }
  });
});
