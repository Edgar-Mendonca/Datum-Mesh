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

## Batches

**Batch 1 (0.10.3): model care, frames and checks.**
- The origin and show/hide eyes in the tree (the user, first).
- Split, reverse and merge members and nodes.
- Point and part-length loads on members, along the member or projected.
- A model check that finds duplicates, loose ends and over-released members, and goes to each.
- Loads table, assignments table, mesh statistics and quality, combinations by rule, and solver settings in the step manager.
- New benchmark rows from the specifications: axial bar, prescribed movement, offset force, round shaft in torsion, CQC with close modes, and block, hole and tube volumes.
- The icon distinctions and the status badges.

**Batch 2 (0.10.4): geometry the engine already has.**
- Split a face; datum planes, axes and points.
- Check overlaps and clearances; exploded view.
- Linked copies of a part; place a part against another.
- Sketch: project edges, find open ends, tangent arcs and circles, driven dimensions, the remaining freedom shown.
- Repairing an imported STEP: repair, delete or replace a face, stitch.

**Batch 3 (0.13): setup.**
- Named sets that report when an edit breaks them.
- Area springs, with compression-only.
- Six-direction end releases and partial springs; inclined supports; eccentric offsets, rigid links, ties and couplings.
- A mass ledger; temperature gradients; equipment loads; Rayleigh damping with its curve.

**Batch 4 (0.14): analyses.**
- Steady vibration.
- Missing mass and scaling for spectra.
- Line search.
- Nonlinear buckling with an imperfection.

**Then:** results tools (0.11), meshing (0.15), design checks (1.0).

## Done from the user's notes so far (on the branch, not released)

- The origin (point, axes, planes) in the tree, with show/hide eyes; eyes on sketches and bodies.
- Each way of drawing a shape has its own icon in the ▾ menus.
- The view cube: light and see-through; its edges and corners pick the views between faces (a corner is isometric).
- The workflow bar: a done step has a light green ground (no ticks); a Tools group with Parts, Library, Python (planned) and Options.
- Standard parts: hexagon bolts, nuts, plain washers and dowel pins in metric sizes, by click or drag onto the view.
- Every feature of Batches 1–4 is in the window as planned, with its release (126 in the register).
