# Datum Mesh: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Mesh: finite-element analysis on the desktop (2D frames and trusses, plane stress and strain, heat conduction in 0.1; 3D, nonlinear and flow later) |
| **Current version** | 0.1.0 (2026-10-28) |
| **Repo** | `Edgar-Mendonca/Datum-Mesh` · branch `claude/upbeat-maxwell-xxve2b` (the first branch pushed, so the default branch) |
| **Stack** | Rust engine (`crates/core`, own solver), Tauri 2, SolidJS + strict TypeScript; no Python, no external solvers in 0.1 |
| **Identifier** | `com.edgarmendonca.datummesh` · files `.dmesh` (JSON, `format` 1) · installer `DatumMesh-x.y.z-setup.exe` + `SHA256SUMS.txt` |
| **Accent** | Crimson `#B83250` (`--blue2` `#9C2944`, `--sky` `#F1B3C1`, `--pale` `#FBEBEF`, `--sel` `#F6DCE3`, `--sel2` `#EDBAC7`; dark `#EF7A93`; dark File sidebar `#5E1A2A`) |
| **Brief** | `Edgar-Mendonca/Datum` › `briefs/DATUM-MESH.md` (hub session 13): every decision made up front |
| **Reference repos** | `Edgar-Mendonca/Datum` (FAMILY, MEMORY, brain), `Edgar-Mendonca/Datum-Plan` (reference code), `Edgar-Mendonca/Datum-Sim` (Plan's skeleton for a canvas app; solver crate layout and verification records), `Edgar-Mendonca/Datum-Atlas` (materials data and sources) |

---

## 1. Architecture

`crates/core` is the engine (pure Rust, no Tauri): the model and its edits, sections, and (from 0.1.1) elements, meshing, the solver, results and benchmarks. `src-tauri` is the shell: commands, files, options, recovery and the updater. `ui` is the window: it draws what the engine returns and sends edits as operations.

Units: the engine and the file use SI base units (m, N, Pa, K, W). The window converts for display in one place (`ui/src/units.ts`): SI shows mm, kN, MPa, °C; US shows in, kip, ksi, °F; frames show coordinates in m (ft), parts in mm (in).

### Files

| File | What it does |
| --- | --- |
| `crates/core/src/model.rs` | The model: analysis kind, nodes, members, sections, materials, shapes (polygons with bulge arcs, circles, holes), supports and loads with targets (node, member, edge, point, body), mesh settings; `check` (every invariant); `.dmesh` read item by item with migration and `repair` (damaged files open as far as possible, with warnings) and write |
| `crates/core/src/doc.rs` | Edit operations (Add, Set, AddMember by points, Move, Delete with cascade, SetSettings, SetMeta, Batch) on a copy, kept only if `check` passes; snapshot undo/redo |
| `crates/core/src/sections.rs` | Section properties (A, I, c) for rectangle, round bar, tube, box, I, channel, angle and custom |
| `crates/core/src/examples.rs` | The worked examples, built in code: blank frame and part, portal frame, roof truss, lifting lug, L-bracket, flange plate, heat sink |
| `crates/core/src/hardening.rs` | FAMILY §11: failed edits change nothing, random edits keep the model valid and round-trip, damaged files, speed gate |
| `crates/core/examples/dump.rs` | Writes `tests_ui/fixture.json` (the engine's real examples) for the UI tests |
| `src-tauri/src/main.rs` | Commands: state, apply, history, examples, new model, section properties, open/save, save file, options, recovery, recent, startup file, open_url, version, updates |
| `src-tauri/src/update.rs` | The updater (Plan's, `REPO` = Datum-Mesh) |
| `ui/src/main.tsx` | Window layout, sidebar (Model, Mesh, Results, Report), status bar (tool hint, cursor, counts, drawing scale), keys, start-up and recovery |
| `ui/src/store.ts` | Window state: the model (`out().m`), selection, picked edge, canvas tool, menus, dialogs |
| `ui/src/api.ts` | Shell calls and the model's shapes in TypeScript |
| `ui/src/units.ts` | Display units (SI/US) and conversions |
| `ui/src/geom.ts` | Arcs from bulges, edge points, normals, SVG paths, hit tests |
| `ui/src/canvas.tsx` | The canvas: pan, zoom, adaptive grid, rulers, snap (grid, nodes, corners), select/marquee/drag-move, draw nodes, beams, truss bars, rectangles, circles, polygons and holes, pick edges, place supports and loads; support and load symbols |
| `ui/src/panels.tsx` | The model tree and the Inspector (node, member, shape with its corners and arcs, edge, support, load, model settings); default support and load values |
| `ui/src/ribbon.tsx` | Title bar, quick access, Tell Me, the ribbon (Home, Geometry, Mesh, Loads, Solve, Results, View, Help, contextual Format); planned tools dimmed with their release |
| `ui/src/cmds.ts` | Every command (ribbon, menus, keys, Tell Me) |
| `ui/src/dialogs.tsx` | Materials, sections, node at coordinates, shortcuts, getting started, glossary, recovery, close |
| `ui/src/backstage.tsx` | File page: examples, recent, export, options, updates |
| `ui/src/print.tsx` | Print: the model drawn to fit the page with a title block, and a model report |
| `ui/src/help.tsx` | The `?` cards |
| `ui/src/roadmap.tsx` | File › Roadmap from `docs/site.json` |
| `ui/src/styles.css`, `ui/src/ui.tsx`, `ui/index.html` | Plan's styles (crimson accent) and components; the icon sprite |
| `tests_ui/run.cjs` | 40 UI checks of the built window with a mocked bridge |
| `tests_ui/realapp.sh` | Screenshots of the real app under Xvfb |
| `docs/PLAN-0.1.md`, `docs/site.json`, `CHANGELOG.md` | The plan, the public summary and roadmap, the change log |

### Release flow

`release.yml` (windows-latest): checks a pushed tag matches the version, takes the notes from CHANGELOG, runs the engine tests, builds the NSIS installer, writes `SHA256SUMS.txt` and publishes the GitHub Release. Run it from the Actions tab (tag pushes are blocked from Claude sessions).

### How we work

1. The user reviews a release and sends screenshots.
2. Reproduce first.
3. Big asks get a plan file and questions; build after the answers.
4. Engine first (with a test), then the UI.
5. Check every change in the real app (Xvfb screenshots, `tests_ui/realapp.sh`).
6. Add a UI test for every bug.
7. Run all checks: clippy `-D warnings`, `cargo test` (with the hardening tests and benchmarks), the UI type-check and UI tests, `cargo deny check`.
8. Release.
9. Report in plain language: what changed, what was checked, what is not done.

## 2. Decisions

| Version | Decision | Why |
| --- | --- | --- |
| plan | Everything in the brief §1 (name, crimson accent, identifier, `.dmesh`, stack, own Rust solver, 12-band blue-to-red contours with viridis in Options, ribbon tabs, logo) | Made with the user in the hub, session 13 |
| plan | The shell and window are copied from Datum Sim, which is Plan's skeleton already adapted to a canvas app, without Sim's Python and run thread | The brief says copy, don't reinvent; Sim's copy is the closest to a drawing app |
| 0.1.0 | Model in SI base units; display units in one window module; frames show m (ft), parts mm (in) | One owner for conversions; no rounding when the user switches units |
| 0.1.0 | One id counter for every item; supports and loads point at a `Target` (node, member, edge of a shape, point, body) | Edits and deletes cascade simply; one support or load type for frames and parts |
| 0.1.0 | Plane geometry as polygons whose segments may bulge into arcs (bulge = tan(θ/4), as in DXF), plus circles; holes are shapes with `hole: true` | Fillets, rounded lugs and slots without a CAD kernel; maps directly to DXF import in 0.2 |
| 0.1.0 | Every edit is checked by `Model::check` on a copy and refused if it breaks anything; the reader repairs damaged files item by item and lists what it dropped | FAMILY §11: failed edits change nothing, damaged files open as far as possible |
| 0.1.0 | `serde_json` with `float_roundtrip` | The hardening test found coordinates changing in the last digit after save and open |
| 0.1.0 | Atlas has no Poisson's ratio: Mesh's library (0.1.3) adds ν from typical values with its own source | Atlas data checked in session 1 |
| 0.1.0 | Nodes take clicks before the load arrows drawn on them | The UI test found the wind load's arrows on the portal frame's eave node stealing the drag |

## 3. Roadmap

✅ done · 🔜 next · ⬜ later

| Version | Item | Status |
| --- | --- | --- |
| plan | Read the hub, Plan, Sim and Atlas; PLAN-0.1, MEMORY, CLAUDE, icon | ✅ |
| 0.1.0 | Shell, updater, Home, File page, the canvas that saves and opens; model, edits, sections, examples, hardening tests; CI and release | ✅ |
| 0.1.1 | Frames and trusses end to end: beam and truss elements, sparse assembly and Cholesky (faer), member loads, self-weight, reactions, equilibrium check, diagrams, member stresses; benchmarks 1–3 | 🔜 |
| 0.1.2 | Meshing (spade CDT + refinement) and T6/T3 plane stress and strain; benchmarks 4–6 and patch tests; contours, probe, deformed shape | ⬜ |
| 0.1.3 | Heat conduction (benchmark 7), materials library from Atlas, results report, verification report, convention build | ⬜ |
| 0.1.4 | Fixes from a clean Windows PC, speed on large meshes, README and screenshots | ⬜ |
| 0.2 | Axisymmetric, 3D frames, modal, buckling, DXF import, thermal stress, Send to Calc and Plot | ⬜ |
| 0.3–1.0 | 3D solids, nonlinear, CFD, multiphysics (open solvers as separate programs), validation report, signed installer | ⬜ |

## 4. Session log

### 2026-10-28: Session 1 (day 1: plan, skeleton, 0.1.0)
- Read the brief, the hub's CLAUDE, MEMORY, FAMILY and brand; Plan's and Sim's code; Atlas's data (31 metals with sources; no Poisson's ratio).
- Wrote PLAN-0.1, CLAUDE.md, this memory, `docs/site.json`, README, CHANGELOG, NOTICE, SECURITY, CONTRIBUTING; drew the icon (L-bracket mesh with a darker hot spot, amber diamond).
- Engine: model, check, repair, edits with undo, sections, six examples; 14 tests including hardening. The hardening tests found two real faults on their first run (float round-trip in the file; the id counter going backwards on repair).
- Shell from Sim without Python; window: ribbon, tree, canvas, Inspector, dialogs, print, help.
- UI tests found two more (nodes under load arrows could not be dragged; arcs were drawn mirrored on screen) and now guard them.
- Checked: clippy, `cargo test`, UI type-check and 40 UI checks, `cargo deny`, screenshots in light and dark.

## 5. Known limits (0.1.0)

- Nothing solves yet: frames arrive in 0.1.1, parts in 0.1.2.
- No copy and paste of geometry; members do not split where they cross.
- Overlapping solid shapes join only when meshed (0.1.2); the canvas draws them overlapping.
