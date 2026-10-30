# Datum Mesh 0.3 and 0.4: plan

Written 2026-10-30 after 0.3.0 (extruded 3D solids), from a brainstorm with the user about where Datum Mesh is heading.

## Where Datum Mesh sits

Datum Mesh is an **analysis** app, in the spirit of two kinds of commercial tool (named here only; never in public words, FAMILY §13):

- **Frames and trusses**, like the analysis side of a structural program: 2D and 3D frames, reactions, member forces, frequencies and buckling. No building design codes or code load combinations (agreed 2026-10-29).
- **Parts**, like a general finite-element program: linear stress, heat and vibration today; yielding, contact and large deflection (0.4) and heat and flow (0.5) later.

It is **not** a CAD program and **not** a pre-processor that prepares models for other solvers. One small app does the whole loop (draw, load, solve, read, verify), and proves its answers.

## How parts get their shape

- **Simple parts are made in Datum Mesh**: a 2D section drawn with sketch tools, then extruded or revolved. This covers most parts engineers check quickly: brackets, plates, lugs, flanges, shafts, hubs, vessels.
- **Complex parts come from real CAD**: the user designs in any CAD program and exports STEP; Datum Mesh meshes it with Gmsh (run as a separate program) and solves it with its own solver.
- Full 3D modelling (a feature tree, sketching on faces, edge fillets) is not planned: it is a CAD product of its own, and the open geometry kernels are LGPL (outside the app's licence rule) or not yet mature.

## Releases

| Release | Work | Checked against |
| --- | --- | --- |
| 0.3.1 | **Sketch tools** for the section: fillet (round) and chamfer (bevel) any corner; combine shapes (union, subtract, intersect) for slots, notches and cut-outs; slots, rounded rectangles, regular polygons, offset outlines; linear and circular patterns of holes | Areas and lengths of the shapes made, against hand values; meshing every result |
| 0.3.2 ✅ | **Revolve** a section about an axis (shafts, hubs, vessels; full turn or a sector with symmetry faces); the 3D view's **cutting plane** and **picking faces** in 3D | NAFEMS LE10 (thick plate under pressure), closed-end cylinder, revolved Lamé |
| 0.3.3 ✅ | **Frames for everyday structural analysis** (added 2026-10-30 at the user's choice): load cases and combinations, member end releases, steel section tables | Hand frames with hinges; combinations by superposition |
| 0.3.4 | **3D heat conduction and natural frequencies** of solids | 3D fin, block and plate vibration modes |
| 0.3.5 | **DXF import** of section outlines (lines, arcs, polylines, circles) | Round trip of the examples |
| 0.4.0 | **STEP import** of any 3D part, meshed by **Gmsh** into 10-node tetrahedra, solved by our own solver | 3D cantilever and Lamé again on tetrahedra, NAFEMS LE10 |
| 0.4.x | Beyond linear: yielding, large deflection, contact | NAFEMS nonlinear benchmarks |
| 0.5 | Heat and flow together | — |

## Decisions (the user's answers, 2026-10-30)

1. Datum Mesh does **not** become a CAD program or a pre-processor for other solvers; complex 3D shapes come from real CAD through STEP import.
2. **Sketch tools come next** (0.3.1), before the cutting plane, which moves to 0.3.2 with revolve.
3. **Gmsh is bundled in the installer** ("don't separate, do a big build"), not a separate download. From 0.4.0:
   - Gmsh (GPL) ships as its own program beside Datum Mesh and is run as a separate process, never linked; its licence and a link to its source go with it, and NOTICE records it.
   - The size budget splits: Datum Mesh itself stays under 5 MB as now; the installer grows by Gmsh's size (to be measured when it is added, likely tens of MB). The release check gets the new total.

## Risks

- **Combining shapes keeps arcs**: a union or subtraction of shapes with arcs must keep the arcs as arcs (so supports and loads on them still mean one edge), not cut them into short lines. This is the hardest part of 0.3.1; if exact arc clipping proves too costly, curved results become many short edges and the tools say so.
- **Tetrahedra from STEP** need a new element (10-node tetrahedra) and checks of their own; Gmsh's Windows build and its size need measuring before the installer budget is set.
