// The whole map (0.10.0, docs/PLAN-1.0.md): every feature of the finished program that is not
// built yet, in one register. The ribbon, the tree, the greyed dialogs and Help › Roadmap are drawn
// from it, behind View › Show planned features. A planned feature never calculates anything: its
// dialog shows every field it will have, disabled, with the release that brings it.

/** Which models a feature is for: 2D frames and trusses, 3D frames, plane parts (stress, strain,
 *  axisymmetric), 3D solids, heat (2D and 3D). */
export type Kind = "frame" | "space" | "plane" | "solid" | "heat";
/** A field of a planned dialog: a number (with its unit), a choice, a tick, words, a table, a pick
 *  in the view, or a curve (a table of two columns drawn as a graph). */
export type PField =
  | { k: "num"; l: string; u?: string; v?: string }
  | { k: "choice"; l: string; o: string[] }
  | { k: "check"; l: string; on?: boolean }
  | { k: "text"; l: string; v?: string }
  | { k: "table"; l: string; cols: string[]; rows?: string[][] }
  | { k: "pick"; l: string; what: string }
  | { k: "curve"; l: string; x: string; y: string };
export type Feat = {
  id: string; label: string; icon: string;
  /** The workflow step (1–6), or 0 for the whole program (View, File). */
  step: number;
  /** The ribbon tab and the group it sits in. */
  tab: "geometry" | "properties" | "loads" | "mesh" | "solve" | "results" | "view";
  group: string;
  /** The release that brings it. */
  v: string;
  for?: Kind[];
  /** What it does, in a sentence or two. */
  what: string;
  fields: PField[];
};

const ALL: Kind[] = ["frame", "space", "plane", "solid", "heat"];
const STRUCT: Kind[] = ["frame", "space", "plane", "solid"];
const SOLID: Kind[] = ["solid"];
const PARTS: Kind[] = ["plane", "solid"];
const FRAMES: Kind[] = ["frame", "space"];

