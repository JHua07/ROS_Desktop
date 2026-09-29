import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { RosTopic } from "../modules/ros";
import {
  describeProcessError, listenRosProcessEvents, startBag, stopRosProcess,
  type ProcessEvent, type ProcessState,
} from "../modules/process";

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
  const sessionId = useRef<number | null>(null);
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
    if (starting.current && sessionId.current === null) {
      earlyEvents.current = [...earlyEvents.current.slice(-(MAX_LINES - 1)), event];
    } else if (event.session_id === sessionId.current) {
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

  function toggleTopic(name: string) {
    setSelected((current) => current.includes(name) ? current.filter((item) => item !== name) : [...current, name]);
  }

  async function start() {
    if (!nativeAvailable || !ready || busy || active || chosen.length === 0 || !output.trim()) return;
    setBusy(true);
    starting.current = true;
    sessionId.current = null;
    earlyEvents.current = [];
    stopRequested.current = false;
    setMessage("");
    setLines([]);
    setState("starting");
    try {
      const session = await startBag(chosen, output.trim());
      sessionId.current = session.session_id;
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
    if (sessionId.current === null || busy || (state !== "starting" && state !== "running")) return;
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

  return <section className="workflow-card bag-panel" aria-label="Bag recorder">
    <h3>Bag recorder</h3>
    <p>Select graph topics and a new output directory. The parent must already exist; existing recordings are never overwritten.</p>
    <fieldset disabled={!nativeAvailable || active || busy || topicNames.length === 0}>
      <legend>Topics to record</legend>
      {topicNames.length === 0 && <p>No topics available. Refresh the graph in Overview.</p>}
      {topicNames.map((name) => <label key={name} style={{ display: "block" }}>
        <input type="checkbox" checked={chosen.includes(name)} onChange={() => toggleTopic(name)} /> {name}
      </label>)}
    </fieldset>
    <label htmlFor="bag-output">Output directory</label>
    <input id="bag-output" type="text" value={output} onChange={(event) => setOutput(event.target.value)}
      placeholder="bags/session-001 or an absolute path" disabled={!nativeAvailable || active || busy}
      aria-describedby="bag-output-help" />
    <p id="bag-output-help">Relative paths resolve from the Pixi workspace. Choose a new directory under an existing parent.</p>
    <div>
      <button type="button" onClick={() => void start()}
        disabled={!nativeAvailable || !ready || active || busy || chosen.length === 0 || !output.trim()}>Start recording</button>
      <button type="button" onClick={() => void stop()}
        disabled={sessionId.current === null || busy || (state !== "starting" && state !== "running")}>Stop</button>
    </div>
    {!nativeAvailable && <p>Native ROS environment unavailable. Bag recording requires the desktop backend.</p>}
    <p role="status">Status: {state}{message && ` — ${message}`}</p>
    <pre aria-label="Bag recording output" style={{ maxHeight: 320, overflow: "auto", whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
      {lines.map((line) => <span key={line.id} data-stream={line.stream}>{line.stream === "stdout" ? "" : `[${line.stream}] `}{line.text}{"\n"}</span>)}
    </pre>
  </section>;
}

export default BagPanel;
