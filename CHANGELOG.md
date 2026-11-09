# Changelog

## 0.10.4 — 2026-11-09

Design foundations, from mechanics formulas: the real beams and columns of a frame, what they carry along their length, and checks that show their working. Not a design code: every factor is yours, and the report says what is not checked.

**Design members**
- Form a design member from members end to end in a straight line (Geometry › Form Design Member); it is found again after splitting or adding nodes, and members that bend, branch, leave a gap or differ are refused with the reason.
- Its card: the chain of members, restraints typed in (distance from the start; holds sideways, twisting or both) and the segments between them; Dissolve undoes it. A broken design member or a restraint off its length shows in the model check.
- The whole section table in Materials › Sections: area, second moments, elastic and plastic moduli, radii of gyration and torsion constant, both ways.

**Results › Design Members**
- Mechanics Checks: stress |N|/A + |M|/Z against the yield strength; the largest compression against the Euler load π²EI/(KL)² both ways (the weak way over the longest length between sideways restraints); deflection from the line between the member's moved ends against span over your ratio. Each combination is solved on its own, so N and M always come from one combination at one place.
- Statuses in words: pass, fail, not checked (and why), missing data (a yield strength, say); click a member for each check's formula and values; a list of what is never checked (lateral–torsional buckling, shear, local buckling, interaction, torsion, connections, any standard).
- Storey Drift: each storey's movement against the one below over its height, against your limit.
- Values Along: axial force, shear, moment and deflection at any distance along a design member, both sides of a point load or joint.

**Verification**
- Benchmark 40 (6 rows): the shear jump under a point load, PL/4 and PL/8 between nodes, 5wL⁴/384EI from the chord with a support sunk 20 mm, N/N_cr with a brace, and the drift of a cantilever column. All pass.

## 0.10.3 — 2026-11-08

Model care and frames: the user's specifications turned into tools, every piece shown and approved before this release. Calculations stay on open, published formulas; no design code's numbers are built in.

**Starting and modelling**
- Four starting points: 2D frame, 3D frame, 2D part (Stress or Heat) and 3D solid.
- The origin (point, axes and planes) in the tree, with show/hide eyes on it, on sketches and on bodies.
- Each way of drawing a shape has its own icon in the ▾ menus; the view cube is light and see-through, and its edges and corners pick the views between faces (a corner is isometric).
- The workflow bar: a done step has a light green ground; a Tools group with Parts, Library, Python (planned) and Options.
- Standard parts: metric hexagon bolts, nuts, plain washers and dowel pins, by click or drag onto the view.

**Frames**
- Point loads and loads over part of a member, even or tapering, along the member or per length of its projection; the shear drops straight down under a point load.
- Split, reverse and merge members, and merge nodes closer than a tolerance; loads, hinges and offsets go with them.
- Member orientation in 3D: an angle, or the section's depth facing a point or a direction, with each member's local axes drawn.
- A model check in the panel under the view: loose and nearly meeting nodes, overlapping members, pieces nothing holds, very short members; a row selects what it is about.
- Tables: loads (case, place, kind, direction, values) and assignments (kind, section, material, hinges, rigid ends, roll), typed into in place.
- Combinations by rule: cases marked permanent, variable or left out with their ψ factors, and your own γ factors; strength with each variable case leading, service and long-term, previewed before they are made.

**Solving and checking**
- Mesh statistics: elements, sizes, how many are good, fair, poor or bad, and the worst ones with where they are.
- Solver settings for frames' large-deflection steps: tolerance, iterations, smallest and most increments, growth.
- Status badges in words, kept apart: the model, the solve and the balance; none claims a design check.
- Things that must never be confused have their own icons (releases and pins, rigid links and members, uniform and gradient temperature, each kind of analysis step).
- Every planned feature shows on its ribbon tab, dimmed, with the release that brings it.

**Fixed**
- A 2D frame loaded only inside its members said it had no loads, and its balance check was skipped.
- A frame free to spin could be answered with huge movements when rounding hid the mechanism; it now says it can move without resistance.

**Verification**
- Benchmark 38: point and part-length member loads (16 rows); benchmark 39: an axial bar, a fixed–fixed beam with a sinking end, a force on an arm, a round shaft in torsion, CQC with close and far modes and a tube's volume (15 rows). All pass.

## 0.10.2 — 2026-11-08

The window brought to the approved mock-ups, screen by screen, and the first tables and managers.

