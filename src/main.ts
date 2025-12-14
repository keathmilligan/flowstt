import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface AudioDevice {
  id: string;
  name: string;
}

interface ModelStatus {
  available: boolean;
  path: string;
}

interface AudioSamplesPayload {
  samples: number[];
}

interface SpeechEventPayload {
  duration_ms: number | null;
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

// FFT processor for spectrogram visualization
class FFTProcessor {
  private size: number;
  private cosTable: Float32Array;
  private sinTable: Float32Array;
  private hannWindow: Float32Array;

  constructor(size: number = 512) {
    // Size must be power of 2
    if ((size & (size - 1)) !== 0) {
      throw new Error("FFT size must be a power of 2");
    }
    this.size = size;
    
    // Pre-compute twiddle factors
    this.cosTable = new Float32Array(size / 2);
    this.sinTable = new Float32Array(size / 2);
    for (let i = 0; i < size / 2; i++) {
      const angle = (-2 * Math.PI * i) / size;
      this.cosTable[i] = Math.cos(angle);
      this.sinTable[i] = Math.sin(angle);
    }
    
    // Pre-compute Hanning window
    this.hannWindow = new Float32Array(size);
    for (let i = 0; i < size; i++) {
      this.hannWindow[i] = 0.5 * (1 - Math.cos((2 * Math.PI * i) / (size - 1)));
    }
  }

  // Compute FFT and return magnitude spectrum (half the size, positive frequencies only)
  process(samples: Float32Array): Float32Array {
    const n = this.size;
    
    // Apply window and prepare complex arrays
    const real = new Float32Array(n);
    const imag = new Float32Array(n);
    
    for (let i = 0; i < n; i++) {
      real[i] = (samples[i] || 0) * this.hannWindow[i];
      imag[i] = 0;
    }
    
    // Bit-reversal permutation
    this.bitReverse(real);
    this.bitReverse(imag);
    
    // Cooley-Tukey FFT
    for (let size = 2; size <= n; size *= 2) {
      const halfSize = size / 2;
      const step = n / size;
      
      for (let i = 0; i < n; i += size) {
        for (let j = 0; j < halfSize; j++) {
          const idx = j * step;
          const tReal = real[i + j + halfSize] * this.cosTable[idx] - imag[i + j + halfSize] * this.sinTable[idx];
          const tImag = real[i + j + halfSize] * this.sinTable[idx] + imag[i + j + halfSize] * this.cosTable[idx];
          
          real[i + j + halfSize] = real[i + j] - tReal;
          imag[i + j + halfSize] = imag[i + j] - tImag;
          real[i + j] += tReal;
          imag[i + j] += tImag;
        }
      }
    }
    
    // Compute magnitude (only positive frequencies, first half)
    const magnitudes = new Float32Array(n / 2);
    for (let i = 0; i < n / 2; i++) {
      magnitudes[i] = Math.sqrt(real[i] * real[i] + imag[i] * imag[i]) / n;
    }
    
    return magnitudes;
  }

  private bitReverse(array: Float32Array): void {
    const n = array.length;
    let j = 0;
    for (let i = 0; i < n - 1; i++) {
      if (i < j) {
        const temp = array[i];
        array[i] = array[j];
        array[j] = temp;
      }
      let k = n / 2;
      while (k <= j) {
        j -= k;
        k /= 2;
      }
      j += k;
    }
  }

  get fftSize(): number {
    return this.size;
  }
}

// Waveform renderer using Canvas
class WaveformRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private animationId: number | null = null;
  private ringBuffer: RingBuffer;
  private isActive: boolean = false;

