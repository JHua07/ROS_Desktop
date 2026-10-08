import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { RosTopic } from "../modules/ros";
import {
  describeProcessError, listenRosProcessEvents, startBag, stopRosProcess,
  type ProcessEvent, type ProcessState,
} from "../modules/process";
import { EmptyIcon, EmptyState, LogScroll, StatusPill } from "./panel-ui";

interface BagPanelProps { topics: RosTopic[]; available: boolean }
type LogLine = { id: number; stream: "stdout" | "stderr" | "status"; text: string };
const MAX_LINES = 500;

export function BagPanel({ topics, available }: BagPanelProps) {
  const nativeAvailable = available && isTauri();
  const topicNames = [...new Set(topics.map((topic) => topic.name))];
  const [selected, setSelected] = useState<string[]>([]);
  const [output, setOutput] = useState("");
  const [state, setState] = useState<ProcessState | "idle">("idle");
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [lines, setLines] = useState<LogLine[]>([]);
  const [topicFilter, setTopicFilter] = useState("");
  const [sessionId, setSessionId] = useState<number | null>(null);
  const sessionIdRef = useRef<number | null>(null);
  const starting = useRef(false);
  const earlyEvents = useRef<ProcessEvent[]>([]);
  const nextLineId = useRef(0);
  const stopRequested = useRef(false);

  useEffect(() => {
    setSelected((current) => current.filter((name) => topicNames.includes(name)));
  }, [topics]); // A refreshed graph can remove previously selected topics.

  const addLine = useCallback((stream: LogLine["stream"], text: string) => {
    const line = { id: ++nextLineId.current, stream, text };
    setLines((previous) => [...previous.slice(-(MAX_LINES - 1)), line]);
  }, []);

  const applyEvent = useCallback((event: ProcessEvent) => {
    if (event.line !== undefined) addLine(event.stream ?? "status", event.line);
    if (event.stream) return; // Streaming output can arrive while a stop is in progress.
    setState(event.state);
    if (event.state === "failed") {
      const detail = event.error ?? `Bag recording failed${event.exit_code === undefined ? "" : ` (exit code ${event.exit_code})`}.`;
      setMessage(detail);
      addLine("status", detail);
    } else if (event.state === "stopped") {
      const detail = `${stopRequested.current ? "Recording stopped" : "Recording exited"}${event.exit_code === undefined ? "" : ` (exit code ${event.exit_code})`}.`;
      setMessage(detail);
      addLine("status", detail);
    }
  }, [addLine]);

  const onEvent = useCallback((event: ProcessEvent) => {
    if (event.kind !== "bag") return;
    if (starting.current && sessionIdRef.current === null) {
      earlyEvents.current = [...earlyEvents.current.slice(-(MAX_LINES - 1)), event];
    } else if (event.session_id === sessionIdRef.current) {
      applyEvent(event);
    }
  }, [applyEvent]);

  useEffect(() => {
    if (!nativeAvailable) { setReady(false); return; }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenRosProcessEvents(onEvent).then((cleanup) => {
      if (disposed) cleanup();
      else { unlisten = cleanup; setReady(true); }
    }).catch((error) => { if (!disposed) setMessage(describeProcessError(error)); });
    return () => { disposed = true; unlisten?.(); setReady(false); };
  }, [nativeAvailable, onEvent]);

  const active = state === "starting" || state === "running" || state === "stopping";
  const chosen = selected.filter((name) => topicNames.includes(name));
  const showTopicFilter = topicNames.length > 20;
  const visibleTopics = showTopicFilter
    ? topicNames.filter((name) => name.toLowerCase().includes(topicFilter.trim().toLowerCase()))
    : topicNames;

  function toggleTopic(name: string) {
    setSelected((current) => current.includes(name) ? current.filter((item) => item !== name) : [...current, name]);
  }

  async function start() {
    if (!nativeAvailable || !ready || busy || active || chosen.length === 0 || !output.trim()) return;
    setBusy(true);
    starting.current = true;
    sessionIdRef.current = null;
    setSessionId(null);
    earlyEvents.current = [];
    stopRequested.current = false;
    setMessage("");
    setLines([]);
    setState("starting");
    try {
      const session = await startBag(chosen, output.trim());
      sessionIdRef.current = session.session_id;
      setSessionId(session.session_id);
      setState(session.state);
      for (const event of earlyEvents.current) {
        if (event.session_id === session.session_id) applyEvent(event);
      }
    } catch (error) {
      setState("failed");
      setMessage(describeProcessError(error));
    } finally {
      earlyEvents.current = [];
      starting.current = false;
      setBusy(false);
    }
  }

  async function stop() {
    if (sessionIdRef.current === null || busy || (state !== "starting" && state !== "running")) return;
    setBusy(true);
    stopRequested.current = true;
    setState("stopping");
    try {
      await stopRosProcess("bag");
    } catch (error) {
      const detail = describeProcessError(error);
      setMessage(detail);
      addLine("status", detail);
      stopRequested.current = false;
      setState("running"); // Stop did not complete; permit another attempt.
    } finally {
      setBusy(false);
    }
  }

  const startDisabledReason = !nativeAvailable
    ? "Requires the desktop app"
    : active
      ? "Recording in progress"
      : busy
        ? "Working"
        : chosen.length === 0
          ? "Select at least one topic"
          : !output.trim()
            ? "Set an output directory"
            : undefined;
  const stopDisabledReason = sessionId === null ? "No recording session" : state !== "starting" && state !== "running" ? "Recording is not running" : undefined;
  const startHint = chosen.length === 0 ? "Select at least one topic to record." : !output.trim() ? "Set an output directory to enable Start." : "";

  return <section className="workflow-card bag-panel" aria-labelledby="bag-title">
    <h3 id="bag-title">Recorder</h3>
    <p>Select graph topics and a new output directory. The parent must already exist; existing recordings are never overwritten.</p>
    <fieldset disabled={!nativeAvailable || active || busy || topicNames.length === 0}>
      <legend>Topics to record</legend>
      {topicNames.length === 0 ? (
        <EmptyState
          compact
          icon={<EmptyIcon name="record" />}
          title="No topics available"
          hint="Refresh the graph in Overview."
        />
      ) : (
        <>
          {showTopicFilter && (
            <input
              className="list-filter"
              type="text"
              value={topicFilter}
              onChange={(event) => setTopicFilter(event.target.value)}
              placeholder={`Filter ${topicNames.length} topics…`}
              aria-label="Filter topics"
            />
          )}
          <div className={`bag-topics-scroll${showTopicFilter ? " list-scroll-tall" : ""}`}>
            {showTopicFilter && (
              <div className="list-heading" aria-hidden="true">
                <span>Topic</span>
                <span>{chosen.length} selected</span>
              </div>
            )}
            {visibleTopics.map((name) => <label key={name} style={{ display: "block" }}>
              <input type="checkbox" checked={chosen.includes(name)} onChange={() => toggleTopic(name)} /> {name}
            </label>)}
            {visibleTopics.length === 0 && (
              <EmptyState
                compact
                icon={<EmptyIcon name="list" />}
                title="No topics match the filter"
                hint={`No results for “${topicFilter.trim()}”.`}
              />
            )}
          </div>
        </>
      )}
    </fieldset>
    <label htmlFor="bag-output">Output directory</label>
    <input id="bag-output" type="text" value={output} onChange={(event) => setOutput(event.target.value)}
      placeholder="bags/session-001 or an absolute path" disabled={!nativeAvailable || active || busy}
      aria-describedby="bag-output-help" />
    <p id="bag-output-help">Relative paths resolve from the Pixi workspace. Choose a new directory under an existing parent.</p>
    <div className="panel-actions">
      <button type="button" className="btn-primary" onClick={() => void start()}
        disabled={!nativeAvailable || !ready || active || busy || chosen.length === 0 || !output.trim()}
        title={startDisabledReason}>Start recording</button>
      <button type="button" className="btn-danger" onClick={() => void stop()}
        disabled={sessionId === null || busy || (state !== "starting" && state !== "running")}
        title={stopDisabledReason}>Stop</button>
    </div>
    {nativeAvailable && !active && startHint && <p className="action-hint" role="status">{startHint}</p>}
    {!nativeAvailable && <p className="action-hint" role="status">Native ROS environment unavailable. Bag recording requires the desktop backend.</p>}
    <div className="status-row" role="status">
      <StatusPill status={state} />
      {message && <span className={`status-message${state === "failed" ? " status-message-error" : ""}`}>{message}</span>}
    </div>
    <LogScroll
      label="Bag recording output"
      lineCount={lines.length}
      empty={
        <EmptyState
          compact
          icon={<EmptyIcon name="terminal" />}
          title="No recording output"
          hint="ros2 bag output appears during recording."
        />
      }
    >
      {lines.map((line) => <span key={line.id} data-stream={line.stream}>{line.stream === "stdout" ? "" : `[${line.stream}] `}{line.text}{"\n"}</span>)}
    </LogScroll>
  </section>;
}

export default BagPanel;
