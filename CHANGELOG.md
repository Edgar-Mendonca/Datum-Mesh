# Changelog

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
