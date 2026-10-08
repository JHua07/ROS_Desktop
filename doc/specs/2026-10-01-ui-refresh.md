# UI refresh design direction (2026-10-01)

**Goal:** Elevate the GUI from functional MVP styling to the polish bar of Linear / Vercel / Figma dark UIs — precise, calm, professional. The benchmark is award-grade product design, not decorative marketing effects.

## Design language: "precision instrument"

A deep-space dark surface system with a single emerald accent (continuity with the existing `#71e1c1` brand mark), generous whitespace, crisp 1px borders, layered translucent cards, and restrained motion. Monospace type for machine data (topics, logs, types), humanist sans for everything else.

## Token system (styles.css `:root`, mandatory)

- Surfaces: `--bg-0` (app base) … `--bg-3` (raised), plus a translucent card surface token.
- Text: `--text-1` (primary), `--text-2` (secondary), `--text-3` (tertiary/labels).
- Accent: `--accent` with `--accent-hover`, `--accent-active`, `--accent-soft` (tinted background), `--accent-text` (on-accent). Keep the emerald family.
- Semantic: `--success/--warn/--danger` each with a soft background variant.
- Borders: `--border-1` (subtle), `--border-2` (strong/hover).
- Spacing on an 4/8pt scale: `--space-1..8` = 4, 8, 12, 16, 24, 32, 48, 64.
- Radius: `--radius-sm` 6, `--radius-md` 10, `--radius-lg` 16, `--radius-full`.
- Type scale: 11, 12, 13.5 (body), 16, 20, 26, 34; weights 450/550/650; `--font-ui` (Segoe UI Variable → system stack) and `--font-mono` (Cascadia Code → Consolas → monospace) for topics/logs/IDs.
- Elevation: `--shadow-sm/md/lg` tuned for dark (soft, wide, low-opacity).
- Motion: `--ease-out: cubic-bezier(0.22,1,0.36,1)`, `--dur-fast: 120ms`, `--dur: 220ms`; every animation honors `prefers-reduced-motion`.

## Component standards

- Buttons: `primary` (accent fill), `secondary` (bordered), `ghost` (text), `danger` variants; hover/active/focus-visible states for each; disabled shows a reason where possible (title or adjacent text).
- Cards: 1px `--border-1`, `--bg-2` translucent surface, `--radius-lg`, no gratuitous shadows; hover lift only where the card is actionable.
- Focus: visible `focus-visible` ring using `--accent` at 2px offset; never remove outlines for keyboard users.
- Status: pulsing dot for live connections, static for idle; semantic color coding consistent across panels.
- Loading: skeleton shimmer blocks instead of bare "Loading…" text where content shape is known; buttons show inline spinners during async work.
- Empty states: every list/panel has a designed empty state (icon + one-line guidance + optional action), never a bare "No topics found."
- Logs/output: mono font, themed thin scrollbars, auto-scroll to tail (with stickiness detection), overflow-wrap.
- Nav: sidebar items with inline SVG icons (stroke 1.5, 18px), active state = accent indicator bar + soft accent background; smooth transitions.
- Scrollbars and text selection themed globally.

## Interaction logic (phase 2 scope)

- Log areas auto-scroll while pinned to bottom; unpinned when the user scrolls up; a "jump to latest" affordance appears.
- Disabled actions explain why (tooltip/adjacent hint), e.g. "Refresh Overview first".
- Tab switching keeps all panels mounted (existing `hidden` pattern) — entrance animation only on first reveal, not on every tab switch.
- Long lists (topics, launch files) get sticky headers and search/filter where count > 20.

## Constraints

- No new npm dependencies — CSS custom properties, keyframes, and inline SVG only.
- All existing panel class hooks (`.graph-overview`, `.topic-echo`, `.launch-panel`, `.bag-panel`, `.workflow-panel`, VizPanel classes) must keep working; restyle them, refine markup only in phase 2.
- Accessibility: contrast ≥ 4.5:1 for body text, aria states preserved, keyboard navigable.
- English UI copy; keep it terse and professional.

## Phases

1. `design-system`: rewrite `styles.css` with the full token system and component standards; restructure the app shell in `App.tsx` (sidebar nav with icons, topbar, layout rhythm); polish `frontend/index.html` (meta, color-scheme, font smoothing). Verify with typecheck/build and Playwright screenshots of the running dev server, iterating at least twice.
2. `panel-polish`: refine each feature panel's markup and panel-specific CSS on top of the tokens (loading skeletons, empty states, log interactions, disabled reasons, list polish).
3. `design-review`: screenshot every tab in browser mode and grade typography, whitespace, visual hierarchy, motion, and interaction; output a prioritized issue list.
4. Fix loop on the issue list until review passes.