export const FEATS: Feat[] = [
  // ---------- 1. Geometry ----------
  { id: "mate", label: "Place Against", icon: "move", step: 1, tab: "geometry", group: "Assembly", v: "0.12", for: SOLID,
    what: "Place a part against another: a face onto a face, two faces lined up, or held at a distance, instead of typing where it goes.",
    fields: [{ k: "choice", l: "How", o: ["Face to face", "Faces lined up", "At a distance", "Axes in line"] }, { k: "pick", l: "Face of the part moved", what: "a face" }, { k: "pick", l: "Face it goes against", what: "a face" }, { k: "num", l: "Distance", u: "mm", v: "0" }, { k: "check", l: "Flip", on: false }] },
  { id: "partition", label: "Split a Face", icon: "cut", step: 1, tab: "geometry", group: "Assembly", v: "0.12", for: PARTS,
    what: "Split a face (or a solid) along a sketch or a plane, so a load or a support can go on part of it.",
    fields: [{ k: "pick", l: "Face or solid to split", what: "a face or a solid" }, { k: "choice", l: "Split along", o: ["A sketch", "A plane", "Two points"] }, { k: "pick", l: "Sketch or plane", what: "a sketch or a plane" }] },
  { id: "refpoint", label: "Reference Point", icon: "node", step: 1, tab: "geometry", group: "Assembly", v: "0.12", for: [...SOLID, "plane"],
    what: "A point in space that a load or a support can act on, joined to faces by a coupling (step 3).",
    fields: [{ k: "num", l: "x", u: "mm" }, { k: "num", l: "y", u: "mm" }, { k: "num", l: "z", u: "mm" }, { k: "text", l: "Name", v: "RP-1" }] },
  { id: "datum", label: "Datum Axis or Point", icon: "plane", step: 1, tab: "geometry", group: "Assembly", v: "0.12", for: SOLID,
    what: "An axis or a point to sketch on, revolve about, pattern round or place parts against.",
    fields: [{ k: "choice", l: "Kind", o: ["Axis through two points", "Axis along an edge", "Point at a vertex", "Point at the middle of an edge", "Point by coordinates"] }, { k: "pick", l: "From", what: "points or an edge" }] },
  { id: "cable", label: "Cable", icon: "truss", step: 1, tab: "geometry", group: "Members", v: "0.13", for: FRAMES,
    what: "A member that only pulls and sags under its own weight: stays, guys, hangers.",
    fields: [{ k: "pick", l: "From node to node", what: "two nodes" }, { k: "num", l: "Area", u: "mm²" }, { k: "num", l: "Pretension", u: "kN" }, { k: "num", l: "Sag at the middle", u: "mm" }] },
  { id: "springel", label: "Spring or Damper", icon: "spring", step: 1, tab: "geometry", group: "Members", v: "0.12", for: STRUCT,
    what: "A spring (stiffness) or a damper (viscous) between two points, or from a point to the ground.",
    fields: [{ k: "choice", l: "Kind", o: ["Spring", "Damper", "Spring and damper"] }, { k: "pick", l: "Between", what: "two points, or a point and the ground" }, { k: "num", l: "Stiffness", u: "kN/m" }, { k: "num", l: "Damping", u: "kN·s/m" }, { k: "choice", l: "Direction", o: ["Along the line", "x", "y", "z"] }] },
  { id: "beta", label: "Member Orientation", icon: "section", step: 1, tab: "geometry", group: "Members", v: "0.10.x", for: ["space"],
    what: "Turn a member's section about its own axis (the beta angle), so a beam's strong axis lies where it should.",
    fields: [{ k: "pick", l: "Members", what: "members" }, { k: "num", l: "Angle", u: "°", v: "0" }, { k: "choice", l: "Or towards", o: ["An angle", "A point", "A direction"] }] },

  // ---------- 2. Materials and sections ----------
  { id: "ortho", label: "Orthotropic", icon: "material", step: 2, tab: "properties", group: "Material models", v: "0.11", for: [...PARTS, "heat"],
    what: "A material stiffer one way than another: wood, rolled sheet, fibre layers; and heat conducted more one way.",
    fields: [{ k: "num", l: "E₁", u: "MPa" }, { k: "num", l: "E₂", u: "MPa" }, { k: "num", l: "E₃", u: "MPa" }, { k: "num", l: "ν₁₂" }, { k: "num", l: "ν₁₃" }, { k: "num", l: "ν₂₃" }, { k: "num", l: "G₁₂", u: "MPa" }, { k: "num", l: "G₁₃", u: "MPa" }, { k: "num", l: "G₂₃", u: "MPa" }, { k: "choice", l: "Directions", o: ["The model's axes", "A part's own axes", "Along a picked edge"] }] },
  { id: "kinematic", label: "Kinematic Hardening", icon: "curve", step: 2, tab: "properties", group: "Material models", v: "0.11", for: PARTS,
    what: "Yielding that remembers which way it went (the Bauschinger effect): loading back and forth, low-cycle fatigue.",
    fields: [{ k: "choice", l: "Hardening", o: ["Kinematic (linear)", "Combined (isotropic and kinematic)", "Chaboche (several back stresses)"] }, { k: "num", l: "Yield stress", u: "MPa" }, { k: "num", l: "Kinematic modulus C", u: "MPa" }, { k: "num", l: "Recall γ" }] },
  { id: "jc", label: "Johnson–Cook", icon: "curve", step: 2, tab: "properties", group: "Material models", v: "0.11", for: PARTS,
    what: "Yield stress rising with strain, strain rate and falling with temperature: metals in crash and impact.",
    fields: [{ k: "num", l: "A", u: "MPa" }, { k: "num", l: "B", u: "MPa" }, { k: "num", l: "n" }, { k: "num", l: "C" }, { k: "num", l: "Reference strain rate", u: "1/s", v: "1" }, { k: "num", l: "m" }, { k: "num", l: "Room temperature", u: "°C", v: "20" }, { k: "num", l: "Melting temperature", u: "°C" }] },
  { id: "damage", label: "Damage and Failure", icon: "del", step: 2, tab: "properties", group: "Material models", v: "0.11", for: PARTS,
    what: "The strain a material can take before it fails, by stress state, rate and temperature; failed elements are removed (fracture, tearing).",
    fields: [{ k: "choice", l: "Rule", o: ["Johnson–Cook failure", "Plastic strain at failure", "Stress at failure"] }, { k: "num", l: "D₁" }, { k: "num", l: "D₂" }, { k: "num", l: "D₃" }, { k: "num", l: "D₄" }, { k: "num", l: "D₅" }, { k: "num", l: "Energy to fail", u: "J/m²" }, { k: "check", l: "Remove failed elements", on: true }] },
  { id: "hyper", label: "Rubber Models", icon: "material", step: 2, tab: "properties", group: "Material models", v: "0.11", for: SOLID,
    what: "Mooney–Rivlin, Yeoh and Ogden for rubbers and soft tissue stretched far (neo-Hookean already works).",
    fields: [{ k: "choice", l: "Model", o: ["Mooney–Rivlin", "Yeoh", "Ogden (3 terms)"] }, { k: "num", l: "C₁₀", u: "MPa" }, { k: "num", l: "C₀₁", u: "MPa" }, { k: "num", l: "C₂₀", u: "MPa" }, { k: "num", l: "C₃₀", u: "MPa" }, { k: "num", l: "Bulk modulus", u: "MPa" }] },
  { id: "tdep", label: "Properties with Temperature", icon: "temp", step: 2, tab: "properties", group: "Material models", v: "0.11", for: ALL,
    what: "Stiffness, yield stress, expansion and conduction that change with temperature, from a table.",
    fields: [{ k: "table", l: "Values against temperature", cols: ["T (°C)", "E (MPa)", "σy (MPa)", "α (1/K)", "k (W/m·K)"], rows: [["20", "", "", "", ""], ["200", "", "", "", ""], ["400", "", "", "", ""]] }] },
  { id: "fit", label: "Material from Test Data", icon: "calc", step: 2, tab: "properties", group: "Material models", v: "0.11", for: PARTS,
    what: "Paste a test curve (stress against strain, from a tensile test); the model is fitted and drawn over the points, with how well it fits.",
    fields: [{ k: "curve", l: "Test points", x: "Strain", y: "Stress (MPa)" }, { k: "choice", l: "Test", o: ["Tension", "Compression", "Shear", "Biaxial"] }, { k: "choice", l: "Fit to", o: ["Elastic-plastic (yield and hardening)", "Johnson–Cook", "Neo-Hookean", "Mooney–Rivlin", "Yeoh", "Ogden"] }, { k: "choice", l: "Values are", o: ["Engineering", "True"] }] },
  { id: "composite", label: "Composite Layup", icon: "section", step: 2, tab: "properties", group: "Sections", v: "1.0", for: ["space", "solid"],
    what: "A plate or shell made of layers, each with its material, thickness and fibre angle.",
    fields: [{ k: "table", l: "Layers (from the bottom)", cols: ["Material", "Thickness (mm)", "Angle (°)"], rows: [["", "", "0"], ["", "", "90"], ["", "", "0"]] }, { k: "choice", l: "Reference direction", o: ["Global x", "Along a picked edge"] }] },
  { id: "concrete", label: "Concrete", icon: "material", step: 2, tab: "properties", group: "Material models", v: "1.0", for: [...FRAMES, ...PARTS],
    what: "Concrete that cracks in tension and crushes in compression, with reinforcement.",
    fields: [{ k: "num", l: "Compressive strength fck", u: "MPa" }, { k: "num", l: "Tensile strength", u: "MPa" }, { k: "num", l: "Fracture energy", u: "N/m" }, { k: "table", l: "Reinforcement", cols: ["Bar Ø (mm)", "Spacing (mm)", "Cover (mm)", "Direction"] }] },

  // ---------- 3. Supports, loads and the analysis ----------
  { id: "nlbuckle", label: "Buckling with Imperfection", icon: "buckle", step: 5, tab: "solve", group: "Planned analyses", v: "0.13", for: [...FRAMES, "solid"],
    what: "Follow a structure past its buckling load from a shape made slightly imperfect after its first buckling modes: the real collapse load, lower than the linear one.",
    fields: [{ k: "table", l: "Imperfection from the buckling modes", cols: ["Mode", "Size (mm)"], rows: [["1", "L/1000"], ["2", ""]] }, { k: "choice", l: "Follow by", o: ["Arc length", "Load control"] }, { k: "num", l: "Largest load factor", v: "2" }] },
  { id: "transient", label: "Heat in Time", icon: "heat", step: 5, tab: "solve", group: "Planned analyses", v: "0.13", for: ["heat"],
    what: "Temperatures changing in time: a part warming up or cooling down, with heat stored by its mass.",
    fields: [{ k: "num", l: "Length of time", u: "s" }, { k: "num", l: "Time step", u: "s" }, { k: "num", l: "Starting temperature", u: "°C", v: "20" }, { k: "choice", l: "Stepping", o: ["Backward Euler", "Crank–Nicolson"] }] },
  { id: "coupled", label: "Heat and Stress Together", icon: "tchange", step: 5, tab: "solve", group: "Planned analyses", v: "0.13", for: PARTS,
    what: "Heat and stress solved together, each changing the other (friction heating, hot forming).",
    fields: [{ k: "num", l: "Length of time", u: "s" }, { k: "num", l: "Time step", u: "s" }, { k: "check", l: "Heat from plastic work", on: true }, { k: "num", l: "Share of plastic work turned to heat", v: "0.9" }] },
  { id: "crash", label: "Crash", icon: "wave", step: 5, tab: "solve", group: "Planned analyses", v: "0.13", for: SOLID,
    what: "Impact with yielding, contact and failure: parts crushed, folded and torn (impact already works for elastic solids).",
    fields: [{ k: "num", l: "Length of time", u: "ms" }, { k: "check", l: "Contact everywhere (general)", on: true }, { k: "check", l: "Remove failed elements", on: true }, { k: "num", l: "Mass scaling: smallest step", u: "µs" }] },
  { id: "combogen", label: "Combinations by Rule", icon: "table", step: 3, tab: "loads", group: "Load cases", v: "0.14", for: FRAMES,
    what: "Make the combinations a design rule asks for from the load cases (ultimate and serviceability, with their factors).",
    fields: [{ k: "choice", l: "Rule", o: ["Ultimate and serviceability (partial factors)", "Your own factors"] }, { k: "table", l: "Factors", cols: ["Case", "Kind", "Ultimate", "Serviceability"] }] },
  { id: "windgen", label: "Wind Loads", icon: "dist", step: 3, tab: "loads", group: "Load generators", v: "1.0", for: FRAMES,
    what: "Wind pressures on a building's faces from the wind speed, the terrain and the building's shape.",
    fields: [{ k: "num", l: "Basic wind speed", u: "m/s" }, { k: "choice", l: "Terrain", o: ["Sea", "Open country", "Suburbs", "Town centre"] }, { k: "choice", l: "Direction", o: ["+x", "−x", "+y", "−y"] }, { k: "pick", l: "Faces it blows on", what: "faces or members" }] },
  { id: "quakegen", label: "Earthquake Loads", icon: "wave", step: 3, tab: "loads", group: "Load generators", v: "1.0", for: FRAMES,
    what: "Storey forces from the building's weight and a design spectrum (the equivalent static method).",
    fields: [{ k: "num", l: "Ground acceleration", u: "g" }, { k: "choice", l: "Ground", o: ["Rock", "Firm", "Soft"] }, { k: "num", l: "Behaviour factor", v: "1.5" }, { k: "choice", l: "Direction", o: ["x", "y"] }] },
  { id: "moving", label: "Moving Load", icon: "force", step: 3, tab: "loads", group: "Load generators", v: "1.0", for: FRAMES,
    what: "A vehicle or a crane crossing a bridge or a runway beam: the worst place for each result (influence lines).",
    fields: [{ k: "table", l: "Axles", cols: ["Load (kN)", "Spacing (m)"], rows: [["", "0"], ["", ""]] }, { k: "pick", l: "Path", what: "members in a row" }, { k: "num", l: "Step", u: "m", v: "0.5" }] },
  { id: "centrifugal", label: "Spinning", icon: "repeat", step: 3, tab: "loads", group: "More loads", v: "0.12", for: PARTS,
    what: "The load on a part spinning about an axis: discs, rotors, flywheels.",
    fields: [{ k: "num", l: "Speed", u: "rpm" }, { k: "pick", l: "Axis", what: "an axis or two points" }] },
  { id: "bolt", label: "Bolt Pretension", icon: "pin", step: 3, tab: "loads", group: "More loads", v: "0.12", for: SOLID,
    what: "A bolt tightened before the loads come: its force (or how far it is drawn up) held, then locked.",
    fields: [{ k: "pick", l: "Bolt (a solid or a cut through it)", what: "a solid" }, { k: "choice", l: "Set by", o: ["Force", "Length drawn up"] }, { k: "num", l: "Force", u: "kN" }, { k: "choice", l: "Then", o: ["Locked at its length", "Force kept"] }] },
  { id: "curves", label: "Load Curves", icon: "curve", step: 3, tab: "loads", group: "More loads", v: "0.12", for: ALL,
    what: "A factor against time, named and shared by any load or support: a ramp, a pulse, a measured record.",
    fields: [{ k: "text", l: "Name", v: "Curve-1" }, { k: "choice", l: "Shape", o: ["Table", "Ramp", "Smooth step", "Half sine pulse", "Periodic (sine)", "From a file (CSV)"] }, { k: "curve", l: "Points", x: "Time (s)", y: "Factor" }] },
  { id: "initstress", label: "Starting Stress", icon: "tchange", step: 3, tab: "loads", group: "Starting conditions", v: "0.12", for: PARTS,
    what: "Stress already in a part before the loads: residual stress from welding or forming, or from an earlier analysis.",
    fields: [{ k: "pick", l: "Where", what: "parts or faces" }, { k: "choice", l: "From", o: ["Values", "An earlier analysis's results"] }, { k: "num", l: "σx", u: "MPa" }, { k: "num", l: "σy", u: "MPa" }, { k: "num", l: "σz", u: "MPa" }, { k: "num", l: "τxy", u: "MPa" }] },
  { id: "inittemp", label: "Starting Temperatures", icon: "temp", step: 3, tab: "loads", group: "Starting conditions", v: "0.12", for: ALL,
    what: "The temperature everywhere at the start, uniform or from a heat analysis.",
    fields: [{ k: "choice", l: "From", o: ["One temperature", "A heat analysis of this model"] }, { k: "num", l: "Temperature", u: "°C", v: "20" }] },
  { id: "largecontact", label: "Sliding Contact", icon: "slot", step: 3, tab: "loads", group: "Interactions", v: "0.13", for: SOLID,
    what: "Contact where faces slide far over each other, and a part folding onto itself (the folds of a crushed tube).",
    fields: [{ k: "choice", l: "Kind", o: ["Between two faces (large sliding)", "A part on itself", "Everything that touches (general)"] }, { k: "pick", l: "Faces", what: "faces or parts" }, { k: "num", l: "Friction μ", v: "0.2" }] },
  { id: "tie", label: "Tie", icon: "union", step: 3, tab: "loads", group: "Interactions", v: "0.12", for: SOLID,
    what: "Glue two faces whose meshes do not match: they move together as one, without contact.",
    fields: [{ k: "pick", l: "Face of the first part", what: "a face" }, { k: "pick", l: "Face of the second part", what: "a face" }, { k: "num", l: "Gap it closes up to", u: "mm", v: "auto" }] },
  { id: "coupling", label: "Coupling", icon: "pin", step: 3, tab: "loads", group: "Interactions", v: "0.12", for: [...SOLID, "plane"],
    what: "Join a face to a reference point: rigidly (kinematic), or spreading the point's load over the face (distributing).",
    fields: [{ k: "pick", l: "Reference point", what: "a reference point" }, { k: "pick", l: "Face", what: "faces" }, { k: "choice", l: "Kind", o: ["Kinematic (the face moves rigidly)", "Distributing (the load spread)"] }, { k: "table", l: "Directions joined", cols: ["x", "y", "z", "θx", "θy", "θz"], rows: [["✓", "✓", "✓", "✓", "✓", "✓"]] }] },
  { id: "rigid", label: "Rigid Body", icon: "solid", step: 3, tab: "loads", group: "Interactions", v: "0.12", for: SOLID,
    what: "A part that moves without changing shape, led by its reference point: a punch, a die, a wall.",
    fields: [{ k: "pick", l: "Part", what: "a part" }, { k: "pick", l: "Reference point", what: "a reference point" }] },
  { id: "connector", label: "Connector", icon: "spring", step: 3, tab: "loads", group: "Interactions", v: "0.12", for: [...SOLID, "space"],
    what: "A joint between two points: a hinge, a bolt, a slider, a spring or a damper, each free or held in its own directions.",
    fields: [{ k: "choice", l: "Kind", o: ["Hinge", "Ball joint", "Slider", "Bolt (beam)", "Spring", "Damper", "Welded"] }, { k: "pick", l: "Between", what: "two points" }, { k: "num", l: "Stiffness", u: "kN/m" }] },

  // ---------- 4. Mesh ----------
  { id: "quality3d", label: "Mesh Quality", icon: "quality", step: 4, tab: "mesh", group: "Planned", v: "0.10.x", for: SOLID,
    what: "How good the 3D mesh is: the shape of each element coloured, the worst ones listed and found in the view.",
    fields: [{ k: "choice", l: "Measure", o: ["Aspect ratio", "Smallest angle", "Volume against its edges"] }, { k: "num", l: "Show worse than", v: "" }] },
  { id: "hex", label: "Hexahedra", icon: "mesh", step: 4, tab: "mesh", group: "Planned", v: "0.14", for: SOLID,
    what: "Brick elements: swept along a solid, mapped on simple blocks, or mostly bricks with tetrahedra to fill.",
    fields: [{ k: "choice", l: "Way", o: ["Swept", "Mapped", "Bricks where possible"] }, { k: "choice", l: "Element", o: ["8-node", "20-node"] }, { k: "num", l: "Size", u: "mm" }] },
  { id: "adapt", label: "Refine by Itself", icon: "refine", step: 4, tab: "mesh", group: "Planned", v: "0.14", for: PARTS,
    what: "Solve, find where the error is largest, refine there, and solve again until the result stops changing.",
    fields: [{ k: "num", l: "Error aimed for", u: "%", v: "5" }, { k: "num", l: "Most rounds", v: "5" }, { k: "choice", l: "Watch", o: ["Von Mises stress", "Displacement", "Temperature"] }] },
  { id: "convergence", label: "Convergence Study", icon: "curve", step: 4, tab: "mesh", group: "Planned", v: "0.14", for: PARTS,
    what: "Solve the same model at several element sizes and graph a result against the size: how fine is fine enough.",
    fields: [{ k: "table", l: "Element sizes", cols: ["Size (mm)"], rows: [["8"], ["4"], ["2"], ["1"]] }, { k: "pick", l: "Result at", what: "a point" }, { k: "choice", l: "Result", o: ["Von Mises stress", "Displacement"] }] },
  { id: "sets", label: "Sets", icon: "list", step: 4, tab: "mesh", group: "Planned", v: "0.14", for: STRUCT,
    what: "Named groups of nodes, elements, faces or parts, to load, support or show together.",
    fields: [{ k: "text", l: "Name", v: "Set-1" }, { k: "choice", l: "Of", o: ["Nodes", "Elements", "Faces", "Parts"] }, { k: "pick", l: "Members", what: "items in the view" }] },

  // ---------- 5. Solve ----------
  { id: "jobs", label: "Job Queue", icon: "list", step: 5, tab: "solve", group: "Jobs", v: "0.15", for: ALL,
    what: "Several analyses run one after another (or saved models), while you keep working; each with its monitor and log.",
    fields: [{ k: "table", l: "Queue", cols: ["Model", "Analysis", "Status", "Time"] }, { k: "check", l: "Tell me when each finishes", on: true }] },
  { id: "output", label: "What to Keep", icon: "report", step: 5, tab: "solve", group: "Jobs", v: "0.15", for: STRUCT,
    what: "Which results each step keeps, and how often: keeps files small and long analyses quick.",
    fields: [{ k: "table", l: "Results", cols: ["Result", "Keep", "Every"], rows: [["Displacement", "✓", "increment"], ["Stress", "✓", "increment"], ["Strain", "", ""], ["Reactions", "✓", "increment"], ["Energies", "✓", "10 steps"]] }] },
  { id: "restart", label: "Restart from a Step", icon: "first", step: 5, tab: "solve", group: "Jobs", v: "0.15", for: STRUCT,
    what: "Carry on from where an analysis stopped, or add a step after it, without solving the first ones again.",
    fields: [{ k: "choice", l: "From", o: ["The last increment reached", "The end of a step"] }, { k: "num", l: "Step", v: "1" }] },
  { id: "tolerance", label: "Solver Settings", icon: "opts", step: 5, tab: "solve", group: "Jobs", v: "0.10.x", for: STRUCT,
    what: "How the nonlinear solver works: the tolerance, the most iterations, how increments are cut and grown, line search.",
    fields: [{ k: "num", l: "Force tolerance", v: "1e-8" }, { k: "num", l: "Most iterations per increment", v: "30" }, { k: "num", l: "Smallest increment", v: "1e-5" }, { k: "check", l: "Line search", on: false }, { k: "check", l: "Automatic increments", on: true }] },
  { id: "parallel", label: "All Cores", icon: "solve", step: 5, tab: "solve", group: "Jobs", v: "0.15", for: STRUCT,
    what: "Assemble and solve on all the processor's cores, for large models.",
    fields: [{ k: "num", l: "Cores", v: "all" }] },
  { id: "deck", label: "Write an Input File", icon: "export", step: 5, tab: "solve", group: "Jobs", v: "0.15", for: STRUCT,
    what: "Write the model as an input file for other open solvers (CalculiX), to check one solver against another.",
    fields: [{ k: "choice", l: "Format", o: ["CalculiX (.inp)"] }, { k: "check", l: "With the mesh", on: true }] },

  // ---------- 6. Results ----------
  { id: "restree", label: "Results Tree", icon: "list", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: ALL,
    what: "Everything solved, as a tree: steps and increments, fields, graphs, cuts, display groups and pictures.",
    fields: [{ k: "check", l: "Show the Results tab", on: true }] },
  { id: "bands", label: "Exact Colour Bands", icon: "contour", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: [...PARTS, "heat"],
    what: "Contours cut exactly at the band edges, with sharp lines between them, and the largest and smallest values marked in the view.",
    fields: [{ k: "num", l: "Bands", v: "12" }, { k: "choice", l: "Scale", o: ["Rainbow", "Blue to red", "Grey"] }, { k: "check", l: "Mark the largest and smallest", on: true }, { k: "check", l: "Lines between bands", on: true }] },
  { id: "cuts", label: "Several Cuts", icon: "cut", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: SOLID,
    what: "More than one cut plane through a solid, each moved and turned in the view.",
    fields: [{ k: "table", l: "Cuts", cols: ["Plane", "Position (mm)", "Show"], rows: [["x", "0", "✓"], ["z", "", ""]] }] },
  { id: "groups", label: "Display Groups", icon: "view", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: ALL,
    what: "Show only some parts, faces or members, to look inside an assembly or at one region.",
    fields: [{ k: "text", l: "Name", v: "Group-1" }, { k: "pick", l: "What to show", what: "parts, faces or members" }] },
  { id: "freebody", label: "Free-Body Forces", icon: "reactions", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: STRUCT,
    what: "The force and moment passed through a cut: what a weld, a bolt group or a section carries.",
    fields: [{ k: "pick", l: "Cut through", what: "faces or a plane" }, { k: "choice", l: "About", o: ["The cut's centre", "A point"] }] },
  { id: "linearise", label: "Stress Through a Wall", icon: "curve", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: PARTS,
    what: "The stress along a line through a wall split into membrane, bending and peak (pressure vessel checks).",
    fields: [{ k: "pick", l: "From", what: "a point on the inside" }, { k: "pick", l: "To", what: "a point on the outside" }] },
  { id: "compare", label: "Compare Analyses", icon: "curve", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: ALL,
    what: "Graph the same result from two analyses (or two meshes, or two materials) on one graph.",
    fields: [{ k: "table", l: "Curves", cols: ["Analysis", "Result", "Where"] }] },
  { id: "design", label: "Member Design Checks", icon: "util", step: 6, tab: "results", group: "Planned", v: "1.0", for: FRAMES,
    what: "Check each steel or concrete member against a design rule: resistance, buckling and deflection, with the working shown.",
    fields: [{ k: "choice", l: "Rule", o: ["Steel: partial factors", "Concrete: partial factors"] }, { k: "num", l: "γM0", v: "1.0" }, { k: "num", l: "γM1", v: "1.0" }, { k: "num", l: "Deflection limit span /", v: "250" }] },
  { id: "export", label: "Export Results", icon: "export", step: 6, tab: "results", group: "Planned", v: "0.10.x", for: ALL,
    what: "Results to a CSV table, a VTU file for other viewers, a picture, or a video of the animation.",
    fields: [{ k: "choice", l: "As", o: ["CSV table", "VTU (ParaView and others)", "PNG picture", "MP4 video"] }, { k: "check", l: "Every increment", on: false }] },

  // ---------- The whole program ----------
  { id: "render", label: "Render Style", icon: "view", step: 0, tab: "view", group: "Planned", v: "0.10.x", for: ALL,
    what: "Shaded, shaded with edges, wireframe or hidden lines, one click each.",
    fields: [{ k: "choice", l: "Style", o: ["Shaded", "Shaded with edges", "Wireframe", "Hidden lines"] }] },
  { id: "filter", label: "Selection Filter", icon: "select", step: 0, tab: "view", group: "Planned", v: "0.10.x", for: ALL,
    what: "Pick only faces, edges, nodes, members or parts, so a click never takes the wrong thing.",
    fields: [{ k: "choice", l: "Pick", o: ["Anything", "Faces", "Edges", "Nodes", "Members", "Parts"] }] },
  { id: "views", label: "Saved Views", icon: "fit", step: 0, tab: "view", group: "Planned", v: "0.10.x", for: ALL,
    what: "Keep up to four views (angle and zoom) and go back to each with one key.",
    fields: [{ k: "table", l: "Views", cols: ["Key", "Name"], rows: [["1", "Front"], ["2", "Iso"], ["3", ""], ["4", ""]] }] },
  { id: "background", label: "Background", icon: "theme", step: 0, tab: "view", group: "Planned", v: "0.10.x", for: ALL,
    what: "A soft gradient, a plain colour, or white for printing behind the model.",
    fields: [{ k: "choice", l: "Background", o: ["Gradient", "Plain", "White (print)"] }] },
  { id: "annotate", label: "Labels and Arrows", icon: "note", step: 0, tab: "view", group: "Planned", v: "0.15", for: ALL,
    what: "Notes, arrows and dimensions on the view, kept with the model and printed in the report.",
    fields: [{ k: "text", l: "Text" }, { k: "pick", l: "Pointing at", what: "a point" }] },
  { id: "models", label: "Several Models", icon: "copy", step: 0, tab: "view", group: "Planned", v: "1.0", for: ALL,
    what: "More than one model in one file, side by side: design options, or a coarse and a fine mesh.",
    fields: [{ k: "table", l: "Models", cols: ["Name", "Analysis"], rows: [["Model-1", ""], ["Model-2", ""]] }] },
  { id: "script", label: "Python Console", icon: "fx", step: 0, tab: "view", group: "Planned", v: "0.15", for: ALL,
    what: "Drive the program from Python: build models, run analyses, read results, add your own materials. A console in the bottom panel, scripts to run.",
    fields: [{ k: "text", l: ">>>", v: "model.add_support('Fixed', face='Body1/F2')" }, { k: "check", l: "Record what I do as a script", on: false }] },
];

/** The model's kind, for `for`. */
export const kindOf = (a: string | undefined): Kind => a === "frame" ? "frame" : a === "space" ? "space" : a === "solid" ? "solid" : a === "heat" || a === "solidheat" ? "heat" : "plane";
/** The planned features for a model, optionally of one tab or step. */
export const planned = (a: string | undefined, f: { tab?: string; step?: number } = {}) => {
  const k = kindOf(a);
  return FEATS.filter(x => (!x.for || x.for.includes(k)) && (f.tab === undefined || x.tab === f.tab) && (f.step === undefined || x.step === f.step));
};
