// Commands: what the ribbon, menus, keys and Tell Me run.
import { Out, Analysis } from "./api";
import { setOut, act, call, sel, setSel, edgeSel, setBackstage, loadOut, toast, setView, setTool, tool, model, setEdgeSel } from "./store";
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
  undo: async () => { const o = await call<Out>("history", { redo: false }); if (o) setOut(o); },
  redo: async () => { const o = await call<Out>("history", { redo: true }); if (o) setOut(o); },

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
  view: (v: "model") => setView(v),
  zoomIn: () => zoomBy(1.25), zoomOut: () => zoomBy(1 / 1.25), fit,
  planned: (what: string, v: string) => toast(`${what} arrives in ${v}. File › Roadmap shows what comes when.`),
};

export const ALL_CMDS: [string, () => void, string?][] = [
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
