import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RosTopic } from "../modules/ros";
import {
  createVizFrameHub,
  describeVizError,
  listenVizFrames,
  listenVizStatus,
  startViz,
  stopViz,
  type VizConfig,
  type VizFrameEvent,
  type VizFrameHub,
  type VizKind,
  type VizStatusEvent,
  type VizTopicConfig,
} from "../modules/viz";
import { ImuView } from "./viz/ImuView";
import { ImageView } from "./viz/ImageView";
import { PointCloudView } from "./viz/PointCloudView";
import { EmptyIcon, EmptyState, StatusPill } from "./panel-ui";
import "./VizPanel.css";

const KIND_BY_TYPE: Record<string, VizKind> = {
  "vlpos/msg/H2oPointCloud": "pointcloud",
  "sensor_msgs/msg/Image": "image",
  "sensor_msgs/msg/Imu": "imu",
};

const KIND_GROUPS: { kind: VizKind; title: string }[] = [
  { kind: "pointcloud", title: "Point clouds" },
  { kind: "image", title: "Camera images" },
  { kind: "imu", title: "IMU" },
];

interface VizPanelProps {
  topics: RosTopic[];
  available: boolean;
}

interface VizCandidate {
  name: string;
  type: string;
  kind: VizKind;
}

type VizState = "idle" | "starting" | "running" | "stopping" | "stopped" | "failed";
interface VizStats {
  frames: number;
  fps: number;
}

const MAX_STATUS_LINES = 50;
const STATS_INTERVAL_MS = 500;