  constructor(canvas: HTMLCanvasElement, bufferSize: number = 8192) {
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
    const width = this.canvas.getBoundingClientRect().width;
    const height = this.canvas.getBoundingClientRect().height;
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

    // Downsample if we have more samples than pixels
    const step = Math.max(1, Math.floor(samples.length / area.width));
    const pointCount = Math.min(samples.length, Math.floor(area.width));

    // Build the path once
    this.ctx.beginPath();
    for (let i = 0; i < pointCount; i++) {
      const sampleIndex = Math.floor(i * step);
      const sample = samples[sampleIndex] || 0;
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
    const width = this.canvas.getBoundingClientRect().width;
    const height = this.canvas.getBoundingClientRect().height;
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
    const width = this.canvas.getBoundingClientRect().width;
    const height = this.canvas.getBoundingClientRect().height;

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

// Spectrogram renderer using Canvas and FFT
class SpectrogramRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private offscreenCanvas: HTMLCanvasElement;
  private offscreenCtx: CanvasRenderingContext2D;
  private animationId: number | null = null;
  private fft: FFTProcessor;
  private sampleBuffer: Float32Array;
  private bufferWriteIndex: number = 0;
  private isActive: boolean = false;
  private imageData: ImageData | null = null;
  private colorLookup: Uint8ClampedArray[];
  private needsNewColumn: boolean = false;
  private pendingMagnitudes: Float32Array | null = null;

  // Layout constants matching waveform
  private readonly leftMargin = 32;
  private readonly bottomMargin = 18;

  constructor(canvas: HTMLCanvasElement, fftSize: number = 512) {
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
    
    this.fft = new FFTProcessor(fftSize);
    this.sampleBuffer = new Float32Array(fftSize);
    this.colorLookup = this.buildColorLookup();
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

  // Pre-compute color lookup table for magnitude -> RGB
  private buildColorLookup(): Uint8ClampedArray[] {
    const lookup: Uint8ClampedArray[] = [];
    const steps = 256;
    
    for (let i = 0; i < steps; i++) {
      const t = i / (steps - 1); // 0 to 1
      const color = this.magnitudeToColor(t);
      lookup.push(color);
    }
    
    return lookup;
  }

  // Heat map: dark gray-blue -> blue -> cyan -> yellow -> red
  // Uses smooth interpolation to avoid hard lines
  private magnitudeToColor(t: number): Uint8ClampedArray {
    const color = new Uint8ClampedArray(4);
    color[3] = 255; // Alpha
    
    // Apply gamma for better visual spread
    t = Math.pow(t, 0.7);
    
    // Define color stops with exact RGB values
    // Each stop: [position, R, G, B]
    const stops: [number, number, number, number][] = [
      [0.00, 10, 15, 26],    // Background color #0a0f1a
      [0.15, 0, 50, 200],    // Blue
      [0.35, 0, 255, 150],   // Cyan
      [0.60, 200, 255, 0],   // Yellow-green
      [0.80, 255, 155, 0],   // Orange
      [1.00, 255, 0, 0],     // Red
    ];
    
    // Find which segment we're in and interpolate
    for (let i = 0; i < stops.length - 1; i++) {
      const [pos1, r1, g1, b1] = stops[i];
      const [pos2, r2, g2, b2] = stops[i + 1];
      
      if (t >= pos1 && t <= pos2) {
        const s = (t - pos1) / (pos2 - pos1);
        color[0] = Math.round(r1 + s * (r2 - r1));
        color[1] = Math.round(g1 + s * (g2 - g1));
        color[2] = Math.round(b1 + s * (b2 - b1));
        return color;
      }
    }
    
    // Fallback for t > 1 (shouldn't happen)
    color[0] = 255;
    color[1] = 0;
    color[2] = 0;
    return color;
  }

  pushSamples(samples: number[]): void {
    for (const sample of samples) {
      this.sampleBuffer[this.bufferWriteIndex] = sample;
      this.bufferWriteIndex++;
      
      // When buffer is full, process FFT
      if (this.bufferWriteIndex >= this.fft.fftSize) {
        this.pendingMagnitudes = this.fft.process(this.sampleBuffer);
        this.needsNewColumn = true;
        this.bufferWriteIndex = 0;
      }
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
    this.sampleBuffer.fill(0);
    this.bufferWriteIndex = 0;
    this.needsNewColumn = false;
    this.pendingMagnitudes = null;
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
    
    const width = this.canvas.getBoundingClientRect().width;
    const height = this.canvas.getBoundingClientRect().height;
    
    // Process pending FFT data
    if (this.needsNewColumn && this.pendingMagnitudes) {
      this.scrollLeft();
      this.drawColumn(this.pendingMagnitudes);
      this.needsNewColumn = false;
      this.pendingMagnitudes = null;
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

  // Convert normalized position (0-1) to fractional frequency bin using log scale
  // Returns a float for interpolation between bins
  private positionToFreqBinFloat(pos: number, numBins: number): number {
    // Map position to log-scaled frequency
    // pos=0 -> minFreq (bottom), pos=1 -> maxFreq (top)
    const minFreq = 20; // 20 Hz minimum (human hearing)
    const maxFreq = 24000; // 24 kHz (Nyquist at 48kHz)
    const minLog = Math.log10(minFreq);
    const maxLog = Math.log10(maxFreq);
    
    // Log interpolation
    const logFreq = minLog + pos * (maxLog - minLog);
    const freq = Math.pow(10, logFreq);
    
    // Convert frequency to bin index (keep as float for interpolation)
    // bin = freq / (sampleRate / numBins / 2) = freq * numBins * 2 / sampleRate
    const binIndex = freq * numBins * 2 / 48000;
    return Math.min(numBins - 1, Math.max(0, binIndex));
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

  // Get magnitude for a pixel row, averaging bins that fall within this pixel's frequency range
  private getMagnitudeForPixel(magnitudes: Float32Array, y: number, height: number): number {
    const numBins = magnitudes.length;
    
    // Get frequency range for this pixel and the next
    const pos1 = (height - 1 - y) / height;
    const pos2 = (height - y) / height;
    
    const bin1 = this.positionToFreqBinFloat(pos1, numBins);
    const bin2 = this.positionToFreqBinFloat(pos2, numBins);
    
    const binLow = Math.max(0, Math.min(bin1, bin2));
    const binHigh = Math.min(numBins - 1, Math.max(bin1, bin2));
    
    // If range spans less than one bin, interpolate
    if (binHigh - binLow < 1) {
      const binFloor = Math.floor(binLow);
      const binCeil = Math.min(numBins - 1, binFloor + 1);
      const frac = binLow - binFloor;
      return (magnitudes[binFloor] || 0) * (1 - frac) + (magnitudes[binCeil] || 0) * frac;
    }
    
    // Otherwise, average all bins in range (weighted by overlap)
    let sum = 0;
    let weight = 0;
    
    const startBin = Math.floor(binLow);
    const endBin = Math.ceil(binHigh);
    
    for (let b = startBin; b <= endBin && b < numBins; b++) {
      // Calculate how much of this bin falls within our range
      const binStart = b;
      const binEnd = b + 1;
      const overlapStart = Math.max(binLow, binStart);
      const overlapEnd = Math.min(binHigh, binEnd);
      const overlapWeight = Math.max(0, overlapEnd - overlapStart);
      
      if (overlapWeight > 0) {
        sum += (magnitudes[b] || 0) * overlapWeight;
        weight += overlapWeight;
      }
    }
    
    return weight > 0 ? sum / weight : 0;
  }

  private drawColumn(magnitudes: Float32Array): void {
    if (!this.imageData) return;
    const data = this.imageData.data;
    const width = this.imageData.width;
    const height = this.imageData.height;
    const numBins = magnitudes.length;
    
    // Find max magnitude for normalization
    let maxMag = 0.001;
    for (let i = 0; i < numBins; i++) {
      if (magnitudes[i] > maxMag) maxMag = magnitudes[i];
    }
    const refLevel = Math.max(maxMag, 0.05);
    
    // Draw column at rightmost position
    const x = width - 1;
    
    for (let y = 0; y < height; y++) {
      // Get magnitude for this pixel (handles both interpolation and averaging)
      const magnitude = this.getMagnitudeForPixel(magnitudes, y, height);
      
      // Normalize to 0-1 range with log scale for magnitude
      const normalizedDb = Math.log10(1 + magnitude / refLevel * 9) / Math.log10(10);
      const normalized = Math.min(1, Math.max(0, normalizedDb));
      
      // Look up color
      const colorIdx = Math.floor(normalized * 255);
      const color = this.colorLookup[colorIdx];
      
      // Set pixel
      const idx = (y * width + x) * 4;
      data[idx] = color[0];
      data[idx + 1] = color[1];
      data[idx + 2] = color[2];
      data[idx + 3] = color[3];
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
    const width = this.canvas.getBoundingClientRect().width;
    const height = this.canvas.getBoundingClientRect().height;
    
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

let deviceSelect: HTMLSelectElement | null;
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

let isRecording = false;
let isMonitoring = false;
let isProcessingEnabled = false;
let wasMonitoringBeforeRecording = false;
let waveformRenderer: WaveformRenderer | null = null;
let spectrogramRenderer: SpectrogramRenderer | null = null;
let audioSamplesUnlisten: UnlistenFn | null = null;
let transcriptionCompleteUnlisten: UnlistenFn | null = null;
let transcriptionErrorUnlisten: UnlistenFn | null = null;
let speechStartedUnlisten: UnlistenFn | null = null;
let speechEndedUnlisten: UnlistenFn | null = null;

async function loadDevices() {
  try {
    const devices = await invoke<AudioDevice[]>("list_audio_devices");

    if (deviceSelect) {
      deviceSelect.innerHTML = "";

      if (devices.length === 0) {
        deviceSelect.innerHTML =
          '<option value="">No audio devices found</option>';
        return;
      }

      devices.forEach((device) => {
        const option = document.createElement("option");
        option.value = device.id;
        option.textContent = device.name;
        deviceSelect?.appendChild(option);
      });

      if (recordBtn) {
        recordBtn.disabled = false;
      }
      if (monitorToggle) {
        monitorToggle.disabled = false;
      }
      if (processingToggle) {
        processingToggle.disabled = false;
      }
    }
  } catch (error) {
    console.error("Failed to load devices:", error);
    if (deviceSelect) {
      deviceSelect.innerHTML = `<option value="">Error loading devices</option>`;
    }
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

function setStatus(message: string, type: "normal" | "loading" | "error" = "normal") {
  if (statusEl) {
    statusEl.textContent = message;
    statusEl.className = "status";
    if (type !== "normal") {
      statusEl.classList.add(type);
    }
  }
}

async function setupAudioListener() {
  if (audioSamplesUnlisten) return;

  audioSamplesUnlisten = await listen<AudioSamplesPayload>("audio-samples", (event) => {
    if (waveformRenderer) {
      waveformRenderer.pushSamples(event.payload.samples);
    }
    if (spectrogramRenderer) {
      spectrogramRenderer.pushSamples(event.payload.samples);
    }
  });
}

async function cleanupAudioListener() {
  if (audioSamplesUnlisten) {
    audioSamplesUnlisten();
    audioSamplesUnlisten = null;
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
  if (!deviceSelect || !monitorToggle) return;

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
      await cleanupAudioListener();
    } catch (error) {
      console.error("Stop monitor error:", error);
      setStatus(`Error: ${error}`, "error");
      monitorToggle.checked = true; // Revert toggle on error
    }
  } else {
    // Start monitoring
    const deviceId = deviceSelect.value;
    if (!deviceId) {
      setStatus("Please select an audio device", "error");
      monitorToggle.checked = false; // Revert toggle
      return;
    }

    try {
      await setupAudioListener();
      await invoke("start_monitor", { deviceId });
      isMonitoring = true;
      monitorToggle.checked = true;
      setStatus("Monitoring...", "loading");
      
      waveformRenderer?.clear();
      waveformRenderer?.start();
      spectrogramRenderer?.clear();
      spectrogramRenderer?.start();
    } catch (error) {
      console.error("Start monitor error:", error);
      setStatus(`Error: ${error}`, "error");
      monitorToggle.checked = false; // Revert toggle on error
      await cleanupAudioListener();
    }
  }
}

async function toggleRecording() {
  if (!deviceSelect || !recordBtn) return;

  if (isRecording) {
    // Stop recording - this returns immediately, transcription happens in background
    try {
      // Pass whether to keep monitoring
      await invoke("stop_recording", { 
        keepMonitoring: wasMonitoringBeforeRecording 
      });
      
      isRecording = false;
      recordBtn.textContent = "Record";
      recordBtn.classList.remove("recording");
      
      // Re-enable monitor button
      if (monitorToggle) {
        monitorToggle.disabled = false;
      }

      // If monitoring was active before, keep it running
      if (wasMonitoringBeforeRecording) {
        // Monitoring continues, update status
        setStatus("Transcribing... (monitoring continues)", "loading");
        // isMonitoring stays true, waveform and spectrogram keep running
      } else {
        // Stop visualization since we weren't monitoring before
        isMonitoring = false;
        waveformRenderer?.stop();
        waveformRenderer?.clear();
        spectrogramRenderer?.stop();
        spectrogramRenderer?.clear();
        await cleanupAudioListener();
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
      // On error, stop everything
      waveformRenderer?.stop();
      waveformRenderer?.clear();
      spectrogramRenderer?.stop();
      spectrogramRenderer?.clear();
      await cleanupAudioListener();
      isMonitoring = false;
      wasMonitoringBeforeRecording = false;
      if (monitorToggle) {
        monitorToggle.checked = false;
      }
    }
  } else {
    // Start recording
    const deviceId = deviceSelect.value;
    if (!deviceId) {
      setStatus("Please select an audio device", "error");
      return;
    }

    // Remember if monitoring was active before recording
    wasMonitoringBeforeRecording = isMonitoring;

    try {
      // Setup listeners if not already
      await setupAudioListener();
      await setupTranscriptionListeners();
      
      await invoke("start_recording", { deviceId });
      isRecording = true;
      isMonitoring = true; // Recording enables monitoring for visualization
      recordBtn.textContent = "Stop";
      recordBtn.classList.add("recording");
      setStatus("Recording...", "loading");
      
      // Disable monitor button during recording (can't toggle it)
      if (monitorToggle) {
        monitorToggle.disabled = true;
      }

      // Start waveform and spectrogram if not already running
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
      // Don't clean up listener if monitoring was already active
      if (!isMonitoring) {
        await cleanupAudioListener();
      }
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  deviceSelect = document.querySelector("#device-select");
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
  closeBtn?.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    const window = getCurrentWindow();
    await window.destroy();
  });

  loadDevices();
  checkModelStatus();
});
