// Shared visualization renderer classes for FlowSTT
// Used by both main window (mini waveform) and visualization window (full renderers)

// Visualization data from backend (pre-computed)
export interface SpectrogramColumn {
  colors: number[]; // RGB triplets for each pixel row
}

// Speech detection metrics from backend
export interface SpeechMetrics {
  amplitude_db: number;      // RMS amplitude in decibels
  zcr: number;               // Zero-crossing rate (0.0 to 0.5)
  centroid_hz: number;       // Estimated spectral centroid in Hz
  is_speaking: boolean;      // Whether speech is currently detected
  is_voiced_pending: boolean; // Whether voiced speech onset is pending
  is_whisper_pending: boolean; // Whether whisper speech onset is pending
  is_transient: boolean;     // Whether current frame is classified as transient
  is_lookback_speech: boolean; // Whether this is lookback-determined speech
  lookback_offset_ms: number | null; // Lookback offset when speech just confirmed
}

export interface VisualizationPayload {
  waveform: number[];                    // Pre-downsampled amplitudes
  spectrogram: SpectrogramColumn | null; // Present when FFT buffer fills
  speech_metrics: SpeechMetrics | null;  // Present when speech processor is active
}

// Ring buffer for storing waveform samples
export class RingBuffer {
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
export class WaveformRenderer {
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
    
    const leftMargin = 40; // Space for Y-axis labels
    const rightMargin = 8; // Space to prevent right edge clipping
    const topMargin = 8; // Space to prevent top edge clipping
    const bottomMargin = 20; // Space for X-axis labels
    const graphWidth = width - leftMargin - rightMargin;
    const graphHeight = height - topMargin - bottomMargin;
    
    this.ctx.strokeStyle = gridColor;
    this.ctx.lineWidth = 1;

