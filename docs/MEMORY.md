# Datum Mesh: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Mesh: finite-element analysis on the desktop (2D frames and trusses, plane stress and strain, heat conduction in 0.1; 3D, nonlinear and flow later) |
| **Current version** | 0.1.4 (2026-10-29) |
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
| `crates/core/src/doc.rs` | Edit operations (Add, Set, AddMember by points, Move, Delete with cascade, InsertCorner, RemoveCorner, JoinCrossings, SetSettings, SetMeta, Batch) on a copy, kept only if `check` passes; snapshot undo/redo; changed = differs from the saved snapshot |
| `crates/core/src/sections.rs` | Section properties (A, I, c) for rectangle, round bar, tube, box, I, channel, angle and custom |
| `crates/core/src/sparse.rs` | The shared linear solver: triplets, supports by elimination, prescribed values, faer sparse Cholesky, a probe solve that catches mechanisms, residual |
| `crates/core/src/mesh.rs` | Meshing parts: outlines (arcs split by the local size) into a spade constrained Delaunay triangulation inside a frame round the part, refined for angle and size; local sizes from refined edges and points grow into the global size (a size field: too-large triangles get a point at their centre, then refinement again); triangles kept where there is material (centroid test); 6-node triangles with midside nodes on true arcs; boundary edges tagged with their shape edge; quality |
| `crates/core/src/plane.rs` | Plane stress and plane strain: T3/T6 elements, edge pressure (× thickness) and edge forces (whole force or per length, for the whole thickness), gravity, point loads, fixed/pinned/roller/symmetry supports (rollers on sloping or curved edges by a stiff spring along the normal), reactions, the equilibrium check (forces, and moments against forces × reach), stresses recovered and averaged at nodes, per-element values, von Mises, principals, safety factor |
| `crates/core/src/heat.rs` | Steady heat conduction: T3/T6, fixed temperatures, flux, convection, generation; heat flux recovered at nodes and per element; heat flows at every support and load and the heat balance |
| `crates/core/src/materials.rs`, `crates/core/data/materials.json`, `tools/materials.py` | The materials library: 54 metals and plastics from Datum Atlas with sources (E, ρ, k, α the middle of the range, yield the low end, ν typical for the family); the script rebuilds the JSON from an Atlas checkout |
| `crates/core/src/lib.rs` | `Results` (frame, plane or heat, tagged by `kind`) and `solve()` for any analysis; tests that every example solves, balances and agrees with hand checks |
| `crates/core/src/wire.rs` | Results packed for the window: long numeric arrays as base64 (f32 values, u32 indices), everything else unchanged; the window's `unwire` (`ui/src/api.ts`) unpacks them |
| `crates/core/examples/bigpart.rs` | A 25 000-element bracket's results as JSON, for timing the window by hand (not a test) |
| `crates/core/src/frame.rs` | Frames and trusses: beam and truss stiffness, linear member loads as consistent nodal loads, self-weight, springs, reactions, equilibrium sums, exact N, V, M and deflection along each member (41 stations), stress and utilisation |
| `crates/core/src/bench.rs` | The verification benchmarks as data (`run()` → rows with result, reference, error, source); the test fails on any row over its tolerance |
| `crates/core/examples/verify.rs` | Writes the dated verification record (`docs/verification/x.y.z.md`) |
| `crates/core/src/examples.rs` | The worked examples, built in code: blank frame and part, portal frame, roof truss, lifting lug, L-bracket, flange plate, heat sink |
| `crates/core/src/hardening.rs` | FAMILY §11: failed edits change nothing, random edits keep the model valid and round-trip, damaged files, speed gate |
| `crates/core/examples/dump.rs` | Writes `tests_ui/fixture.json` (the engine's real examples) for the UI tests |
| `src-tauri/src/main.rs` | Files written safely (temporary file, sync, rename). Commands: state, apply, history, examples, new model, section properties, open/save, save file, options, recovery, recent, startup file, open_url, version, updates |
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
| `ui/src/parts.tsx` | Part results: fields and colour scales (12 bands, rainbow or viridis, safety factor reversed), band paths built once in model coordinates (isobands by clipping) and painted on a canvas under the drawing with one transform, the deformed shape, mesh lines, min/max markers, the probe (grid of drawing triangles), the legend, the Mesh view bar (quality colours), the part Results page, contour images for print |
| `ui/src/results.tsx` | Frame results over the canvas (deflected shape, diagrams, reactions, utilisation, probe), the legend bar, and View › Results (equilibrium, reactions, members, solver) |
| `ui/src/print.tsx` | Print: the model drawn to fit the page with a title block, and a report with the results and the equilibrium check |
| `ui/src/help.tsx` | The `?` cards |
| `ui/src/roadmap.tsx` | File › Roadmap from `docs/site.json` |
| `ui/src/styles.css`, `ui/src/ui.tsx`, `ui/index.html` | Plan's styles (crimson accent) and components; the icon sprite |
| `tests_ui/run.cjs` | 93 UI checks of the built window with a mocked bridge (results in the fixture packed as the shell sends them) |
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
| 0.1.1 | faer 0.24 (MIT) sparse Cholesky with its own fill-reducing ordering, no default features (no rayon, rand, npy) | The brief's choice; 200 000 equations factor in 0.1 s here |
| 0.1.1 | A probe solve (a load equal to the diagonal) alongside every solve; an answer over 1e11 means a mechanism | faer factored an exactly singular matrix when rounding left a tiny positive pivot (found by the spring-chain test) |
| 0.1.1 | Member results are integrated exactly (with linear loads M is a cubic): deflection v = v_a + θ_a s + ∫∫M/EI, stretch from N/EA | Step-by-step integration missed the mid-member deflection by 5e-4 (found by benchmark 2); exact now to 1e-9 |
| 0.1.1 | Distributed loads in global x or y are per length of the member, not projected | The simplest rule to explain; stated in the help and the Inspector |
| 0.1.1 | Moments drawn on the tension side; N and V on the member's local +y | Common practice for moments; one fixed rule for the others, stated in the legend |
| 0.1.1 | After an edit results stay drawn but faded and marked out of date; Solve is F5 (no automatic re-solve) | Clear about what the picture shows; solving is instant for frames, but meshed parts will not be |
| 0.1.2 | A fixes release after a review of 0.1.1 (the user's request): 13 findings, each with a test; meshing moves to 0.1.3, heat/materials/report to 0.1.4, polish to 0.1.5 | The user: "make 0.1.2 a fixes release for 1–13" |
| 0.1.2 | Every edit joins geometry: added or moved nodes merge into a node already there and split members they land on; added or moved members are split at nodes along them (distributed loads split linearly); `Model::check` refuses coincident nodes; repair merges them in old files | A member drawn to the middle of another was not connected (misleading "mechanism" error, or a column carrying nothing); arrow-key moves could stack nodes |
| 0.1.2 | Corners are added and removed by engine ops (`InsertCorner`, `RemoveCorner`) that renumber edge targets; a split edge's loads cover both halves (a total edge force is shared); a corner in an arc splits the arc | The window's own corner editing moved loads to the wrong edge |
| 0.1.2 | The equilibrium check is judged against the size of the loads (forces summed without signs; moments about the origin likewise), forces and moments both; computed in the engine (`imbalance`) | Dividing by the net force gave 6.8e19 % for a moment alone |
| 0.1.2 | Mechanism probe limit 1e15 plus two steps of iterative refinement in every solve | A cantilever in 500 pieces was called a mechanism (probe ≈ 3n⁴); at 1 000 pieces rounding cost 6e-5, refinement recovers 1e-9 |
| 0.1.2 | Results drawing reads the view inside JSX or memos only | Diagrams and utilisation lines were computed once and stayed behind after pan or zoom |
| 0.1.2 | Display values below 1e-9 of their scale are shown as 0 (`clean`); typed numbers go through `parseNum` ("−", "1,5", "1,200", empty refused) | Noise like 1.5e-14 kN·m; negative corners could not be typed; empty became 0 |
| 0.1.2 | Switching units picks a round grid step in the new units (`niceGrid`); member lengths use the drawing's unit | 0.5 m showed as 1.6404 ft |
| 0.1.2 | Open, New, an example or a recent file ask to save unsaved changes first | Work was lost without a question |
| 0.1.3 | The user asked for 0.1.4's work (heat, materials library, report) inside 0.1.3: one release with meshing, plane stress and strain, heat, the library and the report; polish becomes 0.1.4 | The user: "carry on with 0.1.3 and add 0.1.4 part too in 0.1.3" |
| 0.1.3 | A frame of four points round the part before triangulating; minimum area for refinement from the finest local size | Near-collinear points on a sloping edge made hull slivers spade could never refine; a minimum area from the global size skipped the small triangles near a refined hole |
| 0.1.3 | Local sizes are a size field (local size + 0.2 × distance, capped by the global size), applied by inserting centroids of too-large triangles and refining again | Refining only the points on an edge let the mesh coarsen within one element of a hole; the plate-with-a-hole error jumped about (1 %, 0.1 %, 0.4 %) instead of falling. Now 1.1 % → 0.5 % → 0.1 % → 0.04 % |
| 0.1.3 | Benchmark 4 is a quarter plate 200 hole diameters wide with the hole's edge refined, against Kirsch's 3.0 | A plate 20 diameters wide converged to 3.018 (its own finite-size answer), not 3.0 |
| 0.1.3 | Pressure acts per area (× thickness); an edge force is per length of edge for the whole thickness, or the whole force | Found in the app: the L-bracket's 1.5 kN edge force was multiplied by the 10 mm thickness, giving 2.9 MPa instead of 287 MPa; now a test, and a hand check of the bracket |
| 0.1.3 | Stresses: nodal averages (smoothed) and each element's own average (per element), both sent to the window | Comparing the two shows where the mesh is too coarse, without a separate error estimate |
| 0.1.3 | Contours as 12 band paths (Path2D) in model coordinates on a canvas under the SVG; polygons closed with lineTo, never closePath | closePath on 100 000 sub-paths took 10 s in Chromium; now 0.2 s to switch result, 0.8 s to draw 25 000 elements |
| 0.1.3 | The safety factor's colours run red (low) to blue, capped at 10; nodes with no yield strength are grey | A factor of 10⁶ in a lightly loaded corner would otherwise squash the scale |
| 0.1.3 | Moments in the equilibrium check of parts are judged against forces × the model's reach (as frames) | A bore pressure centred on the origin has no moment of its own: the flange showed a 110 % "imbalance" of rounding noise |
| 0.1.3 | `Model::check` looks members' nodes up in a map | Each edit of a 10 000-member frame took 52 ms (a linear search per member); now 14 ms |
| 0.1.4 | Long result arrays go to the window as base64 f32/u32 (`wire.rs`); drawing triangles are rebuilt in the window | 22 MB of JSON for 25 000 elements; now 6.2 MB. Seven significant digits are more than any display shows; equilibrium sums stay in full precision |
| 0.1.4 | A pin load (`Bearing`): pressure p₀ max(0, −n·F̂) on the edge, scaled so the push along F̂ is the whole force | The lug's pin was an even traction all round the hole, pulling on the far side too |
| 0.1.4 | Crossing members join only on request (Format › Join Where They Cross, `Op::JoinCrossings`) | Automatic joining would connect X-bracing that passes without a joint |
| 0.1.4 | The document is "changed" when it differs from the saved snapshot; saving writes a temp file then renames | 0.1.1 review: undo to the saved state stayed changed; a crash mid-save could corrupt the file |
| 0.1.4 | A drag snaps the grabbed point (node, member start, first corner) to the grid, not the offset | 0.1.1 review: items off the grid stayed off it |
| 0.1.4 | Each top-level setting is read on its own; one that cannot be read falls back to its default with a warning | 0.1.1 review: an unknown analysis or a negative counter refused the whole file |
| 0.1.1 | Help › Verification runs `bench::run()` in the app, so the report is always true for the version running; the same rows are tests and the dated record | The brief's headline feature, earlier than planned because it was cheap |

## 3. Roadmap

✅ done · 🔜 next · ⬜ later

| Version | Item | Status |
| --- | --- | --- |
| plan | Read the hub, Plan, Sim and Atlas; PLAN-0.1, MEMORY, CLAUDE, icon | ✅ |
| 0.1.0 | Shell, updater, Home, File page, the canvas that saves and opens; model, edits, sections, examples, hardening tests; CI and release | ✅ |
| 0.1.1 | Frames and trusses end to end: beam and truss elements, sparse assembly and Cholesky (faer), member loads, self-weight, reactions, equilibrium check, diagrams, member stresses; benchmarks 1–3; Help › Verification | ✅ |
| 0.1.2 | Fixes release: the 13 findings of the 0.1.1 review (see Decisions) | ✅ |
| 0.1.3 | Meshing (spade CDT + refinement, graded local sizes), T6/T3 plane stress and strain, steady heat conduction; contours, probe, deformed shape, mesh view; materials library from Atlas; results report; benchmarks 4–8 with convergence (27 checks) | ✅ |
| 0.1.4 | Polish: packed result transfer (22 → 6.2 MB), pin load, Join Where They Cross, the 0.1.1 review's low findings (safe save, undo clears changed, autosave interval, arrows follow drags, drag snaps position, print units and moment row, damaged settings) | ✅ |
| 0.1.5 | Fixes from first use on a clean Windows PC; a user guide in the app's help | 🔜 |
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

- Released 0.1.0 (Release run 1 from the Actions tab). CI run 1 failed on a type error made after the last type check, run 2 on a clippy lint newer than the local toolchain; both fixed, CI green on Linux, Windows and macOS from run 3.

### 2026-10-28: Session 1, continued (day 2 brought forward: 0.1.1)
- Engine: `sparse.rs` (faer), `frame.rs`, `bench.rs` with benchmarks 1–3 (12 checks, all within 1e-9), solver hardening (random frames always balance and their curves meet their nodes; a 60 × 60 grid frame solves inside the speed gate).
- Found by tests: faer accepted a singular matrix (now a probe solve), mid-member deflection off by 5e-4 (now exact), and my own wrong reference for a propped cantilever (the engine was right: 11qL/40).
- Window: Solve (F5), results over the canvas, legend bar, probe, View › Results, Help › Verification, results in the printed report. A support symbol that kept its old position after a fit (props read outside a reactive scope) was fixed and found by screenshot.
- Checked: clippy, 21 engine tests, 52 UI checks, cargo deny, screenshots.

### 2026-10-28: Session 1, continued (review and the 0.1.2 fixes release)
- Released 0.1.1. The user asked to stop and analyse the app. Two passes: an independent code review by a separate agent (it confirmed the solver maths numerically and found 15 issues in the checks, editing and window layers) and my own use of the real app under Xvfb (found the misplaced diagram after a refit, the "× 2e+299" no-load scale, rounding noise, the metric grid in US units, the broken empty card).
- The user chose a fixes release for findings 1–13. Engine: joining and merging on every edit, corner ops, equilibrium scale and moment check, loose-node and no-load warnings, mechanism limit and iterative refinement (27 tests). Window: reactive results drawing, save prompt, no-load legend, `parseNum`, `clean`, `niceGrid`, the empty card (65 UI checks, 13 new). Rechecked in the real app.
- The unreleased 0.1.3 meshing work (mesh.rs with spade, plane.rs with T3/T6) is parked in `git stash` ("WIP 0.1.2: mesh.rs, plane.rs"); its plane tests passed except a roller-on-a-slope case that hit the refinement vertex limit.
- Not fixed (low, from the review): save is not atomic (temp file + rename), undo to the saved state still marks the model changed, the autosave interval needs a restart, load arrows do not follow a drag, drag snaps the offset not the position, the print title block says mm for frames and its equilibrium table has no moment row, a few damaged scalars (negative `next`, unknown `analysis`) still refuse the whole file.

### 2026-10-28: Session 1, continued (0.1.3: parts, heat, materials, report)
- The user asked for 0.1.3 with 0.1.4's work in it. Restored the stashed mesh.rs and plane.rs; fixed the sloping-roller case (a frame round the part); found and fixed a slow check that made edits of large frames take 52 ms each.
- Engine: benchmarks 4–8 (Kirsch, Lamé, NAFEMS LE1 with convergence series; wall and fin; patch tests), heat.rs, materials.rs (54 materials from Atlas), one `solve()` for every analysis. Building benchmark 4 found that local refinement did not grade (now a size field) and that the minimum refinement area hid small triangles. Random parts and a 50 000-node speed gate in the hardening tests (42 engine tests).
- Window: parts.tsx (contours, markers, probe, legend, Mesh view, results page), ribbon Mesh and Results tabs live, Refine Near tool, mesh settings in the Inspector and tree, the library dialog, convergence tables in Help › Verification, the Report view and the part report with contour pictures.
- Found in the real app: the edge-force thickness bug (2.9 MPa instead of 287 MPa in the L-bracket) and 25 s to draw a 25 000-element result (Path2D.closePath); both fixed, the first with an engine test and a hand check.
- Checked: clippy, 42 engine tests, 86 UI checks, cargo deny, real app (bracket, heat sink, mesh view, results, report, a 25 000-element bracket: 103 000 equations in 1.5 s in a debug build).

### 2026-10-29: Session 1, continued (0.1.4: polish)
- The user: "next is 0.1.4 proceed". Scope chosen from the open lists: result transfer size, the 0.1.1 review's low findings, and two known limits (the lug's pin, crossing members).
- Engine: `wire.rs` (packed results, round-trip test), `Bearing` load with a test and a lug hand check, `JoinCrossings` with a test (split loads), saved snapshot for the changed mark (test), settings read one by one (hardening test). 48 engine tests, plus a shell test for safe saving.
- Window: unpacking, pin load tool and drawing, Join Where They Cross, drags land on the grid and arrows follow them, autosave interval re-armed, print title block units and equilibrium with moments, min/max labels kept apart. 93 UI checks.
- Checked: clippy, tests, UI checks, cargo deny, real app (lug with its pin load solved). Timing: a 25 000-element result now arrives as 6.2 MB.
- Not checked: a clean Windows PC (no Windows machine here; CI builds and tests on Windows).

## 5. Known limits (0.1.4)

- Not yet tried on a clean Windows PC by a person (0.1.5).
- Stresses at the corner where a fixed edge meets a free edge, or at a sharp inside corner, keep rising with mesh refinement (a singularity); the help says so, the app does not mark them.
- No thermal stress (0.2); heat and stress are separate models.
- NAFEMS LE1's error is flat between the first two meshes (0.24 %) before falling to 0.09 %.
- A pin load on an edge split in two (a corner added in it) is shared half and half, each half bearing on its own; on a circle hole it is one edge, so this rarely matters.
- No member end releases (hinges inside a frame), no buckling check of members in compression.
- No copy and paste of geometry.
- Overlapping solid shapes join only when meshed (0.1.2); the canvas draws them overlapping.
