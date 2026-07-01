---
name: awesome-design-systems
description: Use when designing or prototyping UI in /server/bigA and you need real-world design-system references before creating Pencil prototypes or formal frontend pages.
---

# Awesome Design Systems

## Overview

This project-level skill wraps the upstream catalog at `alexpate/awesome-design-systems` and turns it into a repeatable design workflow for `/server/bigA`.

Use it together with `awesome-design-md`:

- `awesome-design-md`: provides `DESIGN.md` style context
- `awesome-design-systems`: provides real-world design system precedent

## Workflow

1. Read `references/source-summary.md` first.
2. Pick `1-3` relevant design systems for the current page or workflow.
3. State what patterns will be borrowed and what will be rejected.
4. Feed that decision into the `Pencil MCP` prototype or design note.

## Rules

- Prefer desktop, data-dense, operator-facing systems for `bigA` workbench pages.
- Treat the upstream repo as a catalog, not as a pixel-perfect copy target.
- If the reference choice is unclear, open the upstream catalog and compare before drawing.
- If a task also needs explicit colors, typography, or spacing tokens, load `awesome-design-md` in the same turn.

## When To Read More

- Need reference heuristics or example system families: read `references/source-summary.md`
- Need the full upstream catalog: open `https://github.com/alexpate/awesome-design-systems`
