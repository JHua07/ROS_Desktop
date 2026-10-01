import { isTauri, invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { EnvironmentStatus } from "../modules/environment";
import { fetchRosGraphSnapshot, type RosGraphSnapshot } from "../modules/ros";

interface GraphOverviewProps {
  onSnapshot(snapshot: RosGraphSnapshot | null): void;
}
function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string") return message;
  }
  return "An unknown error occurred while checking the ROS graph.";
}

function nodePath(namespace: string, name: string): string {
  return `${namespace === "/" ? "" : namespace.replace(/\/$/, "")}/${name.replace(/^\//, "")}`;
}

export default function GraphOverview({ onSnapshot }: GraphOverviewProps) {
  const native = isTauri();
  const [environment, setEnvironment] = useState<EnvironmentStatus | null>(null);
  const [snapshot, setSnapshot] = useState<RosGraphSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestId = useRef(0);
  const onSnapshotRef = useRef(onSnapshot);
  onSnapshotRef.current = onSnapshot;

  const refresh = useCallback(async () => {
    if (!native) return;
    const currentRequest = ++requestId.current;
    setLoading(true);
    setError(null);
    setEnvironment(null);
    setSnapshot(null);
    onSnapshotRef.current(null);

    try {
      const status = await invoke<EnvironmentStatus>("environment_status");
      if (currentRequest !== requestId.current) return;
      setEnvironment(status);
      if (!status.available) {
        setError(status.error || "Pixi or the ROS environment is unavailable.");
        return;
      }

      // This command starts/reuses the daemon before querying its XML-RPC graph.
      const graph = await fetchRosGraphSnapshot();
      if (currentRequest !== requestId.current) return;
      setSnapshot(graph);
      onSnapshotRef.current(graph);
    } catch (cause) {
      if (currentRequest !== requestId.current) return;
      setError(errorMessage(cause));
    } finally {
      if (currentRequest === requestId.current) setLoading(false);
    }
  }, [native]);

  useEffect(() => {
    if (native) void refresh();
    else onSnapshotRef.current(null);
    return () => { requestId.current += 1; };
  }, [native, refresh]);

  const pixiAvailable = Boolean(environment?.pixi_path);
  const rosAvailable = Boolean(environment?.available && environment.ros_distro);
  const daemonAvailable = snapshot !== null;

  return (
    <section className="graph-overview" aria-labelledby="graph-overview-title">
      <header className="graph-overview-header">
        <div>
          <h2 id="graph-overview-title">ROS graph overview</h2>
          <p>Inspect topics, nodes, and services from the ROS 2 daemon.</p>
        </div>
        <button type="button" onClick={() => void refresh()} disabled={!native || loading}>
          {loading ? "Refreshing…" : "Refresh graph"}
        </button>
      </header>

      {!native && (
        <p className="graph-overview-notice" role="status">
          Browser mode: graph and environment queries require the Tauri desktop app.
        </p>
      )}

      <dl className="graph-overview-status" aria-label="ROS availability">
        <div><dt>Pixi</dt><dd>{!native ? "Desktop only" : loading && !environment ? "Checking…" : pixiAvailable ? "Available" : "Unavailable"}</dd></div>
        <div><dt>ROS</dt><dd>{!native ? "Desktop only" : loading && !environment ? "Checking…" : rosAvailable ? `Available (${environment?.ros_distro})` : "Unavailable"}</dd></div>
        <div><dt>Daemon</dt><dd>{!native ? "Desktop only" : loading ? "Checking…" : daemonAvailable ? "Connected" : "Unavailable"}</dd></div>
      </dl>

      {environment?.overlay_packages && environment.overlay_packages.length > 0 && (
        <p className="graph-overview-notice" role="status">
          Workspace overlays active: {environment.overlay_packages.join(", ")}.
        </p>
      )}

      {error && <p className="graph-overview-error" role="alert">{error}</p>}
      {loading && <p role="status">Loading ROS graph…</p>}

      {snapshot && (
        <div className="graph-overview-results">
          <div className="graph-overview-counts" aria-label="Graph counts">
            <p><strong>{snapshot.topic_count}</strong> topics</p>
            <p><strong>{snapshot.node_count}</strong> nodes</p>
            <p><strong>{snapshot.service_count}</strong> services</p>
          </div>
          {snapshot.topic_count === 0 && snapshot.node_count === 0 && snapshot.service_count === 0 && (
            <p className="graph-overview-empty">The ROS graph is empty. Start a ROS node and refresh to see it here.</p>
          )}
          <section className="graph-overview-list" aria-labelledby="graph-topics-title">
            <h3 id="graph-topics-title">Topics</h3>
            {snapshot.topics.length === 0 ? <p>No topics found.</p> : (
              <ul>{snapshot.topics.map((topic) => <li key={topic.name}><strong>{topic.name}</strong> <span>{topic.types.join(", ") || "Unknown type"}</span></li>)}</ul>
            )}
          </section>
          <section className="graph-overview-list" aria-labelledby="graph-nodes-title">
            <h3 id="graph-nodes-title">Nodes</h3>
            {snapshot.nodes.length === 0 ? <p>No nodes found.</p> : (
              <ul>{snapshot.nodes.map((node) => <li key={`${node.namespace}/${node.name}`}>{nodePath(node.namespace, node.name)}</li>)}</ul>
            )}
          </section>
          <section className="graph-overview-list" aria-labelledby="graph-services-title">
            <h3 id="graph-services-title">Services</h3>
            {snapshot.services.length === 0 ? <p>No services found.</p> : (
              <ul>{snapshot.services.map((service) => <li key={service.name}><strong>{service.name}</strong> <span>{service.types.join(", ") || "Unknown type"}</span></li>)}</ul>
            )}
          </section>
        </div>
      )}
    </section>
  );
}