export function VizPanel({ topics, available }: VizPanelProps) {
  const nativeAvailable = available && isTauri();
  const candidates = useMemo<VizCandidate[]>(() => {
    const result: VizCandidate[] = [];
    for (const topic of topics) {
      const type = topic.types.find((candidate) => candidate in KIND_BY_TYPE);
      if (type) result.push({ name: topic.name, type, kind: KIND_BY_TYPE[type] });
    }
    return result;
  }, [topics]);

  const [selected, setSelected] = useState<string[]>([]);
  const [maxWidth, setMaxWidth] = useState(960);
  const [downsample, setDownsample] = useState(1);
  const [state, setState] = useState<VizState>("idle");
  const [message, setMessage] = useState("");
  const [statusLines, setStatusLines] = useState<string[]>([]);
  const [stats, setStats] = useState<Record<string, VizStats>>({});

  const sessionRef = useRef<number | null>(null);
  const startingRef = useRef(false);
  const mountedRef = useRef(false);
  const pendingStatusRef = useRef<VizStatusEvent[]>([]);
  const pendingFramesRef = useRef<Map<string, VizFrameEvent>>(new Map());
  const stopRequestedRef = useRef(false);
  const countersRef = useRef(new Map<string, { frames: number; seq: number | null; wall: number; fps: number }>());
  const hubRef = useRef<VizFrameHub | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    // A refreshed graph can remove previously selected topics.
    setSelected((current) => current.filter((name) => candidates.some((candidate) => candidate.name === name)));
  }, [candidates]);

  const applyStatus = useCallback((event: VizStatusEvent) => {
    setState(event.state);
    if (event.line !== undefined) {
      const line = event.line;
      setStatusLines((previous) => [...previous, line].slice(-MAX_STATUS_LINES));
    }
    if (event.state === "failed") {
      setMessage(event.line ?? "Visualization failed.");
      setStats({});
    } else if (event.state === "stopped") {
      setMessage(stopRequestedRef.current ? "Visualization stopped." : "Visualization exited on its own.");
      setStats({});
    }
  }, []);

  const applyFrame = useCallback((frame: VizFrameEvent) => {
    hubRef.current?.publish(frame);
    let counter = countersRef.current.get(frame.topic);
    if (!counter) {
      counter = { frames: 0, seq: null, wall: 0, fps: 0 };
      countersRef.current.set(frame.topic, counter);
    }
    counter.frames += 1;
    counter.seq = frame.seq;
  }, []);

  const onFrame = useCallback(
    (frame: VizFrameEvent) => {
      if (frame.session_id === sessionRef.current) {
        applyFrame(frame);
      } else if (startingRef.current && sessionRef.current === null) {
        // The bridge may publish before the start command resolves.
        pendingFramesRef.current.set(frame.topic, frame);
      }
    },
    [applyFrame],
  );

  const onStatus = useCallback(
    (event: VizStatusEvent) => {
      if (event.session_id === sessionRef.current) {
        applyStatus(event);
      } else if (startingRef.current && sessionRef.current === null) {
        pendingStatusRef.current = [...pendingStatusRef.current.slice(-(MAX_STATUS_LINES - 1)), event];
      }
    },
    [applyStatus],
  );

  useEffect(() => {
    if (!nativeAvailable) return;
    const cleanups = [
      listenVizFrames(onFrame).then((unlisten) => () => unlisten()),
      listenVizStatus(onStatus).then((unlisten) => () => unlisten()),
    ];
    return () => {
      for (const cleanup of cleanups) void cleanup.then((fn) => fn(), () => undefined);
    };
  }, [nativeAvailable, onFrame, onStatus]);

  useEffect(() => {
    if (state !== "running") return;
    const previous = new Map<string, { seq: number | null; wall: number }>();
    const timer = window.setInterval(() => {
      const now = performance.now();
      const snapshot: Record<string, VizStats> = {};
      for (const [topic, counter] of countersRef.current) {
        const last = previous.get(topic);
        if (last && counter.seq !== null && last.seq !== null && now > last.wall) {
          counter.fps = ((counter.seq - last.seq) * 1000) / (now - last.wall);
        }
        previous.set(topic, { seq: counter.seq, wall: now });
        snapshot[topic] = { frames: counter.frames, fps: counter.fps };
      }
      if (mountedRef.current) setStats(snapshot);
    }, STATS_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [state]);

  const active = state === "starting" || state === "running" || state === "stopping";
  const chosen = selected.filter((name) => candidates.some((candidate) => candidate.name === name));
  const candidateByName = useCallback(
    (name: string) => candidates.find((candidate) => candidate.name === name),
    [candidates],
  );
  const chosenByKind = (kind: VizKind) =>
    chosen.flatMap((name) => {
      const candidate = candidateByName(name);
      return candidate && candidate.kind === kind ? [candidate] : [];
    });

  function toggleTopic(name: string) {
    setSelected((current) =>
      current.includes(name) ? current.filter((item) => item !== name) : [...current, name],
    );
  }

  async function start() {
    if (!nativeAvailable || startingRef.current || active || chosen.length === 0) return;
    startingRef.current = true;
    sessionRef.current = null;
    pendingStatusRef.current = [];
    pendingFramesRef.current.clear();
    stopRequestedRef.current = false;
    countersRef.current.clear();
    hubRef.current = createVizFrameHub();
    setStats({});
    setStatusLines([]);
    setMessage("");
    setState("starting");
    try {
      const config: VizConfig = {
        topics: chosen.map((name): VizTopicConfig => {
          const candidate = candidateByName(name);
          if (!candidate) throw new Error(`Topic ${name} is no longer available.`);
          const entry: VizTopicConfig = { topic: name, type: candidate.type, kind: candidate.kind };
          if (candidate.kind === "image") {
            entry.options = {
              max_width: maxWidth > 0 ? maxWidth : 960,
              downsample: downsample > 0 ? downsample : 1,
            };
          }
          return entry;
        }),
      };
      const session = await startViz(config);
      sessionRef.current = session.session_id;
      if (!mountedRef.current) return;
      setState(session.state);
      for (const event of pendingStatusRef.current) {
        if (event.session_id === session.session_id) applyStatus(event);
      }
      for (const frame of pendingFramesRef.current.values()) {
        if (frame.session_id === session.session_id) applyFrame(frame);
      }
    } catch (error) {
      if (mountedRef.current) {
        setState("failed");
        setMessage(describeVizError(error));
      }
    } finally {
      pendingStatusRef.current = [];
      pendingFramesRef.current.clear();
      startingRef.current = false;
    }
  }

  async function stop() {
    if (sessionRef.current === null || state !== "running" && state !== "starting") return;
    stopRequestedRef.current = true;
    setState("stopping");
    try {
      await stopViz();
    } catch (error) {
      if (!mountedRef.current) return;
      setMessage(describeVizError(error));
      stopRequestedRef.current = false;
      setState("running"); // Stop did not complete; permit another attempt.
    }
  }

  const hub = hubRef.current;
  const cloudEntries = chosenByKind("pointcloud").map((candidate, index) => ({
    topic: candidate.name,
    hue: (index * 0.25) % 1,
  }));
  const imageTopics = chosenByKind("image").map((candidate) => candidate.name);
  const imuTopics = chosenByKind("imu").map((candidate) => candidate.name);
  const hasViews = cloudEntries.length > 0 || imageTopics.length > 0 || imuTopics.length > 0;
  const statEntries = Object.entries(stats);

  const startDisabledReason = !nativeAvailable
    ? "Requires the desktop app"
    : active
      ? "Visualization in progress"
      : chosen.length === 0
        ? "Select at least one topic"
        : undefined;
  const stopDisabledReason = state !== "running" && state !== "starting" ? "Visualization is not running" : undefined;

  return (
    <section className="workflow-card viz-panel" aria-labelledby="viz-title">
      <h3 id="viz-title">Live views</h3>
      <p>
        Select point cloud, camera, and IMU topics from the current ROS graph and render live frames in the
        WebView.
      </p>
      {!nativeAvailable && (
        <p className="action-hint" role="status">Native ROS environment unavailable. Live visualization requires the desktop backend.</p>
      )}
      <div className="viz-topic-groups">
        {KIND_GROUPS.map((group) => {
          const groupCandidates = candidates.filter((candidate) => candidate.kind === group.kind);
          return (
            <fieldset key={group.kind} disabled={!nativeAvailable || active}>
              <legend>{group.title}</legend>
              {groupCandidates.length === 0 && (
                <EmptyState
                  compact
                  icon={<EmptyIcon name="views" />}
                  title="No matching topics"
                  hint="Topics of this type appear here after a graph refresh."
                />
              )}
              {groupCandidates.map((candidate) => (
                <label key={candidate.name}>
                  <input
                    type="checkbox"
                    checked={chosen.includes(candidate.name)}
                    onChange={() => toggleTopic(candidate.name)}
                  />{" "}
                  {candidate.name}
                </label>
              ))}
            </fieldset>
          );
        })}
      </div>
      {imageTopics.length > 0 && !active && (
        <div className="viz-image-options">
          <div className="viz-image-option">
            <label htmlFor="viz-max-width">Image max width</label>
            <input
              id="viz-max-width"
              type="number"
              min={1}
              value={maxWidth}
              onChange={(event) => setMaxWidth(Number(event.target.value) || 0)}
              disabled={!nativeAvailable}
            />
          </div>
          <div className="viz-image-option">
            <label htmlFor="viz-downsample">Image downsample</label>
            <input
              id="viz-downsample"
              type="number"
              min={1}
              value={downsample}
              onChange={(event) => setDownsample(Number(event.target.value) || 0)}
              disabled={!nativeAvailable}
            />
          </div>
          <p className="viz-note">Applied to all selected camera topics.</p>
        </div>
      )}
      <div className="panel-actions">
        <button
          type="button"
          className="btn-primary"
          onClick={() => void start()}
          disabled={!nativeAvailable || active || chosen.length === 0}
          title={startDisabledReason}
        >
          Start visualization
        </button>
        <button
          type="button"
          className="btn-danger"
          onClick={() => void stop()}
          disabled={state !== "running" && state !== "starting"}
          title={stopDisabledReason}
        >
          Stop
        </button>
      </div>
      {nativeAvailable && !active && chosen.length === 0 && (
        <p className="action-hint" role="status">Select at least one topic to enable Start.</p>
      )}
      <div className="status-row" role="status">
        <StatusPill status={state} />
        {message && <span className={`status-message${state === "failed" ? " status-message-error" : ""}`}>{message}</span>}
      </div>
      {statusLines.length > 0 && (
        <pre className="viz-status-log" aria-label="Visualization events">
          {statusLines.map((line, index) => (
            <span key={index}>
              {line}
              {"\n"}
            </span>
          ))}
        </pre>
      )}
      {statEntries.length > 0 && (
        <p className="viz-stats">
          {statEntries
            .map(([topic, entry]) => `${topic}: ${entry.frames} frames · ${entry.fps.toFixed(1)} fps`)
            .join(" · ")}
        </p>
      )}
      {active && hub && hasViews && (
        <div className="viz-views">
          {cloudEntries.length > 0 && (
            <div>
              <p className="viz-section-title">Point clouds — drag to orbit, scroll to zoom</p>
              <PointCloudView entries={cloudEntries} hub={hub} />
            </div>
          )}
          {imageTopics.length > 0 && (
            <div>
              <p className="viz-section-title">Camera images</p>
              <div className="viz-grid">
                {imageTopics.map((topic) => (
                  <ImageView key={topic} topic={topic} hub={hub} />
                ))}
              </div>
            </div>
          )}
          {imuTopics.length > 0 && (
            <div>
              <p className="viz-section-title">IMU</p>
              <div className="viz-grid">
                {imuTopics.map((topic) => (
                  <ImuView key={topic} topic={topic} hub={hub} />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
      {!active && (
        <div className="viz-views-empty">
          <EmptyState
            centered
            icon={<EmptyIcon name="views" />}
            title="No live views"
            hint={chosen.length > 0 ? "Start visualization to render the selected topics." : "Select topics above, then start visualization."}
          />
        </div>
      )}
    </section>
  );
}

export default VizPanel;