**The look of the mock-ups**
- The tree: one dense tree of rows with icons, no numbered step circles; counts as plain grey numbers on the right; the ? and + only on the row under the pointer; wider to start with.
- The title bar: the app's name by its mark, New and Open, and in the middle the file with what it is (its analysis, linear or nonlinear, its units).
- Results in 3D on a dark ground with a faint grid; a finished solve opens the tree's Results tab.
- The Results tab is a tree: what was solved at its head; each result with its largest value (a frame's diagrams, deflection, reactions and utilisation; a 3D frame's colour-by results; a part's or solid's contours), a click showing it in the view; Checks (equilibrium, the smallest safety factor); Graphs and tables.
- The results legend is a dark card in the view: the result at its head, the colour bands running up its side with their values, the deformed shape, the cut and the balance check below; ▾ folds it to one line.
- The pop-up card is a property sheet: a label and its value on one line, small section heads, Delete at the foot.
- The bar above the view keeps only what works (the case, the materials, the steps, a frame's snap, shading, fit and zoom, planned features, units); the dimmed controls are gone from it, each still on its tab's Coming menu.
- The workflow bar has Python (planned) and Options at its foot.

**Tables and managers**
- A frame's nodes and members in tables in the bottom panel (Geometry › Nodes & Members): typed into as a sheet is, picked with the view, every change undoable.
- Paste from a sheet: rows of x, y (z) become nodes, rows of x, y to x, y members with their nodes, rows of two node numbers members between them; headings and rows not understood are left out and said in Messages.
- The material manager: a search over the materials, General, Elastic, Yielding and Heat tabs (Damage and Rate planned), and the stress–strain curve the solver uses beside them.
- The step manager: the starting state, then each step numbered with its status; every cell typed into in place.
- A time history: the bar above the view says when the results are, what drove it and its damping; the bottom panel opens Graphs, the movement against time beside the record as given.

**Size**
- The help cards' words load just after the window opens: startup code 482 to 453 kB. The whole window's budget is 1.8 MB (was 1.6 MB).

## 0.10.1 — 2026-11-07

Every module has its place: the tabs and the bar above the view of the approved mock-ups, and the whole map of features still to come, now over a hundred.

**New tabs**
- Assembly (3D solids): New Part, Move or Copy, Turn or Pattern, and what the assembly is made of; coming: repeating a part, checking overlaps, an exploded view, placing one part against another.
- Interactions (3D solids): Contact (two solids pressing and sliding with friction); coming: tie, coupling, rigid body, connectors, finding contact pairs by themselves, self-contact, friction by pair of materials.
- Animate (every model but heat): Play, To the Start, Loop, Speed, the player bar and the deformed shape.

**The bar above the view follows the tab**
- Materials: how many, and the Materials dialog; coming: colour by material or by section.
- Analysis: the steps and whether they are linear, and the Steps dialog; coming: show only the loads of a step.
- A frame's Geometry: snapping to the grid; coming: numbers, sections, releases and local axes drawn, snapping to nodes and midpoints.
- A 3D frame's results: coming: plate results on the top or bottom face.

**The whole map**
- 46 more planned features, each with its greyed dialog: paste from a sheet, node and member tables, splitting a member, portal frame, truss and building generators, rigid ends, sweep, loft, parameters, the material manager and assignments, rate dependence, creep, plate thickness by region, a loads table, inclined supports, water pressure, a force at a point away, seeds and element types, mesh statistics, the step manager, line search, damping, steady and random vibration, graphs of any result, results along a path, energies, storey drift, plate reinforcement, a report builder, video, your own units, templates and running many models.
- Every planned feature now carries the release of the agreed roadmap (0.10.2 tables and managers, 0.11 results tools, 0.12 materials, 0.13 setup, 0.14 harder analyses, 0.15 mesh, jobs and Python, 1.0 design checks); File › Roadmap lists them by release.
- The bottom panel shows Tables (0.10.2) beside Jobs and Python, dimmed.

## 0.10.0 — 2026-11-07

A new look, and the whole program laid out: every module has its place in the window, and the features still to come are already there, dimmed, with the release that brings them.

**The new look**
- A new accent, Petrol, through the whole window, light and dark; loads stay red so they stand out from it.
- A new mark: a tetrahedron, the element the solver uses, in the family tile. The Windows icons are made from it.

**The window**
- A ribbon tab for every module: Geometry, Materials, Loads & BCs (Temperatures & Heat for heat), Mesh (not for frames, which need none), Analysis, Results, View, Tools and Help.
- A bar above the view with what you change most: the load case, what a click picks, how the model is drawn, fit and zoom, and the units. The "Planned features" switch is there too.
- A narrow strip of the step's tools beside the tree.
- The tree has Model and Results tabs; step 6 shows the results there. A filter box at the top leaves only the rows that match.
- The bottom panel shows the Jobs and Python tabs that are coming, dimmed.

**The whole map (planned features)**
- About 60 planned features have their place: on their ribbon tab (under one "Coming" button), in their step of the tree, and in File › Roadmap by release.
- Clicked, a planned feature opens the dialog it will have, every field shown and greyed, with the release that brings it; nothing in it can run.
- View › Show planned features (or the switch above the view) hides them all; it is on to start with.

**File**
- Home: five blank models in a row (heat added), Examples and Roadmap as wide links, and recent models as rows.
- Examples in folders by kind (frames, 3D frames, parts and heat, 3D solids and assemblies, nonlinear, dynamics, verification), with a search across them all.

**Not in this release**
- The tube-crush example's first increment still does not settle; it moves to 0.11.
- Every module's own screens, greyed where not built yet, come in 0.10.1.

## 0.9.1 — 2026-11-07

A window laid out for the work: the steps down the left, the whole middle for the model, a card that pops up when you pick something, and a terminal along the bottom that follows every solve.

**Layout**
- The workflow bar at the far left: Geometry, Materials, Loads, Mesh, Solve, Results, each ticked when done (Mesh is greyed for frames, which need none). Click one and its tools open in the ribbon and the tree goes to it.
- The properties card pops up over the view when you pick something (a member, a load, a face, a feature) and closes with × or Esc, so the model has the whole width. The model's own settings open from its name at the top of the tree; step 6 opens the results card.
- The results card: what was solved, the balance check, each result's smallest and largest value (click one to show it), the smallest safety factor, and the way to the tables, graphs and report.
- The steps panel and the card widen by dragging their edge, and keep their width.
- The old side bar (Model, Mesh, Results, Report) is gone; the tables and the report have a bar with the way back to the model.

**The terminal (bottom panel)**
- Messages: everything the window tells you is kept with its time: each edit (added, changed, deleted, moved), each model opened, each solve started, finished, stopped or failed, every warning and error. Pop-up notes no longer vanish for good.
- Solver monitor: the solve as it runs: the load increment and the iteration, the load reached, how far from balance each iteration is (with a graph), the increments cut back and why, the time reached in an impact, how long it has taken and about how long is left. It opens by itself when a solve takes more than a second.
- Stop ends a solve at its next increment or time step, keeping what it reached, as a collapse does.
- Log: the same as plain lines, to copy.
- The engine reports its own progress (`progress.rs`), from the frame path follower, plasticity of parts and solids, large deformation, contact, impact and time history; the window only shows it.

**Fixes**
- The Mesh view of a 3D solid opened the flat drawing sheet and said there was nothing to mesh; it now draws the elements on the solid in 3D (Mesh › Show Elements).
- Opening a modelled example meshes it with Gmsh off the window's thread: the window no longer freezes for the ten seconds that takes, and says what it is doing.
- Pop-up notes sit above the bottom panel.

**Found with the monitor, not fixed yet**
- The tube-crush example's first increment does not settle (the force out of balance stays near the load for over 100 iterations, cut back five times): why that solve takes minutes. Next.

## 0.9.0 — 2026-11-07

Impact: fast events in a meshed solid, stepped in time.

**Explicit dynamics**
- Solve › Analysis › Impact (explicit dynamics), for meshed solids (modelled or imported): a load put on suddenly (or following a factor against time), or a solid that starts moving, stepped through time by central differences with lumped masses. The time step is found from the mesh's highest frequency and kept at 90 % of the stable limit; set how long to follow it in Solve › Steps (milliseconds: waves cross steel at about 5 km/s).
- Supports & Loads › Velocity: the whole solid (or one face, ▾) starts moving: a bar hitting a wall, a part dropped onto a fixed face.
- The player shows the solid moving in time; the stresses shown are at the moment it moves most, with the balance of loads, inertia forces and reactions then. Results › In Time graphs the largest movement and the energy moving, stored and together.
- New benchmark 37: a bar hit by a sudden end load (the end moves out to twice the static movement at 2L/c, within 1 % and 2 %; the energy kept within 0.1 %), and a block in free flight (exact).
- Our own solver (Bathe §9.2; lumped masses by HRZ), no outside code.
- In the real app a 200 mm steel bar hitting a wall at 10 m/s: the free end moved 0.375 mm (1D theory, 0.40 mm), the stress about ρcv ≈ 400 MPa, the energy kept to 100 %.

**Fixes**
- Switching a step to Impact gives it an impact's own time (1 ms, the stable step) instead of 2 s, which would take millions of steps.

**Not yet**
- Contact, yielding and large movements in time (crash): explicit steps are elastic and small-strain for now.

## 0.8.0 — 2026-11-07

Contact: solids that touch can press on each other and slide, held back by friction.

**Contact between solids**
- Supports & Loads › Contact: pick two solids that touch and a friction coefficient. They are meshed apart (before, touching solids were always joined) and every pair of faces where they meet gets a contact, listed under Contacts in the tree; the Inspector sets the friction and which face is kept out of which.
- Where the faces press they push each other apart; where they would pull they part; along each other they hold until the friction reaches μ times the pressing force, then slide (Coulomb). A solid held only by friction stays held; one held only by frictionless contact is reported as free.
- Results › Contact pressure, and a Contact table under Tables (the force through each contact, its friction, how many nodes touch and slide, the largest pressure and overlap).
- Our own solver: node to surface with a penalty (the faces may overlap by about a thousandth of the movement), friction as a return mapping, found by solving again until which nodes touch and slide stops changing.
- New benchmark 36: a block pressed onto another through meshes that do not match (the force through the contact exact, the stress in both within 2 %), a block dragged over another (friction μN within 0.2 %), and one pushed by less than μN (it holds; the friction is the push).
- Help › Contact between solids.

**Not yet**
- Contact is solved in one linear step: not yet with yielding, large movements or crushing (the folds of a crushed tube pressing on each other), nor bolt pretension; those come with the next steps.

## 0.7.7 — 2026-11-07

Loads off the part, a finer mesh where it matters, and more ways to read the results.

**Remote force**
- Supports & Loads › Remote Force: a force and a moment acting at a point off the face (a pin, a shaft's coupling, a load on a bracket's arm), carried onto the face you click. The face takes the force, and the moment the force makes from that point, spread as a traction that varies linearly over it, so it stays free to deform. It starts at the face's centre; move its point in the Inspector. It is drawn as an arrow at the point with a line to the face.
- New benchmark 35: a cantilever loaded through a remote force half a metre beyond its tip (deflection within 1 % of beam theory, bending stress within 2 %), and a square bar twisted by a remote moment (twist within 3 % of Saint-Venant torsion), each in equilibrium.

**Mesh**
- Mesh › Finer Near a Face: click a face and the solids are meshed again with elements a quarter of the size near it, growing back over a few elements; click it again to take it off. Its size is in the tree under Mesh, and it follows the face when the solids change.

**Results**
- Results › Along a Line: the result shown, graphed along a straight line through the solid (from one side to the other through its middle to start; type the ends).
- The section view under the results is labelled X, Y and Z as the view shows them (Z up).
- The report of a modelled solid names its solids and features, and says how it was meshed (it called the tetrahedra wedges before).

## 0.7.6 — 2026-11-07

More modelling, files in and out, and buildings laid out by level.

**Modelling**
- Sweep: a closed profile carried along a path drawn in another sketch, with an optional twist and a scale at the end (pipes, rails, springs).
- Loft: a solid through closed sections on different planes (a square base to a round top).
- Draft: click a side face to slope it by the draft angle, as a moulded or cast part needs.
- Split: a solid cut in two by a plane square to X, Y or Z; the pieces are meshed joined.
- Measure: click a face or an edge for its area, length, diameter or radius; click a second for the distance and angle between them. Mass lists each solid's volume, area, centre of mass and mass from its material.
- New Sketch ▾ lists the planes you have made (Offset Plane, Levels).

**Files**
- Import › STEP brings in the solids of a STEP file from another CAD program, ready to mesh and analyse (mesh files and DXF sections are under its ▾).
- Export saves the solids as a STEP file or an STL surface.

**Buildings**
- Levels: a plane at each floor and a grid of column lines on the ground, from storey height and bay sizes. New Sketch ▾ › On Level 1 draws that floor's beams at its height; the grid is dashed and never becomes members.
- Closed Shapes become Plates: a rectangle, polygon or circle drawn on its own becomes a slab or wall (0.2 m thick to start), not four beams round it.

**Ribbon**
- The Geometry ribbon fits again: Sweep and Loft, Draft and Split, and Parameters beside Offset Plane take small buttons; Sizes shows only when a tool has a size.

## 0.7.5 — 2026-11-07

3D frames drawn as they will be built, and solids made in one go.

**Frames drawn with their sections**
- Each member of a 3D frame is drawn as its section (I, channel, angle, box, tube, rectangle, circle) swept along it, turned as the solver turns it, so you can see which way up each member is. View › Section shapes turns it off.

**Primitives and Combine**
- Box, Cylinder, Cone and Sphere (Geometry › Primitives) are made in one go from a point and their sizes, and land in the history like any feature.
- Combine joins solids into one, cuts some out of another, or keeps only what they share.

**More sketch shapes**
- A rectangle with rounded corners (the radius beside the button), an ellipse (centre, end of the long axis, a point across) and a spline through the points you click (double-click or Enter on the last one).

## 0.7.4 — 2026-11-07

Frames drawn with the sketch tools, every example's animation checked, and heat on the modelled solids.

**3D frames drawn with the sketch tools**
- New Sketch on a 3D frame works as it does for a part: click a plane (the view turns to it), draw lines and arcs, size them with dimensions and constraints. Finish Sketch makes every line a beam (or a truss bar: Lines become Truss Bars), joined where they meet. Sketch on the next plane for the next frame.
- The plane sketch of before (members and plates drawn one by one) is still there as Members & Plates.
- The sketch starts at the scale of a building (about 10 m across).

**Planes the right way up**
- Sketching on the front or side plane now looks at it from the front (or the right) with Z up the screen: before, a shape drawn upward on the front plane ended up below the floor.

**Animations**
- Every example was solved and its animation checked against what the window draws (a new test runs them all). The two earthquake examples played their peak values as if they grew with a load: a response spectrum is not played now; its modes are (Results › Modes).

**Heat on the modelled solids**
- New example, Heat sink, modelled: five fins made from one sketch, the base at 80 °C and air at 20 °C on the rest. It solves with heat in balancing heat out.

## 0.7.3 — 2026-11-07

The solids you model can now be analysed: meshed, held, loaded and solved.

**Analysis of the modelled solids**
- Go to Supports & Loads (or Mesh, or press Solve) and the solids are meshed by themselves into 10-node tetrahedra by Gmsh, which is now installed with Datum Mesh. The bar over the view says how many elements and nodes.
- Click a face to hold it (Fixed, Pinned, Roller, Symmetry, Held directions) or to load it (Force, Pressure, Face force). A force on a face is shared over the face.
- Faces are named after the step that made them: "Body1: Extrude1 top", "Extrude1 side 3". Supports and loads stay on their faces when you change the design: it is meshed again before the next solve.
- Results as for any solid: von Mises, principal stress, displacement and safety factor in colour on the deformed shape, the cut plane, the probe, the player and the equilibrium check.
- Step 1 shows the solids as modelled; the later steps show the mesh. Mesh › Element Size sets the size; Mesh › Show Elements draws them.

**Crushing and collapse** (brought forward from 0.7.4)
- A new step, Crushing and collapse: yielding with large movements, so a part can be followed past its peak load as it folds. Supports & Loads › Move a Face pushes a face by a set amount, like a crusher's platen; Results › Load–Deflection then draws the force on it against its movement.
- New example, Tube crushed until it collapses: an aluminium tube pushed down 6 mm. The force peaks at its squash load σy A (57.3 kN) and falls as the wall folds.

**Examples modelled with the new tools**
- A bracket with a hole, a stepped shaft (revolved), the crushed tube, and a steel strip's natural frequencies. Each opens with its history, mesh, supports, loads and steps, ready to solve.

**No more 3D / Section switch**
- The view is always 3D. Solids made the older way (a section extruded or turned) have Edit Section to change their outline, and Finish Editing to come back.

**Fixes**
- The File page's Roadmap icon is the same colour as the others.

**How it is built**
- Gmsh (GPL) is installed beside Datum Mesh in its own folder and run as a separate program; Datum Mesh writes the solids out for it and reads back the mesh it makes. Checked: a steel cantilever modelled, meshed and solved agrees with beam theory to within 4 %.
- Crushing: total Lagrangian with small strains and large rotations (Bathe, *Finite Element Procedures* §6.6), von Mises yielding with the consistent tangent (Simo and Hughes). New benchmark 34: a cantilever bent far against the elastica (within 1 %), and a bar pulled past yield by its end face against the exact force (within 0.01 %).

**Not yet**
- Folds pressing on each other need contact (0.8): the tube is crushed 6 mm, not flat. Natural frequencies and heat work on the modelled solids (the strip example); buckling of solids and more examples come in 0.7.4. Finer mesh near chosen faces comes with the interactions in 0.7.7.

## 0.7.2 — 2026-11-06

Modelling that feels right: a history you can edit, sketching on any plane with the view turning to it, a grid that follows the zoom, and no leftovers of the old section view.

**History**
- The tree lists every feature in order (Sketch1, Extrude1, Hole1…), and a timeline under the 3D view shows the same, with buttons to step back and forth.
- Right-click a feature: Edit Feature (its sizes, changed in place), Edit Sketch, Rename (F2), Suppress, Roll Back Here, Roll to the End, Delete (Del). Double-click edits. Right-click a solid, in the tree or the 3D view: Sketch on a face of it, Rename, Delete.
- The browser's own right-click menu never shows any more.
- A feature picked in the history shows in the inspector, with Edit, Suppress and Delete.

**Sketching on a plane**
- New Sketch asks for a plane: click one of the three planes at the origin, or a flat face of a solid, or choose Floor, Front or Side.
- The view turns smoothly to face the plane, and you draw on it over the 3D view. Finish Sketch turns the view back to 3D, framing what was drawn.
- A closed shape finished goes straight on to Extrude: the solid is made at once, and the Extrude panel changes it as you type the distance, press − and +, or turn the mouse wheel over it; Out, In or Both ways; OK keeps it, Cancel takes it away.

**Grid**
- The floor grid has big squares with small squares inside, sized to how close you are looking and centred under the view. The sketch sheet uses the same rule.

**Solids**
- A solid made with the modelling tools has no 3D/Section bar and no section, thickness or sweep settings: those belonged to the old extruded-section solids, which keep them.

**Not yet**
- Supports, loads, meshing and solving on the new solids come in 0.7.3, laid out as the usual analysis steps.

## 0.7.1 — 2026-11-06

The Datum window again, laid out in numbered steps, with full sketching and 3D modelling.

**Steps**
- The tree on the left is laid out in the order a model is made: 1 Geometry, 2 Properties, 3 Supports & Loads, 4 Mesh, 5 Analysis, 6 Results. Each step shows ✓ when it is done or says what it still needs; click it and the ribbon shows that step's tools.
- New Properties step: materials (with Assign to the selection) and sections.

**Sketching** (Geometry › New Sketch, on the floor, front or side plane, on a face of a solid, or on an offset plane)
- Create: line; arc by 3 points or by centre and ends; circle by centre and radius, 2 points or 3 points; rectangle by 2 corners, centre and corner, or 3 points; polygon inside or around a circle or by one side, any number of sides; slot centre to centre or overall. Each shape's ▾ lists its ways; the way last used stays on the button. Construction lines (dashed guides).
- Modify: trim, extend, offset, fillet, chamfer. Pattern: mirror, circular, rectangular.
- Constrain: dimensions typed on the drawing (lengths, diameters, radii, distances and angles, or named parameters such as W / 2); coincident, tangent, parallel, perpendicular, equal, concentric, collinear, midpoint, symmetry, horizontal or vertical, fix. Free curves are drawn blue, fully defined ones black.
- Sketch report, parameters, snapping to the grid and to ends, a preview of each shape as it is placed.

**3D modelling**
- Create: extrude (out, in or both ways), revolve, hole (through or to a depth). Shapes drawn inside a closed shape become holes.
- Modify: fillet and chamfer (click an edge), shell and press pull (click a face).
- Arrange: move, turn or copy; mirror; pattern in rows and columns or round an axis. Construct: offset plane.
- The sketches and solids are kept in the .dmesh file, with autosave and recovery.

**How it is built**
- Sketching and solids come from SolidCraft's open-source geometry engine (MIT or Apache-2.0), working behind the Datum window; nothing of its own window is used.

**Not yet**
- Supports, loads, meshing and solving on these new solids come in 0.7.2 (the solids of 0.6 and the frames still analyse as before).

## 0.7.0 — 2026-11-05

**Withdrawn on 2026-11-06.** This release was the open CAD program it was built on, renamed, rather than a Datum app; it is no longer offered. 0.7.1 brings the Datum window back, laid out in numbered steps, with the CAD program's geometry engine working behind it.

A new start: Datum Mesh is now a full 3D modeller with the analysis laid out in steps.

**A new program**
- Datum Mesh is rebuilt on SolidCraft, the open-source CAD program (MIT or Apache-2.0), in the Datum look: the crimson accent and the Datum mark. It is quick and smooth, and it has a full set of modelling tools.
- Sketch tools with their variants:
  - line and arc chains;
  - circle by centre, by two points or by three points;
  - rectangle by two corners, by three points or by its centre;
  - polygon with any number of sides, drawn inside or outside a circle or by one edge;
  - slots.
  - Dimensions and constraints are typed, and the sketch is solved as you draw.
- 3D tools: extrude, revolve, sweep, loft, fillet, chamfer, shell, hole, patterns and mirror, with a history timeline you can edit. Parts can be joined into assemblies, and STEP files can be opened and saved.
- Design files are `.dmx`. Files from 0.6 and earlier (`.dmesh`) do not open in this version; 0.6.4 stays on the releases page for them.

**Simulation, step by step**
- The Simulation workspace (the workspace menu, top left) puts the analysis in order on one bar:
  1. **Material**: pick it from the library.
  2. **Supports & Loads**: click faces, then Fixed, Roller or Symmetry, or a force (kN, along X, Y, Z) or a pressure (MPa). Each set shows as a chip, and × removes it.
  3. **Mesh**: the element size (or automatic), then Mesh.
  4. **Solve**: says what is still missing, then solves.
  5. **Results**: von Mises stress or movement in colour bands, on the deformed shape, with a legend.
- The study is saved in the design and follows its faces by name.
- Parts are meshed into ten-node tetrahedra by Gmsh. Gmsh is installed beside the program and always runs as a separate program (GPL). The solve uses Datum Mesh's own verified engine and checks equilibrium every time.

**Not in 0.7.0 (they return in 0.7.x)**
- Frames, trusses and plane parts.
- Natural frequencies, buckling, heat, yielding, large deformation and time histories.
- The results player.
- The report.
- All of these remain in the engine and its 268 verification checks; the new window will reach them in the coming releases (docs/MEMORY.md, roadmap).

## 0.6.4 — 2026-11-04

Results that move: play any solved result.

**The player**
- A bar over the drawing of every solved result: Play and Pause, back to the start, a slider to any moment, the speed (0.25× to 4×) and Loop.
- A linear result grows from no load to the whole load: the shape, the colours and the diagrams in proportion, against the whole result's colour scale.
- A nonlinear result (yielding, large deformation, large deflection, past the limit) plays through its increments, the slider showing the load factor; its colours (von Mises stress) grow as it yields.
- A time history plays through its times, the slider showing the time.
- A natural frequency or buckling mode swings, at the speed set; the slider holds it at any point of its swing.

**Engine**
- Nonlinear solves and time histories keep up to 60 frames for playing: the movements of the drawn points (and von Mises stress for parts and solids) at each.

## 0.6.3 — 2026-11-03

Basic modelling: shape and arrange parts in 3D.

**Extrude**
- Finish Sketch on a new part opens the Extrude panel over the 3D view: extrude or revolve, the distance or the angle, and which way (out of the sketch, into it, or both ways evenly). A preview follows as you type; OK keeps it. Geometry › Extrude (or Extrude… in the Inspector) opens it again for the part selected. The Inspector also has Which way.

**Sketch on a face**
- New Sketch, then click a flat face of a part: the new part starts on that face and goes out of it. On a front or back face, Cut Through on the sketch bar cuts the sketch's outlines right through the part: a hole where an outline lies inside it, a notch where it runs across its edge.

**Features** (Geometry › Features)
- Hole: set Ø and click a part's front or back face; the hole goes right through.
- Fillet and Chamfer: set the size and click a part near an edge along its depth; that edge is rounded or bevelled all the way along.
- Cut: select a part and cut it right through another; its outlines become holes there.

**Arrange** (Geometry › Arrange)
- Move or Copy (a row of copies), Turn or Pattern (about an axis through a point; copies round it make a ring), Mirror (in a plane, as a copy or in place). Steps, points and axes are typed with Z up. Parts are picked by clicking them in 3D (Shift or Ctrl to add); the ones picked are tinted.

**Volume and mass**
- The Inspector gives each part's volume and mass, worked out exactly from its sketch (area × depth, or Pappus' theorem when turned).

