# Datum Mesh 0.2 and 0.3: plan

Written 2026-10-29 after 0.1.4. The user asked for a big update: finish the roadmap, then 3D finite elements for simple problems. This plan proposes the order and scope; the open questions are at the end.

## 0.1.5: first use (needs a person on Windows)
- Fixes from a clean Windows PC (the user or a tester installs 0.1.4 and reports).
- A user guide inside Help (topics, worked examples step by step).

## 0.2: more analyses in 2D (small releases, each verified)
| Release | Work | Benchmark |
| --- | --- | --- |
| 0.2.0 | Thermal stress: a temperature field from a heat model (or a uniform change) loads a stress model | Bar with a temperature rise held at both ends (σ = −EαΔT); a free plate with a linear gradient (no stress) |
| 0.2.1 | Axisymmetric solids (pressure vessels, shafts, discs): T6 axisymmetric elements, hoop stress | Thick cylinder (Lamé) again, closed-end vessel, spinning disc |
| 0.2.2 | Natural frequencies of frames and parts (consistent mass, sparse eigen solver: shift-invert Lanczos on faer) | Cantilever and simply supported beam modes; NAFEMS FV-type plate modes |
| 0.2.3 | Buckling of frames (geometric stiffness, eigenvalues) | Euler columns (four end conditions), portal frame sway |
| 0.2.4 | 3D frames (6 degrees of freedom per node, a 3D view) | 3D cantilever in torsion and biaxial bending, space truss |
| 0.2.5 | DXF import of outlines (lines, arcs, polylines, circles) | Round trip of the examples |
| 0.2.6 | Send a result to Datum Calc and probe lines and convergence to Datum Plot (CSV) | — |

## 0.3: 3D solids for simple problems
Geometry is made from the 2D parts the app already draws:
- **Extrude** a part by a thickness (plates, brackets, lugs, flanges with holes).
- **Revolve** a part about an axis (shafts, vessels, hubs; full turn or a sector with symmetry).
- The 2D triangle mesh is swept into layers: 15-node quadratic wedges (or each wedge split into 10-node tetrahedra), so meshing stays robust and needs no new mesher.

| Release | Work | Benchmark |
| --- | --- | --- |
| 0.3.0 | 3D linear static: extrude, wedge elements, face supports and loads (fixed, roller, pressure, force, gravity), the equilibrium check | 3D cantilever (beam theory), patch test, Lamé in 3D |
| 0.3.1 | The 3D viewer: orbit, pan, zoom, section plane, contours on faces, deformed shape, probe | — |
| 0.3.2 | Revolve; symmetry faces | NAFEMS LE10 (thick plate under pressure), closed-end cylinder |
| 0.3.3 | 3D heat conduction and natural frequencies | 3D fin; cantilever block modes |

Not in 0.3: arbitrary 3D shapes and STEP import. They need a general tetrahedral mesher and a CAD kernel; the open ones (Gmsh, TetGen, Open CASCADE) are GPL, AGPL or LGPL and could only ever run as separate programs. That is a later step (0.4+).

## Risks
- The 3D viewer is as much work as the solver: WebGL with lighting, picking faces and a section plane.
- Equation counts: a 3D part of 20 000 quadratic wedges has about 300 000 unknowns; the sparse Cholesky handles it in seconds but needs memory (about 1–2 GB at worst). A conjugate-gradient option may be needed for bigger models.
- Eigenvalue solvers (modal, buckling) are new numerics: faer has dense eigen solvers; a sparse shift-invert Lanczos is ours to write and test.

## Decisions (the user's answers, 2026-10-29)
1. 3D geometry: **extrude and revolve** the 2D parts. STEP and arbitrary shapes later, with an external mesher.
2. Order: **analyses first** (thermal stress, axisymmetric, natural frequencies, buckling, 3D frames), **then 3D solids**, then DXF import and Send to Calc/Plot.
3. The 3D viewer: **three.js** (MIT), recorded in NOTICE.
4. Releases: **small numbered releases**, each verified and reported.

So the order is 0.2.0 thermal stress, 0.2.1 axisymmetric, 0.2.2 natural frequencies, 0.2.3 buckling, 0.2.4 3D frames, 0.3.x 3D solids, then DXF and Calc/Plot. 0.1.5 (first use on Windows) waits for a person to try the app.
