# Datum Mesh to 1.0: the whole map first

Written 2026-11-07 after 0.9.1. The user asked to stop building the window and the features together (each feature found something missing in the window, and back again) and to work as Datum Plot was built: first the whole window, every feature of the finished program in its place, the ones not yet built shown dimmed; then a roadmap that fills them in one release at a time. The user also asked for a window that is easier to use and looks professional, still the Datum way.

## Answers that set the plan (the user, 2026-11-07)

- A planned feature opens its **full dialog, greyed out**: every field it will have, shown and not editable, with what it will do and the release that brings it.
- Planned features show everywhere, behind **View › Show planned features**, on by default; off gives the working window alone.
- Supports and loads go on the geometry before the mesh (carried over by face name when meshed again); node-based loads are an option on top; frames skip the mesh step.
- The window: the workflow bar at the far left, the steps tree, the model across the middle, a card that pops up for what is picked, the terminal along the bottom (built in 0.9.1).

## Phase A: the whole map (0.10.0)

Every feature of the finished program gets its place: the workflow bar, the tree's branches, the ribbon's buttons, the dialogs, the card's fields and the result tools. What works, works. What does not is dimmed with a "planned for x.y" note; it never pretends to work, and nothing is calculated in the window.

- **One register.** `ui/src/features.ts` lists every feature: its id, its step, the analyses it is for, its label and icon, working or planned, the release, and its help (what it does, what you set). The ribbon, the tree, the greyed dialogs, Help › Roadmap and `docs/site.json`'s roadmap are drawn from it, so they cannot disagree. A UI check walks the register: every planned item is in the window, dimmed, and does nothing.
- **Greyed dialogs.** A planned feature's dialog is the dialog it will have, with every field disabled and a band across the top: "Planned for 0.12: …". When the feature is built, the same dialog comes alive.
- **The switch.** View › Show planned features (on). Off hides every planned item.
- **Also in 0.10.0:** the tube crush's first increment settling (found with the solver monitor in 0.9.1).

## The inventory

✅ working · ◐ partly · ○ planned. Each line becomes a register entry.

### 1. Geometry
- ✅ Sketch (lines, arcs, circles, rectangles, polygons, slots, splines, trim, offset, fillet, mirror, patterns, dimensions, constraints)
- ✅ Extrude, revolve, sweep, loft, draft, split, shell, press-pull; join, cut, intersect; fillets and chamfers; the history
- ✅ STEP in and out, DXF in, meshes in (`.msh`, `.inp`)
- ✅ Frames: members, nodes, levels and grids, rigid ends, releases; plates
- ◐ Parts and assembly: placed by coordinates
- ○ Placing parts against each other (face to face, aligned, offset)
- ○ Splitting faces and solids to carry loads on part of a face
- ○ Reference points; datum planes, axes and points
- ○ Cables; springs and dashpots as elements; the beam's orientation angle

### 2. Materials and sections
- ✅ Library; elastic; yielding (J2, isotropic hardening); rubber (neo-Hookean); thermal properties
- ✅ Steel section tables, custom sections, plate thickness
- ○ Orthotropic elastic
- ○ Kinematic and combined hardening
- ○ Johnson–Cook plasticity (strain rate, temperature) and damage, with elements removed when they fail
- ○ Mooney–Rivlin, Yeoh, Ogden
- ○ Properties that change with temperature
- ○ A material from test data: paste a curve, the model is fitted and drawn over it
- ○ Composite layups; concrete (later)

### 3. Supports, loads and the analysis
- Steps:
  - ✅ Linear; nonlinear by load control; arc length; large deformation; natural frequencies; linear buckling; response spectrum; time history; impact (elastic); steady heat
  - ○ Nonlinear buckling with an imperfection taken from a buckling mode
  - ○ Transient heat; thermal stress fully coupled
  - ○ Impact with yielding and contact (crash)
- Load cases:
  - ✅ Cases and combinations
  - ○ A combinations generator; wind and earthquake load generators; moving loads
- Loads:
  - ✅ Point, distributed, pressure, edge, gravity, temperature, remote, velocity
  - ○ Centrifugal; bolt pretension
  - ○ Named load curves (a factor against time), shared by any load
- Supports:
  - ✅ Fixed, pinned, rollers, symmetry, springs, settlement, held directions, moved face
- Starting conditions:
  - ◐ Stress-free temperature; starting velocity
  - ○ Initial stress; an initial temperature field
- Interactions:
  - ✅ Contact with friction (small sliding)
  - ○ Large sliding and self contact
  - ○ Tie (faces glued); coupling to a reference point (kinematic, distributing); rigid bodies
  - ○ Connectors: bolt, hinge, spring, damper

