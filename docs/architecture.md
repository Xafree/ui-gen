# FigCraft architecture

> **Last reviewed:** 2026-10-10 · **Last updated:** 2026-10-30 · **Change:** minor (import pages, the inspection cache and the Components tab (UIGEN lot 4); the playground view: images relayed from the capture service, the engine's `playground.*` commands, the sidecar's `playground.*` methods; earlier: the `figcraft-capture` crate, the `sidecar/` service and the engine's capture port; earlier: the `vectordrawable` crate, L4: Android Vector Drawable export; earlier: the shared hub in `collab-net` and the `figcraft-server` app) · **Target:** Figma Design

How FigCraft is built today. Engine first, a thin swappable UI, everything a command; a Figma-shaped
data model (a scene graph of frames with auto layout, components and variables). ~108,800 lines of
Rust (2026-10-10).

## Crates and layers

`cargo xtask layers` enforces the layering (`xtask/src/layers.rs`): a crate may depend only on
lower layers. Nothing below L6 depends on egui, eframe, rfd or winit, so the UI crate can be
replaced.

| Layer | Crate | Lines | Role |
|---|---|---|---|
| L0 | `figcraft-geom` | ~0.5k | Affine transforms, shape path builders (rounded and smoothed corners, polygons, stars, arcs) on kurbo |
| L0 | `figcraft-color` | ~1.3k | Colours (HSB/HSL, LCH/OKLCH, hex/CSS, Display P3), paints (solid, gradients, image), blend modes |
| L1 | `figcraft-doc` | ~8.5k | Document model: pages, node tree in a persistent map (`imbl`, stable ids), properties, components, styles, variables, prototype data, comments, versions. Serde everywhere |
| L2 | `figcraft-layout` | ~1.8k | Auto layout (horizontal, vertical, wrap, grid; hug/fill/fixed; min/max), constraints, layout grid geometry |
| L2 | `figcraft-text` | ~3.1k | Font database (optional embedded craft-fonts + system scan), shaping (harfrust), bidi, line layout, glyph outlines (skrifa), variable fonts |
| L2 | `figcraft-collab` | ~3.4k | Real-time collaboration core (no networking): synced object/property model, change sets, fractional positions, server ordering and validation, client reconciliation, wire messages |
| L2 | `figcraft-plugin` | ~1.0k | Plugin manifest and the WebAssembly sandbox (wasmi, metered, memory cap, no file system, network or clock) |
| L2 | `figcraft-pathops` | ~1.1k | Booleans (linesweeper), outline stroke with alignment, arrowheads |
| L3 | `figcraft-render` | ~5.4k | Scene → pixels on tiny-skia (CPU): isolated layers for opacity/blend/effects, clips, masks, shadows, blurs (uniform, progressive, background), noise, texture, glass, text; banded in parallel; retained rendering of only what changed; levels of detail when zoomed out |
| L3 | `figcraft-format` | ~5.2k | Native `.figcraft` (zip of `document.json` + images; streamed JSON), local library files, versions, REST API file JSON import (`rest/`) |
| L3 | `figcraft-project` | | UIGEN library project file `uigen.json` (model, validation, deterministic JSON, component names); pure, no disk access, compiles to wasm |
| L4 | `figcraft-svg` | ~3.7k | SVG export and import (usvg) |
| L4 | `figcraft-pdf` | ~2.3k | PDF export (krilla) with print options |
| L4 | `figcraft-vectordrawable` | ~1.2k | Android Vector Drawable (XML) export: baked path data, gradients through `aapt:attr`, clip groups, compatibility modes and warnings |
| L5 | `figcraft-engine` | ~39.3k | Sessions, undo history (`Arc` snapshots), the command registry (`cmd/*`, `spec!`), pointer tools and vector edit mode, snapping and measuring, components sync, libraries, prototype runtime, Dev Mode code generation, the menu catalog (`catalog.rs`) and the keymap (`keymap.rs`), a panic guard |
| L6 | `figcraft-ui-egui` | ~31.9k | The UI3-style frontend: canvas (render worker), toolbar, layers, Design/Prototype/Inspect panels, colour picker, menus, dialogs, presenter, comments, accessibility (AccessKit), the control channel server |
| L6 | `figcraft-mcp` | ~2.4k | MCP server (JSON-RPC over stdio), headless or bridged to a running app |
| L6 | `figcraft-capture` | | UIGEN capture service driver (adapter of the engine's `CaptureBackend` port, native only): runs the Node sidecar `sidecar/service.mjs` as a child process, speaks its JSON-lines protocol version 1 on a dedicated thread, with timeouts, size limits and readable errors; in the browser build every job is unavailable. Protocol: [`uigen/plan-import.md`](uigen/plan-import.md#service-de-capture) |
| L6 | `figcraft-collab-net` | ~3.0k | Live sessions over the network: the WebSocket transport (tungstenite, a reader and a writer thread per connection), the hub that runs a collaboration core server per open file for the session host the app embeds and for `figcraft-server` (behind a `Backend`), the joining and managing clients (publishing), invite links, presence relay and limits; the browser client; the only crate allowed networking dependencies |
| apps | `figcraft`, `figcraft-cli`, `figcraft-web`, `figcraft-server` | ~3.2k | Desktop app (eframe + wgpu), command line, browser build (WASM, WebGPU/WebGL2), collaboration server (files, share links, persistence; [`server.md`](server.md)) |
| plugins | `plugins/` (own workspace) | | Rust plugin SDK and two example plugins, built for wasm32 by `cargo xtask plugins` ([`plugins.md`](plugins.md)) |
| sidecar | `sidecar/` (Node, own `package.json`) | | UIGEN capture service, the one JavaScript exception to "Rust only": installs npm packages into a project's `.uigen/sources/` (always `--ignore-scripts`), lists their components, renders one in Chromium (Playwright, pipe mode) into a DOM snapshot and a PNG, and keeps the playground page of a project's Lit export open (`lit` bundles it, BSD-3-Clause). Opens no port, is never shipped to wasm; tested by `npm --prefix sidecar test` |
| tools | `xtask` | | `ci`, `layers`, `assets`, `parity`, `version`, `icon`, `wasm`, `plugins`, `bench` |

### The capture port

The engine (L5) never starts a process: it declares the port `CaptureBackend` (`submit`, `poll`, `shutdown`) and the
job and error types in `crates/engine/src/capture.rs`. `project.addSource`, `project.setSourceVersion` and
`project.removeSource` validate, check the project folder may be written, submit a job and return `{job}` at once;
`Session::pump_capture()` (every frame in the app, a wait loop in the CLI) applies the finished jobs, which writes
`uigen.json` and raises an event for the toast. `capture.status` lists the jobs. The desktop app and the CLI install
`figcraft_capture::CaptureService` into the `Session`; the browser build installs none and the commands answer
`capture service unavailable in the browser`. A stuck service is killed with its whole tree (`taskkill /T`, `kill -KILL`
on its process group). One that crashed alone leaves npm and Chromium behind: on Unix the process group still finds
them; on Windows the orphans are found by the parent id the dead service had (a PowerShell query), which fails only if
that id was reused first (a job object would need `unsafe`). A clean exit (code 0) is not swept: the service stops its
own children.

### Original references and applying a theme

An import keeps what it measured against: each variant carries `uigen.reference` (per theme, the path of the PNG under
`.uigen/captures/`, the size of the root box, a `distinct` flag and the text boxes with their fonts), the set carries
`uigen.importOptions`. The image stays on disk; there is no locked capture layer in the document.
`uigen.compare` (`engine/src/uigen_compare/`) renders the variant in its own box on a copy of the document (following the
Light or Dark mode), reads the PNG through `file_access` confined to `<project>/.uigen/captures` whether or not
automation roots are active (`measure::read_original`), and returns the pixel and text verdicts. Applying a theme is a
rebinding by name: `uigen.applyTheme` moves each variable binding of the library's components to the variable of the same
name and type in the chosen collection, in one undo step, then writes `appliedTheme` to `uigen.json`.

### The playground view

The Playground tab shows a live web page (the library project's Lit export and a model) without a second window: **the
view is made of relayed images**. The page lives in the capture service's Chromium; the app shows its images and sends
it the pointer and keyboard. Three roles, none of which knows the others' internals:

- **Engine (L5, `assets/playground/`, commands `playground.*`)** owns the state: the export being shown (a snapshot of
  the active tab's document, `stale` once it changes), the model, the settings and their own history, and the targets
  each setting drives. It plans the export with `lit_export::plan`, writes `<project>/.uigen/playground/` through
  `file_access`, composes the model into the library's tags, and hands the service `playground.*` calls through the
  same `CaptureBackend` port (`CaptureJob::Playground`), tracked by the playground and never listed by `capture.status`.
  `Session::playground_poll` asks for an image when none is on its way, `playground_take_frame` hands over the latest.
  The settings never touch the document: they become a token override sheet pushed to the page.
- **Interface (L6, `ui-egui/src/panels/playground*.rs`)** polls an image about thirty times a second (every 250 ms
  while the service is busy), draws it, and translates egui's pointer, wheel and keyboard into `playground.input`
  events. Top bar, settings bar, and the save and code windows act only through `app.run`.
- **Service (`sidecar/playground.mjs`, `sidecar/lit-bundle.mjs`)** bundles the exported package (esbuild in memory,
  `lit` from `sidecar/node_modules`, never installed in the project), keeps one page, swaps the text of two `<style>`
  elements to change the tokens without a reload, renders images (a CDP screencast says when the page changed; each image is a
  `Page.captureScreenshot`, JPEG quality 85, PNG on request) and applies the input. It reads staging files and style sheets; it does not know the document or the component names.

The page's isolation (offline context, remote requests blocked, no popups) is the one of `render`; the exported module
runs in Chromium only. Measured on the real service: a setting reaches the next image in about 34 ms (median over 20).
Limits: no clipboard, no text selection and no accessibility tree of the page; the browser build has no tab.
Protocol: [`uigen/plan-import.md`](uigen/plan-import.md#playground-lot-7).

### Translating a render into layers

A capture result is a DOM snapshot (version 1: boxes, computed styles, text runs, SVG markup, and since lot 3 the
`className`, the CSS variables behind a style (`vars`) and the running animations). The translation is split along the
layers so the pure part stays pure:

- `figcraft_dom::translate_snapshot` (L4, wasm-safe, never sees the engine) turns **one** snapshot into layers:
  frames with fills, borders, shadows, radii, clipping and opacity; flex containers into auto layout when the layout
  engine puts the children back on the browser's boxes (within 1 px); text in the installed font nearest the one
  Chromium used; SVG through `usvg`; empty wrappers dropped; unique sibling names. It returns `Translated {root,
  warnings, layers}` where each `LayerFacts` carries the layer's path, the values a token could stand for
  (`TokenFact`), the transition it declares (`TransitionFact`) and the effect it draws (`Effect::Ripple`,
  `Spinner`). What it cannot carry is a Dev Mode annotation on the layer, never an invented value.
- `figcraft-engine` (L5, `uigen_import/`) assembles an import from many translations, in one undo step
  (`Session::grouped("Import component")`): `plan` (axes, grid, one render per cell) → `pending` (the `inspect`
  job, then the renders, a single event, the first failure ends the import) → `assemble` (translate each variant,
  `component.create`, `component.combineVariants`, lay out the grid, `uigen.source`) → `tokens` (the collection
  `<package> <version>` with a mode per theme, `variable.bind` on every colour, radius, padding, gap, stroke and
  opacity) → `motion` (`ChangeTo` interactions between the `Default`, `Hover` and `Pressed` variants, ripple and
  spinner flags in `plugin_data`).

### Import pages, the inspection cache and the Components tab

A staged import (`uigen.importComponent {stage: true}`) lands in an **import page**, `- Imported · <package> <version>`,
of the file instead of the current page. The page is found again by `plugin_data["uigen.importPage"]`, never by its
name, and its name starts with a hyphen, so the library rule (page = component) never counts it and the Lit export
never sees it. The engine (L5) owns all of it: `uigen_import/staging.rs` (`import_page_name`, `is_import_page`,
`sections`) and the commands `uigen.imported` and `uigen.placeImported` (`cmd/uigen_place.rs`). Placing copies the whole
set onto a page of the library with `Document::duplicate_all_into` (new ids, nothing points back to the source), and a
page that already holds a component hands everything it holds to a new page right after it, in the same undo step.

`uigen.listComponents` runs `inspect` and keeps the validated, bounded answer **in the session**
(`uigen_import/catalog.rs`), by project, package and version; there is no disk cache. `pump_capture` files the result,
a finished `project.setSourceVersion` or `project.removeSource` forgets it, and `Session::inspected` is what the
interface reads.

The interface (L6) only shows and relays. The Components tab (`panels/assets_panel.rs`, `assets_panel/imported.rs`)
lists one "Imported · <package> <version>" section per source of the project, after the local styles and before the
enabled libraries, and a tile dropped on the canvas runs `uigen.placeImported`. The "Import a component" window
(`dialogs/import_component.rs`, desktop only) opens on the `dialog.importComponent` request of `uigen.importWindow` and
always imports with `stage: true`. Two files of one project show the same sections (the sources are the project's) but
each holds its own imported content.

## Data model

A `Document` holds pages, a persistent map of nodes (frames, groups, sections, rectangles,
ellipses, polygons, stars, lines, vector networks, text with styled runs, boolean groups,
components, component sets, instances, slices), styles, variable collections, comments, versions
and images by hash. Instances keep their main's id and per-layer overrides; the engine keeps their
materialised children in sync when a main changes (directly for value, size and add/remove edits,
by a rebuild otherwise). Snapshots are cheap (`Arc` and structural sharing), which gives unlimited
undo with one step per gesture.

## Commands: one path for every surface

Every user-visible behaviour is a command in `crates/engine/src/cmd/*` (455 ids after #101)
with an id, label, parameter docs and undo flag. The menus (generated from the catalog), keyboard
shortcuts, Actions palette, CLI (`figcraft-cli run`), JSON control channel (`--control <port>`) and
MCP server all dispatch the same ids, so agents can do anything a person can; WebAssembly plugins call them too, within their manifest's permissions. Commands never open
dialogs themselves; they request UI, and programmatic callers drop those requests.
`cargo xtask parity` measures the catalog's coverage ([`parity-checklist.md`](parity-checklist.md)).

## Rendering

The CPU renderer is the single source of pixels for the canvas, exports and the browser. The
desktop canvas renders on a worker thread at the device pixel ratio and keeps its last image,
re-rendering only the region a pan uncovers or an edit touches; overlays (selection, handles,
guides, labels) are drawn by egui on top. A seeded random walk checks the retained canvas against
full renders. SVG and PDF export walk the same scene and embed rasters where those formats can't
express an effect.

## File I/O

`.figcraft` is the native format (atomic saves, autosave checkpoints; the desktop app keeps
recovery copies of unsaved documents that reopen after a crash or a forced quit). Imports: REST API
file JSON with variables and images, SVG, raster images, clipboard
content. Exports: PNG, JPG, WebP, SVG, PDF, code. There is no `.fig` reader yet
([`file-format-parity.md`](file-format-parity.md)).

## Robustness

Workspace lints deny `unwrap`, `expect`, `panic!` and friends in shipped code; `unsafe_code` is
forbidden. `figcraft_engine::guard` turns a panicking command into an error and rolls the document
back; the desktop app also guards each UI frame. Junk-parameter sweeps run every command with bad
input.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-30 | minor | The playground view (UIGEN lot 7): the Playground tab shows images relayed from the capture service's Chromium and sends it the pointer and keyboard; the engine owns the state (`playground.*` commands, settings as token overrides, own history), the sidecar the page |
| 2026-10-30 | minor | UIGEN lot 4: import pages (`- Imported · <package> <version>`), the in-session inspection cache, the imported sections of the Components tab, dropping an imported component on the canvas and the "Import a component" window |
| 2026-10-18 | minor | Added `vectordrawable` (L4), the Android Vector Drawable (XML) writer |
| 2026-10-11 | minor | `collab-net` hosts through one hub for the app's sessions and the standalone server; added the `figcraft-server` app |
| 2026-10-10 | minor | Merged main after #101–#103: WASM plugins (plugins area 0% → 35% breadth, 20% ready; ecosystem 5% → 20%) and the multiplayer core without networking (multiplayer 5% → 15% breadth, 5% ready; collaboration 8% → 12%); menu breadth 97.5% → 98.5% (200/203); ready for real work ~46% → ~47%; full parity 420–790 h → 390–745 h; beta 210–380 h → 200–365 h. Fixed the stated weight sums (areas 1–16 sum to 94, total 113; percentages were already computed with those) |
| 2026-10-10 | major | Created from the code as of PR #99 |
