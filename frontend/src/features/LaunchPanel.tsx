import { isTauri } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  describeProcessError,
  listLaunchTargets,
  listenRosProcessEvents,
  startLaunch,
  stopRosProcess,
  type LaunchArgument,
  type ProcessEvent,
  type ProcessState,
} from "../modules/process";
import { EmptyIcon, EmptyState, LogScroll, Spinner, StatusPill } from "./panel-ui";

type OutputLine = { id: number; text: string };

function parseArguments(text: string): LaunchArgument[] {
  return text.split(/\r?\n/).map((line, index) => {
    if (line.trim() === "") return null;
    const separator = line.indexOf(":=");
    const name = line.slice(0, separator);
    const value = line.slice(separator + 2);
    if (separator < 0 || !/^[A-Za-z_][A-Za-z_0-9]*$/.test(name) || /[\p{Cc}]/u.test(value)) {
      throw new Error(`Invalid argument on line ${index + 1}. Use name:=value (identifier name, no control characters).`);
    }
    return { name, value };
  }).filter((argument): argument is LaunchArgument => argument !== null);
}

export function LaunchPanel({ available }: { available: boolean }) {
  const nativeAvailable = available && isTauri();
  const [targets, setTargets] = useState<string[]>([]);
  const [target, setTarget] = useState("");
  const [argumentText, setArgumentText] = useState("");
  const [loading, setLoading] = useState(false);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [state, setState] = useState<ProcessState | "idle">("idle");
  const [error, setError] = useState("");
  const [lines, setLines] = useState<OutputLine[]>([]);
  const sessionId = useRef<number | null>(null);
  const starting = useRef(false);
  const earlyEvents = useRef<ProcessEvent[]>([]);
  const nextLineId = useRef(0);

  const refresh = useCallback(async () => {
    if (!nativeAvailable) return;
    setLoading(true);
    setError("");
    try {
      const found = await listLaunchTargets();
      setTargets(found);
      setTarget((current) => found.includes(current) ? current : (found[0] ?? ""));
    } catch (reason) {
      setTargets([]);
      setTarget("");
      setError(describeProcessError(reason));
    } finally {
      setLoading(false);
    }
  }, [nativeAvailable]);

  const handleEvent = useCallback((event: ProcessEvent) => {
    if (event.kind !== "launch") return;
    if (starting.current && sessionId.current === null) {
      earlyEvents.current = [...earlyEvents.current.slice(-499), event];
      return;
    }
    if (event.session_id !== sessionId.current) return;
    setState((current) => current === "stopping" && event.state === "running" ? current : event.state);
    if (event.line !== undefined || event.error || event.exit_code !== undefined) {
      const detail = event.line !== undefined
        ? `[${event.stream ?? "output"}] ${event.line}`
        : event.error ? `[status] ${event.error}` : `[status] Exit code: ${event.exit_code}`;
      const line = { id: ++nextLineId.current, text: detail };
      setLines((previous) => [...previous.slice(-499), line]);
    }
  }, []);

  useEffect(() => {
    if (!nativeAvailable) { setReady(false); return; }
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listenRosProcessEvents(handleEvent).then((cleanup) => {
      if (disposed) cleanup();
      else { unlisten = cleanup; setReady(true); }
    }).catch((reason) => { if (!disposed) setError(describeProcessError(reason)); });
    return () => { disposed = true; unlisten?.(); setReady(false); };
  }, [nativeAvailable, handleEvent]);

  useEffect(() => { if (nativeAvailable) void refresh(); }, [nativeAvailable, refresh]);

  async function start() {
    if (!nativeAvailable || !ready || !target || busy || state === "running" || state === "starting" || state === "stopping") return;
    setError("");
    let arguments_: LaunchArgument[];
    try { arguments_ = parseArguments(argumentText); }
    catch (reason) { setError(describeProcessError(reason)); return; }
    setBusy(true);
    starting.current = true;
    sessionId.current = null;
    earlyEvents.current = [];
    setLines([]);
    setState("starting");
    try {
      const session = await startLaunch(target, arguments_);
      sessionId.current = session.session_id;
      starting.current = false;
      setState(session.state);
      for (const event of earlyEvents.current) handleEvent(event);
      earlyEvents.current = [];
    } catch (reason) {
      starting.current = false;
      earlyEvents.current = [];
      setState("idle");
      setError(describeProcessError(reason));
    } finally { setBusy(false); }
  }

  async function stop() {
    if (!nativeAvailable || busy || (state !== "starting" && state !== "running")) return;
    setBusy(true);
    setError("");
    try { await stopRosProcess("launch"); setState((current) => current === "stopped" || current === "failed" ? current : "stopping"); }
    catch (reason) { setError(describeProcessError(reason)); }
    finally { setBusy(false); }
  }

  const active = state === "starting" || state === "running" || state === "stopping";
  const startDisabledReason = !nativeAvailable
    ? "Requires the desktop app"
    : !ready
      ? "Connecting to the backend"
      : loading
        ? "Scanning workspace"
        : active
          ? "Launch in progress"
          : busy
            ? "Working"
            : !target
              ? "No launch file selected — refresh files"
              : undefined;
  const stopDisabledReason = !nativeAvailable ? "Requires the desktop app" : state !== "starting" && state !== "running" ? "Launch is not running" : undefined;
  return (
    <section className="status-card" aria-labelledby="launch-title">
      <header className="panel-header">
        <div>
          <h3 id="launch-title">Launch runner</h3>
          <p>Select a workspace launch file and optionally provide one <code>name:=value</code> argument per line.</p>
        </div>
        <button
          type="button"
          className="btn-secondary"
          onClick={() => void refresh()}
          disabled={!nativeAvailable || loading}
          title={!nativeAvailable ? "Requires the desktop app" : loading ? "Scanning workspace" : undefined}
        >
          {loading && <Spinner />}
          {loading ? "Scanning…" : "Refresh files"}
        </button>
      </header>
      {!nativeAvailable && <p className="action-hint" role="status">Launch requires the native ROS environment; browser preview is read-only.</p>}
      <label htmlFor="launch-target">Launch target</label>
      <select id="launch-target" value={target} onChange={(event) => setTarget(event.target.value)} disabled={!nativeAvailable || loading || active || busy}>
        {targets.length === 0 && <option value="">No launch files available</option>}
        {targets.map((item) => <option key={item} value={item}>{item}</option>)}
      </select>
      <div>
        <label htmlFor="launch-arguments">Arguments</label>
        <textarea id="launch-arguments" value={argumentText} onChange={(event) => setArgumentText(event.target.value)} rows={4} placeholder={"robot_name:=my_robot\nuse_sim_time:=true"} disabled={!nativeAvailable || active || busy} />
      </div>
      <div className="panel-actions">
        <button
          type="button"
          className="btn-primary"
          onClick={() => void start()}
          disabled={!nativeAvailable || !ready || !target || loading || busy || active}
          title={startDisabledReason}
        >
          Start launch
        </button>
        <button
          type="button"
          className="btn-danger"
          onClick={() => void stop()}
          disabled={!nativeAvailable || busy || (state !== "starting" && state !== "running")}
          title={stopDisabledReason}
        >
          Stop launch
        </button>
      </div>
      {nativeAvailable && !loading && !error && targets.length === 0 && (
        <p className="action-hint" role="status">No launch files found in the workspace. Add a launch file and refresh.</p>
      )}
      <div className="status-row" role="status">
        <StatusPill status={state} />
        {loading && <span className="status-message">Scanning workspace…</span>}
      </div>
      {error && <p role="alert">{error}</p>}
      <LogScroll
        label="Launch output"
        lineCount={lines.length}
        empty={
          <EmptyState
            compact
            icon={<EmptyIcon name="terminal" />}
            title="No launch output"
            hint="Output appears when a launch file runs."
          />
        }
      >
        {lines.map((line) => <div key={line.id}>{line.text}</div>)}
      </LogScroll>
    </section>
  );
}
export default LaunchPanel;
