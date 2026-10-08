import { useCallback, useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { RosTopic } from "../modules/ros";
import {
  describeProcessError,
  listenRosProcessEvents,
  startTopicEcho,
  stopRosProcess,
  type ProcessEvent,
  type ProcessSession,
  type ProcessState,
} from "../modules/process";
import { EmptyIcon, EmptyState, LogScroll, StatusPill } from "./panel-ui";

interface TopicEchoProps {
  topics: RosTopic[];
  available: boolean;
}

type EchoState = "idle" | ProcessState;
interface EchoLine {
  stream: "stdout" | "stderr";
  text: string;
}

const MAX_LINES = 500;

export function TopicEcho({ topics, available }: TopicEchoProps) {
  const [selectedTopic, setSelectedTopic] = useState(topics[0]?.name ?? "");
  const [activeTopic, setActiveTopic] = useState("");
  const [session, setSession] = useState<ProcessSession | null>(null);
  const [state, setState] = useState<EchoState>("idle");
  const [lines, setLines] = useState<EchoLine[]>([]);
  const [message, setMessage] = useState("");
  const sessionRef = useRef<number | null>(null);
  const startingRef = useRef(false);
  const pendingRef = useRef<ProcessEvent[]>([]);
  const stopRequestedRef = useRef(false);
  const listenerRef = useRef<Promise<UnlistenFn> | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    if (!topics.some((topic) => topic.name === selectedTopic)) {
      setSelectedTopic(topics[0]?.name ?? "");
    }
  }, [topics, selectedTopic]);

  const applyEvent = useCallback((event: ProcessEvent) => {
    if (event.stream && event.line !== undefined) {
      setLines((previous) => [...previous, { stream: event.stream!, text: event.line! }].slice(-MAX_LINES));
    }
    // Output emitted during shutdown must not regress the displayed state to running.
    if (!event.stream) {
      setState(event.state);
      if (event.state === "failed") {
        setMessage(event.error ?? `Echo exited unexpectedly${event.exit_code === undefined ? "" : ` (code ${event.exit_code})`}.`);
      } else if (event.state === "stopped") {
        setMessage(stopRequestedRef.current ? "Echo stopped." : "Echo exited on its own.");
      }
    }
  }, []);

  const onEvent = useCallback((event: ProcessEvent) => {
    if (event.kind !== "topic_echo") return;
    if (event.session_id === sessionRef.current) {
      applyEvent(event);
    } else if (startingRef.current && sessionRef.current === null) {
      // The native process may emit output or even exit before invoke resolves.
      pendingRef.current = [...pendingRef.current, event].slice(-MAX_LINES);
    }
  }, [applyEvent]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      void listenerRef.current?.then((unlisten) => unlisten(), () => undefined);
    };
  }, []);

  async function start() {
    if (!available || !selectedTopic || startingRef.current || state === "running" || state === "stopping") return;
    startingRef.current = true;
    sessionRef.current = null;
    pendingRef.current = [];
    stopRequestedRef.current = false;
    setSession(null);
    setActiveTopic(selectedTopic);
    setLines([]);
    setMessage("");
    setState("starting");
    try {
      // Install the listener before spawning; retain it for tail output after stop.
      if (!listenerRef.current) {
        listenerRef.current = listenRosProcessEvents(onEvent).catch((error: unknown) => {
          listenerRef.current = null;
          throw error;
        });
      }
      await listenerRef.current;
      if (!mountedRef.current) return;
      const started = await startTopicEcho(selectedTopic);
      if (!mountedRef.current) return;
      sessionRef.current = started.session_id;
      setSession(started);
      setState(started.state);
      for (const event of pendingRef.current) {
        if (event.session_id === started.session_id) applyEvent(event);
      }
    } catch (error) {
      if (mountedRef.current) {
        setState("failed");
        setMessage(describeProcessError(error));
      }
    } finally {
      pendingRef.current = [];
      startingRef.current = false;
    }
  }

  async function stop() {
    if (sessionRef.current === null || state === "stopping") return;
    stopRequestedRef.current = true;
    setState("stopping");
    try {
      await stopRosProcess("topic_echo");
    } catch (error) {
      setMessage(describeProcessError(error));
      stopRequestedRef.current = false;
      setState("failed");
    }
  }

  const active = state === "starting" || state === "running" || state === "stopping";

  const startDisabledReason = !available
    ? "Requires the ROS graph — refresh Overview in the desktop app"
    : topics.length === 0
      ? "The current graph has no topics"
      : !selectedTopic
        ? "No topic selected"
        : active
          ? "Echo already active"
          : undefined;
  const stopDisabledReason = session === null ? "No echo session yet" : state !== "running" ? "Echo is not running" : undefined;

  return (
    <section className="workflow-card topic-echo" aria-labelledby="topic-echo-title">
      <h3 id="topic-echo-title">Echo stream</h3>
      <p>Stream messages from a topic in the current ROS graph.</p>
      <label htmlFor="topic-echo-select">Topic</label>
      <select
        id="topic-echo-select"
        value={selectedTopic}
        onChange={(event) => setSelectedTopic(event.target.value)}
        disabled={!available || active || topics.length === 0}
        title={topics.length === 0 ? "No topics in the current graph — refresh Overview" : undefined}
      >
        {topics.length === 0 && <option value="">No topics available</option>}
        {topics.map((topic) => <option key={topic.name} value={topic.name}>{topic.name}</option>)}
      </select>
      <div className="panel-actions">
        <button
          type="button"
          className="btn-primary"
          onClick={() => void start()}
          disabled={!available || !selectedTopic || active}
          title={startDisabledReason}
        >
          Start echo
        </button>
        <button
          type="button"
          className="btn-danger"
          onClick={() => void stop()}
          disabled={session === null || state !== "running"}
          title={stopDisabledReason}
        >
          Stop
        </button>
      </div>
      {!available && <p className="action-hint" role="status">Native ROS environment unavailable. Topic echo requires the desktop backend.</p>}
      <div className="status-row" role="status">
        <StatusPill status={state} />
        {activeTopic && <code>{activeTopic}</code>}
        {message && <span className={`status-message${state === "failed" ? " status-message-error" : ""}`}>{message}</span>}
      </div>
      <LogScroll
        label="Topic echo output"
        lineCount={lines.length}
        empty={
          <EmptyState
            compact
            icon={<EmptyIcon name="terminal" />}
            title="No echo output"
            hint={activeTopic ? `Start an echo on ${activeTopic} to stream messages.` : "Start an echo to stream messages here."}
          />
        }
      >
        {lines.map((entry, index) => <span key={index} data-stream={entry.stream}>{entry.stream === "stderr" ? "[stderr] " : ""}{entry.text}{"\n"}</span>)}
      </LogScroll>
    </section>
  );
}

export default TopicEcho;
