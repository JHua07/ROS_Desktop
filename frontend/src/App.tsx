import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type BackendState = "idle" | "checking" | "ready" | "unavailable";

function App() {
  const [backendState, setBackendState] = useState<BackendState>("idle");

  async function checkBackend() {
    setBackendState("checking");
    try {
      await invoke<string>("ping");
      setBackendState("ready");
    } catch {
      setBackendState("unavailable");
    }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand-mark">RD</div>
        <div>
          <p className="eyebrow">ROS 2 workspace</p>
          <h1>ROS Desktop</h1>
        </div>
        <nav aria-label="Primary navigation">
          <button className="nav-item nav-item-active" type="button">Overview</button>
          <button className="nav-item" type="button">Nodes</button>
          <button className="nav-item" type="button">Topics</button>
          <button className="nav-item" type="button">Services</button>
        </nav>
        <div className="sidebar-footer">
          <span className="status-dot status-dot-warm" />
          <span>ROS environment pending</span>
        </div>
      </aside>

      <section className="content">
        <header className="topbar">
          <div>
            <p className="eyebrow">Desktop control center</p>
            <h2>Build the bridge to your robot</h2>
          </div>
          <span className="version-chip">v0.1.0</span>
        </header>

        <section className="hero-card">
          <div>
            <span className="section-label">Workspace ready</span>
            <h3>A calm place to inspect and control ROS 2.</h3>
            <p>
              The shell can check its Tauri backend. Pixi and RoboStack will
              provide the ROS environment when the first workflow is selected.
            </p>
            <button className="primary-button" type="button" onClick={checkBackend}>
              {backendState === "checking" ? "Checking backend…" : "Check backend"}
            </button>
          </div>
          <div className="hero-orbit" aria-hidden="true">
            <span className="orbit orbit-one" />
            <span className="orbit orbit-two" />
            <span className="orbit-core">ROS</span>
          </div>
        </section>

        <section className="status-grid" aria-label="Workspace status">
          <article className="status-card">
            <div className="status-card-heading"><span className="status-dot status-dot-good" />Frontend</div>
            <strong>Online</strong>
            <p>React and Vite are ready for feature work.</p>
          </article>
          <article className="status-card">
            <div className="status-card-heading"><span className={`status-dot ${backendState === "ready" ? "status-dot-good" : "status-dot-warm"}`} />Tauri backend</div>
            <strong>{backendState === "ready" ? "Connected" : backendState === "unavailable" ? "Browser mode" : "Ready to check"}</strong>
            <p>The first IPC command is available from the native shell.</p>
          </article>
          <article className="status-card">
            <div className="status-card-heading"><span className="status-dot status-dot-warm" />ROS environment</div>
            <strong>Not configured</strong>
            <p>Jazzy package selection starts with the first ROS workflow.</p>
          </article>
        </section>
      </section>
    </main>
  );
}

export default App;