### 4. Mesh
- ✅ Automatic size; finer near a face or edge; element order; quality of 2D meshes
- ○ Quality report of 3D meshes
- ○ Hexahedra; swept and mapped meshes
- ○ Refinement by itself where the error is largest, solved again until it settles
- ○ A mesh convergence study (the result against the element size)
- ○ Named sets of nodes and elements

### 5. Solve
- ✅ Solver monitor, Stop, Log (0.9.1)
- ◐ Solver settings: increments (the tolerance is fixed)
- ○ A job queue: several analyses run one after another while you keep working
- ○ What results to keep, and how often
- ○ Restart from a step
- ○ Solving on all the processor's cores
- ○ Writing the model as an input file for other open solvers

### 6. Results
- ✅ Contours, deformed shape, animation, probe, a cut plane, graphs along a line and in time, load–deflection, tables and the equilibrium check, envelope, mode shapes, report and print
- ○ The Results tree: steps and increments, fields, graphs, cuts, display groups, images
- ○ Exact colour bands; the largest and smallest values marked in the view; the legend and the solve's details drawn in the view
- ○ Several cut planes; free-body forces through a cut
- ○ Stress through a wall (linearisation)
- ○ Two analyses compared on one graph
- ○ Design checks of steel and concrete members
- ○ Export: CSV, VTU, images, video

### Across the program
- ✅ Units, examples, help, verification, the updater
- ○ Render: shaded, wireframe, hidden lines, edges
- ○ A selection filter (faces, edges, nodes, bodies)
- ○ Saved views 1–4; display groups (show and hide parts)
- ○ Labels and arrows on the view for the report
- ○ Several models in one file (design options side by side)
- ○ A start screen with recent files (with pictures) and a tutorial
- ○ Scripting in Python (a console in the bottom panel, scripts to run); command-line batch runs

## A professional window (part of 0.10.0, then every release)

The user: the window still feels a bit unprofessional; it has to be easier and look professional, the Datum way. The family's look (FAMILY §2–9: the crimson accent, the ribbon, the File page, help, printing) stays; what changes is the discipline:

- **Type.** One scale: 13 px text, 12 px secondary, 11 px captions; monospace only for numbers and units. The uppercase monospace labels on every group, step and button become sentence case in the text face (group names under the ribbon may stay small caps).
- **Quieter chrome.** One help button per panel and per dialog, not one per group and branch; tooltips carry the rest. Counts and ticks in the tree become small and grey. Fewer borders, more even spacing (an 8 px grid).
- **Buttons.** Sentence case ("Delete", not "DELETE"); one primary button per dialog; the same height everywhere.
- **Icons.** One set, one stroke weight, one size per place; the workflow bar's icons drawn for it.
- **The view.** A soft gradient behind the model (Settings: gradient, plain, white for print); the legend in the view with round numbers; the solve's details in a corner; the view cube and render buttons together.
- **Colour.** The crimson for the one thing that matters on screen (the current step, the primary button, the selection); results in their own colour scale; warnings amber, errors red, done green.
- **Checks.** Screenshots of each step in light and dark before each release, compared with the last.

## Phase B: filling it in

Each release turns a group of dimmed items into working ones, each with its benchmark and its line in the verification report. The order can change; the register says where each item stands.

| Release | Group |
| --- | --- |
| 0.10.0 | The whole map (Phase A); the professional pass; the tube crush settling |
| 0.10.x | View controls (render, selection filter, saved views, display groups); the Results tree; colour bands and markers; result tools as pop-ups; export |
| 0.11 | Materials: kinematic hardening, Johnson–Cook and damage, Mooney–Rivlin, Yeoh, Ogden, temperature-dependent properties, fitting to test data |
| 0.12 | Setup: named load curves, starting conditions, tie, coupling and reference points, connectors, bolt pretension, centrifugal loads |
| 0.13 | Nonlinear buckling with imperfections; transient heat; coupled thermal stress; crash (yielding and contact in impact) |
| 0.14 | Mesh: hexahedra, refinement by itself, convergence studies, sets |
| 0.15 | Solve: job queue, output choices, restart; scripting in Python |
| 1.0 | Design checks of members; several models per file; every item above working |

## How each release is checked

As now (CLAUDE.md): clippy, the Rust tests and benchmarks, the window checks, `cargo deny`, the real app under Xvfb with screenshots. Phase A adds the register check (every planned item present, dimmed, inert, its dialog greyed) and the screenshot comparison of each step.
