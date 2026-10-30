// Commands: what the ribbon, menus, keys and Tell Me run.
import { batch } from "solid-js";
import { Out, Analysis, Mesh, invoke, fromWire, unwire, modesFromWire, num, isFrame, isBuckle } from "./api";
import { setOut, act, call, sel, setSel, edgeSel, setBackstage, loadOut, toast, setView, view, setTool, tool, model, setEdgeSel, results, setResults, setStale, solving, setSolving, setTab, show, setShow, Show, meshed, setMeshed, setMeshStale, setModes, setModesStale, setModeSel, setModeOn } from "./store";
import { fieldAfterSolve } from "./parts";
import { fit, zoomBy, cancelDraft } from "./canvas";
import { defaultLoad } from "./panels";
import * as D from "./dialogs";
import * as K from "./sketch";
import { niceGrid } from "./units";

export const C = {
  // ---------- file ----------
  save: async (asNew = false) => { const o = await call<Out | null>("save", { asNew }); if (o) { setOut(o); toast("Saved.", true); } },
  // Each of these replaces the open model, so unsaved changes are offered for saving first (0.1.1 bug: they were lost).
  open: () => D.keepChanges(() => loadOut("open", { path: null }).then(o => { if (o) setTimeout(fit, 60); })),
  openPath: (path: string) => D.keepChanges(() => loadOut("open", { path }).then(o => { if (o) setTimeout(fit, 60); })),
  newModel: (example: string | null, analysis: Analysis = "frame") => D.keepChanges(() => loadOut("new_model", { example, analysis }).then(o => { if (o) setTimeout(fit, 60); })),
  print: () => setBackstage("print"),
  undo: async () => { const o = await call<Out>("history", { redo: false }); if (o) { setOut(o); if (results()) setStale(true); if (meshed()) setMeshStale(true); } },
  redo: async () => { const o = await call<Out>("history", { redo: true }); if (o) { setOut(o); if (results()) setStale(true); if (meshed()) setMeshStale(true); } },

  // ---------- solving ----------
  solve: async () => {
    if (solving()) return;
    const m = model(); if (!m) return;
    setSolving(true);
    try {
      const r = fromWire(await invoke("solve")), first = !results();
      batch(() => {
        setResults(r); setStale(false); setTab("results"); setModeOn(false);
        if (r.kind === "plane" || r.kind === "heat") { fieldAfterSolve(r.kind === "heat" ? "heat" : m.analysis); setMeshed(null); setMeshStale(false); if (view() === "mesh") setView("model"); }
        if (r.kind === "solid") { fieldAfterSolve("solid"); if (view() === "mesh") setView("model"); }
      });
      // The legend bar takes room under the canvas the first time: fit the model again.
      if (first) setTimeout(fit, 40);
      const bad = r.imbalance > 1e-6, ms = r.stats.ms < 1 ? "under 1 ms" : r.stats.ms < 2000 ? `${Math.round(r.stats.ms)} ms` : `${(r.stats.ms / 1000).toFixed(1)} s`;
      const what = r.kind === "frame" || r.kind === "space" ? `${r.stats.equations} equations` : r.kind === "solid" ? `${r.elements.toLocaleString()} elements in ${r.layers} ${r.layers === 1 ? "layer" : "layers"}, ${r.stats.equations.toLocaleString()} equations` : `${r.mesh.tris.length.toLocaleString()} elements, ${r.stats.equations.toLocaleString()} equations`;
      const bal = r.kind === "heat" ? (bad ? "Heat in and out do not balance: see Results." : "Heat in balances heat out.") : bad ? "The reactions do not balance the loads: see Results." : "Reactions balance the loads.";
      toast(r.warnings.length ? r.warnings.join(" ") : `Solved: ${what} in ${ms}. ${bal}`, !r.warnings.length && !bad);
    } catch (e) { setResults(null); toast(String(e)); }
    finally { setSolving(false); }
  },
  /** The lowest natural frequencies and their mode shapes (0.2.2). */
  modes: async (count = 6) => {
    const m = model(); if (!m || solving()) return;
    setSolving(true);
    try {
      const r = modesFromWire(await invoke("modes", { count }));
      if (isBuckle(r)) return;
      batch(() => { setModes(r); setModesStale(false); setModeSel(0); setModeOn(true); setView("model"); setTab("results"); });
      toast(`${r.modes.length} natural frequencies: the lowest is ${num(r.modes[0].f, 4)} Hz.`, true);
    } catch (e) { toast(String(e)); }
    finally { setSolving(false); }
  },
  /** The lowest buckling load factors of a frame and their shapes (0.2.3). */
  buckling: async (count = 6) => {
    const m = model(); if (!m || solving()) return;
    setSolving(true);
    try {
      const r = modesFromWire(await invoke("buckling", { count }));
      if (!isBuckle(r)) return;
      batch(() => { setModes(r); setModesStale(false); setModeSel(0); setModeOn(true); setView("model"); setTab("results"); });
      const low = Math.min(r.modes[0]?.lambda ?? Infinity, ...r.bars.map(b => b.lambda));
      toast(r.modes.length ? `The lowest buckling load factor is ${num(low, 4)}.` : r.bars.length ? `No frame buckling; the weakest truss bar buckles at a load factor of ${num(low, 4)}.` : "Nothing buckles under these loads.", true);
      for (const w of r.warnings) toast(w);
    } catch (e) { toast(String(e)); }
    finally { setSolving(false); }
  },
  /** Mesh the part without solving, and show it (the Mesh view). */
  mesh: async () => {
    const m = model(); if (!m) return;
    if (isFrame(m.analysis)) return toast("Frames and trusses are not meshed: each member is one element. Meshing is for parts (plane stress, plane strain, heat).");
    if (solving()) return;
    setSolving(true);
    try {
      const ms: Mesh = unwire(await invoke("mesh")), first = !meshed() && view() !== "mesh";
      batch(() => { setMeshed(ms); setMeshStale(false); setView("mesh"); });
      if (first) setTimeout(fit, 40);
      toast(`Meshed: ${ms.tris.length.toLocaleString()} ${ms.order === 2 ? "6-node" : "3-node"} triangles, ${ms.nodes.length.toLocaleString()} nodes; smallest angle ${Math.round(ms.quality.min_angle)}°.`, true);
    } catch (e) { toast(String(e)); }
    finally { setSolving(false); }
  },
  /** Mesh settings: element size (0 = automatic), element order, local sizes. */
  meshSettings: (s: Partial<{ size: number; order: number; min_angle: number; refine: { at: any; size: number }[] }>) => {
    const m = model(); if (!m) return;
    return act({ SetSettings: { s: { analysis: m.analysis, units: m.units, grid: m.grid, thickness: m.thickness, mesh: { ...m.mesh, ...s } } } });
  },
  show: (s: Partial<Show>) => setShow({ ...show(), ...s }),

  // ---------- editing ----------
  tool: (t: string) => { cancelDraft(); setTool(tool() === t && t !== "select" ? "select" : t); },
  del: () => { if (sel().length) act({ Delete: { ids: sel() } }).then(o => { if (o) setSel([]); }); else if (edgeSel()) toast("An edge belongs to its shape: select the shape to delete it."); },
  selectAll: () => { const m = model(); if (m) setSel([...m.nodes, ...m.members, ...m.shapes].map(x => x.id)); },
  /** Gravity or heat generation on the whole model (one of each). */
  body: async (kind: "gravity" | "heat" | "warming") => {
    const m = model(); if (!m) return;
    const have = m.loads.find(l => l.kind.kind === kind);
    if (have) { setSel([have.id]); return toast(`${{ gravity: "Gravity", heat: "Heat generation", warming: "A temperature change" }[kind]} is already on: change it in the Inspector.`); }
    const o = await act({ Add: { item: { Load: { id: 0, at: { on: "body" }, kind: defaultLoad(kind, isFrame(m.analysis)) } } } });
    if (o?.created.length) { setSel(o.created); setEdgeSel(null); }
  },
  analysis: (a: Analysis) => { const m = model(); if (m) act({ SetSettings: { s: { analysis: a, units: m.units, grid: m.grid, thickness: m.thickness, mesh: m.mesh } } }); },
  // The grid becomes a round step in the new units (0.1.1: 0.5 m showed as 1.6404 ft).
  units: (u: "SI" | "US") => { const m = model(); if (m && m.units !== u) act({ SetSettings: { s: { analysis: m.analysis, units: u, grid: niceGrid(m.grid, u, isFrame(m.analysis)), thickness: m.thickness, mesh: m.mesh } } }); },
  view: (v: "model" | "results" | "mesh" | "report") => setView(v),
  zoomIn: () => zoomBy(1.25), zoomOut: () => zoomBy(1 / 1.25), fit,
  toastSize: () => toast("Type the element size in the Inspector, under Mesh (click empty canvas to show the model's settings). 0 means automatic."),
  planned: (what: string, v: string) => toast(`${what} arrives in ${v}. File › Roadmap shows what comes when.`),
};

