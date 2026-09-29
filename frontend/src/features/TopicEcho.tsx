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

  return (
    <section className="workflow-card topic-echo" aria-label="Topic echo">
      <h3>Topic echo</h3>
      <p>Stream messages from a topic in the current ROS graph.</p>
      <label htmlFor="topic-echo-select">Topic</label>
      <select
        id="topic-echo-select"
        value={selectedTopic}
        onChange={(event) => setSelectedTopic(event.target.value)}
        disabled={!available || active || topics.length === 0}
      >
        {topics.length === 0 && <option value="">No topics available</option>}
        {topics.map((topic) => <option key={topic.name} value={topic.name}>{topic.name}</option>)}
      </select>
      <button type="button" onClick={() => void start()} disabled={!available || !selectedTopic || active}>
        Start echo
      </button>
      <button type="button" onClick={() => void stop()} disabled={session === null || state !== "running"}>
        Stop
      </button>
      {!available && <p>Native ROS environment unavailable. Topic echo requires the desktop backend.</p>}
      <p role="status">Status: {state}{activeTopic && ` · ${activeTopic}`}{message && ` — ${message}`}</p>
      <pre aria-label="Topic echo output" style={{ maxHeight: 320, overflow: "auto", whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
        {lines.map((entry, index) => <span key={index} data-stream={entry.stream}>{entry.stream === "stderr" ? "[stderr] " : ""}{entry.text}{"\n"}</span>)}
      </pre>
    </section>
  );
}

export default TopicEcho;
