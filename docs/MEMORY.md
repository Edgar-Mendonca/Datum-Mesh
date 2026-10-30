# Datum Mesh: project memory

> **Paste this file at the start of each new session**, or just open the repo: `CLAUDE.md` tells Claude to read it.
> It records what exists, why, and what comes next. Update it at the end of every session.

| | |
| --- | --- |
| **Product** | Datum Mesh: finite-element analysis on the desktop (2D frames and trusses, plane stress and strain, heat conduction in 0.1; 3D, nonlinear and flow later) |
| **Current version** | 0.3.0 (2026-10-29) |
| **Repo** | `Edgar-Mendonca/Datum-Mesh` · branch `claude/upbeat-maxwell-xxve2b` (the first branch pushed, so the default branch) |
| **Stack** | Rust engine (`crates/core`, own solver), Tauri 2, SolidJS + strict TypeScript; no Python, no external solvers in 0.1 |
| **Identifier** | `com.edgarmendonca.datummesh` · files `.dmesh` (JSON, `format` 1) · installer `DatumMesh-x.y.z-setup.exe` + `SHA256SUMS.txt` |
| **Accent** | Crimson `#B83250` (`--blue2` `#9C2944`, `--sky` `#F1B3C1`, `--pale` `#FBEBEF`, `--sel` `#F6DCE3`, `--sel2` `#EDBAC7`; dark `#EF7A93`; dark File sidebar `#5E1A2A`) |
| **Brief** | `Edgar-Mendonca/Datum` › `briefs/DATUM-MESH.md` (hub session 13): every decision made up front |
| **Reference repos** | `Edgar-Mendonca/Datum` (FAMILY, MEMORY, brain), `Edgar-Mendonca/Datum-Plan` (reference code), `Edgar-Mendonca/Datum-Sim` (Plan's skeleton for a canvas app; solver crate layout and verification records), `Edgar-Mendonca/Datum-Atlas` (materials data and sources) |

## Purpose (agreed 2026-10-29)

To show any engineer or student how a part or structure carries load, heat and vibration (where it is stressed, how far it moves, how hot it gets) in a free app they can trust because it proves its own answers.

- **For:** mechanical, aerospace and process engineers checking parts (brackets, lugs, vessels, shafts, heat sinks); students and lecturers learning FEA; small firms without a commercial licence.
- **Different because:** trust you can see (benchmarks solved in the app, the equilibrium check, saying what was not verified); free and open (MIT, no licence server); simple (draw, load, solve, read); honest about its limits.
- **Frames stay in Datum Mesh** (2D and 3D frames and trusses, with frequencies and buckling): they carry the parts engineers check and teach FEA well. They do **not** grow into building design: no design-code checks or code load combinations.
- **Not:** a design-code checker, a CAD program, or a whole commercial suite on day one.
- **In the spirit of** (agreed 2026-10-30): the analysis side of a structural program for frames, and a general finite-element program for parts; not CAD, not a pre-processor for other solvers. Simple parts are drawn here (2D sketch tools, extrude, revolve); complex ones come from real CAD as STEP (0.4). Plan: `docs/PLAN-0.3-0.4.md`.
- **Leads to:** 3D parts (0.3: extruded and revolved, then any shape from CAD), beyond linear (0.4: yielding, large deflection, contact, through open solvers run as separate programs), heat and flow (0.5), validated 1.0 (validation report, user guide, signed installer).

---

## 1. Architecture

`crates/core` is the engine (pure Rust, no Tauri): the model and its edits, sections, and (from 0.1.1) elements, meshing, the solver, results and benchmarks. `src-tauri` is the shell: commands, files, options, recovery and the updater. `ui` is the window: it draws what the engine returns and sends edits as operations.

Units: the engine and the file use SI base units (m, N, Pa, K, W). The window converts for display in one place (`ui/src/units.ts`): SI shows mm, kN, MPa, °C; US shows in, kip, ksi, °F; frames show coordinates in m (ft), parts in mm (in).

### Files

| File | What it does |
| --- | --- |
| `crates/core/src/model.rs` | The model: analysis kind, nodes, members, sections, materials, shapes (polygons with bulge arcs, circles, holes), supports and loads with targets (node, member, edge, point, body), mesh settings; `check` (every invariant); `.dmesh` read item by item with migration and `repair` (damaged files open as far as possible, with warnings) and write |
| `crates/core/src/doc.rs` | Edit operations (Add, Set, AddMember by points, Move, Delete with cascade, InsertCorner, RemoveCorner, JoinCrossings, Fillet, Chamfer, Combine, Offset, Pattern, Make, SetSettings, SetMeta, Batch; `retarget` moves supports, loads and local sizes to new edges) on a copy, kept only if `check` passes; snapshot undo/redo; changed = differs from the saved snapshot |
| `crates/core/src/sketch.rs` | Sketch tools (0.3.1): `corner` (fillet or chamfer), `combine` (i_overlay on short pieces, each result edge traced back to the line or circle it lies on so arcs come back as arcs, corners where the exact curves meet, the input edges under each new edge for moving supports and loads), `offset`, `make` (slot, rounded rectangle, regular polygon), `turn` |
| `ui/src/sketch.tsx` | The sketch tools' dialogs and the Combine buttons (selection order: first picked is kept) |
| `crates/core/src/cases.rs` | Load cases and combinations (0.3.3): `Which`, `select` (a case's loads, or a combination's scaled), `label`, `envelope` (every combination solved, member extremes and reactions) |
| `crates/core/src/release.rs` | Hinges by static condensation: `Release<N>` (T, Tᵀ K T, Tᵀ f, the released rotations recovered) shared by frame.rs and space.rs |
| `crates/core/src/steel.rs` | IPE, HEA, HEB tables (EN 10365 dimensions with root radii and table areas) |
| `ui/src/cases.tsx` | The case picker, the cases and combinations dialog, the envelope, the section tables dialog |
| `crates/core/src/dxf.rs` | DXF (0.3.5): `read` (entities, units, chaining loose pieces into rings, holes by nesting depth, arcs over a half turn split) and `write` (closed LWPOLYLINEs and CIRCLEs in mm); `SAMPLE` drawing for tests |
| `crates/core/src/sections.rs` | Section properties (A, I, c) for rectangle, round bar, tube, box, I, channel, angle and custom |
| `crates/core/src/sparse.rs` | The shared linear solver: triplets, supports by elimination, prescribed values, faer sparse Cholesky, a probe solve that catches mechanisms, residual |
| `crates/core/src/mesh.rs` | Meshing parts: outlines (arcs split by the local size) into a spade constrained Delaunay triangulation inside a frame round the part, refined for angle and size; local sizes from refined edges and points grow into the global size (a size field: too-large triangles get a point at their centre, then refinement again); triangles kept where there is material (centroid test); 6-node triangles with midside nodes on true arcs; boundary edges tagged with their shape edge; quality |
| `crates/core/src/plane.rs` | Plane stress and plane strain: T3/T6 elements, edge pressure (× thickness) and edge forces (whole force or per length, for the whole thickness), gravity, point loads, fixed/pinned/roller/symmetry supports (rollers on sloping or curved edges by a stiff spring along the normal), reactions, the equilibrium check (forces, and moments against forces × reach), stresses recovered and averaged at nodes, per-element values, von Mises, principals, safety factor |
| `crates/core/src/heat.rs` | Steady heat conduction: T3/T6, fixed temperatures, flux, convection, generation; heat flux recovered at nodes and per element; heat flows at every support and load and the heat balance |
| `crates/core/src/materials.rs`, `crates/core/data/materials.json`, `tools/materials.py` | The materials library: 54 metals and plastics from Datum Atlas with sources (E, ρ, k, α the middle of the range, yield the low end, ν typical for the family); the script rebuilds the JSON from an Atlas checkout |
| `crates/core/src/lib.rs` | `Results` (frame, plane or heat, tagged by `kind`) and `solve()` for any analysis; tests that every example solves, balances and agrees with hand checks |
| `crates/core/src/wire.rs` | Results packed for the window: long numeric arrays as base64 (f32 values, u32 indices), everything else unchanged; the window's `unwire` (`ui/src/api.ts`) unpacks them |
| `crates/core/examples/bigpart.rs` | A 25 000-element bracket's results as JSON, for timing the window by hand (not a test) |
| `crates/core/src/solid.rs` | 3D solids (0.3.0; revolved 0.3.2; heat and frequencies 0.3.4: `heat`, `modes`, `build` shared by the static solve and the modes): the section's 2D mesh swept into layers (`Swept`, with `map`/`pos`: a revolved part's axis nodes are one node on every level and a full turn's last level is its first), line supports (`line_nodes`), held directions, `Volume` (every node, field values and tetrahedra for the window's cutting plane), 18- and 6-node wedges (triangle × line shape functions), faces for loads and supports (side faces from edges, end faces), penalty rollers on sloping faces, nodal stresses averaged by volume, principal stresses, the outer surface for the window (`Surface`), `MAX_UNKNOWNS` |
| `crates/core/src/space.rs` | 3D frames: local axes (`axes`), the 12×12 beam (axial, torsion, bending both ways) and truss bars, member loads in any direction, supports and springs in six directions, results along members (N, Vy, Vz, T, My, Mz, 3D deflection), six-way equilibrium |
| `ui/src/plane3.ts` | Working planes of a 3D frame: the projected model the canvas edits (`planeModel`), faint members off the plane, depths, and `lift` turning canvas edits into 3D |
| `ui/src/space3.tsx` | 3D frames without three.js: the colour-by choice and range, the deflection scale, the 3D legend and the plane bar (loaded at startup) |
| `ui/src/fonts.css` | The IBM Plex fonts as woff2 only |
| `tools/size.cjs` | The window's size budgets (startup code 400 kB, whole window 1.6 MB), run by CI |
| `ui/src/view3d.tsx` | Loaded lazily (the only module importing three.js). The three.js 3D view: members, nodes, supports, loads, the deflected shape coloured by a result, picking and placing, the probe, the plane bar and the 3D legend |
| `crates/core/src/modal.rs` | Natural frequencies and buckling (`subspace` shared, `Kind`): subspace iteration on one sparse Cholesky of K, Rayleigh–Ritz with a small dense Cholesky and Jacobi eigen solve; mode shapes scaled to a largest movement of 1 |
| `ui/src/modes.tsx` | Modes on the drawing (frames along members; parts through the contour canvas, 16 cached swing steps), the mode list, the animation, the frequency table |
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
| `tests_ui/run.cjs` | 142 UI checks of the built window with a mocked bridge (results in the fixture packed as the shell sends them) |
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
| 0.2.0 | The user asked for a big update toward 3D FEA for simple problems; plan in `docs/PLAN-0.2-0.3.md`. Answers: 3D parts by extruding or revolving 2D parts; analyses first (thermal stress, axisymmetric, frequencies, buckling, 3D frames), then 3D solids, then DXF and Calc/Plot; three.js for the 3D view; small numbered releases | The user's answers, 2026-10-29 |
| 0.2.0 | Thermal stress inside a stress model: fixed temperatures and convection in a stress or strain model are solved as heat conduction on the same mesh first; a uniform `Warming` body load adds to it; `t_ref` is the stress-free temperature | One model, one mesh, no linking of two files |
| 0.2.0 | A solve whose residual is over 1e-8 warns that the supports barely hold the part | Rollers on curved edges are stiff springs, so a part free to turn about an arc's centre was not caught as a mechanism (found by the random parts) |
| 0.2.0 | Points moved onto arcs are moved back when a triangle would fold | A hole crossing the outline folded a sliver (found by the random parts) |
| 0.2.1 | Axisymmetric solids share plane.rs: B and D carry a fourth (hoop) strain, used only by `Axi`; weights 2πr; edge forces and pressures are totals round the circle; nodes on the axis are held radially without a support; the equilibrium check sums axial forces only | One element code for plane and turned solids |
| 0.2.1 | The automatic element size is also capped at √(area / (0.43 × 800)) (about 800 triangles) | The vessel's 10 mm wall got one element through it at a twenty-fifth of the part's height |
| 0.3.0 | Datum Mesh does not become CAD or a pre-processor; sketch tools (0.3.1) before the cutting plane; Gmsh bundled in the installer from 0.4 (separate program; app itself stays under 5 MB, installer grows by Gmsh's size) | The user's answers 2026-10-30 (docs/PLAN-0.3-0.4.md) |
| 0.3.0 | 3D solids are the 2D mesh swept through the thickness into wedges (18-node from quadratic, 6-node from linear), not a tetrahedral mesher | Robust (the 2D mesher's quality), no new mesher, exact layers through the thickness; arbitrary 3D shapes wait for an outside mesher (0.4+) |
| 0.3.0 | End faces are a target of their own (`Target::End { front }`); side faces are the part's edges | No face picking in 3D needed to load a solid; old files unchanged |
| 0.3.0 | The sparse system can store only its lower half (`System::lower`), and faer runs on every core (`rayon`, MIT/Apache) | A 3D solid of 178 000 unknowns took 22.7 s and 3.1 GB on one core; on four, 15 s; 90 000 take 6 s and 1.2 GB |
| 0.3.0 | A 3D solid is refused beyond 200 000 unknowns, with how to coarsen | 500 000 needed 9.9 GB; one test run was killed out of memory: a crash in the app |
| 0.3.5 | A home-made nested dissection (BFS level separators, thinned) was tried for large 3D solves and dropped: measured interleaved on one machine it was no faster than faer's AMD (8.6 s against 8.2 s for 90 000 unknowns), and 4× slower on frames. An earlier "11.7 → 8 s" came from timing on a busy machine; compare orderings interleaved in one run | Speed for large 3D models needs a multilevel partitioner (METIS-like, under a permissive licence) or a different solver; kept on the list |
| 0.3.5 | DXF import adds shapes to the open part in one step; shapes sorted solids first, largest first; non-finite numbers skipped | Undo restores; a garbled file never makes a NaN corner |
| 0.3.4 | 3D heat is its own analysis (`Analysis::SolidHeat`, `is_solid()`), sharing the solid's sweep, faces and window; natural frequencies of solids reuse `build` (stiffness and supports) with consistent masses through `modal::lowest` | One geometry path for stress, vibration and heat; the window tells them apart by result kind |
| 0.3.3 | Load cases: `Load.case` (a case that does not exist means the first; adding the first case moves every load into it), `Model.cases`, `Model.combos` with factors; a combination is solved as one model with scaled loads (linear), the envelope solves each combination | Superposition is exact for every analysis here; one solve per combination keeps the code one path |
| 0.3.3 | Hinges by static condensation of the member's released end rotations (one transform for K, loads, mass and K_G); in 3D both bending rotations, and twisting too at a node no rigid beam holds (else its moment was lost to a held rotation: found by the random 3D frames) | No extra unknowns, and nodes whose beams are all hinged are held like truss joints |
| 0.3.3 | Section tables from EN 10365 nominal dimensions; properties computed (root fillets included), not copied; each row keeps its table area for a test | No data source in the hub; computing from dimensions checked against two published values (A, and Iy for three sizes) catches typing slips |
| 0.3.2 | Revolve is a property of a 3D solid (`Model.revolve`, radians, 0 = extrude) about the y axis, (x, y) → (x cos θ, y, x sin θ); layers at most 22.5° (quadratic) | One solid path for both; the back and front faces of a sector are its cut faces, so symmetry works as for extrusions |
| 0.3.2 | Elements pinched on the axis keep their repeated node: the pair's diagonal block counts twice; stresses there are read 2 % inside the element | Found by the round bar through the axis (it was 50 % off before); exact to 1e-13 after |
| 0.3.2 | The cutting plane is drawn by the window from `Volume` (tetrahedra on corner nodes, values interpolated linearly), the part beyond it hidden by a clipping plane | Interactive slicing with no round trip to the engine; it shows computed values, it computes none |
| 0.3.2 | `SupportKind::Hold {x, y, z}` and `Target::Line {shape, edge, at}` for solids | NAFEMS LE10 needs a face held in x and y only and a line held vertically |
| 0.3.2 | The user chose (2026-10-30) a frames release next (0.3.3: load cases and combinations, end releases, section tables), then 3D heat and frequencies (0.3.4) and DXF (0.3.5) | Closes the most visible gap for structural users without code design |
| 0.3.1 | Combining shapes uses i_overlay (MIT/Apache) on outlines cut into pieces (span/1000, at least 360 per circle), then traces each result edge to the input line or circle under it: runs on one circle become one arc (split if over a half turn), corners are put where the exact curves meet, input corners are kept exactly | Keeps arcs as arcs (a support on a round edge stays one edge) without an exact arc-clipping kernel; areas agree with hand values to 1e-9 |
| 0.3.1 | Sketch tools are engine ops (Fillet, Chamfer, Combine, Offset, Pattern, Make) applied like every other edit, with supports and loads moved to the edges they lie on; a combined outline starts at the first shape's first corner | One undo step each; the window only collects numbers; edge numbers stay put where they can (a real-app finding) |
| 0.3.0 | The window gets a solid's outer surface only (nodes, triangles, fields there); extremes over every node | A 90 000-unknown solid's surface is a small part of its nodes |
| 0.2.6 | Budgets: installer under 5 MB (release.yml), window startup code under 400 kB and whole window under 1.6 MB (tools/size.cjs, CI); speed: typical models under 1 s, large ones (about 100 000+ unknowns) under 10 s, timed in the verification record (`bench::timings`) | The user asked that the app stay small and optimised; budgets that fail the build keep it so |
| 0.2.6 | three.js is loaded only when a 3D frame is shown (`lazy` view3d.tsx); fonts as woff2 only | Startup code 863 → 282 kB |
| 0.2.6 | 3D buckling is flexural only: the geometric stiffness has no twisting term | Without warping stiffness an I-section twists under axial load at a small fraction of its real load (the portal gave λ = 0.25); flexural-torsional and lateral-torsional buckling need a warping degree of freedom, later |
| 0.2.6 | 3D frames reuse the plane frame's modal solvers: `space::assemble(m, pieces)` with beams cut in 8, consistent masses (polar mass for twisting), geometric stiffness both ways | One path for supports and loads in static, modal and buckling solves |
| 0.2.5 | Datum Mesh's purpose written down (top of this file); frames stay in the app but stop short of building design (no code checks or code combinations) | Keeps one clear purpose: analysis of parts and structures, verified, not code design |
| 0.2.5 | The Windows publisher is "Datum" (`bundle.publisher`), as for the whole family (FAMILY §9); the copyright stays "© 2026 Edgar Mendonca" | One publisher name across the family's installers |
| 0.2.4 | 3D frames are a separate analysis (`Space`) with y up and z as depth; the plane model gains z, roll, fz, mx, my, gz and six-way springs as serde defaults | Old files open unchanged; a plane frame is the 3D frame with z = 0 (benchmark 13 checks the portal to 1e-13) |
| 0.2.4 | Member axes: local y = z × x (the plane frame's local y), or global y for a member along z; roll turns y and z about x | Plane frames keep their signs and moments in 3D; beams along z have their depth upright |
| 0.2.4 | Members are drawn on working planes (Front, Side, Plan at a depth) by the existing canvas through a projected model, and seen in a separate three.js view | Reuses every 2D tool (snap, draw, move, supports, loads); no 3D drawing tool to learn; the 3D view only views, picks and places |
| 0.2.4 | three.js 0.185.1 (MIT) for the 3D view, pinned exactly | The 3D solids of 0.3 will use the same view |
| 0.2.3 | Buckling solves B φ = μ K φ for the largest μ = 1/λ, so K (positive) stays the matrix factorised and the small problem stays well posed when B has both signs | Members in tension make K_G indefinite; λ ≤ 0 (buckling with the loads reversed) is dropped |
| 0.2.3 | Beams are cut into 8 pieces (`frame::SPLIT`) for modes and buckling, not for the static solve | One cubic element is exact statically but 21 % high for a pinned column's buckling; 8 pieces give 3e-5 to 5e-4 |
| 0.2.3 | Truss bars are checked on their own as pinned struts rather than cut | A pin-jointed bar has no bending unknowns; π²EI/L² is its exact answer |
| 0.2.2 | Natural frequencies by subspace iteration (p = max(2q, q + 8) vectors, one factorisation of K) rather than a Lanczos code | Simple and robust for the few lowest modes asked for; checked against a mass-spring chain to 1e-9 |
| 0.2.2 | Frame assembly (`frame::assemble`) and the plane solver's assembly (`plane::run` with a modes request) are shared by static and modal solves | One place for supports and held unknowns |
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
| 0.1.5 | Fixes from first use on a clean Windows PC; a user guide in the app's help (waits for a person on Windows) | ⬜ |
| 0.2.0 | Thermal stress (uniform change, temperatures from heat conduction on the same mesh, frames), heated plate example, benchmark 9 (32 checks) | ✅ |
| 0.2.1 | Axisymmetric solids (hoop strain, 2πr weights, axis held radially, axial equilibrium, axisymmetric heat for thermal stress), pressure vessel example, benchmark 10 (39 checks), area-aware automatic size | ✅ |
| 0.2.2 | Natural frequencies of frames and parts (modal.rs subspace iteration, consistent masses), mode list and animation, benchmark 11 (47 checks) | ✅ |
| 0.2.3 | Buckling of frames (geometric stiffness, beams cut in 8, modal::buckling on the pencil B φ = μ K φ), truss bars as struts, benchmark 12 (54 checks) | ✅ |
| 0.2.4 | 3D frames (space.rs 12-unknown beams and truss bars, sections Iy and J, doc ops in 3D and Repeat), working planes (plane3.ts), three.js 3D view (view3d.tsx), benchmark 13 (69 checks) | ✅ |
| 0.3.0 | 3D solids by extrusion: solid.rs (swept wedges, faces, supports, loads, stresses, surface for the window), `Target::End`, layers, half-stored sparse system, solver on every core, 200 000-unknown limit; window: 3D/Section, end-face buttons, contoured surface, legend, Results page; benchmark 15 (85 checks) | ✅ |
| 0.3.1 | Sketch tools: fillet and chamfer corners, combine shapes (union, subtract, intersect) with arcs kept, slots, rounded rectangles, polygons, offsets, linear and circular patterns; sketch.rs, i_overlay; benchmark 16 (94 checks) | ✅ |
| 0.3.2 | Revolved solids (full turn or sector, through the axis), held directions and line supports, face picking in 3D, cutting plane; benchmark 17 with NAFEMS LE10 (105 checks); 3D vessel example | ✅ |
| 0.3.3 | Frames: load cases and combinations with an envelope, hinges (2D and 3D, static, modes, buckling), IPE/HEA/HEB tables with root radii; braced frame example; benchmark 18 (120 checks) | ✅ |
| 0.3.4 | 3D heat conduction (SolidHeat: temperatures, flux, flows, balance, cut) and natural frequencies of 3D solids (modes on the surface in the 3D view); heat sink in 3D example; benchmark 19 (131 checks) | ✅ |
| 0.3.5 | DXF import and export of sections, benchmark 20 (134 checks); a quicker ordering for large 3D solids tried and dropped (no gain) | ✅ |
| later | Large 3D solves: a multilevel partitioner or an iterative solver | ⬜ |
| later | Frames: P-delta, plates and shells, response spectrum | ⬜ |
| 0.4.0 | STEP import, Gmsh bundled in the installer (separate program), 10-node tetrahedra | 🔜 |
| later | DXF import, Send to Calc and Plot; STEP and arbitrary 3D shapes with an external mesher | ⬜ |
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

### 2026-10-29: Session 1, continued (plan for 0.2–0.3; 0.2.0 thermal stress)
- The user asked for a big update to reach 3D FEA for simple problems, finishing the roadmap first. Wrote `docs/PLAN-0.2-0.3.md` and asked four questions; answers recorded in Decisions.
- 0.2.0: `Warming` body load, `t_ref`, temperatures solved on the same mesh before the stress (plane.rs `temperatures`), thermal strain in frame members, benchmark 9 (five checks, exact), the heated plate example with a hand check. The random-part tests found two faults (a barely held part without a warning; a folded sliver where a hole crosses the outline), both fixed with tests.
- Window: Thermal group on the Loads tab (Temperature Change, Edge Temperature, Convection), temperature supports on edges of stress models, the stress-free temperature in the Inspector, Temperature among the results of a warmed part. 97 UI checks.

### 2026-10-29: Session 1, continued (0.2.1 axisymmetric)
- Released 0.2.0. Axisymmetric solids in plane.rs and heat.rs; benchmark 10 (Lamé cylinder and sphere with convergence, an axial patch test, free warming); the pressure vessel example with a pr/t hand check; random axisymmetric parts in the hardening tests (including parts across the axis, which must be refused). Window: the analysis, the axis on the canvas, radial/axial/hoop names, axial-only equilibrium. Found in the real app: the vessel's wall had one element through it; the automatic size now looks at the area.

### 2026-10-29: Session 1, continued (0.2.2 natural frequencies)
- Released 0.2.1. modal.rs, frame and plane masses, `modes()` for every stress analysis, benchmark 11 (beams against Euler–Bernoulli, a plate strip's bending and stretching), a test that every example vibrates with rising frequencies. Window: Natural Frequencies, the mode list and animation, the frequency table on the Results page and in print. Found in the app: the part's fill covered its mode.

### 2026-10-29: Session 1, continued (0.2.3 buckling of frames)
- Released 0.2.2. modal.rs generalised (`subspace` for vibration and buckling); frame.rs gains `kg_local`, `scatter`, `split` and `buckling`; the assembly cuts beams into pieces on request and keeps every inner node's position. Benchmark 12: Euler columns held four ways, the second mode, a warmed fixed column and a truss bar as a strut. Window: Solve › Buckling, the load factors under the canvas, the buckled shapes, the load factor and bar tables on the Results page and in print. Found in the app: bars were listed by internal id; now named Member n as in the tree.

### 2026-10-29: Session 1, continued (0.2.4 3D frames)
- Released 0.2.3. Engine: `Analysis::Space`, z on nodes, roll on members, 3D loads and springs (serde defaults), section Iy, J (Roark, Bredt, open thin-walled), doc ops in 3D (node finding, splitting, crossings by closest points) and `Op::Repeat`; space.rs; benchmark 13; the platform example; random 3D frames balance. Window: the plane bar and working planes, the three.js view, 3D fields in the Inspector and dialogs, Repeat, the 3D Results page and report. Found by the UI checks: Repeat sent no member kind for the joins. Found in the app: the 3D example tiles reused the plane frame picture.

### 2026-10-29: Session 1, continued (0.2.5 publisher)
- Released 0.2.4. Windows publisher set to "Datum" in tauri.conf.json (the only place it was written; docs/FAMILY.md only links to the hub). Patch release 0.2.5.

### 2026-10-29: Session 1, continued (0.2.6 3D frame dynamics, optimisation)
- Released 0.2.5 (publisher Datum). Wrote down the purpose with the user; frames stay in Datum Mesh but stop short of building design. Engine: space.rs `assemble` with splitting, `modes`, `buckling`, benchmark 14, `bench::timings` and `building()`. Window: kinds space and buckle3, mode shapes swinging in the 3D view, three.js split out and loaded lazily (space3.tsx), woff2 fonts only, size budgets in CI and release. Found by benchmark 14: an I-section twisted under axial load at λ = 0.25 without warping stiffness; 3D buckling made flexural only and said so.

### 2026-10-29: Session 1, continued (0.3.0 3D solids)
- Released 0.2.6. Engine: `Analysis::Solid`, `Target::End`, `MeshSettings.layers`, `Traction.fz`; solid.rs; `System::lower`; faer with rayon; benchmark 15 (patch test, cantilever, Lamé in 3D); the 3D bracket and blank solid examples; random solids balance; timings for the 3D bracket. Window: 3D/Section plane bar with end-face buttons, the extruded part and the contoured deformed surface in the 3D view, the probe, the solid legend, Results page and report, Inspector fields. Found while building: diagonal blocks added twice (the matrix was indefinite; the patch test caught it), and an out-of-memory kill on a fine mesh (the unknowns limit). Found in the app: the tree said "not meshed" after a solid was solved.

### 2026-10-30: Session 1, continued (0.3.1 sketch tools)
- Agreed the direction with the user (docs/PLAN-0.3-0.4.md): simple parts by sketch tools plus extrude/revolve, complex parts from STEP via Gmsh bundled from 0.4. Engine: sketch.rs (fillet, chamfer, combine with arcs recovered, offset, slot, rounded rectangle, polygon, turn), doc ops Fillet, Chamfer, Combine, Offset, Pattern, Make, `retarget` (renumber built on it; a total edge force split between edges now keeps its fz), benchmark 16, random combinations with area identities, sketch ops in the random-edit test. Window: Ready-made, Combine and Modify groups on Geometry and Format, sketch.tsx dialogs, ⌒ in the corner table, Tell Me entries, help card, icons. Found while building: arcs over a half turn from a union (now split), segments spanning collinear edges of both shapes (now traced by carrier, not piece). Found in the app: a subtracted outline started at another corner, renumbering its edges (now starts at the first shape's first corner).

### 2026-10-30: Session 1, continued (0.3.2 revolved solids)
- Released 0.3.1. Asked by the user how close the app is to a structural analysis program for frames; listed the gaps (load cases and combinations, end releases, section tables, P-delta, plates, response spectrum); the user chose to finish 0.3.2 and then do a frames release (0.3.3). Engine: `Model.revolve`, `Settings.revolve`, `Swept` node map (axis and seam merged), generic end-face normals, `SupportKind::Hold`, `Target::Line` (renumbered, combined and patterned with their edges), `Volume`, benchmark 17 (Lamé closed-end quarter and full turn, bar through the axis, NAFEMS LE10), random revolved solids, the 3D vessel example and its timing. Window: extruded/revolved choice and angle, revolved preview (lathe), face outlines round the turn, picking faces by clicking, Hold Directions tool and checkboxes, Where: face or line, the cutting plane (clipping and slices of tetrahedra) under the view. Found while building: the pinched-node assembly bug (a bar through the axis was 50 % off). Found in the app: the analysis was still called "3D solid (extruded)". Measured: a 90 000-unknown 3D factorisation takes 10 s on this machine (AMD ordering); nested dissection is on the roadmap.

### 2026-10-30: Session 1, continued (0.3.3 frames release)
- Released 0.3.2. The user asked whether the app is still on course to be like the structural and general FEA programs (answered with a table of what is done and missing) and whether the name fits (they chose to keep Datum Mesh). Engine: cases.rs (select, label, envelope), release.rs (condensation), hinges in frame.rs and space.rs (split pieces keep the end hinges; ball joints where no rigid beam holds a node), `Item::Case`/`Item::Combo` with cascade on delete, `Member.hinge`, `Load.case`, I-section root radius with fillet properties, steel.rs tables, `solve_for`/`buckling_for`, benchmark 18, random hinges and the combination superposition test, the braced frame example. Shell: `solve`/`buckling` take a case or combination, `envelope`, `sections_library`. Window: case picker on Loads, the cases dialog, loads filtered by the picked case and new loads put in it, the legend's load case line, Results › Envelope, hinge checkboxes and circles, Load case in the load Inspector, From the tables in Sections, root radius field, help card. Found while building: lost moments at nodes only hinged 3D beams meet (ball joints); a dialog left under a closed one could not be closed with Escape.

### 2026-10-30: Session 1, continued (0.3.4 3D heat and frequencies)
- Released 0.3.3. Engine: `Analysis::SolidHeat`, `is_solid`, solid.rs split into `build` and the static solve, `modes` (consistent masses, `Modes::Solid` with the surface), `heat` (conduction, flux, convection and generation on faces, fixed temperatures on faces and lines, flows and balance, flux at nodes, volume for the cut), flux and convection allowed on end faces, benchmark 19, random heat and vibration of solids, the 3D heat sink example, timings for both. Window: Home › 3D Heat, heat tools and face picking for it, the 3D heat legend and Results page, `solidDisp`, the solid's modes swinging in the 3D view, Natural Frequencies enabled for solids, help. Measured: six frequencies of the 3D bracket take 2.3 s (budget 3 s).

### 2026-10-30: Session 1, continued (0.3.5 DXF and ordering)
- Released 0.3.4. Engine: dxf.rs (read, write, sample), benchmark 20, DXF fuzz hardening. Tried a nested dissection ordering for large 3D solves (through faer's low-level Cholesky with a custom permutation): first timings looked 30 % faster, but interleaved runs on the same machine showed no gain (8.6 s against AMD's 8.2 s) and frames got 4× slower, so it was removed before release; the 0.3.x "OVER" rows for the 90 000-unknown solid are mostly the machine's load. Shell: `import_dxf` (file dialog, units, one undo step) and `export_dxf`. Window: Geometry › DXF with a units choice, File › Export › Section as DXF, help card, stale planned-release labels fixed. Found in the app: "1 lines or arcs … do not close" (now singular).

## 5. Known limits (0.3.5)

- DXF: ASCII only (not binary DXF); blocks (INSERT), splines and ellipses are left out; the drawing's layers are not used to tell part from holes (nesting is). Large 3D solids (about 90 000 unknowns) take 8 to 12 s depending on the machine's load; faster needs a multilevel partitioner or another solver.

- 3D heat: steady only (no transient); no thermal stress in 3D solids yet (temperatures are not passed to a 3D stress solve, and uniform temperature changes are left out with a warning). 3D frequencies: consistent masses, the lowest 30 at most; no point masses. Mode search on large solids is slow (several seconds above 20 000 unknowns).

- Frames: combinations are solved one at a time (a few ms each for frames); the envelope is over combinations, or over cases when there are none; no P-delta, plates or response spectrum yet. A 3D hinge releases both bending moments (not one at a time). The tables' J leaves out the root fillets (a little below the tables). Load cases apply to every analysis; combinations to structural ones (not heat).

- 3D solids: the solve of large ones (about 90 000 unknowns) takes over 10 s here; the AMD ordering fills in a lot in 3D. No point supports or springs on solids; no thermal loads, pin loads or natural frequencies yet. The cutting plane shows values interpolated linearly inside each element (on corner nodes).

- Sketch tools: corners are rounded or bevelled only between two straight edges; offsets need arcs that meet their neighbours smoothly; a combined result whose arcs came from flattened pieces has its arc ends where the exact curves meet (to about 1e-9 of the part); no canvas picking of corners (by the dialog or the corner table).


- 3D frames: buckling by bending only (no flexural-torsional or lateral-torsional buckling: no warping stiffness); utilisation leaves out the shear stress of twisting; an angle bends about its leg axes (the product of inertia is left out); a channel's shear centre is not modelled.

- Buckling: frames and trusses only, in their own plane; linear (eigenvalue) buckling of a perfect frame; no lateral-torsional buckling or design-code checks.

- Natural frequencies: the six lowest only (up to 30 through the engine), no point masses yet, no damping or forced response.

- Axisymmetric loads are round the axis only (no bending of a shaft by a side load: that needs Fourier modes or 3D).

- Temperatures for thermal stress come from steady conduction only (no transient heating).

- Not yet tried on a clean Windows PC by a person (0.1.5).
- Stresses at the corner where a fixed edge meets a free edge, or at a sharp inside corner, keep rising with mesh refinement (a singularity); the help says so, the app does not mark them.
- NAFEMS LE1's error is flat between the first two meshes (0.24 %) before falling to 0.09 %.
- A pin load on an edge split in two (a corner added in it) is shared half and half, each half bearing on its own; on a circle hole it is one edge, so this rarely matters.
- No copy and paste of geometry.
- Overlapping solid shapes join only when meshed (0.1.2); the canvas draws them overlapping.