/** The equilibrium check from the engine: the largest force or moment imbalance, relative to the size of the loads. */
export const imbalanceOf = (r: { imbalance: number }) => r.imbalance;

export const ALL_CMDS: [string, () => void, string?][] = [
  ["Solve", () => C.solve(), "F5"], ["Natural frequencies and mode shapes", () => C.modes()], ["Buckling load factors and shapes", () => C.buckling()], ["Mesh the part", () => C.mesh()], ["Refine the mesh near an edge or point", () => C.tool("refine")], ["Quadratic elements (6-node)", () => C.meshSettings({ order: 2 })], ["Linear elements (3-node)", () => C.meshSettings({ order: 1 })], ["Automatic element size", () => C.meshSettings({ size: 0 })],
  ["Von Mises stress", () => C.show({ field: "vm" })], ["Displacement", () => C.show({ field: "umag" })], ["Safety factor", () => C.show({ field: "sf" })], ["Temperature", () => C.show({ field: "t" })], ["Heat flux", () => C.show({ field: "qmag" })], ["Mesh lines on results", () => C.show({ mesh: !show().mesh })], ["Materials library", () => D.library()], ["Results table", () => setView("results")], ["Verification", () => D.verification()],
  ["Deflected shape", () => C.show({ deformed: !show().deformed })], ["Bending moment diagram", () => C.show({ diagram: "m" })], ["Shear force diagram", () => C.show({ diagram: "v" })], ["Axial force diagram", () => C.show({ diagram: "n" })],
  ["Reactions", () => C.show({ reactions: !show().reactions })], ["Utilisation colours", () => C.show({ util: !show().util })],
  ["Save", () => C.save(), "Ctrl+S"], ["Save As", () => C.save(true)], ["Open", C.open, "Ctrl+O"], ["New model", () => setBackstage("home"), "Ctrl+N"], ["Print / PDF", C.print, "Ctrl+P"],
  ["Undo", C.undo, "Ctrl+Z"], ["Redo", C.redo, "Ctrl+Y"], ["Delete", C.del, "Del"], ["Select all", C.selectAll, "Ctrl+A"], ["Select", () => C.tool("select"), "Esc"],
  ["Draw a node", () => C.tool("node")], ["Draw beams", () => C.tool("beam")], ["Draw truss bars", () => C.tool("truss")], ["Add a node at coordinates", () => D.nodeAt()],
  ["Draw a rectangle", () => C.tool("rect")], ["Draw a circle", () => C.tool("circle")], ["Draw a polygon", () => C.tool("poly")], ["Cut a rectangular hole", () => C.tool("hrect")], ["Cut a round hole", () => C.tool("hcircle")],
  ["Slot", () => K.makeShape("slot")], ["Rounded rectangle", () => K.makeShape("rounded")], ["Regular polygon (hexagon, …)", () => K.makeShape("polygon")], ["Round corners (fillet)", () => K.corners(true)], ["Bevel corners (chamfer)", () => K.corners(false)],
  ["Union: join two shapes", () => K.combine("union")], ["Subtract: cut one shape out of another", () => K.combine("subtract")], ["Intersect: keep where two shapes overlap", () => K.combine("intersect")], ["Offset an outline", K.offset], ["Linear pattern of shapes", () => K.pattern(false)], ["Circular pattern of holes", () => K.pattern(true)],
  ["Sections", () => D.sections()], ["Materials", () => D.materials()],
  ["Fixed support", () => C.tool("sup:fixed")], ["Pinned support", () => C.tool("sup:pinned")], ["Roller support", () => C.tool(isFrame(model()?.analysis) ? "sup:rollerx" : "sup:roller")], ["Spring support", () => C.tool("sup:spring")], ["Symmetry", () => C.tool("sup:symmetry")],
  ["Force", () => C.tool("load:force")], ["Moment", () => C.tool("load:moment")], ["Distributed load", () => C.tool("load:dist")], ["Pressure", () => C.tool("load:pressure")], ["Edge force", () => C.tool("load:traction")], ["Pin load (bearing in a hole)", () => C.tool("load:bearing")], ["Gravity (self-weight)", () => C.body("gravity")],
  ["Fixed temperature", () => C.tool("sup:temperature")], ["Heat flux", () => C.tool("load:flux")], ["Convection", () => C.tool("load:convection")], ["Heat generation", () => C.body("heat")], ["Temperature change (thermal stress)", () => C.body("warming")],
  ["Frame or truss model", () => C.analysis("frame")], ["3D frame model", () => C.analysis("space")], ["Repeat members along the depth (3D frames)", () => D.repeat()], ["3D view", () => import("./plane3").then(p => p.setPlane("3d"))], ["Front plane (x-y)", () => import("./plane3").then(p => p.setPlane("xy"))], ["Side plane (z-y)", () => import("./plane3").then(p => p.setPlane("zy"))], ["Plan (x-z)", () => import("./plane3").then(p => p.setPlane("xz"))], ["Plane stress model", () => C.analysis("stress")], ["Plane strain model", () => C.analysis("strain")], ["Heat conduction model", () => C.analysis("heat")], ["Axisymmetric model (turned about the y axis)", () => C.analysis("axi")],
  ["SI units", () => C.units("SI")], ["US units", () => C.units("US")],
  ["Zoom in", C.zoomIn], ["Zoom out", C.zoomOut], ["Fit to window", fit, "F"], ["Options", () => setBackstage("options")], ["Check for updates", () => setBackstage("about")], ["Roadmap", () => setBackstage("roadmap")],
  ["Keyboard shortcuts", () => D.shortcuts(), "F1"], ["Getting started", () => D.gettingStarted()], ["Glossary", () => D.glossary()],
];
