# Changelog

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
