// Commands: what the ribbon, menus, keys and Tell Me run.
import { batch } from "solid-js";
import { Out, Analysis, FrameResults, invoke } from "./api";
import { setOut, act, call, sel, setSel, edgeSel, setBackstage, loadOut, toast, setView, setTool, tool, model, setEdgeSel, results, setResults, setStale, solving, setSolving, setTab, show, setShow, Show } from "./store";
import { fit, zoomBy, cancelDraft } from "./canvas";
import { defaultLoad } from "./panels";
import * as D from "./dialogs";

export const C = {
  // ---------- file ----------
  save: async (asNew = false) => { const o = await call<Out | null>("save", { asNew }); if (o) { setOut(o); toast("Saved.", true); } },
  open: () => loadOut("open", { path: null }).then(o => { if (o) setTimeout(fit, 60); }),
  openPath: (path: string) => loadOut("open", { path }).then(o => { if (o) setTimeout(fit, 60); }),
  newModel: (example: string | null, analysis: Analysis = "frame") => loadOut("new_model", { example, analysis }).then(o => { if (o) setTimeout(fit, 60); }),
  print: () => setBackstage("print"),
  undo: async () => { const o = await call<Out>("history", { redo: false }); if (o) { setOut(o); if (results()) setStale(true); } },
  redo: async () => { const o = await call<Out>("history", { redo: true }); if (o) { setOut(o); if (results()) setStale(true); } },

  // ---------- solving ----------
  solve: async () => {
    if (solving()) return;
    const m = model(); if (!m) return;
    if (m.analysis !== "frame") return C.planned("Meshing and solving parts", "0.1.2");
    setSolving(true);
    try {
      const r = await invoke<FrameResults>("solve"), first = !results();
      batch(() => { setResults(r); setStale(false); setTab("results"); });
      // The legend bar takes room under the canvas the first time: fit the model again.
      if (first) setTimeout(fit, 40);
      const bad = imbalanceOf(r) > 1e-6;
      toast(r.warnings.length ? r.warnings.join(" ") : `Solved: ${r.stats.equations} equations in ${r.stats.ms < 1 ? "under 1" : Math.round(r.stats.ms)} ms. ${bad ? "The reactions do not balance the loads: see Results." : "Reactions balance the loads."}`, !r.warnings.length && !bad);
    } catch (e) { setResults(null); toast(String(e)); }
    finally { setSolving(false); }
  },
  show: (s: Partial<Show>) => setShow({ ...show(), ...s }),

  // ---------- editing ----------
  tool: (t: string) => { cancelDraft(); setTool(tool() === t && t !== "select" ? "select" : t); },
  del: () => { if (sel().length) act({ Delete: { ids: sel() } }).then(o => { if (o) setSel([]); }); else if (edgeSel()) toast("An edge belongs to its shape: select the shape to delete it."); },
  selectAll: () => { const m = model(); if (m) setSel([...m.nodes, ...m.members, ...m.shapes].map(x => x.id)); },
  /** Gravity or heat generation on the whole model (one of each). */
  body: async (kind: "gravity" | "heat") => {
    const m = model(); if (!m) return;
    const have = m.loads.find(l => l.kind.kind === kind);
    if (have) { setSel([have.id]); return toast(kind === "gravity" ? "Gravity is already on: change it in the Inspector." : "Heat generation is already on: change it in the Inspector."); }
    const o = await act({ Add: { item: { Load: { id: 0, at: { on: "body" }, kind: defaultLoad(kind, m.analysis === "frame") } } } });
    if (o?.created.length) { setSel(o.created); setEdgeSel(null); }
  },
  analysis: (a: Analysis) => { const m = model(); if (m) act({ SetSettings: { s: { analysis: a, units: m.units, grid: m.grid, thickness: m.thickness, mesh: m.mesh } } }); },
  units: (u: "SI" | "US") => { const m = model(); if (m) act({ SetSettings: { s: { analysis: m.analysis, units: u, grid: m.grid, thickness: m.thickness, mesh: m.mesh } } }); },
  view: (v: "model" | "results") => setView(v),
  zoomIn: () => zoomBy(1.25), zoomOut: () => zoomBy(1 / 1.25), fit,
  planned: (what: string, v: string) => toast(`${what} arrives in ${v}. File › Roadmap shows what comes when.`),
};

