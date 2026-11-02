# Changelog

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
