import { useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import GraphOverview from "./features/GraphOverview";
import TopicEcho from "./features/TopicEcho";
import LaunchPanel from "./features/LaunchPanel";
import BagPanel from "./features/BagPanel";
import VizPanel from "./features/VizPanel";
import type { RosGraphSnapshot } from "./modules/ros";
import "./styles.css";

type View = "overview" | "echo" | "launch" | "bag" | "viz";
const tabs: { key: View; label: string }[] = [
  { key: "overview", label: "Overview" }, { key: "echo", label: "Topic echo" },
  { key: "launch", label: "Launch" }, { key: "bag", label: "Bag recorder" },
  { key: "viz", label: "Visualization" },
];
export default function App() {
  const [view, setView] = useState<View>("overview");
  const [graph, setGraph] = useState<RosGraphSnapshot | null>(null);
  const native = isTauri();
  // Keep all workflows mounted so event subscriptions and stop controls survive tab changes.
  return <main className="app-shell">
    <aside className="sidebar">
      <div className="brand-mark">RD</div>
      <div><p className="eyebrow">ROS 2 workspace</p><h1>ROS Desktop</h1></div>
      <nav aria-label="Primary navigation">{tabs.map(tab =>
        <button key={tab.key} className={`nav-item ${view === tab.key ? "nav-item-active" : ""}`}
          type="button" onClick={() => setView(tab.key)}>{tab.label}</button>)}</nav>
      <div className="sidebar-footer"><span className={`status-dot ${native ? "status-dot-good" : "status-dot-warm"}`} />
        {native ? "Desktop connection" : "Browser preview"}</div>
    </aside>
    <section className="content">
      <header className="topbar"><div><p className="eyebrow">Desktop control center</p><h2>Inspect and control ROS 2</h2></div><span className="version-chip">v0.1.0</span></header>
      <div hidden={view !== "overview"}><GraphOverview onSnapshot={setGraph} /></div>
      <div hidden={view !== "echo"}><TopicEcho topics={graph?.topics ?? []} available={native && graph !== null} /></div>
      <div hidden={view !== "launch"}><LaunchPanel available={native && graph !== null} /></div>
      <div hidden={view !== "bag"}><BagPanel topics={graph?.topics ?? []} available={native && graph !== null} /></div>
      <div hidden={view !== "viz"}><VizPanel topics={graph?.topics ?? []} available={native && graph !== null} /></div>
      {view !== "overview" && graph === null && <p className="workflow-note">ROS graph unavailable. Refresh Overview in the desktop app before starting a workflow.</p>}
    </section>
  </main>;
}
