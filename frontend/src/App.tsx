import { useEffect, useReducer, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import GraphOverview from "./features/GraphOverview";
import TopicEcho from "./features/TopicEcho";
import LaunchPanel from "./features/LaunchPanel";
import BagPanel from "./features/BagPanel";
import VizPanel from "./features/VizPanel";
import type { RosGraphSnapshot } from "./modules/ros";
import "./styles.css";

type View = "overview" | "echo" | "launch" | "bag" | "viz";
const tabs: { key: View; label: string; blurb: string }[] = [
  { key: "overview", label: "Overview", blurb: "Inspect topics, nodes, and services across the ROS 2 graph." },
  { key: "echo", label: "Topic echo", blurb: "Stream live messages from any topic in the graph." },
  { key: "launch", label: "Launch", blurb: "Run workspace launch files with arguments." },
  { key: "bag", label: "Bag recorder", blurb: "Record selected topics to a rosbag on disk." },
  { key: "viz", label: "Visualization", blurb: "Monitor images, point clouds, and IMU data live." },
];

const iconProps = {
  width: 18,
  height: 18,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
} as const;

function NavIcon({ kind }: { kind: View }) {
  switch (kind) {
    case "overview":
      return (
        <svg {...iconProps}>
          <rect width="7" height="7" x="3" y="3" rx="1" />
          <rect width="7" height="7" x="14" y="3" rx="1" />
          <rect width="7" height="7" x="14" y="14" rx="1" />
          <rect width="7" height="7" x="3" y="14" rx="1" />
        </svg>
      );
    case "echo":
      return (
        <svg {...iconProps}>
          <polyline points="4 17 10 11 4 5" />
          <line x1="12" x2="20" y1="19" y2="19" />
        </svg>
      );
    case "launch":
      return (
        <svg {...iconProps}>
          <circle cx="12" cy="12" r="10" />
          <polygon points="10 8 16 12 10 16" />
        </svg>
      );
    case "bag":
      return (
        <svg {...iconProps}>
          <circle cx="12" cy="12" r="10" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      );
    case "viz":
      return (
        <svg {...iconProps}>
          <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
          <path d="m3.3 7 8.7 5 8.7-5" />
          <path d="M12 22V12" />
        </svg>
      );
  }
}

function BrandMark() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
      <line x1="7.4" y1="11.1" x2="16.3" y2="6.5" />
      <line x1="7.4" y1="12.9" x2="16.3" y2="17.5" />
      <circle cx="5.4" cy="12" r="1.9" />
      <circle cx="18.6" cy="5.4" r="1.9" />
      <circle cx="18.6" cy="18.6" r="1.9" />
    </svg>
  );
}

export default function App() {
  const [view, setView] = useState<View>("overview");
  const [graph, setGraph] = useState<RosGraphSnapshot | null>(null);
  const native = isTauri();
  // Panels enter (fade + rise) on first reveal only — never on every tab switch.
  const revealedRef = useRef(new Set<View>());
  const [bump, refresh] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    if (revealedRef.current.has(view)) return;
    const frame = requestAnimationFrame(() => {
      revealedRef.current.add(view);
      refresh();
    });
    return () => cancelAnimationFrame(frame);
  }, [view, bump]);
  const revealed = (key: View) => `panel${revealedRef.current.has(key) ? " panel-in" : ""}`;
  const activeTab = tabs.find((tab) => tab.key === view) ?? tabs[0];
  // Keep all workflows mounted so event subscriptions and stop controls survive tab changes.
  return <main className="app-shell">
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark"><BrandMark /></span>
        <div className="brand-text">
          <p className="eyebrow">ROS 2 workspace</p>
          <h1>ROS Desktop</h1>
        </div>
      </div>
      <nav aria-label="Primary navigation">{tabs.map(tab =>
        <button key={tab.key} className={`nav-item ${view === tab.key ? "nav-item-active" : ""}`}
          type="button" aria-current={view === tab.key ? "page" : undefined} onClick={() => setView(tab.key)}>
          <NavIcon kind={tab.key} /><span>{tab.label}</span>
        </button>)}</nav>
      <div className="sidebar-footer">
        <span className={`status-dot ${native ? "status-dot-good" : "status-dot-warm"}`} aria-hidden="true" />
        <div className="sidebar-footer-text">
          <strong>{native ? "Desktop connection" : "Browser preview"}</strong>
          <span>{native ? "Tauri backend active" : "Read-only mode"}</span>
        </div>
      </div>
    </aside>
    <section className="content">
      <header className="topbar">
        <div>
          <p className="eyebrow">Desktop control center</p>
          <h2>{activeTab.label}</h2>
          <p className="topbar-sub">{activeTab.blurb}</p>
        </div>
        <div className="topbar-meta">
          <span className={`conn-pill ${native ? "conn-pill-good" : ""}`}>
            <span className={`status-dot ${native ? "status-dot-good" : "status-dot-warm"}`} aria-hidden="true" />
            {native ? "Connected" : "Browser"}
          </span>
          <span className="version-chip">v0.1.0</span>
        </div>
      </header>
      <div className="content-inner">
        <div className={revealed("overview")} hidden={view !== "overview"}><GraphOverview onSnapshot={setGraph} /></div>
        <div className={revealed("echo")} hidden={view !== "echo"}><TopicEcho topics={graph?.topics ?? []} available={native && graph !== null} /></div>
        <div className={revealed("launch")} hidden={view !== "launch"}><LaunchPanel available={native && graph !== null} /></div>
        <div className={revealed("bag")} hidden={view !== "bag"}><BagPanel topics={graph?.topics ?? []} available={native && graph !== null} /></div>
        <div className={revealed("viz")} hidden={view !== "viz"}><VizPanel topics={graph?.topics ?? []} available={native && graph !== null} /></div>
        {view !== "overview" && graph === null && <p className="workflow-note">ROS graph unavailable. Refresh Overview in the desktop app before starting a workflow.</p>}
      </div>
    </section>
  </main>;
}