**Verification**
- Benchmark 33: a reversed block, a plate cut through, drilled, rounded and notched, a ring and a torus turned, a mirror copy and a ring of copies; exact volumes and meshed volumes against hand formulas.

**Fixed**
- A selected part was let go of after every change.
- An automatic element size gave every part about 15 000 tetrahedra, so three small copies went over the node limit; the parts now share one budget by volume.
- When the parts cannot be meshed together, the 3D view shows their outlines (and the bar says why) instead of a misleading picture.
- A garbled file whose part's material was dropped would not open.
- The 3D frame's Repeat took its step in y-up axes; it is now X, Y, Z with Z up, as shown elsewhere.

## 0.6.2 — 2026-11-03

Sketch on a plane: drawing in 3D starts with a sketch, square to you, on a plane you pick.

**New Sketch**
- Geometry › New Sketch, then click one of the three squares at the origin (Front X-Z, Side Y-Z, Top X-Y) or a button on the bar. The view turns square to the plane and the sketch lies over the model, lined up with it, so what you draw sits where it goes in 3D.
- A Sketch tab opens with the drawing tools, and Finish Sketch and Cancel. Finish keeps what you drew; Cancel takes back every change made while sketching.
- The sketch bar names the plane, moves it along its normal (type the offset, or pick a plane that already has nodes), takes typed points (across, up, or @d-across, d-up from the last point), and turns the view: Turn the View looks round the model in 3D with the sketch kept open, starting from the sketch's side; Look At comes back square to it.
- A line nearly level or upright with the last point snaps level or upright. Sizes are typed, not solved: there are no sketch constraints to keep.

