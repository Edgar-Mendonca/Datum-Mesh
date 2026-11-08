# The plan from the two specifications (2026-11-08)

The user wrote two long specifications for Datum Mesh: structural analysis and design, and modelling, sketching and assembly. They were read in full and set against the code. This is what we take from them, in our own words, and in what order. Releases now go out only when the user has approved every piece of a batch from screenshots (docs/MEMORY.md).

## What we take

Everything a small team can build in our own solver and with our own geometry engine. The engine already registers about 570 modelling commands, of which the window uses about 75. Many modelling items therefore need only a button, a dialog and a test.

## What we leave out

- National design codes and their copyrighted tables. If one steel check comes at 1.0, it is written from a published edition we may use, with worked examples.
- Other programs' native files and database round trips. STEP, DXF, CSV and a CalculiX input file cover exchange.
- Offshore suites, blast, fire, fatigue, reliability; detailing (rebar schedules, connection design).
- Freeform and class-A surfaces, scan fitting, mould tools, sheet-metal unfolding, motion studies, 3D sketching, automatic midsurfacing, cyclic sectors, fluid domains.

## Rules we follow

1. **One way for every command:** select before or after starting, parameters with units, a preview, Apply / OK / Cancel. Cancel changes nothing. A button that is off says why.
2. **Separate statuses:** "input valid", "solve converged" and "check passed" are shown apart, never as one green "safe". Results go stale after an edit. What did not run says "not checked", never "pass".
3. **Icons:** our own 24×24 SVGs in four kinds (action, item, relation, status). Things that must never be confused get different shapes:
   - mass (a block) and force (an arrow);
   - member release (an open circle) and pin support;
   - rigid link (a thick line) and member;
   - uniform temperature and gradient;
   - linear and nonlinear analysis; modal and spectrum analysis.
   Status badges carry words, not colour alone.
4. **Honest results:** say whether values are averaged at nodes or per element; always show the deformation scale. Combinations are solved together, which stays correct for tension-only members.
5. **Load regions survive edits:** when a geometry edit makes a loaded face ambiguous, the window says so rather than guessing.

## Decisions of 2026-11-08 (after the third specification)

- **Four starting points:** 2D frame (beams and trusses), 3D frame (frames, trusses and plates in space), 2D part (a flat slice: plane stress, plane strain, axisymmetric) and 3D solid. Heat becomes a choice inside the 2D part, as it already is in the 3D solid; the separate "Blank heat part" tile goes.
- **Open formulas only.** Datum Mesh calculates from published mechanics and methods we may use; no licensed standard, its tables or factors enter the code. The third specification (Structural Analysis and Design v2) asks for steel checks to one national standard; we take from it everything that is plain mechanics and the honest way of reporting, and leave its code tables out. A design standard comes at 1.0, and only one that is free to use.
- Checks are labelled "mechanics checks, not a design code" until then.

## The roadmap from 0.10.2

**0.10.3 · Model care and frames** (one release when the user has approved every piece)
- Done on the branch: origin and eyes in the tree; an icon for each drawing way; the view cube; the workflow bar and Tools; standard parts; every planned feature in the window; point and part-length loads on members (benchmark 38); the four starting points; split, reverse and merge members, merge nodes; the model check (in the panel under the view); member orientation with local axes; loads and assignments tables; every planned feature shown on its ribbon tab with its release.
- To do: combinations by rule; mesh statistics and quality; solver settings; the icon distinctions and status badges; benchmark rows (axial bar, prescribed movement, offset force, round shaft in torsion, close modes, block, hole and tube volumes).

**0.10.4 · Design foundations, from mechanics formulas** (from the third specification)
- Design members: form one from a chain of members, dissolve it, inspect it; a selection mode for them.
- Restraints and segments: where a member is braced; segments that extra mesh nodes never change.
- Values at any distance along a design member, with both values at a jump.
- Section properties: A, I, Z (elastic), S (plastic), r and J, from the shape.
- Mechanics checks: stress against yield; axial force against the Euler load with a given effective length; deflection from the line between the ends against span over a chosen ratio; storey drift.
- Honest statuses (pass, fail, not checked, not supported, missing data), a calculation trace for every check and a report that lists what was not checked.
- Tests from the specification: F01 section properties, F07 segments under remeshing, F08 values at stations and jumps, F09 relative deflection with moving ends, F11 combined actions kept together, F12 unlike sections refused.

**0.10.5 · Geometry the engine already has** (the old Batch 2)
- Split a face; datum planes, axes and points; overlaps and clearances; exploded view; linked copies; place a part against another; sketch: project edges, open ends, tangent arcs, driven dimensions, freedom left; repairing an imported STEP; section view.

**0.11 · Results tools:** results tree and exact colour bands, several cuts, display groups, free-body forces, stress through a wall, compare analyses, export, graphs of any result and along a path, energies, the forces of the governing case, CSV in and out, report builder, video.

**0.12 · Materials:** orthotropic, kinematic hardening, Johnson–Cook, damage, rubber models, properties with temperature, fitting to test data, rate dependence.

**0.13 · Setup** (the old Batch 3): sets, area springs, six-direction releases and partial springs, inclined supports, eccentric offsets, rigid links, ties and couplings, connectors, bolt pretension, load curves, mass ledger, temperature gradients, equipment loads, Rayleigh damping, frame generators (portal, truss, building grid).

**0.14 · Analyses** (the old Batch 4): steady vibration, missing mass, line search, buckling with an imperfection, notional loads, heat in time, heat and stress together, sliding and self-contact, crash.

**0.15 · Meshing and jobs:** hexahedra, refinement by itself, convergence study, seeds, element types, job queue, restarts, all cores, input files, building in stages, Python console, templates.

**1.0 · Complete:** design checks to a standard that is free to use, plate reinforcement, wind, earthquake and moving loads, concrete and composites, several models in one file, every planned feature working.

## Done from the user's notes so far (on the branch, not released)

- The origin (point, axes, planes) in the tree, with show/hide eyes; eyes on sketches and bodies.
- Each way of drawing a shape has its own icon in the ▾ menus.
- The view cube: light and see-through; its edges and corners pick the views between faces (a corner is isometric).
- The workflow bar: a done step has a light green ground (no ticks); a Tools group with Parts, Library, Python (planned) and Options.
- Standard parts: hexagon bolts, nuts, plain washers and dowel pins in metric sizes, by click or drag onto the view.
- Every feature of Batches 1–4 is in the window as planned, with its release (126 in the register).
- Point loads and part-length loads on members (2D and 3D frames): along the member or projected, even or tapering; the beam is cut under each, so diagrams jump and kink exactly; loads follow when a member is cut by a new joint. Benchmark 38 (16 rows, all to 1e-9). A frame that rounding hid as a mechanism now says so (an answer that does not balance and moves further than the model is big).
