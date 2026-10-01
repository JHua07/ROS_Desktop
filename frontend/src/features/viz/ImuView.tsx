import { memo, useEffect, useRef, useState } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import { decodeBase64, type VizFrameEvent, type VizFrameHub } from "../../modules/viz";

interface ImuViewProps {
  topic: string;
  hub: VizFrameHub;
}

interface ImuLatest {
  t: number;
  accel: [number, number, number];
  gyro: [number, number, number];
}

const WINDOW_SECONDS = 10;
const MAX_SAMPLES = 4000; // ~20 s at 200 Hz; the chart window shows the latest 10 s.
const PAYLOAD_BYTES = 64; // f64 stamp + f64[3] accel + f64[3] gyro, little-endian.
const REDRAW_INTERVAL_MS = 100;
const PLOT_HEIGHT = 220;

const SERIES: { label: string; color: string }[] = [
  { label: "ax (m/s²)", color: "#ff6b6b" },
  { label: "ay (m/s²)", color: "#71e1c1" },
  { label: "az (m/s²)", color: "#6ba8ff" },
  { label: "gx (rad/s)", color: "#c792ea" },
  { label: "gy (rad/s)", color: "#f0bf74" },
  { label: "gz (rad/s)", color: "#8be9a8" },
];

export const ImuView = memo(function ImuView({ topic, hub }: ImuViewProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const plotRef = useRef<uPlot | null>(null);
  const timesRef = useRef<number[]>([]);
  const valuesRef = useRef<number[][]>([[], [], [], [], [], []]);
  const firstStampRef = useRef<number | null>(null);
  const lastDrawRef = useRef(0);
  const [latest, setLatest] = useState<ImuLatest | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const plot = new uPlot(
      {
        width: host.clientWidth || 320,
        height: PLOT_HEIGHT,
        legend: { show: true },
        // Live streams update the window continuously; drag-zoom selections would fight it.
        cursor: { drag: { x: false, y: false } },
        series: [
          { label: "t (s)" },
          ...SERIES.map((series) => ({
            label: series.label,
            stroke: series.color,
            width: 1.2,
            points: { show: false },
          })),
        ],
      },
      [[], [], [], [], [], [], []],
      host,
    );
    plotRef.current = plot;
    const observer = new ResizeObserver(() => {
      plot.setSize({ width: host.clientWidth || 320, height: PLOT_HEIGHT });
    });
    observer.observe(host);
    return () => {
      observer.disconnect();
      plot.destroy();
      plotRef.current = null;
      timesRef.current = [];
      valuesRef.current = [[], [], [], [], [], []];
      firstStampRef.current = null;
      lastDrawRef.current = 0;
    };
  }, []);

  useEffect(() => {
    const unsubscribe = hub.subscribe(topic, (frame) => {
      const bytes = decodeBase64(frame.data_b64);
      if (bytes.byteLength < PAYLOAD_BYTES) return;
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      const stamp = view.getFloat64(0, true);
      const accel: [number, number, number] = [view.getFloat64(8, true), view.getFloat64(16, true), view.getFloat64(24, true)];
      const gyro: [number, number, number] = [view.getFloat64(32, true), view.getFloat64(40, true), view.getFloat64(48, true)];
      if (firstStampRef.current === null) firstStampRef.current = stamp;
      const t = stamp - firstStampRef.current;
      const times = timesRef.current;
      times.push(t);
      const values = valuesRef.current;
      values[0].push(accel[0]);
      values[1].push(accel[1]);
      values[2].push(accel[2]);
      values[3].push(gyro[0]);
      values[4].push(gyro[1]);
      values[5].push(gyro[2]);
      if (times.length > MAX_SAMPLES) {
        const drop = times.length - MAX_SAMPLES;
        times.splice(0, drop);
        for (const series of values) series.splice(0, drop);
      }
      const now = performance.now();
      if (now - lastDrawRef.current < REDRAW_INTERVAL_MS) return;
      lastDrawRef.current = now;
      const latestTime = times[times.length - 1];
      let from = 0;
      while (from < times.length && times[from] < latestTime - WINDOW_SECONDS) from += 1;
      const plot = plotRef.current;
      // uPlot runs on plain arrays at runtime; its AlignedData type only lists TypedArrays.
      const data = [times.slice(from), ...values.map((series) => series.slice(from))] as unknown as uPlot.AlignedData;
      plot?.setData(data);
      setLatest({ t, accel, gyro });
    });
    return unsubscribe;
  }, [topic, hub]);

  return (
    <figure className="viz-card">
      <figcaption>
        {topic}
        {latest &&
          ` — a [${latest.accel.map((value) => value.toFixed(2)).join(", ")}] ω [${latest.gyro.map((value) => value.toFixed(3)).join(", ")}]`}
      </figcaption>
      <div className="viz-imu-plot" ref={hostRef} />
      <p className="viz-note">Rolling {WINDOW_SECONDS}s window. Click legend entries to toggle series.</p>
    </figure>
  );
});

export default ImuView;