**3D solids**
- New Sketch makes a new part on the plane picked and opens its sketch with the Rectangle tool; Finish selects the part so its depth or revolve angle can be set in the Inspector; an empty sketch makes no part. Edit Sketch in the Inspector opens a part's sketch the same way.

**3D frames**
- Beams, truss bars, nodes and plates are drawn in a sketch on a plane too (the Sketch tab has Line (Beam), Truss Bar, Node and the plate tools). The Front/Side/Plan bar above the 3D view, Tab to turn the plane, and the 3D typed X, Y, Z point give way to the sketch.
- While sketching, the pointer and the status bar give coordinates with Z up.

## 0.6.1 — 2026-11-03

A cleaner start: examples in their own place, and 3D with Z up, a view cube and planes to draw on.

**Examples**
- File › Home shows the blank models to start from; the worked examples have their own page, File › Examples, under headings (frames and trusses, 3D frames, plane parts and heat, 3D solids and assemblies, nonlinear, dynamics), each from simple to harder. The Examples button on the Home tab lists them by heading too.

**3D, Z up**
- 3D models are shown with Z up, as CAD programs do: the floor is the X-Y plane. Coordinates, loads, supports, part placement and results are shown and typed that way. Files are unchanged, and plane frames and sections keep x across and y up.
- A view cube at the top right: click Front, Back, Left, Right, Top or Bottom to look from that side; the house goes back to the corner view. An axes key at the bottom left turns with the view.
- A floor grid that stays the same size as you draw and fades into the distance, with the axes at the origin.
- 3D models open in the 3D view. A blank 3D solid offers a new part; a blank 3D frame shows three small planes at the origin.

**Drawing 3D frames**
- Pick Beam, then a plane to draw on: one of the three squares at the origin (Front X-Z, Side Y-Z, Top X-Y), or the bar above the view. The view turns to face the plane, and still turns freely. The plane is drawn at a fixed size, faint, and the rest of the model stays in view.

## 0.6.0 — 2026-11-02

Parts and assembly: 3D models made of several parts.

**Parts**
- Geometry › New Part adds a part and opens its sketch. Draw its outline and holes with the usual tools, then extrude it by a depth or revolve it about its axis (a full turn for a ring, a tube or a shaft).
- Each part has its own material and element size, and is moved and turned into place in the Inspector. The tree lists the parts; 3D shows each in its own colour; Sketch shows the sketch of the part picked beside it.
- Parts that touch are tied together where they touch, so they act as one. Supports and loads go on a part's faces: click the face in 3D.
- Static stress, natural frequencies, heat and plasticity solve the whole assembly. Each part is meshed into tetrahedra on its own and the parts are joined.
- A new example: an aluminium tube standing between two steel plates, the first step towards crushing a tube.

**Fixed**
- Opening a saved 3D frame dropped every member running along z (depth), and the loads on them: a member's ends were compared by x and y only. Every example now opens exactly as it was saved, checked by a test.
- An empty 3D frame showed a large tilted grid off to one side: the floor and the work plane now cover the same space, and the plane stands on the floor.

**Trust**
- Help › Verification: 257 checks, all passing. New: benchmark 32, assemblies: a cantilever sketched, extruded and turned into place (within 0.8 % of beam theory), the same cantilever as two parts tied end to end (within 0.02 % of one part), a bar of steel and aluminium in series (exact to 2 × 10⁻⁶), a thick cylinder's wall revolved as a part (Lamé, 2 × 10⁻⁶), and heat through two parts of different conductivity (3 × 10⁻⁶).

## 0.5.9 — 2026-11-02

Easier to find your way, and 3D frames drawn in 3D.

**Draw 3D frames in the 3D view**
- Pick Beam, Truss or Node and click in the 3D view. Points snap to nodes, else to the grid on a work plane; each click goes on from the last point, and the plane moves through it.
- Tab turns the work plane (Front, Side, Plan) through the last point, so the next member can go off in any direction, along z too.
- Type a point and press Enter: x, y, z, or @dx, dy, dz from the last point. The point under the pointer shows its coordinates and the member's length. Esc stops at the last point.
- An empty 3D frame opens on a view of a few metres of space, ready to draw in. Front, Side and Plan still show one plane flat, for drawing many members on it.

**Icons coloured by meaning**
- Supports are green, loads red, heat orange, geometry and parts blue, the mesh teal, materials amber, solving green and results in their own colours, in the ribbon and the model tree alike.

**Public text**
- Product and file-format names are trademarks of their owners and are used only to describe file compatibility (README, NOTICE). The checks that keep commercial product names out of the app's text now cover its documents and the whole window.

## 0.5.8 — 2026-11-01

3D frames turn far.

**Large rotations in 3D frames**
- A large-deflection step on a 3D frame now follows large rotations in space: every beam is cut into pieces that turn as rigid bodies and bend a little on their own (corotational beams), so bending both ways, twist and their coupling are all taken in. Before, a 3D frame's large-deflection step was second order (P-delta) only.
- Past limit loads (arc length) works for 3D frames too, so snap-through shows in 3D as it does in plane frames.
- Results › Load–Deflection for 3D frames: the load against the movement in x, y or z of the node the step follows, or how far it has turned; the deflected shape is drawn true to size. Send to Datum Plot takes the curve.
- A new example, the curved cantilever bent out of its plane: a 45° bend pushed down at its tip, bending and twisting far.
- With plates in the model, a large-deflection step on a 3D frame stays second order (P-delta), as before.

**Trust**
- Help › Verification: 246 checks, all passing. New: benchmark 31, large rotations in 3D frames: a cantilever rolled a quarter turn about either axis (within 0.05 % of the elastica), a skew cantilever bent far by a load across it (within 0.1 %), a bar twisted 2 rad (exact), and the 45° bend of Bathe and Bolourchi (within 1.1 % of the published tip, whose sources differ by about 1 %). Lee's frame built in 3D snaps through at the plane frame's limit load.

## 0.5.7 — 2026-11-01

Imported meshes yield and stretch.

**Nonlinear steps on imported meshes**
- Solve › Steps on a mesh of tetrahedra: an **elastic-plastic** step (von Mises with hardening, up to collapse) or a **large-deformation** step (rubber, and parts that move far), as for swept 3D solids, with the plastic strain, the load–deflection curve and true-size drawing.
- The same solvers serve swept and imported solids: they now take any mesh's integration points, so each improvement reaches both.

**Trust**
- Help › Verification: 233 checks, all passing. New: benchmark 30, the earlier nonlinear answers on 10-node tetrahedra: the thick cylinder's collapse pressure (within 0.3 % of Hill), a bar hardening past yield (exact), a rubber bar stretched to twice its length (exact), and a cantilever bent far (within 0.12 % of the elastica). Benchmarks 26 and 27 give the same numbers as before to the last digit.

## 0.5.6 — 2026-11-01

Imported meshes vibrate and conduct heat.

