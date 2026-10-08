# UI refresh: design system, shell, and review-driven polish

Date: 2026-10-01 through 2026-10-08
Status: Implemented and verified (browser preview); desktop-only states pending manual regression

## Goal

Raise the interface from functional to award-grade product quality, using the
precision-instrument direction in `doc/specs/2026-10-01-ui-refresh.md`
(dark surfaces, emerald accent, 8 pt spacing grid, restrained motion) as the
contract. The benchmark is the polish level of Linear / Vercel / Figma Dark —
professional tools, not marketing pages.

## Pipeline (multi-agent, four phases)

1. **design-system** — rewrote `styles.css` around a full custom-property
   token system (surfaces, text tiers, accent, semantic colors, spacing,
   radius, type scale, elevation, motion), rebuilt the `App.tsx` shell
   (icon sidebar with active indicator, dynamic topbar with connection pill,
   1240 px content rhythm, first-show panel entrance), and hardened
   `index.html` (color-scheme, no-flash critical style, inline favicon).
   Iterated twice against its own Playwright screenshots.
2. **panel-polish** — refined all five panels: shared `panel-ui.tsx`
   components (StatusPill, EmptyState with hand-drawn icons, LogScroll with
   stick-to-bottom and jump-to-latest, Spinner), semantic button variants,
   skeleton loaders, bag topic filter with sticky header, disabled-reason
   titles and hints, VizPanel token alignment. Zero behavior changes.
3. **design-review** (independent, read-only) — scored the result 39/50
   across typography, whitespace, hierarchy, motion, interaction; produced a
   P0/P1/P2 list with computed WCAG contrast ratios and confirmed no phase-2
   regressions.
4. **ui-fixer** — resolved all 18 findings (below).

## Review findings and resolutions

- **P0** `--text-3` tertiary text failed WCAG 4.5:1 at the smallest sizes
  (3.7–4.2:1 measured). Lightened to `#7e8aa4` (≈4.9–5.1:1 on used surfaces);
  one token fixes every usage site.
- **P1** restored visible input focus rings; tokenized the type scale
  (`--fs-*`, off-scale 10/11.5/12.5/13 px sizes collapsed onto the ladder);
  unified the four "environment unavailable" notices on `.action-hint` with
  `role="status"`; moved the Launch refresh control into the panel header to
  restore label-control proximity; tokenized all raw `rgba`/hex literals
  (~45 semantic tokens added, visual values unchanged); gave Overview
  lists the >20-entry filter and sticky header that Bag already had.
- **P2** fixed the space-token-as-font-size misuse, unified uppercase label
  letter-spacing, `panel-in` duration back on token, `aria-labelledby`
  section naming, h4 sub-list headings, copy casing, sticky-header composite
  background, mono font for the bag output path, atomic label+input pairs in
  viz image options, and a restrained 120 ms fade for the jump button.
- Desktop code finding: BagPanel's Stop disabled-state derived from a ref
  (render-invisible); now state-backed with the ref kept for stale-closure-
  safe event filtering. Behavior otherwise unchanged.

## Verification

- `pnpm --dir frontend typecheck` and `pnpm --dir frontend build` pass
  (2026-10-08; chunk-size warning is the known three.js bundle size).
- Playwright screenshots for all five tabs at 1440×900 plus an 800 px
  overview, stored under `.workbuddy/ui-review/phase1..4/` (not committed).
- No console errors in the browser preview; no phase-2 regressions found by
  the independent reviewer or the lead's screenshot pass.

## Pending

- Manual desktop regression for states the browser preview cannot show:
  skeleton loaders, jump-to-latest, >20-entry filters (needs a live graph),
  running/failed/stopped pills, real-data list density, input focus ring.
- Optional follow-ups: three.js code splitting for the bundle-size warning,
  a raw-byte (non-base64) channel for high-rate viz frames.
