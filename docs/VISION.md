# Datum Mesh: vision and roadmap

Agreed with the user on 2026-10-31, after 0.3.5. This replaces the direction in `PLAN-0.3-0.4.md` from 0.4 on.

## The aim

**One open-source platform for structural and finite-element analysis, written by us, that proves its own answers.** Frames, plates and shells, solids, heat and vibration in one model; linear and nonlinear, static and dynamic, in one engine. Small, quick and simple to use, however much it grows.

What it covers, in three layers:

- structural analysis: frames, plates, load cases and combinations, P-delta, dynamics; no design codes;
- nonlinear finite elements: large deflection, plasticity, rubber, dynamics, contact;
- in time, general and explicit analysis: steps, explicit dynamics, impact.

It is **not** a CAD program and not a pre-processor for other solvers. Geometry is drawn simply (sections, extrude, revolve, DXF) or comes in from other tools (meshes, and later CAD files through a separate mesher).

## How it is built

**One engine: a core and modules.**

```
core       model, steps, sets, assembly, sparse solver,
           Newton and arc-length loop, time loop, eigen solver, results
frames     beams and trusses, hinges, P-delta and large rotation, tension-only, cables
plates     plate and shell elements (slabs, walls, thin parts)
solids     plane, axisymmetric and 3D elements (wedges, then tetrahedra and hexahedra)
materials  elastic, thermal, plasticity, rubber
heat       steady and transient conduction
contact    later
bench      the verification, a section for each module
```

- **Every analysis is a sequence of steps** (static, nonlinear static, frequency, buckling, dynamic, heat), applied to one model that may hold frames, plates and solids together. The window shows the steps; simple models have one.
- **Modules plug into the core through a small fixed interface**: each element gives its stiffness, its internal force and its mass for given movements; each material gives its stress and tangent for given strain. A module can change without touching the others, and its benchmarks run on every build, so a change in solids cannot quietly break frames.
- **One app, one file, one ribbon.**

## Our own solver

- We write the mechanics ourselves, from the published methods: K.J. Bathe, *Finite Element Procedures*, and the classic papers (Simo and Hughes for plasticity, Newmark and Hilber–Hughes–Taylor for dynamics, Crisfield for arc-length, Battini and Pacoste or Crisfield for corotational beams). **We do not port other programs' code**, permissive or not.
- Outside code is only infrastructure, like a maths library: faer (sparse matrices and eigenvalues, MIT) and, when large 3D models need it, METIS (ordering, Apache-2.0). GPL and AGPL programs (Gmsh, CalculiX, TetGen) are only ever run as separate programs, as optional add-ons, never in the installer.
- Other open projects (FieldCraft, FEBio) are read for what they verify and how they organise, not copied.

## Proving it

Every element, material and solution method gets checks against known answers before it ships, shown in Help › Verification and written to `docs/verification/`. The benchmarks planned for each release are in the table below; sources in `MEMORY.md` (Decisions, 2026-10-31).

## Roadmap

| Release | Work | Checked against |
| --- | --- | --- |
| **0.4.0** | **The core**: steps in the model, the Newton and arc-length loop, the engine split into modules, every check that passes today still passing. **First use: second-order (P-delta) and large-deflection frames**, by corotational beams | AISC 360 second-order benchmark problems (closed-form amplification); a cantilever rolled into a circle by an end moment (NAFEMS NL5); the elastica under an end load (NL6); Lee's frame snap-through (NL7) |
| 0.4.1 | **Plates and shells** for slabs and walls, joined to frames | Scordelis–Lo roof, pinched cylinder, Roark's plate formulas |
| 0.4.2 | **Dynamics for structures**: response spectrum and time history (Newmark) on frames and plates | single mass-spring closed forms, a beam's step response, NAFEMS forced vibration |
| 0.4.3 | **Structural details**: tension-only and compression-only members, support settlement, member offsets, rigid floors | hand checks |
| 0.5 | **Nonlinear materials for solids**: plasticity, rubber; tetrahedra; mesh import (CalculiX-format `.inp`, Gmsh `.msh`, STL) | Hill's thick cylinder (first yield and collapse), NAFEMS NL1–NL4, Rivlin's homogeneous tests |
| 0.5.9 | **Easier modelling, first step**: icons coloured by meaning (supports, loads, heat, geometry, mesh, materials, results); 3D frames drawn straight in the 3D view (snapping to nodes and to a work plane that turns through the last point; typed points) | UI checks; the real app |
| 0.6.0 | **Parts and assembly**: a model holds several parts, each sketched on a plane and extruded or revolved into a solid (shells and rigid surfaces come with contact, 0.6.2); each part placed (moved, turned) in the assembly and meshed on its own (structured meshes for swept parts). No CAD kernel: our own sketch tools and sweeps (decided 2026-11-02; a geometry library and an external mesher only if general shapes are needed later) | the swept-solid benchmarks, per part; placement checks |
| 0.6.1 | **Explicit dynamics**: central difference with lumped masses, prescribed velocities and displacements in time, energy balance | wave in a bar, a mass on a spring, energy balance |
| 0.6.2 | **Contact**: shell parts and rigid surfaces; rigid surfaces against parts (penalty), then self-contact; first use the axial crushing of a thin tube between rigid plates (force–displacement, folds) | Hertz, NAFEMS contact patch test, a tube's mean crushing force against published formulas |
| later | Implicit transient and transient heat for solids; shells that yield; composites and damage (impact on a plate) | NAFEMS T3; Taylor bar (against experiment, labelled so) |
| 1.0 | A validation report across everything; a signed installer | |

After 0.4.3 the frames-and-structures side is complete; from 0.5 the same engine grows into nonlinear and dynamic solids.

## Size

Datum Mesh itself stays small (under 5 MB today; the aim is to stay well under 20 MB as the engine grows). Optional add-ons (a mesher for CAD files) download separately.
