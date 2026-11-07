<p align="center"><img src="docs/brand/datum-mesh-icon.svg" width="96" alt="Datum Mesh"></p>

<h1 align="center">Datum Mesh</h1>

<p align="center">Finite-element analysis on the desktop: stresses, deflections and temperatures in parts and frames.</p>

<p align="center">
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/releases"><img src="https://img.shields.io/github/v/release/Edgar-Mendonca/Datum-Mesh?include_prereleases&color=0F6E8C" alt="Release"></a>
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Edgar-Mendonca/Datum-Mesh/ci.yml?label=CI" alt="CI"></a>
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/releases"><img src="https://img.shields.io/github/downloads/Edgar-Mendonca/Datum-Mesh/total?color=0F6E8C" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0C1A2B" alt="Windows">
  <img src="https://img.shields.io/badge/Rust-engine-B7410E" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB" alt="Tauri 2">
  <img src="https://img.shields.io/badge/SolidJS-window-2C4F7C" alt="SolidJS">
</p>

<p align="center">
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/releases/latest"><img src="https://img.shields.io/badge/Download_for_Windows-0F6E8C?style=for-the-badge" alt="Download for Windows"></a>
  <a href="CHANGELOG.md"><img src="https://img.shields.io/badge/What's_new-0C1A2B?style=for-the-badge" alt="What's new"></a>
  <a href="docs/MEMORY.md"><img src="https://img.shields.io/badge/Memory_&_roadmap-5B6878?style=for-the-badge" alt="Memory and roadmap"></a>
</p>

![Datum Mesh: an L-bracket solved, with von Mises stress contours, the deformed shape and the largest stress at the fillet](docs/images/bracket-solved.png)

**Purpose:** To show any engineer or student how a part or structure carries load, heat and vibration (where it is stressed, how far it moves, how hot it gets) in a free app they can trust because it proves its own answers.

Draw a frame of beams and truss bars, or the outline of a part with its holes; put on the supports and loads; solve, and see the deflections, stresses and temperatures. Datum Mesh is for mechanical, structural, process and aerospace engineers, students and lecturers. It is free and open source (MIT), Windows first, installs in seconds and updates itself. More on the family site: [edgar-mendonca.github.io/Datum/apps/mesh](https://edgar-mendonca.github.io/Datum/apps/mesh).

## What it does

- **Frames and trusses, solved**: draw beams and truss bars on a grid, put on supports and loads, press Solve (F5). See the deflected shape, the moment, shear and axial diagrams, the reactions, stresses and utilisation; hover a member to read the values there.
- **Plasticity**: parts that yield (von Mises with hardening), in increments up to collapse, with plastic strain contours and the load–deflection curve.
- **Parts, meshed and solved**: plane stress, plane strain, axisymmetric solids (vessels, shafts), steady heat conduction and thermal stress on automatic triangle meshes (6-node by default), finer where you ask. Contours of von Mises and principal stress, displacement, safety factor, temperature and heat flux; the deformed shape; hover anywhere to read the values.
- **Natural frequencies** and mode shapes of frames and parts, moving on screen.
- **Buckling of frames**: load factors and buckled shapes, with every squeezed truss bar checked as a strut.
- **Large deflection and P-delta**: plane and 3D frames solved in steps in their deflected shape (large rotations in space for 3D frames), by Newton increments or by arc length past the limit load (snap-through), with the load–deflection curve.
- **3D solids**: a part's section extruded by its thickness or revolved about an axis (shafts, hubs, vessels), solved in 3D for stress, natural frequencies or heat conduction, coloured in a 3D view, with a cutting plane to see inside; faces picked by clicking them.
- **3D frames and trusses**: drawn on front, side and plan planes, repeated into bays, seen in a 3D view and coloured by their results, with natural frequencies and buckling.
- **Structural details**: tension-only and compression-only members (bracing, cables), support settlement, rigid floors, rigid end zones at joints, and second order (P-delta) in 3D frames.
- **Dynamics**: response spectrum (CQC or SRSS) and time history (Newmark) for frames and plates, with point masses; the modes' share of the mass, and the motion over time.
- **Plates and shells**: slabs, walls and panels drawn on those planes, joined to the beams, coloured by stress, moments or deflection in 3D, and vibrating with the frame.
- **The equilibrium check** after every solve (the heat balance for heat models), and **Help › Verification**: 192 benchmark checks with known answers solved in front of you, from beam theory, Euler columns and 3D frames to the plate with a hole, the thick cylinder, NAFEMS LE1 and LE10, conduction through a fin and the NAFEMS large-deflection beams (NL5 to NL7), plates against Navier's series and the Scordelis–Lo roof and pinched cylinder, dynamics against the closed forms of one and two masses, plasticity against Hill's limit analysis, with mesh convergence.
- **Materials** from Datum Atlas (54 metals and plastics), each with its source.
- **Sections** (rectangle, round bar, tube, box, I, channel, angle, or your own values) with their properties worked out, and European steel tables (IPE, HEA, HEB).
- **Load cases and combinations** with your own factors, an envelope of every member's forces over all combinations, and **hinged member ends**.
- **DXF**: import a section drawn in CAD (outlines, arcs and holes) and export one back.
- **Drawing parts**: rectangles, circles and polygons with arc edges, and holes; slots, rounded rectangles and regular polygons; rounded and bevelled corners; union, subtract and intersect with arcs kept true; offsets and patterns of holes; supports and loads on edges and points, including a pin load that bears on one side of a hole.
- **Units**: SI (mm, kN, MPa) or US (in, kip, ksi); files keep SI.
- **Worked examples**: a portal frame, a roof truss, a braced steel frame with load cases, a snap-through frame, a steel platform in 3D, a floor slab on columns, a frame and a 3D building in an earthquake, tension-only bracing, a thick tube yielding, a lifting lug, an L-bracket with a fillet, a flange plate, a heat sink (also in 3D), a pressure vessel (also revolved in 3D) and a heated plate.
- **Print to PDF**: the model with a title block and a report with the mesh, contour pictures, the largest values, reactions and the equilibrium check; a `?` card beside every tool, light and dark themes, recovery of unsaved work, updates from inside the app.

| Release | What arrives |
| --- | --- |
| 0.4.3 | Large deflection in 3D frames, tension-only members, settlements, offsets, rigid floors |
| 0.5–0.7 | Plasticity and rubber, tetrahedra and mesh import, implicit and explicit dynamics, contact |

The vision: [docs/VISION.md](docs/VISION.md).

Everything is computed by Datum Mesh's own engine in Rust (`crates/core`), with tests against known answers.

## Build it

```bash
npm ci
npm run build:ui
cargo run -p datum-mesh          # the app
cargo test --workspace           # the engine's tests
npm run test:ui                  # the window's tests
```

Datum Mesh is part of the [Datum family](https://edgar-mendonca.github.io/Datum/) of free desktop tools for engineers, scientists, researchers and analysts. MIT licence.

Product and file-format names are trademarks of their owners and are used only to describe file compatibility.
