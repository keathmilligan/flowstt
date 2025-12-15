import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface AudioDevice {
  id: string;
  name: string;
  source_type: "Input" | "System" | "Mixed";
}

interface ModelStatus {
  available: boolean;
  path: string;
}

interface SpeechEventPayload {
  duration_ms: number | null;
}

// Visualization data from backend (pre-computed)
interface SpectrogramColumn {
  colors: number[]; // RGB triplets for each pixel row
}

interface VisualizationPayload {
  waveform: number[];                    // Pre-downsampled amplitudes
  spectrogram: SpectrogramColumn | null; // Present when FFT buffer fills
}

// Ring buffer for storing waveform samples
class RingBuffer {
  private buffer: Float32Array;
  private writeIndex: number = 0;
  private filled: boolean = false;

  constructor(capacity: number) {
    this.buffer = new Float32Array(capacity);
  }

  push(samples: number[]): void {
    for (const sample of samples) {
      this.buffer[this.writeIndex] = sample;
      this.writeIndex = (this.writeIndex + 1) % this.buffer.length;
      if (this.writeIndex === 0) {
        this.filled = true;
      }
    }
  }

  // Get samples in order (oldest to newest)
  getSamples(): Float32Array {
    if (!this.filled) {
      // Return only the filled portion
      return this.buffer.slice(0, this.writeIndex);
    }
    // Return samples in chronological order
    const result = new Float32Array(this.buffer.length);
    const secondPart = this.buffer.slice(this.writeIndex);
    const firstPart = this.buffer.slice(0, this.writeIndex);
    result.set(secondPart, 0);
    result.set(firstPart, secondPart.length);
    return result;
  }

  clear(): void {
    this.buffer.fill(0);
    this.writeIndex = 0;
    this.filled = false;
  }

  get length(): number {
    return this.filled ? this.buffer.length : this.writeIndex;
  }
}