/** The largest imbalance between the loads and the reactions, relative to the loads. */
export const imbalanceOf = (r: FrameResults) => {
  const s = Math.max(Math.abs(r.applied[0]), Math.abs(r.applied[1]), 1e-30);
  return Math.max(Math.abs(r.applied[0] + r.reacted[0]), Math.abs(r.applied[1] + r.reacted[1])) / s;
};

export const ALL_CMDS: [string, () => void, string?][] = [
  ["Solve", () => C.solve(), "F5"], ["Results table", () => setView("results")], ["Verification", () => D.verification()],
  ["Deflected shape", () => C.show({ deformed: !show().deformed })], ["Bending moment diagram", () => C.show({ diagram: "m" })], ["Shear force diagram", () => C.show({ diagram: "v" })], ["Axial force diagram", () => C.show({ diagram: "n" })],
  ["Reactions", () => C.show({ reactions: !show().reactions })], ["Utilisation colours", () => C.show({ util: !show().util })],
  ["Save", () => C.save(), "Ctrl+S"], ["Save As", () => C.save(true)], ["Open", C.open, "Ctrl+O"], ["New model", () => setBackstage("home"), "Ctrl+N"], ["Print / PDF", C.print, "Ctrl+P"],
  ["Undo", C.undo, "Ctrl+Z"], ["Redo", C.redo, "Ctrl+Y"], ["Delete", C.del, "Del"], ["Select all", C.selectAll, "Ctrl+A"], ["Select", () => C.tool("select"), "Esc"],
  ["Draw a node", () => C.tool("node")], ["Draw beams", () => C.tool("beam")], ["Draw truss bars", () => C.tool("truss")], ["Add a node at coordinates", () => D.nodeAt()],
  ["Draw a rectangle", () => C.tool("rect")], ["Draw a circle", () => C.tool("circle")], ["Draw a polygon", () => C.tool("poly")], ["Cut a rectangular hole", () => C.tool("hrect")], ["Cut a round hole", () => C.tool("hcircle")],
  ["Sections", () => D.sections()], ["Materials", () => D.materials()],
  ["Fixed support", () => C.tool("sup:fixed")], ["Pinned support", () => C.tool("sup:pinned")], ["Roller support", () => C.tool(model()?.analysis === "frame" ? "sup:rollerx" : "sup:roller")], ["Spring support", () => C.tool("sup:spring")], ["Symmetry", () => C.tool("sup:symmetry")],
  ["Force", () => C.tool("load:force")], ["Moment", () => C.tool("load:moment")], ["Distributed load", () => C.tool("load:dist")], ["Pressure", () => C.tool("load:pressure")], ["Edge force", () => C.tool("load:traction")], ["Gravity (self-weight)", () => C.body("gravity")],
  ["Fixed temperature", () => C.tool("sup:temperature")], ["Heat flux", () => C.tool("load:flux")], ["Convection", () => C.tool("load:convection")], ["Heat generation", () => C.body("heat")],
  ["Frame or truss model", () => C.analysis("frame")], ["Plane stress model", () => C.analysis("stress")], ["Plane strain model", () => C.analysis("strain")], ["Heat conduction model", () => C.analysis("heat")],
  ["SI units", () => C.units("SI")], ["US units", () => C.units("US")],
  ["Zoom in", C.zoomIn], ["Zoom out", C.zoomOut], ["Fit to window", fit, "F"], ["Options", () => setBackstage("options")], ["Check for updates", () => setBackstage("about")], ["Roadmap", () => setBackstage("roadmap")],
  ["Keyboard shortcuts", () => D.shortcuts(), "F1"], ["Getting started", () => D.gettingStarted()], ["Glossary", () => D.glossary()],
];