    // Horizontal grid lines (amplitude levels) - tighter spacing
    const horizontalLines = 8;
    for (let i = 0; i <= horizontalLines; i++) {
      const y = topMargin + (graphHeight / horizontalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(leftMargin, y);
      this.ctx.lineTo(leftMargin + graphWidth, y);
      this.ctx.stroke();
    }

    // Vertical grid lines (time divisions) - tighter spacing
    const verticalLines = 16;
    for (let i = 0; i <= verticalLines; i++) {
      const x = leftMargin + (graphWidth / verticalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(x, topMargin);
      this.ctx.lineTo(x, topMargin + graphHeight);
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
      const y = topMargin + yPositions[i] * graphHeight;
      this.ctx.fillText(yLabels[i], leftMargin - 4, y);
    }

    // Draw X-axis labels (time in seconds, 0 = now on right)
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";
    
    // Ring buffer holds 512 samples, each emit ~10ms, so ~80ms visible
    // Labels show time ago (0 = now on right, older on left)
    const timeLabels = ["-80ms", "-60ms", "-40ms", "-20ms", "0"];
    for (let i = 0; i < timeLabels.length; i++) {
      const x = leftMargin + (graphWidth / (timeLabels.length - 1)) * i;
      this.ctx.fillText(timeLabels[i], x, topMargin + graphHeight + 4);
    }
  }

  // Get the drawable area dimensions (excluding margins)
  private getDrawableArea(): { x: number; y: number; width: number; height: number } {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    const leftMargin = 40;
    const rightMargin = 8;
    const topMargin = 8;
    const bottomMargin = 20;
    return {
      x: leftMargin,
      y: topMargin,
      width: width - leftMargin - rightMargin,
      height: height - topMargin - bottomMargin
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
export class SpectrogramRenderer {
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
  private readonly leftMargin = 40;
  private readonly rightMargin = 8;
  private readonly topMargin = 8;
  private readonly bottomMargin = 20;

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
    const drawableWidth = Math.floor(rect.width - this.leftMargin - this.rightMargin);
    const drawableHeight = Math.floor(rect.height - this.topMargin - this.bottomMargin);
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
    const drawableWidth = width - this.leftMargin - this.rightMargin;
    const drawableHeight = height - this.topMargin - this.bottomMargin;
    this.ctx.drawImage(
      this.offscreenCanvas,
      0, 0, this.offscreenCanvas.width, this.offscreenCanvas.height,
      this.leftMargin, this.topMargin, drawableWidth, drawableHeight
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
    
    const graphWidth = width - this.leftMargin - this.rightMargin;
    const graphHeight = height - this.topMargin - this.bottomMargin;
    
    this.ctx.strokeStyle = gridColor;
    this.ctx.lineWidth = 1;

    // Horizontal grid lines at log-spaced frequencies
    const gridFrequencies = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    for (const freq of gridFrequencies) {
      const yPos = this.freqToYPosition(freq);
      const y = this.topMargin + yPos * graphHeight;
      this.ctx.beginPath();
      this.ctx.moveTo(this.leftMargin, y);
      this.ctx.lineTo(this.leftMargin + graphWidth, y);
      this.ctx.stroke();
    }

    // Vertical grid lines (time divisions) - 16 lines to match waveform
    const verticalLines = 16;
    for (let i = 0; i <= verticalLines; i++) {
      const x = this.leftMargin + (graphWidth / verticalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(x, this.topMargin);
      this.ctx.lineTo(x, this.topMargin + graphHeight);
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
      const y = this.topMargin + yPos * graphHeight;
      this.ctx.fillText(labelNames[i], this.leftMargin - 4, y);
    }

    // Draw X-axis labels (time in seconds, 0 = now on right)
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";
    
    // Spectrogram scrolls with FFT columns, each ~10ms at 48kHz
    // Canvas width determines visible duration - estimate ~2.5s based on typical width
    const timeLabels = ["-2.5s", "-2s", "-1.5s", "-1s", "-0.5s", "0"];
    for (let i = 0; i < timeLabels.length; i++) {
      const x = this.leftMargin + (graphWidth / (timeLabels.length - 1)) * i;
      this.ctx.fillText(timeLabels[i], x, this.topMargin + graphHeight + 4);
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

// Buffered metric entry for delay buffer
interface BufferedMetric {
  amplitude: number;
  zcr: number;
  centroid: number;
  speaking: boolean;
  voicedPending: boolean;
  whisperPending: boolean;
  transient: boolean;
  isLookbackSpeech: boolean;
}

// Speech Activity renderer - visualizes speech detection algorithm components
// Includes a 200ms delay buffer to allow lookback results to be displayed correctly
export class SpeechActivityRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private animationId: number | null = null;
  private isActive: boolean = false;
  
  // Ring buffers for each metric (stores normalized 0-1 values)
  private amplitudeBuffer: Float32Array;
  private zcrBuffer: Float32Array;
  private centroidBuffer: Float32Array;
  private speakingBuffer: Uint8Array;  // 0 or 1
  private lookbackSpeechBuffer: Uint8Array; // 0 or 1 - lookback-determined speech
  private voicedPendingBuffer: Uint8Array;
  private whisperPendingBuffer: Uint8Array;
  private transientBuffer: Uint8Array;
  
  private bufferSize: number;
  private writeIndex: number = 0;
  private filled: boolean = false;
  
  // Delay buffer for retroactive lookback insertion
  // At ~10ms per metric emit, 20 entries ≈ 200ms delay
  private readonly delayBufferSize = 20;
  private delayBuffer: BufferedMetric[] = [];
  
  // Layout constants
  private readonly leftMargin = 40;
  private readonly rightMargin = 8;
  private readonly topMargin = 8;
  private readonly bottomMargin = 20;
  
  // Normalization ranges
  private readonly minDb = -60;
  private readonly maxDb = 0;
  private readonly maxZcr = 0.5;
  private readonly maxCentroid = 8000;
  
  // Threshold values for reference lines
  private readonly voicedThresholdDb = -40;
  private readonly whisperThresholdDb = -50;

  constructor(canvas: HTMLCanvasElement, bufferSize: number = 256) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      throw new Error("Could not get canvas 2D context");
    }
    this.ctx = ctx;
    this.bufferSize = bufferSize;
    
    // Initialize ring buffers
    this.amplitudeBuffer = new Float32Array(bufferSize);
    this.zcrBuffer = new Float32Array(bufferSize);
    this.centroidBuffer = new Float32Array(bufferSize);
    this.speakingBuffer = new Uint8Array(bufferSize);
    this.lookbackSpeechBuffer = new Uint8Array(bufferSize);
    this.voicedPendingBuffer = new Uint8Array(bufferSize);
    this.whisperPendingBuffer = new Uint8Array(bufferSize);
    this.transientBuffer = new Uint8Array(bufferSize);
    
    this.setupCanvas();
  }

  private setupCanvas(): void {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.ctx.scale(dpr, dpr);
  }

  pushMetrics(metrics: SpeechMetrics): void {
    // Normalize amplitude from dB to 0-1 range
    const normalizedAmplitude = Math.max(0, Math.min(1, 
      (metrics.amplitude_db - this.minDb) / (this.maxDb - this.minDb)
    ));
    
    // Normalize ZCR (already 0 to ~0.5)
    const normalizedZcr = Math.min(1, metrics.zcr / this.maxZcr);
    
    // Normalize centroid
    const normalizedCentroid = Math.min(1, metrics.centroid_hz / this.maxCentroid);
    
    // Create buffered metric
    const bufferedMetric: BufferedMetric = {
      amplitude: normalizedAmplitude,
      zcr: normalizedZcr,
      centroid: normalizedCentroid,
      speaking: metrics.is_speaking,
      voicedPending: metrics.is_voiced_pending,
      whisperPending: metrics.is_whisper_pending,
      transient: metrics.is_transient,
      isLookbackSpeech: false,
    };
    
    // Add to delay buffer
    this.delayBuffer.push(bufferedMetric);
    
    // If we have a lookback offset, mark previous entries as lookback speech
    if (metrics.lookback_offset_ms !== null && metrics.lookback_offset_ms > 0) {
      // Calculate how many entries back to mark
      // Each entry is ~10ms, so offset_ms / 10 = number of entries
      const entriesToMark = Math.min(
        Math.ceil(metrics.lookback_offset_ms / 10),
        this.delayBuffer.length - 1 // Don't mark current entry
      );
      
      // Mark previous entries as lookback speech
      for (let i = 0; i < entriesToMark; i++) {
        const idx = this.delayBuffer.length - 2 - i;
        if (idx >= 0) {
          this.delayBuffer[idx].isLookbackSpeech = true;
          this.delayBuffer[idx].speaking = true; // Also mark as speaking
        }
      }
    }
    
    // If delay buffer is full, transfer oldest entry to ring buffers
    if (this.delayBuffer.length > this.delayBufferSize) {
      const oldest = this.delayBuffer.shift()!;
      this.transferToRingBuffer(oldest);
    }
  }
  
  private transferToRingBuffer(metric: BufferedMetric): void {
    this.amplitudeBuffer[this.writeIndex] = metric.amplitude;
    this.zcrBuffer[this.writeIndex] = metric.zcr;
    this.centroidBuffer[this.writeIndex] = metric.centroid;
    this.speakingBuffer[this.writeIndex] = metric.speaking ? 1 : 0;
    this.lookbackSpeechBuffer[this.writeIndex] = metric.isLookbackSpeech ? 1 : 0;
    this.voicedPendingBuffer[this.writeIndex] = metric.voicedPending ? 1 : 0;
    this.whisperPendingBuffer[this.writeIndex] = metric.whisperPending ? 1 : 0;
    this.transientBuffer[this.writeIndex] = metric.transient ? 1 : 0;
    
    this.writeIndex = (this.writeIndex + 1) % this.bufferSize;
    if (this.writeIndex === 0) {
      this.filled = true;
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
    this.amplitudeBuffer.fill(0);
    this.zcrBuffer.fill(0);
    this.centroidBuffer.fill(0);
    this.speakingBuffer.fill(0);
    this.lookbackSpeechBuffer.fill(0);
    this.voicedPendingBuffer.fill(0);
    this.whisperPendingBuffer.fill(0);
    this.transientBuffer.fill(0);
    this.delayBuffer = [];
    this.writeIndex = 0;
    this.filled = false;
    this.drawIdle();
  }

  private animate = (): void => {
    if (!this.isActive) return;
    this.draw();
    this.animationId = requestAnimationFrame(this.animate);
  };

  private getDrawableArea(): { x: number; y: number; width: number; height: number } {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    return {
      x: this.leftMargin,
      y: this.topMargin,
      width: width - this.leftMargin - this.rightMargin,
      height: height - this.topMargin - this.bottomMargin
    };
  }

  // Get samples in chronological order (oldest to newest)
  private getSamplesInOrder<T extends Float32Array | Uint8Array>(buffer: T): T {
    if (!this.filled) {
      return buffer.slice(0, this.writeIndex) as T;
    }
    const result = new (buffer.constructor as any)(buffer.length);
    const secondPart = buffer.slice(this.writeIndex);
    const firstPart = buffer.slice(0, this.writeIndex);
    result.set(secondPart, 0);
    result.set(firstPart, secondPart.length);
    return result;
  }

  private draw(): void {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    const area = this.getDrawableArea();

    // Clear canvas
    const bgColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-bg")
      .trim() || "#1e293b";
    this.ctx.fillStyle = bgColor;
    this.ctx.fillRect(0, 0, width, height);

    // Draw grid first (underneath everything)
    this.drawGrid(width, height);

    // Get ordered samples
    const amplitudes = this.getSamplesInOrder(this.amplitudeBuffer);
    const zcrs = this.getSamplesInOrder(this.zcrBuffer);
    const centroids = this.getSamplesInOrder(this.centroidBuffer);
    const speaking = this.getSamplesInOrder(this.speakingBuffer);
    const lookbackSpeech = this.getSamplesInOrder(this.lookbackSpeechBuffer);
    const voicedPending = this.getSamplesInOrder(this.voicedPendingBuffer);
    const whisperPending = this.getSamplesInOrder(this.whisperPendingBuffer);
    const transients = this.getSamplesInOrder(this.transientBuffer);

    const sampleCount = amplitudes.length;
    if (sampleCount === 0) return;

    // Draw speech state bar (semi-transparent, at top)
    // Now includes lookback speech in a different color
    this.drawSpeechBar(speaking, lookbackSpeech, area);

    // Draw metric lines
    // Amplitude (gold/yellow)
    this.drawMetricLine(amplitudes, area, "rgba(245, 158, 11, 0.75)", 1);
    
    // ZCR (cyan)
    this.drawMetricLine(zcrs, area, "rgba(6, 182, 212, 0.75)", 1);
    
    // Spectral centroid (magenta)
    this.drawMetricLine(centroids, area, "rgba(217, 70, 239, 0.75)", 1);

    // Draw state markers
    this.drawStateMarkers(voicedPending, area, "rgba(34, 197, 94, 0.7)"); // Green for voiced pending
    this.drawStateMarkers(whisperPending, area, "rgba(59, 130, 246, 0.7)"); // Blue for whisper pending
    this.drawStateMarkers(transients, area, "rgba(239, 68, 68, 0.7)"); // Red for transients
  }

  private drawSpeechBar(
    speaking: Uint8Array,
    lookbackSpeech: Uint8Array,
    area: { x: number; y: number; width: number; height: number }
  ): void {
    if (speaking.length === 0) return;

    const barHeight = area.height * 0.15; // 15% of graph height
    const offset = this.bufferSize - speaking.length;

    // First pass: draw lookback speech regions (bright blue color)
    this.ctx.fillStyle = "rgba(59, 130, 246, 0.7)"; // Bright blue for lookback
    let inLookback = false;
    let lookbackStartX = 0;

    for (let i = 0; i <= lookbackSpeech.length; i++) {
      const isLookback = i < lookbackSpeech.length && lookbackSpeech[i] === 1;
      const x = area.x + ((offset + i) / this.bufferSize) * area.width;

      if (isLookback && !inLookback) {
        inLookback = true;
        lookbackStartX = x;
      } else if (!isLookback && inLookback) {
        inLookback = false;
        this.ctx.fillRect(lookbackStartX, area.y, x - lookbackStartX, barHeight);
      }
    }

    // Second pass: draw confirmed speech regions (green color, on top of lookback)
    this.ctx.fillStyle = "rgba(34, 197, 94, 0.5)"; // Semi-transparent green for confirmed
    let inSpeech = false;
    let speechStartX = 0;

    for (let i = 0; i <= speaking.length; i++) {
      // Only draw confirmed speech where it's NOT lookback speech
      const isSpeaking = i < speaking.length && speaking[i] === 1;
      const isLookbackHere = i < lookbackSpeech.length && lookbackSpeech[i] === 1;
      const isConfirmedSpeech = isSpeaking && !isLookbackHere;
      const x = area.x + ((offset + i) / this.bufferSize) * area.width;

      if (isConfirmedSpeech && !inSpeech) {
        inSpeech = true;
        speechStartX = x;
      } else if (!isConfirmedSpeech && inSpeech) {
        inSpeech = false;
        this.ctx.fillRect(speechStartX, area.y, x - speechStartX, barHeight);
      }
    }
  }

  private drawMetricLine(
    values: Float32Array,
    area: { x: number; y: number; width: number; height: number },
    color: string,
    lineWidth: number
  ): void {
    if (values.length === 0) return;

    this.ctx.beginPath();
    this.ctx.strokeStyle = color;
    this.ctx.lineWidth = lineWidth;

    // Always use full buffer size for consistent positioning (scrolls from right)
    for (let i = 0; i < values.length; i++) {
      // Position relative to full buffer - data appears on right side
      const x = area.x + ((this.bufferSize - values.length + i) / this.bufferSize) * area.width;
      // Invert Y: 0 at bottom, 1 at top
      const y = area.y + area.height - values[i] * area.height;

      if (i === 0) {
        this.ctx.moveTo(x, y);
      } else {
        this.ctx.lineTo(x, y);
      }
    }

    this.ctx.stroke();
  }

  private drawStateMarkers(
    states: Uint8Array,
    area: { x: number; y: number; width: number; height: number },
    color: string
  ): void {
    if (states.length === 0) return;

    this.ctx.fillStyle = color;
    const markerRadius = 2;

    for (let i = 0; i < states.length; i++) {
      if (states[i] === 1) {
        // Position relative to full buffer - data appears on right side
        const x = area.x + ((this.bufferSize - states.length + i) / this.bufferSize) * area.width;
        // Draw marker at bottom of graph
        const y = area.y + area.height - markerRadius - 2;
        
        this.ctx.beginPath();
        this.ctx.arc(x, y, markerRadius, 0, Math.PI * 2);
        this.ctx.fill();
      }
    }
  }

  private drawGrid(width: number, height: number): void {
    const gridColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-grid")
      .trim() || "rgba(255, 255, 255, 0.08)";
    const textColor = getComputedStyle(document.documentElement)
      .getPropertyValue("--waveform-text")
      .trim() || "rgba(255, 255, 255, 0.5)";
    
    const graphWidth = width - this.leftMargin - this.rightMargin;
    const graphHeight = height - this.topMargin - this.bottomMargin;

    // Regular grid lines
    this.ctx.strokeStyle = gridColor;
    this.ctx.lineWidth = 1;

    // Horizontal grid lines (5 divisions)
    const horizontalLines = 5;
    for (let i = 0; i <= horizontalLines; i++) {
      const y = this.topMargin + (graphHeight / horizontalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(this.leftMargin, y);
      this.ctx.lineTo(this.leftMargin + graphWidth, y);
      this.ctx.stroke();
    }

    // Vertical grid lines (match other graphs with 16 divisions)
    const verticalLines = 16;
    for (let i = 0; i <= verticalLines; i++) {
      const x = this.leftMargin + (graphWidth / verticalLines) * i;
      this.ctx.beginPath();
      this.ctx.moveTo(x, this.topMargin);
      this.ctx.lineTo(x, this.topMargin + graphHeight);
      this.ctx.stroke();
    }

    // Draw threshold lines (slightly heavier than regular grid)
    this.ctx.strokeStyle = "rgba(255, 255, 255, 0.15)";
    this.ctx.lineWidth = 1.5;

    // -40dB threshold (voiced)
    const voicedY = this.topMargin + graphHeight - ((this.voicedThresholdDb - this.minDb) / (this.maxDb - this.minDb)) * graphHeight;
    this.ctx.beginPath();
    this.ctx.moveTo(this.leftMargin, voicedY);
    this.ctx.lineTo(this.leftMargin + graphWidth, voicedY);
    this.ctx.stroke();

    // -50dB threshold (whisper)
    const whisperY = this.topMargin + graphHeight - ((this.whisperThresholdDb - this.minDb) / (this.maxDb - this.minDb)) * graphHeight;
    this.ctx.beginPath();
    this.ctx.moveTo(this.leftMargin, whisperY);
    this.ctx.lineTo(this.leftMargin + graphWidth, whisperY);
    this.ctx.stroke();

    // Draw Y-axis labels
    this.ctx.fillStyle = textColor;
    this.ctx.font = "9px system-ui, sans-serif";
    this.ctx.textAlign = "right";
    this.ctx.textBaseline = "middle";

    // dB labels (primary scale for amplitude)
    const dbLabels = [0, -20, -40, -50, -60];
    for (const db of dbLabels) {
      const normalizedY = (db - this.minDb) / (this.maxDb - this.minDb);
      const y = this.topMargin + graphHeight - normalizedY * graphHeight;
      const label = db === -40 ? "-40V" : db === -50 ? "-50W" : `${db}`;
      this.ctx.fillText(label, this.leftMargin - 3, y);
    }

    // X-axis time labels (0 = now on right)
    this.ctx.textAlign = "center";
    this.ctx.textBaseline = "top";
    
    // Buffer holds 256 metrics, each emit ~10ms, so ~2.56s visible
    const timeLabels = ["-2.5s", "-2s", "-1.5s", "-1s", "-0.5s", "0"];
    for (let i = 0; i < timeLabels.length; i++) {
      const x = this.leftMargin + (graphWidth / (timeLabels.length - 1)) * i;
      this.ctx.fillText(timeLabels[i], x, this.topMargin + graphHeight + 4);
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

// Mini waveform renderer - simplified version for the main window header
export class MiniWaveformRenderer {
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
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    const samples = this.ringBuffer.getSamples();

    // Clear canvas with transparent background
    this.ctx.clearRect(0, 0, width, height);

    if (samples.length === 0) {
      this.drawIdleLine(width, height);
      return;
    }

    const centerY = height / 2;
    const amplitude = (height / 2 - 2) * 1.5;
    const pointCount = samples.length;

    // Build the path
    this.ctx.beginPath();
    for (let i = 0; i < pointCount; i++) {
      const sample = samples[i] || 0;
      const x = (i / pointCount) * width;
      const clampedSample = Math.max(-1, Math.min(1, sample));
      const y = centerY - clampedSample * amplitude;

      if (i === 0) {
        this.ctx.moveTo(x, y);
      } else {
        this.ctx.lineTo(x, y);
      }
    }

    // Draw gray waveform line
    this.ctx.strokeStyle = "#888888";
    this.ctx.lineWidth = 1.5;
    this.ctx.stroke();
  }

  drawIdle(): void {
    const dpr = window.devicePixelRatio || 1;
    const width = this.canvas.width / dpr;
    const height = this.canvas.height / dpr;
    this.ctx.clearRect(0, 0, width, height);
    this.drawIdleLine(width, height);
  }

  private drawIdleLine(width: number, height: number): void {
    const centerY = height / 2;
    this.ctx.strokeStyle = "#888888";
    this.ctx.lineWidth = 1;
    this.ctx.beginPath();
    this.ctx.moveTo(0, centerY);
    this.ctx.lineTo(width, centerY);
    this.ctx.stroke();
  }

  resize(): void {
    this.setupCanvas();
  }
}
