import { memo, useCallback, useEffect, useRef, useState } from "react";
import { decodeBase64, type VizFrameEvent, type VizFrameHub } from "../../modules/viz";

interface ImageViewProps {
  topic: string;
  hub: VizFrameHub;
}

interface ImageStats {
  width: number;
  height: number;
  fps: number;
  note: string;
}

const STATS_INTERVAL_MS = 250;

export const ImageView = memo(function ImageView({ topic, hub }: ImageViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const offscreenRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef<VizFrameEvent | null>(null);
  const fpsRef = useRef({ ema: 0, lastTns: 0 });
  const statsDueRef = useRef(0);
  const [stats, setStats] = useState<ImageStats | null>(null);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const width = container.clientWidth || 1;
    const height = container.clientHeight || 1;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const offscreen = offscreenRef.current;
    if (!offscreen) return;
    // Contain-fit the frame inside the container, aspect preserved.
    const scale = Math.min(width / offscreen.width, height / offscreen.height);
    const drawWidth = offscreen.width * scale;
    const drawHeight = offscreen.height * scale;
    ctx.drawImage(offscreen, (width - drawWidth) / 2, (height - drawHeight) / 2, drawWidth, drawHeight);
  }, []);

  useEffect(() => {
    const unsubscribe = hub.subscribe(topic, (frame) => {
      const width = frame.meta.width ?? 0;
      const height = frame.meta.height ?? 0;
      const encoding = (frame.meta.encoding ?? "").toLowerCase();
      const bytesPerPixel = frame.meta.bytes_per_pixel ?? (encoding === "gray8" ? 1 : 3);
      const bytes = decodeBase64(frame.data_b64);
      frameRef.current = frame;
      if (width <= 0 || height <= 0 || encoding !== "rgb8" && encoding !== "gray8") {
        updateStats(width, height, `Unsupported encoding ${frame.meta.encoding ?? "(none)"}.`);
        return;
      }
      if (bytes.byteLength < width * height * bytesPerPixel) {
        updateStats(width, height, "Incomplete frame payload.");
        return;
      }
      const image = new ImageData(width, height);
      if (encoding === "rgb8") {
        for (let index = 0, offset = 0; index < width * height; index += 1, offset += 3) {
          const pixel = index * 4;
          image.data[pixel] = bytes[offset];
          image.data[pixel + 1] = bytes[offset + 1];
          image.data[pixel + 2] = bytes[offset + 2];
          image.data[pixel + 3] = 255;
        }
      } else {
        for (let index = 0; index < width * height; index += 1) {
          const pixel = index * 4;
          const value = bytes[index];
          image.data[pixel] = value;
          image.data[pixel + 1] = value;
          image.data[pixel + 2] = value;
          image.data[pixel + 3] = 255;
        }
      }
      let offscreen = offscreenRef.current;
      if (!offscreen || offscreen.width !== width || offscreen.height !== height) {
        offscreen = document.createElement("canvas");
        offscreen.width = width;
        offscreen.height = height;
        offscreenRef.current = offscreen;
      }
      offscreen.getContext("2d")?.putImageData(image, 0, 0);
      const tracking = fpsRef.current;
      if (tracking.lastTns > 0 && frame.t_ns > tracking.lastTns) {
        const rate = 1e9 / (frame.t_ns - tracking.lastTns);
        tracking.ema = tracking.ema === 0 ? rate : tracking.ema * 0.8 + rate * 0.2;
      }
      tracking.lastTns = frame.t_ns;
      updateStats(width, height, "");
      draw();
    });
    return unsubscribe;
  }, [topic, hub, draw]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => draw());
    observer.observe(container);
    return () => observer.disconnect();
  }, [draw]);

  function updateStats(width: number, height: number, note: string) {
    const now = performance.now();
    if (note === "" && now < statsDueRef.current) return; // Throttle caption refreshes; canvas still blits every frame.
    statsDueRef.current = now + STATS_INTERVAL_MS;
    setStats({ width, height, fps: fpsRef.current.ema, note });
  }

  return (
    <figure className="viz-card">
      <figcaption>
        {topic}
        {stats && stats.note === "" && ` — ${stats.width}×${stats.height}${stats.fps > 0 ? ` · ${stats.fps.toFixed(1)} fps` : ""}`}
      </figcaption>
      <div className="viz-canvas-wrap" ref={containerRef}>
        <canvas ref={canvasRef} className="viz-canvas" />
      </div>
      {stats?.note && <p className="viz-note">{stats.note}</p>}
    </figure>
  );
});

export default ImageView;