// Waveform renderer using Canvas
class WaveformRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private animationId: number | null = null;
  private ringBuffer: RingBuffer;
  private isActive: boolean = false;

  constructor(canvas: HTMLCanvasElement, bufferSize: number = 512) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("Could not get canvas 2D context");
    }
    this.ctx = ctx;
    this.ringBuffer = new RingBuffer(bufferSize);
    this.setupCanvas();
  }

  private setupCanvas(): void {
    // Handle high DPI displays
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.ctx.scale(dpr, dpr);
  }

  pushSamples(samples: number[]): void {
    this.ringBuffer.push(samples);
  }

  start(): void {
    if (this.isActive) return;
    this.isActive = true;
    this.animate();
  }

  stop(): void {
    this.isActive = false;
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  get active(): boolean {
    return this.isActive;
  }

  clear(): void {
    this.ringBuffer.clear();
    this.drawIdle();
  }

  private animate = (): void => {
    if (!this.isActive) return;
    this.draw();
    this.animationId = requestAnimationFrame(this.animate);
  };

  private draw(): void {
    // Use cached dimensions to avoid layout thrashing
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    const samples = this.ringBuffer.getSamples();

    // Clear canvas
    this.ctx.fillStyle = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-bg")
      .trim() || "#1e293b";
    this.ctx.fillRect(0, 0, width, height);

    // Draw grid
    this.drawGrid(width, height);

    // Get drawable area (excluding axis labels)
    const area = this.getDrawableArea();

    if (samples.length === 0) {
      this.drawCenterLine(area);
      return;
    }

    // Get colors
    const waveformColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-color")
      .trim() || "#3b82f6";
    const glowColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-glow")
      .trim() || "rgba(59, 130, 246, 0.5)";

    const centerY = area.y + area.height / 2;
    const amplitude = (area.height / 2 - 4) * 1.5; // Increased amplitude scale

    // Draw all samples - each sample maps to a portion of the width
    const pointCount = samples.length;

    // Build the path once
    this.ctx.beginPath();
    for (let i = 0; i < pointCount; i++) {
      const sample = samples[i] || 0;
      const x = area.x + (i / pointCount) * area.width;
      // Clamp the sample to prevent drawing outside canvas
      const clampedSample = Math.max(-1, Math.min(1, sample));
      const y = centerY - clampedSample * amplitude;

      if (i === 0) {
        this.ctx.moveTo(x, y);
      } else {
        this.ctx.lineTo(x, y);
      }
    }

    // Draw glow layer (thicker, blurred)
    this.ctx.save();
    this.ctx.strokeStyle = glowColor;
    this.ctx.lineWidth = 6;
    this.ctx.filter = "blur(4px)";
    this.ctx.stroke();
    this.ctx.restore();

    // Draw main waveform line
    this.ctx.strokeStyle = waveformColor;
    this.ctx.lineWidth = 2;
    this.ctx.stroke();
  }

  private drawGrid(width: number, height: number): void {
    const gridColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-grid")
      .trim() || "rgba(255, 255, 255, 0.08)";
    const textColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-text")
      .trim() || "rgba(255, 255, 255, 0.5)";
    
    const leftMargin = 32; // Space for Y-axis labels
    const bottomMargin = 18; // Space for X-axis labels
    const graphWidth = width - leftMargin;
    const graphHeight = height - bottomMargin;
    
    this.ctx.strokeStyle = gridColor;
    this.ctx.lineWidth = 1;

    // Horizontal grid lines (amplitude levels) - tighter spacing
    const horizontalLines = 8;
    for (let i = 0; i <= horizontalLines; i++) {
      const y = (graphHeight / horizontalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(leftMargin, y);
      this.ctx.lineTo(width, y);
      this.ctx.stroke();
    }

    // Vertical grid lines (time divisions) - tighter spacing
    const verticalLines = 16;
    for (let i = 0; i <= verticalLines; i++) {
      const x = leftMargin + (graphWidth / verticalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, graphHeight);
      this.ctx.stroke();
    }

    // Draw Y-axis labels (amplitude)
    this.ctx.fillStyle = textColor;
    this.ctx.font = "10px system-ui, sans-serif";
    this.ctx.textAlign = "right";
    this.ctx.textBaseline = "middle";
    
    const yLabels = ["1.0", "0.5", "0", "-0.5", "-1.0"];
    const yPositions = [0, 0.25, 0.5, 0.75, 1];
    for (let i = 0; i < yLabels.length; i++) {
      const y = yPositions[i] * graphHeight;
      this.ctx.fillText(yLabels[i], leftMargin - 4, y);
    }

    // Draw X-axis labels (time in seconds)
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";
    
    // Assuming ~0.5 seconds of visible audio in buffer
    const timeLabels = ["0.0s", "0.1s", "0.2s", "0.3s", "0.4s", "0.5s"];
    for (let i = 0; i < timeLabels.length; i++) {
      const x = leftMargin + (graphWidth / (timeLabels.length - 1)) * i;
      this.ctx.fillText(timeLabels[i], x, graphHeight + 4);
    }
  }

  // Get the drawable area dimensions (excluding margins)
  private getDrawableArea(): { x: number; y: number; width: number; height: number } {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    const leftMargin = 32;
    const bottomMargin = 18;
    return {
      x: leftMargin,
      y: 0,
      width: width - leftMargin,
      height: height - bottomMargin
    };
  }

  drawIdle(): void {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;

    this.ctx.fillStyle = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-bg")
      .trim() || "#1e293b";
    this.ctx.fillRect(0, 0, width, height);
    this.drawGrid(width, height);
    const area = this.getDrawableArea();
    this.drawCenterLine(area);
  }

  private drawCenterLine(area: { x: number; y: number; width: number; height: number }): void {
    const lineColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-line")
      .trim() || "#475569";
    this.ctx.strokeStyle = lineColor;
    this.ctx.lineWidth = 1;
    this.ctx.beginPath();
    const centerY = area.y + area.height / 2;
    this.ctx.moveTo(area.x, centerY);
    this.ctx.lineTo(area.x + area.width, centerY);
    this.ctx.stroke();
  }
}

// Spectrogram renderer using Canvas - receives pre-computed RGB colors from backend
class SpectrogramRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private offscreenCanvas: HTMLCanvasElement;
  private offscreenCtx: CanvasRenderingContext2D;
  private animationId: number | null = null;
  private isActive: boolean = false;
  private imageData: ImageData | null = null;
  private columnQueue: number[][] = []; // Queue of pending columns
  private maxQueueSize: number = 60; // Limit queue to prevent memory growth

  // Layout constants matching waveform
  private readonly leftMargin = 32;
  private readonly bottomMargin = 18;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("Could not get canvas 2D context");
    }
    this.ctx = ctx;
    
    // Create offscreen canvas for spectrogram data
    this.offscreenCanvas = document.createElement("canvas");
    const offCtx = this.offscreenCanvas.getContext("2d");
    if (!offCtx) {
      throw new Error("Could not get offscreen canvas 2D context");
    }
    this.offscreenCtx = offCtx;
    
    this.setupCanvas();
  }

  private setupCanvas(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    
    // Setup main canvas with scaling for crisp text
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.ctx.scale(dpr, dpr);
    
    // Setup offscreen canvas for spectrogram (drawable area only)
    const drawableWidth = Math.floor(rect.width - this.leftMargin);
    const drawableHeight = Math.floor(rect.height - this.bottomMargin);
    this.offscreenCanvas.width = drawableWidth * dpr;
    this.offscreenCanvas.height = drawableHeight * dpr;
    
    // Create ImageData for pixel manipulation
    this.imageData = this.offscreenCtx.createImageData(
      drawableWidth * dpr,
      drawableHeight * dpr
    );
    this.fillBackground();
  }

  private fillBackground(): void {
    if (!this.imageData) return;
    const data = this.imageData.data;
    // Dark blue-gray background color (matches --waveform-bg: #0a0f1a)
    for (let i = 0; i < data.length; i += 4) {
      data[i] = 10;     // R
      data[i + 1] = 15;  // G
      data[i + 2] = 26;  // B
      data[i + 3] = 255; // A
    }
  }

  // Push a pre-computed spectrogram column (RGB triplets from backend)
  pushColumn(colors: number[]): void {
    // Queue the column for processing during render
    if (this.columnQueue.length < this.maxQueueSize) {
      this.columnQueue.push(colors);
    }
    // If queue is full, drop oldest to prevent lag buildup
    else {
      this.columnQueue.shift();
      this.columnQueue.push(colors);
    }
  }

  start(): void {
    if (this.isActive) return;
    this.isActive = true;
    this.animate();
  }

  stop(): void {
    this.isActive = false;
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  get active(): boolean {
    return this.isActive;
  }

  clear(): void {
    this.columnQueue = [];
    this.fillBackground();
    this.drawIdle();
  }

  private animate = (): void => {
    if (!this.isActive) return;
    this.draw();
    this.animationId = requestAnimationFrame(this.animate);
  };

  private draw(): void {
    if (!this.imageData) return;
    
    // Use cached dimensions from setupCanvas to avoid layout thrashing
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    
    // Process queued columns from backend
    const columnsToProcess = Math.min(
      this.columnQueue.length,
      Math.max(2, Math.ceil(this.columnQueue.length / 4))
    );
    
    for (let i = 0; i < columnsToProcess; i++) {
      const column = this.columnQueue.shift()!;
      this.scrollLeft();
      this.drawColumn(column);
    }
    
    // Clear main canvas
    const bgColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-bg")
      .trim() || "#000032";
    this.ctx.fillStyle = bgColor;
    this.ctx.fillRect(0, 0, width, height);
    
    // Put spectrogram ImageData to offscreen canvas, then draw to main canvas
    this.offscreenCtx.putImageData(this.imageData, 0, 0);
    
    // Draw offscreen canvas to main canvas in the drawable area
    const drawableWidth = width - this.leftMargin;
    const drawableHeight = height - this.bottomMargin;
    this.ctx.drawImage(
      this.offscreenCanvas,
      0, 0, this.offscreenCanvas.width, this.offscreenCanvas.height,
      this.leftMargin, 0, drawableWidth, drawableHeight
    );
    
    // Draw grid on top of spectrogram
    this.drawGrid(width, height);
  }

  private scrollLeft(): void {
    if (!this.imageData) return;
    const data = this.imageData.data;
    const width = this.imageData.width;
    const height = this.imageData.height;
    
    // Shift each row left by 1 pixel
    for (let y = 0; y < height; y++) {
      const rowStart = y * width * 4;
      // Copy pixels from x+1 to x
      for (let x = 0; x < width - 1; x++) {
        const destIdx = rowStart + x * 4;
        const srcIdx = rowStart + (x + 1) * 4;
        data[destIdx] = data[srcIdx];
        data[destIdx + 1] = data[srcIdx + 1];
        data[destIdx + 2] = data[srcIdx + 2];
        data[destIdx + 3] = data[srcIdx + 3];
      }
    }
  }

  // Convert frequency (Hz) to Y position (0-1, where 0=top, 1=bottom)
  private freqToYPosition(freq: number): number {
    const minFreq = 20;
    const maxFreq = 24000;
    const minLog = Math.log10(minFreq);
    const maxLog = Math.log10(maxFreq);
    
    const logFreq = Math.log10(Math.max(minFreq, Math.min(maxFreq, freq)));
    const pos = (logFreq - minLog) / (maxLog - minLog);
    return 1 - pos; // Invert so high freq is at top
  }

  // Draw a column of pre-computed RGB colors from backend
  private drawColumn(colors: number[]): void {
    if (!this.imageData) return;
    const data = this.imageData.data;
    const width = this.imageData.width;
    const height = this.imageData.height;
    
    // Colors array has RGB triplets, one per pixel row
    const numPixels = Math.floor(colors.length / 3);
    
    // Draw column at rightmost position
    const x = width - 1;
    
    // Scale backend pixels to canvas height
    const scaleY = numPixels / height;
    
    for (let y = 0; y < height; y++) {
      // Map canvas y to backend pixel (with scaling)
      const srcY = Math.floor(y * scaleY);
      const srcIdx = Math.min(srcY, numPixels - 1) * 3;
      
      // Set pixel with colors from backend
      const idx = (y * width + x) * 4;
      data[idx] = colors[srcIdx] || 10;       // R
      data[idx + 1] = colors[srcIdx + 1] || 15; // G
      data[idx + 2] = colors[srcIdx + 2] || 26; // B
      data[idx + 3] = 255;                      // A
    }
  }

  private drawGrid(width: number, height: number): void {
    const gridColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--spectrogram-grid")
      .trim() || "rgba(255, 255, 255, 0.12)";
    const textColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-text")
      .trim() || "rgba(255, 255, 255, 0.5)";
    
    const graphWidth = width - this.leftMargin;
    const graphHeight = height - this.bottomMargin;
    
    this.ctx.strokeStyle = gridColor;
    this.ctx.lineWidth = 1;

    // Horizontal grid lines at log-spaced frequencies
    const gridFrequencies = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    for (const freq of gridFrequencies) {
      const yPos = this.freqToYPosition(freq);
      const y = yPos * graphHeight;
      this.ctx.beginPath();
      this.ctx.moveTo(this.leftMargin, y);
      this.ctx.lineTo(width, y);
      this.ctx.stroke();
    }

    // Vertical grid lines (time divisions) - 16 lines to match waveform
    const verticalLines = 16;
    for (let i = 0; i <= verticalLines; i++) {
      const x = this.leftMargin + (graphWidth / verticalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(x, 0);
      this.ctx.lineTo(x, graphHeight);
      this.ctx.stroke();
    }

    // Draw Y-axis labels at log-spaced frequencies
    this.ctx.fillStyle = textColor;
    this.ctx.font = "10px system-ui, sans-serif";
    this.ctx.textAlign = "right";
    this.ctx.textBaseline = "middle";
    
    // Frequency labels (log scale)
    const labelFrequencies = [100, 500, 1000, 5000, 20000];
    const labelNames = ["100", "500", "1k", "5k", "20k"];
    for (let i = 0; i < labelFrequencies.length; i++) {
      const yPos = this.freqToYPosition(labelFrequencies[i]);
      const y = yPos * graphHeight;
      this.ctx.fillText(labelNames[i], this.leftMargin - 4, y);
    }

    // Draw X-axis labels (time in seconds)
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";
    
    const timeLabels = ["0.0s", "0.1s", "0.2s", "0.3s", "0.4s", "0.5s"];
    for (let i = 0; i < timeLabels.length; i++) {
      const x = this.leftMargin + (graphWidth / (timeLabels.length - 1)) * i;
      this.ctx.fillText(timeLabels[i], x, graphHeight + 4);
    }
  }

  drawIdle(): void {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    
    const bgColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-bg")
      .trim() || "#1e293b";
    this.ctx.fillStyle = bgColor;
    this.ctx.fillRect(0, 0, width, height);
    
    this.drawGrid(width, height);
  }

  resize(): void {
    this.setupCanvas();
  }
}

// DOM elements
let source1Select: HTMLSelectElement | null;
let source2Select: HTMLSelectElement | null;
let recordBtn: HTMLButtonElement | null;
let monitorToggle: HTMLInputElement | null;
let processingToggle: HTMLInputElement | null;
let statusEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let modelWarning: HTMLElement | null;
let modelPathEl: HTMLElement | null;
let downloadModelBtn: HTMLButtonElement | null;
let downloadStatusEl: HTMLElement | null;
let waveformCanvas: HTMLCanvasElement | null;
let spectrogramCanvas: HTMLCanvasElement | null;
let closeBtn: HTMLButtonElement | null;

// State
let isRecording = false;
let isMonitoring = false;
let isProcessingEnabled = false;
let wasMonitoringBeforeRecording = false;
let allDevices: AudioDevice[] = [];
let waveformRenderer: WaveformRenderer | null = null;
let spectrogramRenderer: SpectrogramRenderer | null = null;
let visualizationUnlisten: UnlistenFn | null = null;
let transcriptionCompleteUnlisten: UnlistenFn | null = null;
let transcriptionErrorUnlisten: UnlistenFn | null = null;
let speechStartedUnlisten: UnlistenFn | null = null;
let speechEndedUnlisten: UnlistenFn | null = null;

async function loadDevices() {
  try {
    // Load all available sources
    allDevices = await invoke<AudioDevice[]>("list_all_sources");

    // Populate both source dropdowns
    populateSourceDropdown(source1Select, true);  // Has "None" option, select first device
    populateSourceDropdown(source2Select, false); // Has "None" option, select "None"
    
    // Enable controls if we have at least one device
    const hasDevices = allDevices.length > 0;
    if (recordBtn) {
      recordBtn.disabled = !hasDevices;
    }
    if (monitorToggle) {
      monitorToggle.disabled = !hasDevices;
    }
    if (processingToggle) {
      processingToggle.disabled = !hasDevices;
    }
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

function hasAnySourceSelected(): boolean {
  const { source1Id, source2Id } = getSelectedSources();
  return source1Id !== null || source2Id !== null;
}

// Handle source selection changes - reconfigure capture if active
async function onSourceChange() {
  if (!isMonitoring && !isRecording) {
    // Not active, nothing to do
    return;
  }

  if (!hasAnySourceSelected()) {
    // No sources selected - stop everything
    if (isRecording) {
      // Can't record with no sources - stop recording
      setStatus("Recording stopped: no sources selected", "error");
      try {
        await invoke("stop_recording", { keepMonitoring: false });
      } catch (e) {
        console.error("Error stopping recording:", e);
      }
      isRecording = false;
      isMonitoring = false;
      if (recordBtn) {
        recordBtn.textContent = "Record";
        recordBtn.classList.remove("recording");
      }
      if (monitorToggle) {
        monitorToggle.checked = false;
        monitorToggle.disabled = false;
      }
      waveformRenderer?.stop();
      waveformRenderer?.clear();
      spectrogramRenderer?.stop();
      spectrogramRenderer?.clear();
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
      waveformRenderer?.stop();
      waveformRenderer?.clear();
      spectrogramRenderer?.stop();
      spectrogramRenderer?.clear();
      await cleanupVisualizationListener();
      setStatus("");
    }
    return;
  }

  // Reconfigure with new sources
  const { source1Id, source2Id } = getSelectedSources();

  if (isRecording) {
    // Restart recording with new sources
    try {
      await invoke("start_recording", { source1Id, source2Id });
      const statusText = source1Id && source2Id 
        ? "Recording (Mixed)..." 
        : "Recording...";
      setStatus(statusText, "loading");
    } catch (error) {
      console.error("Error reconfiguring recording:", error);
      setStatus(`Error: ${error}`, "error");
    }
  } else if (isMonitoring) {
    // Restart monitoring with new sources
    try {
      await invoke("start_monitor", { source1Id, source2Id });
      const statusText = source1Id && source2Id 
        ? "Monitoring (Mixed)..." 
        : "Monitoring...";
      setStatus(statusText, "loading");
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

function setStatus(message: string, type: "normal" | "loading" | "error" = "normal") {
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
    // Push pre-downsampled waveform data
    if (waveformRenderer) {
      waveformRenderer.pushSamples(event.payload.waveform);
    }
    // Push pre-computed spectrogram column when available
    if (spectrogramRenderer && event.payload.spectrogram) {
      spectrogramRenderer.pushColumn(event.payload.spectrogram.colors);
    }
  });
}

async function cleanupVisualizationListener() {
  if (visualizationUnlisten) {
    visualizationUnlisten();
    visualizationUnlisten = null;
  }
}

async function setupTranscriptionListeners() {
  if (transcriptionCompleteUnlisten) return;

  transcriptionCompleteUnlisten = await listen<string>("transcription-complete", (event) => {
    if (resultEl) {
      resultEl.textContent = event.payload;
    }
    if (isMonitoring) {
      setStatus("Monitoring...", "loading");
    } else {
      setStatus("Transcription complete");
    }
  });

  transcriptionErrorUnlisten = await listen<string>("transcription-error", (event) => {
    console.error("Transcription error:", event.payload);
    setStatus(`Transcription error: ${event.payload}`, "error");
  });
}

// Cleanup function for transcription listeners (called on app cleanup if needed)
export function cleanupTranscriptionListeners() {
  if (transcriptionCompleteUnlisten) {
    transcriptionCompleteUnlisten();
    transcriptionCompleteUnlisten = null;
  }
  if (transcriptionErrorUnlisten) {
    transcriptionErrorUnlisten();
    transcriptionErrorUnlisten = null;
  }
}

async function setupSpeechEventListeners() {
  if (speechStartedUnlisten) return;

  speechStartedUnlisten = await listen<SpeechEventPayload>("speech-started", (_event) => {
    console.log("[Speech] Started speaking");
  });

  speechEndedUnlisten = await listen<SpeechEventPayload>("speech-ended", (event) => {
    const duration = event.payload.duration_ms;
    console.log(`[Speech] Stopped speaking (duration: ${duration}ms)`);
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

async function toggleProcessing() {
  if (!processingToggle) return;

  const newState = processingToggle.checked;
  try {
    if (newState) {
      await setupSpeechEventListeners();
    }
    await invoke("set_processing_enabled", { enabled: newState });
    isProcessingEnabled = newState;
    console.log(`Voice processing ${isProcessingEnabled ? "enabled" : "disabled"}`);
    if (!newState) {
      cleanupSpeechEventListeners();
    }
  } catch (error) {
    console.error("Toggle processing error:", error);
    // Revert toggle on error
    processingToggle.checked = !newState;
  }
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
      
      waveformRenderer?.stop();
      waveformRenderer?.clear();
      spectrogramRenderer?.stop();
      spectrogramRenderer?.clear();
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
      
      const statusText = source1Id && source2Id 
        ? "Monitoring (Mixed)..." 
        : "Monitoring...";
      setStatus(statusText, "loading");
      
      waveformRenderer?.clear();
      waveformRenderer?.start();
      spectrogramRenderer?.clear();
      spectrogramRenderer?.start();
    } catch (error) {
      console.error("Start monitor error:", error);
      setStatus(`Error: ${error}`, "error");
      monitorToggle.checked = false;
      await cleanupVisualizationListener();
    }
  }
}

async function toggleRecording() {
  if (!recordBtn) return;

  if (isRecording) {
    // Stop recording
    try {
      await invoke("stop_recording", { 
        keepMonitoring: wasMonitoringBeforeRecording 
      });
      
      isRecording = false;
      recordBtn.textContent = "Record";
      recordBtn.classList.remove("recording");
      
      // Re-enable monitor toggle
      if (monitorToggle) {
        monitorToggle.disabled = false;
      }

      if (wasMonitoringBeforeRecording) {
        setStatus("Transcribing... (monitoring continues)", "loading");
      } else {
        isMonitoring = false;
        waveformRenderer?.stop();
        waveformRenderer?.clear();
        spectrogramRenderer?.stop();
        spectrogramRenderer?.clear();
        await cleanupVisualizationListener();
        setStatus("Transcribing...", "loading");
      }

      if (resultEl) {
        resultEl.textContent = "Processing audio...";
      }
      
      wasMonitoringBeforeRecording = false;
    } catch (error) {
      console.error("Stop recording error:", error);
      setStatus(`Error: ${error}`, "error");
      isRecording = false;
      recordBtn.textContent = "Record";
      recordBtn.classList.remove("recording");
      if (monitorToggle) {
        monitorToggle.disabled = false;
      }
      waveformRenderer?.stop();
      waveformRenderer?.clear();
      spectrogramRenderer?.stop();
      spectrogramRenderer?.clear();
      await cleanupVisualizationListener();
      isMonitoring = false;
      wasMonitoringBeforeRecording = false;
      if (monitorToggle) {
        monitorToggle.checked = false;
      }
    }
  } else {
    // Start recording
    if (!hasAnySourceSelected()) {
      setStatus("Please select at least one audio source", "error");
      return;
    }

    const { source1Id, source2Id } = getSelectedSources();
    wasMonitoringBeforeRecording = isMonitoring;

    try {
      await setupVisualizationListener();
      await setupTranscriptionListeners();
      
      await invoke("start_recording", { 
        source1Id,
        source2Id,
      });
      isRecording = true;
      isMonitoring = true;
      recordBtn.textContent = "Stop";
      recordBtn.classList.add("recording");
      
      const statusText = source1Id && source2Id 
        ? "Recording (Mixed)..." 
        : "Recording...";
      setStatus(statusText, "loading");
      
      // Disable monitor toggle during recording (can't toggle it)
      if (monitorToggle) {
        monitorToggle.disabled = true;
      }
      // Source selects remain enabled so user can change sources on the fly

      if (!waveformRenderer?.active) {
        waveformRenderer?.clear();
      }
      waveformRenderer?.start();
      if (!spectrogramRenderer?.active) {
        spectrogramRenderer?.clear();
      }
      spectrogramRenderer?.start();

      if (resultEl) {
        resultEl.textContent = "Recording in progress...";
      }
    } catch (error) {
      console.error("Start recording error:", error);
      setStatus(`Error: ${error}`, "error");
      wasMonitoringBeforeRecording = false;
      if (!isMonitoring) {
        await cleanupVisualizationListener();
      }
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  source1Select = document.querySelector("#source1-select");
  source2Select = document.querySelector("#source2-select");
  recordBtn = document.querySelector("#record-btn");
  monitorToggle = document.querySelector("#monitor-toggle");
  processingToggle = document.querySelector("#processing-toggle");
  statusEl = document.querySelector("#status");
  resultEl = document.querySelector("#transcription-result");
  modelWarning = document.querySelector("#model-warning");
  modelPathEl = document.querySelector("#model-path");
  downloadModelBtn = document.querySelector("#download-model-btn");
  downloadStatusEl = document.querySelector("#download-status");
  waveformCanvas = document.querySelector("#waveform-canvas");
  spectrogramCanvas = document.querySelector("#spectrogram-canvas");

  // Initialize waveform renderer
  if (waveformCanvas) {
    waveformRenderer = new WaveformRenderer(waveformCanvas);
    waveformRenderer.drawIdle();
  }

  // Initialize spectrogram renderer
  if (spectrogramCanvas) {
    spectrogramRenderer = new SpectrogramRenderer(spectrogramCanvas);
    spectrogramRenderer.drawIdle();
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
  });

  // Setup transcription listeners early
  setupTranscriptionListeners();

  closeBtn = document.querySelector("#close-btn");

  recordBtn?.addEventListener("click", toggleRecording);
  monitorToggle?.addEventListener("change", toggleMonitor);
  processingToggle?.addEventListener("change", toggleProcessing);
  downloadModelBtn?.addEventListener("click", downloadModel);
  source1Select?.addEventListener("change", onSourceChange);
  source2Select?.addEventListener("change", onSourceChange);
  closeBtn?.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    const window = getCurrentWindow();
    await window.destroy();
  });

  loadDevices();
  checkModelStatus();
});
