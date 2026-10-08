import { isTauri, invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { EnvironmentStatus } from "../modules/environment";
import { fetchRosGraphSnapshot, type RosGraphSnapshot } from "../modules/ros";
import { EmptyIcon, EmptyState, Spinner, StatusDot } from "./panel-ui";

const LIST_FILTER_THRESHOLD = 20;

function FilterableList<T>({
  id,
  title,
  columnLabel,
  filterLabel,
  items,
  keyOf,
  renderItem,
  empty,
}: {
  id: string;
  title: string;
  columnLabel: string;
  filterLabel: string;
  items: T[];
  keyOf: (item: T) => string;
  renderItem: (item: T) => ReactNode;
  empty: ReactNode;
}) {
  const [query, setQuery] = useState("");
  const showFilter = items.length > LIST_FILTER_THRESHOLD;
  const needle = query.trim().toLowerCase();
  const visible = showFilter && needle ? items.filter((item) => keyOf(item).toLowerCase().includes(needle)) : items;
  return (
    <section className="graph-overview-list" aria-labelledby={id}>
      <h4 id={id}>{title}</h4>
      {items.length === 0 ? (
        empty
      ) : showFilter ? (
        <>
          <input
            className="list-filter"
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={`Filter ${items.length} ${title.toLowerCase()}…`}
            aria-label={filterLabel}
          />
          <div className="list-scroll-tall">
            <div className="list-heading" aria-hidden="true">
              <span>{columnLabel}</span>
              <span>{needle ? `${visible.length} of ${items.length}` : `${items.length} total`}</span>
            </div>
            <ul>{visible.map(renderItem)}</ul>
            {visible.length === 0 && (
              <EmptyState
                compact
                icon={<EmptyIcon name="list" />}
                title="No matches"
                hint={`No results for “${query.trim()}”.`}
              />
            )}
          </div>
        </>
      ) : (
        <ul>{items.map(renderItem)}</ul>
      )}
    </section>
  );
}

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

  type Tone = "neutral" | "accent" | "warn";
  const dot = (tone: Tone, text: string) => (
    <><StatusDot tone={tone} /> {text}</>
  );
  const checking = loading && !environment;
  const pixiEntry = !native ? dot("neutral", "Desktop only") : checking ? dot("warn", "Checking…") : pixiAvailable ? dot("accent", "Available") : dot("neutral", "Unavailable");
  const rosEntry = !native ? dot("neutral", "Desktop only") : checking ? dot("warn", "Checking…") : rosAvailable ? dot("accent", `Available (${environment?.ros_distro})`) : dot("neutral", "Unavailable");
  const daemonEntry = !native ? dot("neutral", "Desktop only") : loading ? dot("warn", "Checking…") : daemonAvailable ? dot("accent", "Connected") : dot("neutral", "Unavailable");

  return (
    <section className="graph-overview" aria-labelledby="graph-overview-title">
      <header className="graph-overview-header">
        <div>
          <h3 id="graph-overview-title">Graph snapshot</h3>
          <p>Environment, daemon, and workspace overlays at a glance.</p>
        </div>
        <button
          type="button"
          className="btn-primary"
          onClick={() => void refresh()}
          disabled={!native || loading}
          title={!native ? "Requires the Tauri desktop app" : loading ? "Refreshing the ROS graph" : undefined}
        >
          {loading && <Spinner />}
          {loading ? "Refreshing…" : "Refresh graph"}
        </button>
      </header>

      {!native && (
        <p className="graph-overview-notice" role="status">
          Browser mode: graph and environment queries require the Tauri desktop app.
        </p>
      )}

      <dl className="graph-overview-status" aria-label="ROS availability">
        <div><dt>Pixi</dt><dd>{pixiEntry}</dd></div>
        <div><dt>ROS</dt><dd>{rosEntry}</dd></div>
        <div><dt>Daemon</dt><dd>{daemonEntry}</dd></div>
      </dl>

      {environment?.overlay_packages && environment.overlay_packages.length > 0 && (
        <p className="graph-overview-notice" role="status">
          Workspace overlays active: {environment.overlay_packages.join(", ")}.
        </p>
      )}

      {error && <p className="graph-overview-error" role="alert">{error}</p>}
      {loading && (
        <div className="graph-skeleton" role="status" aria-label="Loading ROS graph">
          <div className="graph-skeleton-counts">
            {[0, 1, 2].map((index) => <div key={index} className="skeleton skeleton-count" />)}
          </div>
          {[0, 1, 2].map((list) => (
            <div key={list} className="graph-skeleton-list" aria-hidden="true">
              <div className="skeleton skeleton-line skeleton-line-heading" />
              {Array.from({ length: 6 }, (_, row) => (
                <div key={row} className="skeleton skeleton-line" style={{ width: `${94 - ((row * 13 + list * 7) % 42)}%` }} />
              ))}
            </div>
          ))}
        </div>
      )}

      {snapshot && (
        <div className="graph-overview-results">
          <div className="graph-overview-counts" aria-label="Graph counts">
            <p><strong>{snapshot.topic_count}</strong> topics</p>
            <p><strong>{snapshot.node_count}</strong> nodes</p>
            <p><strong>{snapshot.service_count}</strong> services</p>
          </div>
          {snapshot.topic_count === 0 && snapshot.node_count === 0 && snapshot.service_count === 0 && (
            <EmptyState
              icon={<EmptyIcon name="inbox" />}
              title="The ROS graph is empty"
              hint="Start a ROS node and refresh to see it here."
              centered
            />
          )}
          <FilterableList
            id="graph-topics-title"
            title="Topics"
            columnLabel="Topic"
            filterLabel="Filter topics"
            items={snapshot.topics}
            keyOf={(topic) => topic.name}
            renderItem={(topic) => <li key={topic.name}><strong>{topic.name}</strong> <span>{topic.types.join(", ") || "Unknown type"}</span></li>}
            empty={<EmptyState compact icon={<EmptyIcon name="list" />} title="No topics" hint="Start a publisher and refresh the graph." />}
          />
          <FilterableList
            id="graph-nodes-title"
            title="Nodes"
            columnLabel="Node"
            filterLabel="Filter nodes"
            items={snapshot.nodes}
            keyOf={(node) => `${node.namespace}/${node.name}`}
            renderItem={(node) => <li key={`${node.namespace}/${node.name}`}>{nodePath(node.namespace, node.name)}</li>}
            empty={<EmptyState compact icon={<EmptyIcon name="nodes" />} title="No nodes" hint="Running nodes appear here after refresh." />}
          />
          <FilterableList
            id="graph-services-title"
            title="Services"
            columnLabel="Service"
            filterLabel="Filter services"
            items={snapshot.services}
            keyOf={(service) => service.name}
            renderItem={(service) => <li key={service.name}><strong>{service.name}</strong> <span>{service.types.join(", ") || "Unknown type"}</span></li>}
            empty={<EmptyState compact icon={<EmptyIcon name="services" />} title="No services" hint="Services appear once their nodes are running." />}
          />
        </div>
      )}
    </section>
  );
}
