<p align="center"><img src="docs/brand/datum-mesh-icon.svg" width="96" alt="Datum Mesh"></p>

<h1 align="center">Datum Mesh</h1>

<p align="center">Finite-element analysis on the desktop: stresses, deflections and temperatures in parts and frames.</p>

<p align="center">
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/releases"><img src="https://img.shields.io/github/v/release/Edgar-Mendonca/Datum-Mesh?include_prereleases&color=B83250" alt="Release"></a>
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Edgar-Mendonca/Datum-Mesh/ci.yml?label=CI" alt="CI"></a>
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/releases"><img src="https://img.shields.io/github/downloads/Edgar-Mendonca/Datum-Mesh/total?color=B83250" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT">
  <img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0C1A2B" alt="Windows">
  <img src="https://img.shields.io/badge/Rust-engine-B7410E" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB" alt="Tauri 2">
  <img src="https://img.shields.io/badge/SolidJS-window-2C4F7C" alt="SolidJS">
</p>

<p align="center">
  <a href="https://github.com/Edgar-Mendonca/Datum-Mesh/releases/latest"><img src="https://img.shields.io/badge/Download_for_Windows-B83250?style=for-the-badge" alt="Download for Windows"></a>
  <a href="CHANGELOG.md"><img src="https://img.shields.io/badge/What's_new-0C1A2B?style=for-the-badge" alt="What's new"></a>
  <a href="docs/MEMORY.md"><img src="https://img.shields.io/badge/Memory_&_roadmap-5B6878?style=for-the-badge" alt="Memory and roadmap"></a>
</p>

![Datum Mesh: a portal frame solved, with its bending moment diagram, deflected shape and reactions](docs/images/portal-solved.png)

Draw a frame of beams and truss bars, or the outline of a part with its holes; put on the supports and loads; solve, and see the deflections, stresses and temperatures. Datum Mesh is for mechanical, structural, process and aerospace engineers, students and lecturers. It is free and open source (MIT), Windows first, installs in seconds and updates itself. More on the family site: [edgar-mendonca.github.io/Datum/apps/mesh](https://edgar-mendonca.github.io/Datum/apps/mesh).

## What it does

- **Frames and trusses, solved**: draw beams and truss bars on a grid, put on supports and loads, press Solve (F5). See the deflected shape, the moment, shear and axial diagrams, the reactions, stresses and utilisation; hover a member to read the values there.
- **The equilibrium check** after every solve, and **Help › Verification**: benchmarks with known answers solved in front of you.
- **Sections** (rectangle, round bar, tube, box, I, channel, angle, or your own values) with their properties worked out.
- **Parts**: rectangles, circles and polygons with arc edges, and holes; plane stress, plane strain and heat conduction; supports and loads on edges.
- **Units**: SI (mm, kN, MPa) or US (in, kip, ksi); files keep SI.
- **Worked examples**: a portal frame, a roof truss, a lifting lug, an L-bracket with a fillet, a flange plate and a heat sink.
- **Print to PDF** with a title block and the results, a `?` card beside every tool, light and dark themes, recovery of unsaved work, updates from inside the app.

| Release | What arrives |
| --- | --- |
| 0.1.3 | Parts meshed (6-node triangles) and solved: stress and displacement contours, deformed shape, probe; plate with a hole, thick cylinder and NAFEMS LE1 benchmarks |
| 0.1.4 | Heat conduction, the materials library from Datum Atlas, the full results report |

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
