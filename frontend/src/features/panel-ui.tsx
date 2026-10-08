import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

/* ---------- Status pill ---------- */

export type PanelStatus = "idle" | "starting" | "running" | "stopping" | "stopped" | "failed";

const STATUS_TONE: Record<PanelStatus, string> = {
  idle: "neutral",
  starting: "accent",
  running: "accent",
  stopping: "warn",
  stopped: "success",
  failed: "danger",
};

const DOT_CLASS: Record<string, string> = {
  neutral: "status-dot-neutral",
  accent: "status-dot-good",
  warn: "status-dot-warm",
  success: "status-dot-success",
  danger: "status-dot-danger",
};

export function StatusPill({ status, label }: { status: PanelStatus; label?: string }) {
  const tone = STATUS_TONE[status] ?? "neutral";
  const text = label ?? status;
  return (
    <span className={`status-pill status-pill-${tone}`}>
      <span className={`status-dot ${DOT_CLASS[tone]}`} aria-hidden="true" />
      {text.charAt(0).toUpperCase() + text.slice(1)}
    </span>
  );
}

export function StatusDot({ tone }: { tone: "neutral" | "accent" | "warn" | "success" | "danger" }) {
  return <span className={`status-dot ${DOT_CLASS[tone]}`} aria-hidden="true" />;
}

/* ---------- Empty state ---------- */

const emptyIconProps = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
} as const;

type EmptyIconName = "list" | "nodes" | "services" | "launch" | "record" | "views" | "terminal" | "inbox";

export function EmptyIcon({ name }: { name: EmptyIconName }) {
  switch (name) {
    case "list":
      return (
        <svg {...emptyIconProps}>
          <line x1="9" y1="6" x2="21" y2="6" />
          <line x1="9" y1="12" x2="21" y2="12" />
          <line x1="9" y1="18" x2="21" y2="18" />
          <circle cx="4" cy="6" r="0.9" fill="currentColor" stroke="none" />
          <circle cx="4" cy="12" r="0.9" fill="currentColor" stroke="none" />
          <circle cx="4" cy="18" r="0.9" fill="currentColor" stroke="none" />
        </svg>
      );
    case "nodes":
      return (
        <svg {...emptyIconProps}>
          <rect width="7" height="7" x="3" y="3" rx="1" />
          <rect width="7" height="7" x="14" y="3" rx="1" />
          <rect width="7" height="7" x="14" y="14" rx="1" />
          <rect width="7" height="7" x="3" y="14" rx="1" />
        </svg>
      );
    case "services":
      return (
        <svg {...emptyIconProps}>
          <path d="M9 7V3" />
          <path d="M15 7V3" />
          <rect x="6" y="7" width="12" height="6" rx="2" />
          <path d="M12 13v8" />
        </svg>
      );
    case "launch":
      return (
        <svg {...emptyIconProps}>
          <circle cx="12" cy="12" r="10" />
          <polygon points="10 8 16 12 10 16" />
        </svg>
      );
    case "record":
      return (
        <svg {...emptyIconProps}>
          <circle cx="12" cy="12" r="10" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      );
    case "views":
      return (
        <svg {...emptyIconProps}>
          <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
      );
    case "terminal":
      return (
        <svg {...emptyIconProps}>
          <polyline points="4 17 10 11 4 5" />
          <line x1="12" x2="20" y1="19" y2="19" />
        </svg>
      );
    case "inbox":
      return (
        <svg {...emptyIconProps}>
          <path d="M22 12h-6l-2 3h-4l-2-3H2" />
          <path d="M5 5h14l3 7v7H2v-7Z" />
        </svg>
      );
  }
}

export function EmptyState({
  icon,
  title,
  hint,
  compact = false,
  centered = false,
}: {
  icon: ReactNode;
  title: string;
  hint?: string;
  compact?: boolean;
  centered?: boolean;
}) {
  const classes = ["empty-state"];
  if (compact) classes.push("empty-state-compact");
  if (centered) classes.push("empty-state-centered");
  return (
    <div className={classes.join(" ")}>
      <span className="empty-state-icon" aria-hidden="true">{icon}</span>
      <div className="empty-state-text">
        <p className="empty-state-title">{title}</p>
        {hint && <p className="empty-state-hint">{hint}</p>}
      </div>
    </div>
  );
}

/* ---------- Auto-scrolling log ---------- */

const PIN_THRESHOLD_PX = 28;

function useLogTail(lineCount: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [pinned, setPinned] = useState(true);

  const onScroll = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < PIN_THRESHOLD_PX);
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (el && pinned) el.scrollTop = el.scrollHeight;
  }, [pinned, lineCount]);

  const jump = useCallback(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
    setPinned(true);
  }, []);

  return { ref, pinned, onScroll, jump };
}

export function LogScroll({
  label,
  lineCount,
  empty,
  children,
}: {
  label: string;
  lineCount: number;
  empty?: ReactNode;
  children: ReactNode;
}) {
  const { ref, pinned, onScroll, jump } = useLogTail(lineCount);
  return (
    <div className="log-scroll">
      <div ref={ref} onScroll={onScroll} className="log-scroll-body" role="log" aria-label={label}>
        {lineCount === 0 && empty ? empty : children}
      </div>
      {!pinned && lineCount > 0 && (
        <button type="button" className="log-jump" onClick={jump}>
          Jump to latest
        </button>
      )}
    </div>
  );
}

/* ---------- Inline spinner ---------- */

export function Spinner() {
  return <span className="spinner" aria-hidden="true" />;
}
