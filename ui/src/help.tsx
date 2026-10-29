// "?" help: a small button beside a tool; a click opens a short card saying what the tool is for
// and how to use it. One place for every explanation, so the whole app teaches itself.
import { For, Show } from "solid-js";
import { openDialog } from "./store";
import { Dialog } from "./ui";

type Topic = { t: string; what: string; how?: string[]; tip?: string };
export const HELP: Record<string, Topic> = {
  // Home
  analysis: { t: "Kind of model", what: "What Datum Mesh solves: a frame or truss of members, or a part in plane stress, plane strain or heat conduction.",
    how: ["Frame or truss: members between nodes; results are deflections, member forces and moments.", "Axisymmetric: a solid turned about the y axis, from its half-section (vessels, shafts, discs).", "Plane stress: a thin plate loaded in its plane (set its thickness in the Inspector).", "Plane strain: a long body with the same section all along, such as a dam, a pipe wall or a retaining wall.", "Heat conduction: steady temperatures and heat flow in the same part."] },
  units: { t: "Units", what: "How values are shown and typed: SI (mm, kN, MPa, °C) or US (in, kip, ksi, °F). Frames show coordinates in m or ft.", tip: "The file always keeps SI base units, so switching never rounds anything." },
  select: { t: "Select", what: "Pick, move and delete items on the canvas.",
    how: ["Click an item; Shift or Ctrl adds to the selection; drag on empty space to select an area.", "Drag a selection to move it; it lands on the grid. The arrow keys move it one grid step.", "Click near a part's outline to pick one edge, for supports and loads."] },
  examples: { t: "Examples", what: "Worked models: a portal frame, a roof truss, a lifting lug, an L-bracket with a fillet, a flange plate with bolt holes and a heat sink." },
  solve: { t: "Solve", what: "Builds the equations of every element, applies the supports and loads, and solves them (F5). Parts are meshed first.",
    how: ["Beams are Euler–Bernoulli elements: exact for members loaded at their ends and by linear distributed loads.", "If the model can move without resistance (missing supports, a truss without triangles) Solve says so instead of giving numbers.", "After an edit the results are kept but marked out of date; press F5 again."] },
  equilibrium: { t: "Equilibrium check", what: "The reactions must balance the applied loads: ΣFx, ΣFy and ΣM come to zero. Datum Mesh adds them up after every solve and shows the difference, so a wrong answer cannot hide.", tip: "For heat conduction it is the heat balance: what flows in through fluxes, convection and generation leaves through the fixed temperatures." },
  diagrams: { t: "Diagrams", what: "Axial force, shear force and bending moment along every member.",
    how: ["Bending moments are drawn on the tension side of the member.", "Axial force is positive in tension; shear and axial are drawn on the member's left side, looking from its start to its end.", "Hover a member to read the values at that point."] },
  verification: { t: "Verification", what: "Benchmarks with known answers (beam theory, hand calculations, the Kirsch and Lamé solutions, NAFEMS LE1, conduction through a wall and a fin, patch tests), solved by this version of Datum Mesh and compared with the reference. They also run as tests before every release.", tip: "The mesh convergence tables show the error falling as the elements get smaller." },
  // Geometry
  members: { t: "Members", what: "Beams carry axial force, shear and bending; truss bars only axial force (they are pinned at both ends).",
    how: ["Pick Beam or Truss Bar, then click two points: a node is made at each, or an existing node is used.", "Drawing continues from the last point; Esc or a right-click stops.", "A member drawn to the middle of another joins it there. Members that cross are left apart (as X-bracing usually is): select them and use Format › Join Where They Cross to join them.", "Node at… adds a node at typed coordinates."] },
  nodes: { t: "Nodes", what: "Points where members join, and where supports and point loads go. The Inspector edits a node's coordinates." },
  sections: { t: "Sections", what: "The cross-sections of members: rectangle, round bar, tube, box, I, channel, angle, or your own area and second moment.",
    how: ["Pick a shape and type its dimensions; the area A, second moment I and extreme fibre distance c are worked out for you.", "Every member uses one section; change it in the Inspector or on the Format tab."] },
  materials: { t: "Materials", what: "Young's modulus, Poisson's ratio, density, yield strength, conductivity and expansion, each with its source.", how: ["From the library…: metals and plastics from Datum Atlas, with their sources (Poisson's ratio: typical values for the family).", "Add a copy to make your own; type its values and where they come from."], tip: "Library values are typical for the grade; the yield strength is the low end of its range." },
  shapes: { t: "Shapes", what: "The outline of a part: rectangles, circles and polygons. Overlapping solid shapes join; holes cut out of them.",
    how: ["Rectangle: click two corners. Circle: click the centre, then a point on the edge.", "Polygon: click each corner; click the first corner, double-click or press Enter to close it.", "Edit corners in the Inspector; an edge can be an arc (give its angle)."] },
  holes: { t: "Holes", what: "Shapes cut out of the part: bolt holes, pin holes, slots. Any shape can be made a hole in the Inspector or on the Format tab." },
  corners: { t: "Corners", what: "The polygon's corners in order. Arc ° turns the edge after a corner into an arc through that angle: positive bulges outwards on a counter-clockwise outline (90° makes a quarter circle)." },
  import: { t: "Import", what: "Reading outlines from DXF drawings arrives in 0.2." },
  snap: { t: "Snap", what: "Points land on the grid, or on a node or corner near the pointer. The grid step is in the Inspector when nothing is selected." },
  // Mesh
  mesh: { t: "Mesh", what: "Splits a part into triangles, with angles kept above a minimum (25° by default). Solve meshes the part for you; Mesh shows it first.",
    how: ["Element size: automatic is a twenty-fifth of the part's larger side, or finer for a thin part (about 800 triangles); finer gives better stresses and takes longer.", "Refine Near: click an edge or a point to make the elements a quarter of the size there; they grow back gradually.", "The Mesh view colours triangles by their smallest angle: under 20° is poor."],
    tip: "Check a result by solving again with a finer mesh: if the number hardly changes, the mesh is fine enough." },
  contours: { t: "Contours", what: "The part coloured by a result in 12 bands, from blue (low) to red (high); the colour-blind-safe scale is in Options.",
    how: ["Pick the result under the canvas. Hover anywhere to read the values there.", "Smoothed averages stresses where elements meet; Per element shows each element's own value. Where the two differ a lot, the mesh is too coarse.", "The safety factor is the yield strength over the von Mises stress; its colours run red (low) to blue (safe), and over 10 shows as 10+."],
    tip: "A sharp inside corner, or the corner where a fixed edge meets a free one, has no finite stress in theory: its value keeps rising as the mesh gets finer. Round the corner (a fillet) or judge the stress a little away from it." },
  elements: { t: "Elements", what: "Quadratic triangles (6 nodes, curved stress inside each) are accurate and the default; linear triangles (3 nodes, constant stress) are an option." },
  // Loads
  supports: { t: "Supports", what: "Where the model is held.",
    how: ["Fixed: no movement and, for beams, no turning. Pinned: no movement, free to turn.", "Roller: free to slide one way. On a part's edge it slides along the edge.", "Symmetry: model half a symmetric part and hold the cut edge across it.", "Pick the kind, then click a node, an edge or a point."] },
  loads: { t: "Loads", what: "What acts on the model: forces and moments at nodes, distributed loads on members, pressure and forces on edges, and gravity.",
    how: ["Pick the kind, then click where it acts; set the value in the Inspector.", "Up and +x are positive; pressure is positive pushing into the part.", "Self-weight uses each material's density."] },
  modes: { t: "Natural frequencies", what: "The frequencies at which the model vibrates on its own, and the shape of each vibration (Solve › Natural Frequencies).",
    how: ["Masses come from each material's density; the supports hold it as for a static solve.", "Pick a mode under the canvas to see its shape move; the swing has no size of its own and is drawn to a tenth of the model.", "View › Results lists every frequency with its period."],
    tip: "Keep a machine's running speed and the loads that shake it well away from these frequencies." },
  buckle: { t: "Buckling", what: "The load factor λ at which a frame's loads, every one of them times λ, make it buckle sideways, and the shape it buckles in (Solve › Buckling).",
    how: ["A static solve finds the axial force in every member first; members in compression soften the frame until it buckles.", "Beams are cut into eight pieces each for the buckled shapes; a truss bar in compression is also checked on its own as a pinned strut, π²EI/L².", "View › Results lists the load factors and the bars."],
    tip: "This is the buckling of a perfect frame in its own plane. Imperfections, yielding and buckling out of the plane come sooner: keep λ well above 1 and check members to your design code." },
  axi: { t: "Axisymmetric", what: "A solid turned about an axis (a pressure vessel, a shaft, a disc, a nozzle), solved from its half-section: the y axis is the axis of turning and x is the radius.",
    how: ["Draw the half-section right of the y axis (x ≥ 0); edges on the axis are held from moving away from it by themselves.", "Loads and reactions are totals for the full circle: a point force is a ring of force.", "Results: radial, axial and hoop stress (σr, σz, σθ), and radial and axial displacement."],
    tip: "Only the forces along the axis are summed in the equilibrium check: radial pushes round a circle cancel by themselves." },
  thermal: { t: "Thermal stress", what: "Stress from temperature: a part or member that is warmed grows, and where it cannot grow freely it pushes on what holds it.",
    how: ["Temperature Change: the whole model warms (or cools) by the same amount from the stress-free temperature.", "In a part, hold edges at temperatures (and add convection): Solve finds the temperatures first, on the same mesh, then the stress. Temperature appears among the results.", "The stress-free temperature is in the Inspector with nothing selected (20 °C unless you change it)."],
    tip: "A free part with temperatures varying in a straight line through it has no thermal stress; uneven heating, or holding it, makes stress." },
  pin: { t: "Pin load", what: "A pin or bolt pulling on the part through a hole: the whole force, pressing on the side of the hole it pushes into, most in line with the force and falling away as the cosine (no pressure on the far side).", how: ["Pick Pin Load on the Loads tab, then click the hole's edge.", "Type the force the pin puts on the part in the Inspector (up is positive)."], tip: "An even force all round the hole (Edge Force) also pulls on the far side, which a real pin cannot do." },
  heat: { t: "Heat", what: "Temperatures held on edges, heat flux into edges, convection to a fluid, and heat generated inside." },
  // Results and view
  results: { t: "Results", what: "Frames: the deflected shape, member diagrams, reactions, utilisation. Parts: contours of stress, displacement, safety factor, temperature or heat flux, the deformed shape, and the largest and smallest values. Both: the equilibrium check.",
    how: ["Results › Deflected Shape, Bending Moment, Shear Force and Axial Force switch the pictures; the legend shows the largest values.", "Hover a member: the probe reads N, V, M and the deflection there.", "View › Results lists the reactions, every member's largest forces, stress and utilisation, and the solver report."] },
  report: { t: "Report", what: "Print the model and its results to PDF: title block, model, mesh, materials, loads, supports, result pictures with legends, maxima and reactions." },
  zoom: { t: "Zoom", what: "The wheel zooms about the pointer; Fit shows the whole model. Pan with the middle button, Space + drag or Shift + wheel." },
  show: { t: "Show", what: "The grid, snapping to it, and the rulers along the canvas." },
  // Panels
  tree: { t: "Model", what: "Everything in the model by kind. Click an item to select it; + adds one." },
  inspector: { t: "Inspector", what: "The values of the selected item, in display units. With nothing selected: the model's name, kind, units, grid and thickness." },
  canvas: { t: "Canvas", what: "The model drawn to scale, y up. The rulers show coordinates; the status bar shows where the pointer is." },
};

/** The small "?" beside a tool. */
export const Help = (p: { k: string }) => <button class="help" title={`What is ${HELP[p.k]?.t ?? "this"}?`} onClick={e => { e.stopPropagation(); e.preventDefault(); helpCard(p.k); }}>?</button>;

export const helpCard = (k: string) => openDialog(() => {
  const h = HELP[k];
  return (
    <Dialog title={h?.t ?? "Help"}>
      <div class="helpcard"><p>{h?.what}</p>
        <Show when={h?.how?.length}><h4>How to use it</h4><ul><For each={h!.how}>{x => <li>{x}</li>}</For></ul></Show>
        <Show when={h?.tip}><p class="muted">{h!.tip}</p></Show></div>
    </Dialog>
  );
});