**Natural frequencies of imported meshes**
- Solve › Natural Frequencies on a mesh of tetrahedra: the lowest frequencies and their mode shapes, with consistent masses (each material's density), shown swinging in 3D.

**Heat conduction in imported meshes**
- Set Analysis to 3D heat conduction (Inspector, nothing selected): fixed temperatures, heat flux and convection on the mesh's faces, heat made inside; temperatures, heat flux and the heat balance.
- New example: the pipe elbow carrying hot oil, cooled by air.

**Trust**
- Help › Verification: 226 checks, all passing. New: benchmark 29: a bar's stretching vibration (exact to 2e-8), a cantilever's bending vibration (within 0.3 % of beam theory, its two equal modes equal), a plate with heat made inside (exact), and radial conduction through a thick cylinder wall (temperature within 0.003 %, heat flow within 0.001 %).

**Fixes**
- The colour bar labelled a narrow range with the same number on every band (114.1 to 114.5 °C read 114): its labels now carry the digits that tell them apart.
- The model's name could be crowded out of the top of the tree by its analysis.

## 0.5.5 — 2026-11-01

Parts of any shape: tetrahedra and imported meshes.

**Import a mesh**
- Geometry › Import › **Mesh** (or File › New › 3D solid › From a mesh file): a tetrahedral mesh made in a mesher, as Gmsh `.msh` (ASCII, format 2.2 or 4.1) or `.inp` in the CalculiX input format (C3D4 and C3D10), in the length unit you choose.
- The outer surface is split into faces where it turns more than 35°, and the file's named surfaces (Gmsh physical surfaces, `*SURFACE` and `*NSET`) keep their names. The faces are listed in the tree and shown in 3D.
- Supports (fixed, pinned, roller, symmetry, held directions) and loads (pressure, face forces, gravity) go on faces: pick the tool, click a face. The mesh's material is chosen in the Inspector.
- Solved with our own 4-node and 10-node tetrahedra for static stress: contours, deformed shape, cutting plane, probe, reactions and the equilibrium check, as for every 3D solid.
- New example: a steel pipe elbow meshed in 10-node tetrahedra.

**Trust**
- Help › Verification: 219 checks, all passing. New: benchmark 28, tetrahedra: a bar pulled (exact for both elements), a cantilever against beam theory with shear (within 0.8 %, its clamped root a little stiffer), and a thick cylinder against Lamé (hoop stress within 0.1 %, bore growth within 0.02 %).
- Mesh files are read with checks; a fuzz test throws 6 000 damaged `.msh` and `.inp` files at the readers.

**Window**
- Printing and the report load when first opened, so the window starts lighter.

## 0.5.4 — 2026-11-01

Datum Mesh talks to the rest of the family.

**Datum Link**
- Results › **Send to Datum Plot**: a load–deflection curve, a time history or the natural frequencies, as a table (SI units) that opens in Datum Plot for graphs.
- Results › **Send to Datum Calc**: the largest stress, movement, moment or temperature, the smallest safety factor and the lowest frequency, as values with units for a hand check.
- Materials › **From Datum Atlas…**: a material sent from Datum Atlas joins the model with its values and source, checked first.
- When the other app is not installed, the data is saved and copied, ready to paste, and the message says where to get it. The packet format is shared by all eight Datum apps (`docs/LINKS.md`).

**Window**
- Clearer icons for tools that looked alike: plane stress and plane strain, heat in 2D and 3D, temperature change and edge temperature, rollers along x and y, the moment, shear and axial diagrams, utilisation, mesh quality and mesh lines, reports.
- Fixes: on the Loads tab the loads group's name was pushed out of sight by its column of four buttons.

**Trust**
- Every packet read from another program is checked; a fuzz test throws 3 000 damaged ones at the reader.

## 0.5.3 — 2026-11-01

A clearer model tree, and a guided start.

**The model tree**
- Every branch and every item has its own icon (beams and truss bars, nodes, shapes and holes, materials, sections, each kind of support and load), and each branch folds with a guide line down its items; long lists start folded.
- Each branch says whether it is done (✓) or needs something (!), and why when you point at it: no supports yet, no loads, a material without the yield strength an elastic-plastic step needs, results out of date.
- An empty branch shows what to add and starts the right tool when clicked.
- New branches, in the order a model is built: loads grouped by load case, combinations, the analysis steps, and the results (the drawing, the tables and equilibrium, the report).
- Right-click an item to delete it. The ? and + on every branch stay.

**New study**
- File › New (Ctrl+N) asks what you are analysing (a 2D truss, 2D frame, 3D frame, 2D part, 3D solid, or heat in a part or solid), which kind (plane stress, plane strain or turned; extruded or revolved) and what you want to know (static, large deflection, plastic, rubber, earthquake…), then makes the model with that analysis set up and the right drawing tool ready.

## 0.5.2 — 2026-11-01

Rubber, and 3D solids that move far.

**Large deformation of 3D solids**
- Solve › Steps › **Add a large-deformation step** on a 3D solid: it is followed through large movements and strains, every material a neo-Hookean solid with its own E and ν. Rubber stretches and squashes as it really does; steel parts bend far while staying elastic. Newton's method on the deformed solid (total Lagrangian), the load in increments.
- The deformed shape is drawn true to size; stresses are true stresses on the deformed solid; Results › **Load–Deflection** draws the load against the largest movement.
- Help › Rubber and large deformation.
- New example: a soft rubber finger curling down under a push at its tip.

**Trust**
- Help › Verification: 208 checks, all passing. New: benchmark 27, a rubber bar stretched to twice its length and squeezed to 0.6 of it (stretch, thinning and true stress exact to 1e-14 against the neo-Hookean answer), and a slender cantilever bent far by an end load against the elastica (tip within 0.1 % down and 0.4 % in), balanced about the bent shape.

## 0.5.1 — 2026-11-01

3D solids that yield.

**Plasticity in 3D solids**
- Solve › Steps › **Add an elastic-plastic step** now works on 3D solids too, extruded or revolved: von Mises with each material's hardening, the load in increments, each solved by Newton's method with the consistent tangent; B-bar keeps the wedge elements from locking as they yield. If the solid cannot carry the whole load it collapses, and the solve says at what share of it.
- Results: **Equivalent plastic strain** on the 3D view, and Results › **Load–Deflection** for 3D solids.
- New example: a holed steel plate in 3D pulled past yield, the plastic zones spreading across the net section from the hole.

**Trust**
- Help › Verification: 197 checks, all passing. New: benchmark 26, plasticity in 3D solids: the collapse pressure of a thick cylinder held between its end faces (within 0.4 %) and of a thick sphere revolved a slice (within 0.6 %) against Hill's limit analysis, and a bar pulled past yield with hardening (stretch and plastic strain exact, balanced to 3e-13).

## 0.5.0 — 2026-11-01

Parts that yield: the first nonlinear material.

**Plasticity**
- Solve › Steps › **Add an elastic-plastic step** on a part (plane stress, plane strain or axisymmetric): the load goes on in increments and, where the von Mises stress reaches a material's yield strength, it yields, hardening as its new **Hardening** value says (Materials; 0 for perfectly plastic). The stress spreads to the rest of the part.
- Each increment is solved by Newton's method with the consistent tangent of the von Mises return mapping; plane strain and turned parts use B-bar so yielding, which keeps the volume, does not lock the elements. If the part cannot carry the whole load it collapses, and the solve says at what share of it.
- Results: **Equivalent plastic strain** contours, and Results › **Load–Deflection**: the load against the largest movement, with when it first yields.
- New example: a thick steel tube yielding under internal pressure.

**Trust**
- Help › Verification: 192 checks, all passing. New: benchmark 25, the collapse pressure of a thick cylinder in plane strain (within 0.4 %) and of a thick sphere (axisymmetric, converging to within 0.6 %) against Hill's limit analysis, and a strip pulled past yield with hardening (stretch and plastic strain exact).

## 0.4.5 — 2026-11-01

Rigid end zones at joints.

**Member end offsets**
- Inspector › **Rigid end at the start / end**: the length of a beam inside a deep column (or of a column inside a deep beam) where it cannot bend. The member bends only between its rigid zones, which are drawn thick; a hinge sits at the face of the joint. They act in every solve: static, second order, large deflection, natural frequencies, buckling and dynamics, plane and 3D.
- The frame in an earthquake now has rigid ends of half a column's depth on its beams.

**Fixes**
- Time histories keep their balance check sharp on frames with very stiff parts (rigid zones, rigid floors): each step is refined with the same factors.

**Trust**
- Help › Verification: 188 checks, all passing. New in benchmark 24: a cantilever with a rigid zone at its root (tip deflection over its flexible length, within 0.001 %; root moment exact) and a 3D beam fixed at both ends with rigid zones under a uniform load (within 0.001 %, balanced).

## 0.4.4 — 2026-11-01

Rigid floors for buildings.

**Rigid floors**
- Model › **Rigid floors** (3D frames): every floor, the nodes at one height above the lowest, moves as one rigid plate in its own plane, sliding along x and z and turning about y together, as a concrete floor ties a building's columns. It acts in static solves, second order, natural frequencies and dynamics, so floors sway and twist as one in an earthquake.
- New example: a three-storey steel building in 3D with rigid floors and their masses, shaken by a design spectrum: its modes sway along z, twist, and sway along x.

**Polish**
- Results › Dynamics shows modes that move no mass along the shaking as 0, not as round-off.
- Pictures for the newer examples on the start page.

**Trust**
- Help › Verification: 184 checks, all passing. New in benchmark 24: four cantilever columns tied only by a rigid floor and pushed at one corner, against rigid-diaphragm statics (the floor's sway and twist, within 0.0001 %), and the floor's ties balance among themselves.

## 0.4.3 — 2026-11-01

Structural details: bracing that only pulls, supports that settle, and second order in 3D.

**Members that only pull or only push**
- Inspector › **Carries**: tension only (a rod, a cable, a slender brace) or compression only (a strut on a bearing). Under the loads, a member that would carry the other kind of force drops out and the frame is solved again, until nothing changes. Slack members are drawn dashed and counted under the drawing.
- New example: a two-storey frame braced with crossed rods that only pull, under wind, with one support settled.

**Support settlement**
- Loads › **Settlement**: a support that sinks or slides by a set amount, in the directions it holds; the moments and reactions it causes are in the results. It belongs to a load case and is scaled by combinations. Plane and 3D frames.

**Second order in 3D frames**
- A large-deflection step on a 3D frame now solves it to second order (P-delta): every member's axial force softens or stiffens it, found again until it settles; the frame balances in its deflected shape.

**Trust**
- Help › Verification: 181 checks, all passing. New: benchmark 24, an X-braced panel with tension-only braces against statics (exact, both ways), a beam on a compression-only post that lets go (exact), a propped cantilever whose prop sinks, plane and 3D (exact), and a 3D flagpole bent about its weak axis to second order against the closed form (within 0.0001 %), balanced in its deflected shape.

## 0.4.2 — 2026-10-31

Dynamics for structures: response spectrum and time history.

**Dynamics**
- Loads › **Mass**: a mass at a node, for natural frequencies and dynamics (it is in every load case and adds no weight to a static solve). Natural frequencies now include point masses, also on weightless members.
- Solve › Steps › **Add a response spectrum**: shake a frame (plane or 3D, with its plates) along x, y or z by a design spectrum you type as (period, acceleration) points; choose the damping, the number of modes and CQC or SRSS. The peak forces, moments, movements and reactions are combined from the modes.
- Solve › Steps › **Add a time history**: a factor on a load case against time, or the ground's acceleration against time, followed step by step (Newmark's average acceleration, Rayleigh damping at the first two modes). The model shows the results at the time of the largest movement, with the inertia forces among the loads so the reactions balance.
- Results › **Dynamics**: a spectrum's modes with their periods, spectral accelerations, participation and share of the mass (aim for 90 %); a time history's movement against time and every node's largest movement.
- New example: a three-storey steel frame in an earthquake.

**Trust**
- Help › Verification: 170 checks, all passing. New: benchmark 23, a mass on a cantilever column against the closed forms (frequency exact; a step load's peak 2H/k, the ground stepping, resonance with 5 % damping, all within 0.01 %; a flat spectrum's movement and base shear exact), a two-storey shear frame's frequencies and spectrum (within 0.003 %), and the dynamic balance of a time history on the floor slab with its plates.
- Random dynamic steps on every frame example solve and balance, or say why.

## 0.4.1 — 2026-10-31

Plates and shells: slabs, walls and panels in 3D frames.

**Plates**
- On a working plane of a 3D frame (Front, Side or Plan, at a depth), Geometry › **Plate**: draw a rectangular, polygonal or round plate, with openings. Set the thickness for new plates on the ribbon, or each plate's in the Inspector.
- Plates bend (thin-plate theory) and carry forces in their own plane, and join the frame: frame nodes on a plate become its mesh nodes, and beams along a plate's edge are joined to it all along. Plates on different planes join along an edge they share.
- Supports on a plate's edges (fixed or pinned), Loads › **Plate Pressure** on the whole plate, and self-weight. Plates are meshed into triangles automatically (Mesh › Element Size).
- Results: in the 3D view the plates are coloured by von Mises stress (the worse face), movement, moments or membrane forces, on the deflected shape; the Results page lists their largest and smallest values and where. Natural frequencies include the plates, and their mode shapes move in the 3D view.
- New example: a floor slab on four columns with edge beams.

**Trust**
- Help › Verification: 157 checks, all passing. New: benchmark 22, square plates simply supported and clamped (Navier's series and Timoshenko's tables, within 0.6 %), a panel bent in its own plane (exact elasticity, 0.5 %), the Scordelis–Lo roof (0.8 %) and the pinched cylinder (0.4 %) from the standard shell problems, a slab drawn in a 3D frame (0.3 %, its edge reactions exact), and the slab's first natural frequency (0.3 %).
- Random plates on random planes, with random edge supports and pressures, solve and balance or say why.

## 0.4.0 — 2026-10-31

The engine grows a core: analysis steps, and frames that bend far.

**Large deflection and P-delta for frames**
- Solve › **Steps**: solve a plane frame in steps. A **large-deflection** step applies the load in increments and finds the frame's balance in its deflected shape: second-order (P-delta) effects, large rotations, and the loss of stiffness as a frame nears the most it can carry. A step **past limit loads** follows the path by arc length, over the largest load and down again, so a frame that snaps through shows it.
- Each step has its load (a case, a combination, or the one picked on the Loads tab) times a factor, its increments, how many pieces each beam is cut into, and the node its curve follows. Steps carry on from where the one before ended.
- Results › **Load–Deflection**: the load against a node's movement or rotation through the solution, with the first limit load named and the unstable part of the path marked.
- The deflected shape of a large-deflection solution is drawn true to size; the diagrams, reactions and equilibrium check are for the deflected frame.
- New example: a snap-through frame (Lee's frame), solved past its limit load.

**The engine**
- One engine of a core and modules: the core (sparse solver, eigenvalues, load cases, and a new path solver: Newton and arc length) with frames and solids as modules on it. The vision for what comes next is in docs/VISION.md.

**Trust**
- Help › Verification: 149 checks, all passing. New: benchmark 21, a beam-column and a flagpole against the closed-form second-order answers of AISC 360 (within 0.13 %), a cantilever rolled into a full circle by an end moment (NAFEMS NL5; the tip comes back to the root exactly), a column bent far past buckling against the exact elastica (NAFEMS NL6, within 0.2 %), and Lee's frame snapping through (NAFEMS NL7: first limit PL²/EI = 18.56 against 18.54).
- Damaged files with bad steps or a revolve angle out of range open with the bad values put right, and say so.

## 0.3.5 — 2026-10-30

Sections from CAD.

**DXF**
- Geometry › **DXF**: import a part's section drawn in CAD (ASCII DXF). Lines and arcs that meet end to end, polylines with their arcs, and circles become outlines; outlines inside others become holes; arcs stay arcs. Units come from the drawing or are picked when importing. Splines, ellipses, text and blocks are left out, and the import says how many. One step to undo.
- File › Export › **Section as DXF**: the part's outline and holes as closed polylines and circles in millimetres, for CAD.

**Fixes**
- Tools marked as planned no longer give release numbers that have passed.

**Trust**
- Help › Verification: 134 checks, all passing. New: benchmark 20, every example's section written to a DXF and read back (areas the same to 10⁻⁹), and a drawing of loose lines and arcs in inches joined into a plate with a slot and a hole (areas exact).
- Damaged DXF files (cut short, lines dropped, numbers garbled or swapped: 300 of them) are read as far as they go or refused with a reason, never a crash, and nothing read is not a number.

## 0.3.4 — 2026-10-30

3D solids conduct heat and vibrate.

**3D heat conduction**
- Home › **3D Heat**: a part's section extruded or revolved, as for a 3D solid, solved for steady heat conduction. Fix temperatures on faces, add heat flux, convection to air (on side faces and the back and front faces) and heat generation, by clicking faces in the 3D view or edges in Section.
- Temperatures and heat flux (in x, y, z and its size) coloured over the part, with the cutting plane to see inside, the probe, and a Results page with the largest and smallest values and the heat flowing through each fixed temperature and each load, adding up to the heat balance.
- New example: the heat sink as a 3D solid, cooled by air on every face.

**Natural frequencies of 3D solids**
- Solve › **Natural Frequencies** now works for 3D solids, extruded or revolved, with consistent masses: the lowest frequencies are listed under the 3D view and each mode swings on the part's surface. Six frequencies of the 3D L-bracket (18 000 unknowns) take about 2 s.

**Trust**
- Help › Verification: 131 checks, all passing. New: benchmark 19, a wall with fixed face temperatures (exact), a pipe wall revolved a quarter turn (temperature and heat flow within 0.001 %), a pin fin cooled by air against the fin equation (0.03 %), a slab heating itself (exact), and three vibrations: a bar's axial frequency (exact to 10⁻⁸), a 3D cantilever's bending frequency (0.03 %) and a revolved shaft's (0.1 %).
- Random 3D solids, extruded and revolved, now also conduct heat (their heat always balances) and vibrate (their frequencies positive and rising), or say why they cannot.

## 0.3.3 — 2026-10-30

Frames for everyday structural analysis: load cases and combinations, hinges, steel section tables.

**Load cases and combinations**
- Loads › **Cases and Combinations**: name load cases (dead, imposed, wind, …) and build combinations of them, each case times your own factor. The loads already drawn go into the first case.
- Pick a case or a combination on the Loads tab: only its loads are drawn, new loads go into the picked case, and Solve (and Buckling) solve it. The legend says which one the results are for. A load's case can be changed in the Inspector.
- Results › **Envelope**: every combination solved in turn, with each member's largest and smallest axial force, its largest shear, moment, twisting and deflection, its largest utilisation and the combination that gives it, and the range of every reaction.
- Datum Mesh does not apply a design code: the factors are yours.

**Hinges**
- A beam can have a **hinge** at either end (Inspector): it carries no bending moment there, as in a pinned beam-to-column connection. In 3D both bending moments are released; twisting is still carried, unless no rigid beam holds the node (a ball joint). Hinges act in the static solve, natural frequencies and buckling, and are drawn as small circles.

**Steel section tables**
- Sections › **From the tables**: European rolled IPE, HEA and HEB sections (56 sizes, EN 10365 nominal dimensions). Their properties are worked out from the dimensions with the root fillets (a new root radius for I-sections), and agree with the tables.

**Examples and fixes**
- New example: a two-storey braced steel frame with HEB columns, IPE beams pinned at their ends, a braced bay, three load cases and three combinations.
- Escape closes a dialog left open under one that has just closed (focus was lost).

**Trust**
- Help › Verification: 120 checks, all passing. New: benchmark 18, a beam hinged at both ends between fixed supports (simply supported: wL²/8, 5wL⁴/384EI, its natural frequency and, as a column, π²EI/L²), a beam with a hinge partway (statics), a 3D beam hinged both ways, a combination 1.35 G + 1.5 Q and its envelope against superposition, and the tables' second moments (IPE 300, HEA 200, HEB 200, within 0.01 %) and areas (all 56 within 0.5 %).
- Random frames now have hinges, and random load cases always add up: a combination's results equal its cases' results times their factors, in 2D and 3D.

## 0.3.2 — 2026-10-30

Revolved 3D solids, a cutting plane through the results, and faces picked in 3D.

**Revolved solids**
- A 3D solid can now be **revolved** about the y axis instead of extruded (Inspector, with nothing selected): shafts, hubs, discs and vessels from their half-section, drawn right of the axis.
- Turn it a full 360°, or a sector (90°, say) with symmetry on its two cut faces, which are then the back and front faces. The section can reach the axis: the part is solid through its middle.
- New example: the pressure vessel revolved a quarter turn.

**Supports**
- **Held directions**: hold only x, y or z of a face (Loads › Hold Directions, or a support's Inspector).
- A support can be a **line along a face** at any fraction of the way through (Inspector › Where), as NAFEMS LE10 asks for.

**3D view**
- **Click a face** in the 3D view to put the support or load picked on it: a side face, or the back or front face.
- **Cutting plane**: once solved, cut across x, y or z (under the view) and slide the plane through the part; the inside is coloured by the result shown.

**Trust**
- Help › Verification: 105 checks, all passing. New: benchmark 17, a closed-end thick cylinder revolved a quarter turn and a full turn against Lamé (hoop stress 0.2 %, axial 0.7 %, bore movement 0.007 %), a round bar through the axis squeezed end to end (exact to 1 part in 10¹³), and **NAFEMS LE10**, the thick plate under pressure (σyy at D within 1.4 % of −5.38 MPa).
- Random 3D solids now include revolved ones (some through the axis, some a full turn), held directions and line supports: all 60 solve with their reactions balancing the loads, or say why they cannot.

## 0.3.1 — 2026-10-30

Sketch tools: shape a part's section with rounds, bevels, cut-outs and patterns before it is solved.

**Sketch tools** (Geometry, and Format on a selected shape)
- **Round Corners** (fillet) and **Bevel Corners** (chamfer): any corner between two straight edges, one or all at once, at a typed radius or distance. ⌒ in the Inspector's corner table rounds just that corner.
- **Union**, **Subtract** and **Intersect** two shapes: click one, Shift+click the other. Arcs and circles stay true arcs and circles, so a support or load on a round edge is still on one edge. Rings left inside become holes. Supports and loads move to the edges they lie on, and an outline keeps its edge numbers where it can.
- Ready-made **Slot**, **Rounded Rectangle** and **Regular Polygon** from typed sizes, as a shape or a hole.
- **Offset** an outline in or out (a wall of a given thickness, a clearance round a hole); arcs keep their centres.
- **Linear** and **Circular Patterns**: copies of shapes in a row or round a centre (bolt circles), with the supports and loads on their edges.
- Every tool is one step to undo, and is in Tell Me (Ctrl+K).

**Trust**
- Help › Verification: 94 checks, all passing. New: benchmark 16, the areas of eight shapes made by the sketch tools against hand formulas (to 1 part in 10⁹), and the plate with a hole again, its hole cut by Subtract, giving the same stress concentration of 3.0.
- Random pairs of shapes (60 of them, circles, rectangles, slots and rounded rectangles) are combined all three ways: the areas always add up (union plus intersection equals the two shapes), and every result passes the model's checks and meshes.

## 0.3.0 — 2026-10-29

3D solids: a part's outline extruded by its thickness and solved in 3D.

**3D solids**
- Home › **3D Solid**: draw the outline and holes in **Section**, exactly as for a plane part, and set the thickness; the part is extruded along z and shown in the 3D view.
- Supports and loads go on faces: an edge in Section stands for its side face, and the **back** and **front** faces have their own buttons while a support or load is being placed. Fixed, roller and symmetry supports; pressures, face forces (now with Fz too) and gravity.
- The section's mesh is swept through the thickness in layers of wedges: 18-node wedges from the quadratic mesh (accurate), 6-node from the linear one. Layers are automatic, or set in the Inspector.
- Once solved, the surface is coloured by any of 14 results (von Mises, σ1, σ3, each stress and shear, the displacements, the safety factor) on the deformed shape; hover to read a value. The Results page and report give the largest and smallest values anywhere inside, with where, the reactions and the equilibrium check in six sums.
- New examples: the L-bracket as a 3D solid, and a blank 3D solid.

**Quicker solves**
- The sparse solver now uses every core of the computer, and big 3D models store half the matrix: about 90 000 unknowns of a 3D solid solve in about 6 s and 1.2 GB on four cores.
- A 3D solid is limited to 200 000 unknowns, with a message saying how to make the mesh coarser, so a too-fine mesh cannot run the computer out of memory.

**Trust**
- Help › Verification: 85 checks, all passing. New: a 3D patch test (a uniform stress reproduced to 1 part in 10¹⁴, with the thinning), a 3D cantilever against Timoshenko beam theory (0.005 %) and its bending stress, and a thick cylinder in 3D against Lamé (hoop stress 0.5 %, bore movement 0.003 %).
- Random 3D solids (40 of them) solve with their reactions balancing the loads in all six sums, or say why they cannot.

## 0.2.6 — 2026-10-29

3D frames vibrate and buckle; the app starts lighter and keeps to size and speed budgets.

**3D frames**
- Solve › **Natural Frequencies** and **Buckling** now work for 3D frames: beams bend both ways and twist as they vibrate, and buckle by bending about either axis. The shapes swing in the 3D view; the lists, tables and printed report are as for plane frames.
- Truss bars in compression are checked on their own about their weaker axis.
- Buckling in 3D is by bending only: twisting and lateral-torsional buckling of open sections (I, channel) are not checked yet, and the Results page says so.

**Lighter and quicker**
- The 3D view's code loads only when a 3D frame is shown: the window starts with a third of the code it had (282 kB, was 863 kB).
- The fonts are bundled once, not twice (the window is 1.1 MB, was 1.2 MB).
- Size budgets checked on every build: the installer stays under 5 MB, the window's startup code under 400 kB.
- Solve times are in the verification record: the examples solve in milliseconds, a fine mesh of 141 000 unknowns in about 1.5 s, and six frequencies of a 12-storey 3D building frame (147 000 unknowns) in about 2.3 s.

**Trust**
- Help › Verification: 76 checks, all passing. New: a 3D cantilever's frequencies across its width, across its depth and in twisting; a column buckling about its weak and then its strong axis; and the portal frame in 3D, whose plane frequencies and buckling factors are found again to 1 part in 10¹².

## 0.2.5 — 2026-10-29

- Windows now lists the publisher as Datum.

## 0.2.4 — 2026-10-29

3D frames: frames and trusses in space, drawn on planes and seen in 3D.

**3D frames**
- Home › **3D Frame**: beams that stretch, twist and bend about both axes of their section, and truss bars, anywhere in space. y is up and z is depth.
- Members are drawn on a working plane: **Front** (x-y), **Side** (z-y) or **Plan** (x-z), at a depth you pick or type; members off the plane are shown faint. The status bar reads x, y and z.
- The **3D view** shows the whole frame with its supports and loads: turn it with the left button, pan with the right, zoom with the wheel; Front, Top and Iso buttons. Click nodes and members to select them, or to put supports and loads on them.
- Geometry › **Repeat** copies the selected members (or the whole frame) a step along any direction, with their supports and loads, and joins the copies with beams or truss bars: draw one frame, repeat it into bays.
- The Inspector takes z for nodes, Fz and moments about x, y and z, member loads along z, gravity in z, springs in all six directions, and the **roll** of a beam's section about its axis.
- Sections give the second moment about the other axis and the torsion constant J (thin-walled formulas for boxes, I and channel sections; your own values for custom sections).

**Results in 3D**
- Once solved, the members are drawn on their deflected shape and coloured by utilisation, axial force, torque, either bending moment, shear or deflection, with a colour scale. Hover a member to read N, Vy, Vz, T, Mz, My and the deflection.
- The Results page and the printed report have six sums in the equilibrium check, six reactions per support and every member's forces; the report draws the frame isometric.

**Trust**
- Help › Verification: 69 checks, all passing. New: a 3D cantilever bending each way, twisting and with its section rolled, under point and member loads; one along z; an L-shaped cantilever bent in plan, where bending and twisting add up; a tripod of truss bars; and the portal frame solved in 3D, giving the plane solver's answers to 1 part in 10¹³.
- Random 3D frames (150 of them) solve with their reactions balancing the loads, or say why they cannot.

**Not yet in 3D**: natural frequencies and buckling (plane frames have both).

## 0.2.3 — 2026-10-29

Buckling of frames: how far the loads can grow before a frame buckles.

**Buckling**
- Solve › **Buckling**: the six lowest load factors of a frame or truss and their buckled shapes. Every load on the model, self-weight and warming included, times the load factor buckles it.
- The axial forces come from a static solve first; members in compression soften the frame until it buckles. Beams are cut into eight pieces each, so a column drawn as one member buckles as it should.
- Truss bars in compression are also checked on their own as pinned struts (π²EI/L²), weakest first, and the bar that buckles before the frame is named.
- The load factors are listed on the Results page and in the printed report; a factor below 1 is flagged.

**Better natural frequencies**
- Beams are cut into eight pieces for natural frequencies too, so higher modes of frames drawn with few members are now accurate.

**Trust**
- Help › Verification: 54 checks, all passing. New: Euler's buckling loads for a column held four ways (pinned, flagpole, fixed and pinned, fixed at both ends) and its second mode, all within 0.05 %; a column held at both ends and warmed; a truss bar checked as a strut.

## 0.2.2 — 2026-10-29

Natural frequencies: how frames and parts vibrate on their own.

**Natural frequencies**
- Solve › **Natural Frequencies**: the six lowest frequencies and their mode shapes, for frames and trusses and for parts in plane stress, plane strain or axisymmetric.
- Pick a mode under the canvas to see it move; a frame bends along each member, a part deforms and is coloured by how far it moves. Back to the static results with one click.
- Masses come from each material's density (consistent masses: beams, truss bars and triangles); the supports hold the model as for a static solve. A model free to float away, or without density, says so.
- The frequencies, periods and ω are listed on the Results page and in the printed report.

**Trust**
- Help › Verification: 47 checks, all passing. New: the first three frequencies of a cantilever and of a simply supported beam against beam theory (within 0.05 %), and a plate strip's bending (0.16 % below beam theory, as shear and rotary inertia make it) and stretching frequencies.
- The solver is checked against the exact frequencies of a chain of masses and springs, to one part in a billion.

**Fixed**
- A part's fill no longer covers its mode shape (found in the app).

## 0.2.1 — 2026-10-29

Axisymmetric solids: pressure vessels, shafts, discs and nozzles solved from their half-section.

**Axisymmetric**
- Home › **Axisymmetric**: draw the half-section to the right of the y axis; it is turned a full circle about it. The canvas shows the axis.
- Results named as engineers read them: radial, axial and hoop stress (σr, σz, σθ), radial and axial displacement, von Mises, principal stresses and the safety factor.
- Points on the axis are held from moving off it by themselves. Loads and reactions are totals for the full circle; the equilibrium check sums the forces along the axis.
- Thermal stress works here too: temperatures are solved round the axis, then the stress.
- A new worked example, **Pressure vessel**: a cylinder with a hemispherical head under 2 MPa. The hoop stress in the cylinder agrees with pr/t.

**Trust**
- Help › Verification: 39 checks, all passing. New: a long thick cylinder (Lamé) and a thick hollow sphere under pressure (hoop stress and growth of the bore), each on three meshes with the error falling; a round bar pulled along its axis and a free tube warmed evenly, both exact.

**Meshing**
- The automatic element size is finer for thin parts (about 800 triangles over the part), so a thin wall gets several elements through it instead of one.

## 0.2.0 — 2026-10-29

Thermal stress: parts and frames that are warmed or cooled push on what holds them. The first release on the way to 3D (see the roadmap).

**Thermal stress**
- **Temperature Change** (Loads tab): the whole model warms or cools by the same amount from its stress-free temperature (20 °C unless you change it, in the Inspector with nothing selected).
- In a part, hold edges at temperatures and add convection: Solve finds the temperatures first, on the same mesh, then the stress. Temperature appears among the results, beside the stresses.
- Frame members grow by αΔT per length; where they cannot, they push on their supports.
- Plane strain holds the length along the body, as it should: σz = ν(σx + σy) − EαΔT.
- A new worked example, **Heated plate**: a steel plate with a hole, fixed at a cool wall and heated at its other edge.

**Trust**
- Help › Verification: 32 checks, all passing. New: a bar held at both ends and warmed (σ = −EαΔT), a block held on every side in plane strain (σ = −EαΔT/(1 − 2ν)), a free plate with a temperature varying in a straight line through it (no stress at all), and a beam fixed at both ends and warmed (N = −EAαΔT).

**Fixed**
- A part held only by a roller on one curved edge can turn about the edge's centre; it now says the supports barely hold it, instead of giving numbers without a warning. Found by the random-model tests.
- Where a round hole crosses a round outline, a tiny triangle could fold inside out; it no longer does. Found by the random-model tests.
- Labels of the largest and smallest values near the right edge of the canvas no longer run off it.

## 0.1.4 — 2026-10-29

Polish: large parts open faster, a pin load for lugs and clevises, members joined where they cross when you ask, and the small problems left over from the 0.1.1 review.

**Loads and drawing**
- **Pin load**: a pin or bolt pulling on the part through a hole. It presses only on the side of the hole it pushes into, most in line with the force, and the part carries exactly the force you give. The lifting lug example now uses it.
- **Join Where They Cross** (Format tab): select members that cross and join them with a node at each crossing; loads along them are split there. Crossing members are otherwise left apart, as X-bracing usually is.
- Dragging lands what you grab on the grid, even if it started off the grid.
- Load arrows and edge loads move with their node or shape while you drag it.

**Speed**
- Results of large parts reach the window about three times smaller: a 25 000-element part sends 6 MB instead of 22 MB. Values keep seven significant digits; the sums behind the equilibrium check keep full precision.

**Your work**
- Saving writes a temporary file and then swaps it in, so a crash or a full disk never leaves a half-written model.
- Undoing back to the saved model clears the "changed" mark.
- A new autosave interval in Options applies at once, without restarting.
- A damaged file whose settings cannot be read (an unknown kind of analysis, a negative id counter, a negative thickness) now opens with the default settings and says which were reset, instead of refusing to open.

**Print**
- The title block gives the units the model actually uses (frames: m for the drawing, mm, kN, kN·m and MPa).
- A frame's printed equilibrium check now includes the moments, and says whether the reactions balance the loads.

**And** the largest and smallest values' labels no longer overlap when they are close together.

## 0.1.3 — 2026-10-28

Parts are meshed and solved: plane stress, plane strain and steady heat conduction, with stress and temperature contours, a probe, the deformed shape, a materials library and a printed results report. Help › Verification now covers every benchmark in the plan.

**Meshing**
- Solve (F5) splits a part into triangles automatically, keeping their angles above a minimum (25° by default) and holes and fillets round.
- Element size: automatic, coarser or finer (Mesh tab, or type it in the Inspector). Refine Near: click an edge or a point to make the elements a quarter of the size there; they grow back gradually into the rest of the part.
- Quadratic 6-node triangles (accurate, the default) or linear 3-node triangles.
- The Mesh view shows the triangles coloured by quality (smallest angle), with the numbers of elements and nodes, the smallest angle and the worst aspect ratio.

**Solving parts**
- Plane stress (thin plates, with a thickness) and plane strain (long bodies).
- Supports on edges and points: fixed, pinned, roller (sliding along the edge, also on sloping and curved edges) and symmetry.
- Loads: pressure on edges, edge forces (the whole force or per length), point forces, and gravity from each material's density.
- **Heat conduction**: fixed temperatures, heat flux, convection to a fluid and heat generated inside; temperatures and heat flux.

**Results of parts**
- Contours in 12 bands, blue to red (or the colour-blind-safe scale in Options): von Mises stress, principal stresses, σx, σy, τxy, σz in plane strain, displacement, the safety factor against yield, temperature and heat flux.
- Smoothed (averaged at nodes) or per element, so you can see where the mesh is too coarse.
- The deformed shape, drawn larger by a round factor you can change; mesh lines on or off.
- The largest and smallest values marked on the part; hover anywhere to read the values at that point.
- The equilibrium check for parts (forces and moments), and the heat balance for heat models: what flows in equals what flows out.
- View › Results: the check, the reactions or heat flows at every support and load, the largest and smallest value of every result with where it is, and the mesh and solver report.

**Materials library**
- 54 metals and plastics copied from Datum Atlas, each with its source: steels, stainless steels, cast irons, aluminium, copper, titanium, magnesium and nickel alloys, and common plastics. Home › Materials › From the library. Atlas has no Poisson's ratio, so typical values for each family are used and said so.

**Report**
- The printed report and the new Report view now include, for parts, the mesh and its quality, contour pictures with their scales, the largest and smallest values, the reactions or heat flows and the equilibrium check.

**Trust**
- Help › Verification: 27 checks, all passing. New: a plate with a hole (stress concentration 3.0, Kirsch), a thick cylinder under pressure (Lamé), NAFEMS LE1 (the elliptic membrane, 92.7 MPa), a plane wall and a fin cooled by convection, and patch tests for 3- and 6-node triangles in stress, strain and heat. The plate, cylinder and LE1 are solved on three or four meshes, and the error falls as the mesh gets finer. The record is in [docs/verification/0.1.3.md](docs/verification/0.1.3.md).

**Also**
- Editing large frames is faster: a model with 10 000 members took 50 ms per edit, now 14 ms.

## 0.1.2 — 2026-10-28

A fixes release: thirteen problems found by reviewing and using 0.1.1, each now covered by a test.

**Results you can trust on screen**
- The moment, shear and axial diagrams and the utilisation colours now follow when you pan or zoom; before, they could stay behind, away from their members.
- The equilibrium check compares against the size of the loads, so loads that cancel out (a moment on its own, a balanced pair of forces) no longer report a huge imbalance, and the moments are now checked as well as the forces.
- A model with no loads says so, instead of drawing a meaningless deflected shape.
- A load on a node that no member reaches is named in a warning instead of being dropped silently.
- Rounding noise such as 1.5e-14 kN·m is shown as 0.
- Members divided into many short pieces (a beam in 1 000 parts) are no longer mistaken for a mechanism, and they solve to full accuracy.

**Drawing**
- A member drawn to the middle of another now joins it there: the member is split, and loads along it are split with it. Before, the two crossed without touching.
- A node dragged, moved with the arrow keys or typed onto another node joins it, instead of leaving two separate nodes at the same place.
- Adding or removing a corner of a part keeps every support and load on the edge it was on; a corner added in an arc keeps the arc's shape.

**Typing and units**
- Negative numbers can be typed into the corner table, "1,5" is read as 1.5 and "1,200" as 1200, and clearing a field no longer sets it to 0.
- Switching to US units gives the grid a round step in feet or inches; member lengths in the tables use the same unit as the drawing.

**Your work**
- Opening a model, an example or a recent file asks whether to save the changes to the one that is open.

**And** the blank model shows its "draw a frame" card again.

## 0.1.1 — 2026-10-28

Frames and trusses solve: press Solve (F5) and see the deflected shape, the moment, shear and axial diagrams, the reactions and the equilibrium check.

**Solving**
- 2D beam elements (axial force, shear and bending) and truss bars (axial force only), solved with a sparse solver in a few milliseconds.
- Point forces and moments at nodes, uniform and linearly varying loads on members (along x, along y or across the member), and self-weight from each material's density.
- Fixed, pinned, roller and spring supports.
- A model that can move without resistance (a missing support, a truss without triangles) says so instead of giving numbers.

**Results**
- The deflected shape, drawn larger by a round factor you can change with a slider.
- Bending moment (drawn on the tension side), shear force and axial force diagrams, with the values at the ends and the peaks.
- Reactions as arrows with their values.
- Utilisation colours: below 50 %, 50 to 100 %, and over 100 % of the yield strength.
- A probe: hover a member to read N, V, M and the deflection at that point.
- **The equilibrium check**: the reactions are added up and compared with the loads after every solve, on screen and in the report.
- View › Results: reactions, every member's largest forces, deflection, stress and utilisation, and the solver report.
- After an edit, the results stay on screen but are marked out of date until you solve again.

**Trust**
- **Help › Verification** solves the benchmarks in front of you and compares them with the known answers: a cantilever (tip deflection, rotation and root moment), a simply supported beam under a uniform load, and two trusses worked by hand. All 12 checks agree to better than one part in a billion. The same checks run as tests before every release; the record is in [docs/verification/0.1.1.md](docs/verification/0.1.1.md).

**Printing**: the report page adds the equilibrium check, the reactions and the member results when the model is solved.

## 0.1.0 — 2026-10-28

The first version: the Datum Mesh window, the drawing canvas, and models that save and open. Solving arrives in the next releases: frames and trusses in 0.1.1, parts in 0.1.2.

**Frames and trusses**
- Draw beams and truss bars on a grid: click two points for each member; a node is made at each end, or an existing node is used.
- Nodes at typed coordinates; move anything by dragging or with the arrow keys, always landing on the grid.
- Sections: rectangle, round bar, tube, box, I, channel, angle, or your own values; the area, second moment of area and extreme fibre distance are worked out for you.

**Parts**
- Rectangles, circles and polygons, with edges that can be arcs (fillets and rounded ends), and holes cut out of them.
- Plane stress (with a thickness), plane strain and heat conduction.

**Supports and loads**
- Fixed, pinned, roller, spring and symmetry supports on nodes, edges and points.
- Forces, moments, uniform and linearly varying member loads, pressure and forces on edges, gravity.
- Heat: fixed temperatures, heat flux, convection and heat generation.
- Every value in SI (mm, kN, MPa, °C) or US (in, kip, ksi, °F) units; files keep SI, so switching never rounds anything.

**Worked examples**: a portal frame with wind and roof loads, a roof truss, a lifting lug, an L-bracket with a fillet, a flange plate with bolt holes, and a heat sink.

**And** the Datum family window: ribbon, File page, options, print to PDF with a title block, a ? card beside every tool, Tell Me (Ctrl+K), light and dark themes, recovery of unsaved work, and updates from inside the app. Damaged model files open as far as possible and say what was left out.
