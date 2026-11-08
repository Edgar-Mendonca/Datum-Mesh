// The geometry engine in the window (0.7.1): sketches on a plane and solids made from them, by
// the engine behind crates/cad (the shell's cad_run). This file keeps what the engine last
// returned (the scene), the drawing tools with their ways of drawing, and the sketch canvas: a
// sheet over the 3D view, square to the sketch's plane, where clicks place the points.
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { invoke, Out } from "./api";
import { toast, note, setTab, model, setOut } from "./store";
import { fmt } from "./units";

export type CadBody = { name: string; pos: number[]; nrm: number[]; tris: number[]; face: number[]; edges: number[][]; closed: boolean };
export type CadCurve = { id: string; pts: [number, number][]; construction: boolean; kind: string; start: string | null; end: string | null; center: string | null; fixed: boolean };
export type CadPoint = { id: string; at: [number, number]; fixed: boolean };
export type CadMark = { id: string; name: string; value: number | null; at: [number, number] };
export type CadSketch = { id: number; name: string; origin: number[]; x: number[]; y: number[]; curves: CadCurve[]; points: CadPoint[]; marks: CadMark[]; profiles: number; status: string };
export type CadScene = { bodies: CadBody[]; sketches: CadSketch[]; features: { id: number; name: string; kind: string; suppressed: boolean }[]; marker: number | null; active: number | null; undo: boolean; redo: boolean; planes?: { name: string; plane: { origin: number[]; x_dir: number[]; y_dir: number[] } | null }[] };

export const [cad, setCad] = createSignal<CadScene | null>(null);
/** Runs one engine command; the scene comes back. False (and a message) when it is refused. */
/** The solids have changed since they were meshed (0.7.3): the mesh is made again before it is used. */
export const [meshOld, setMeshOld] = createSignal(false);
export const cadRun = async (id: string, params: object = {}) => {
  try { setCad(await invoke<CadScene>("cad_run", { id, params })); setMeshOld(true); note(`Geometry: ${id.replace(/^(\w+)\./, (_, g) => `${g} › `).replace(/[._]/g, " ")}`, "info", "action"); return true; } catch (e) { toast(String(e)); return false; }
};
/** One engine command's own reply (a report, a list); null (and a message) when refused. */
export const cadQuery = async <T,>(id: string, params: object = {}) => { try { return await invoke<T>("cad_query", { id, params }); } catch (e) { toast(String(e)); return null; } };
export const cadLoad = async () => { try { setCad(await invoke<CadScene>("cad_scene")); } catch { /* no engine yet */ } };
window.addEventListener("cad-reload", () => { pickTool(null); setCad3Tool(null); cadLoad().then(() => setMeshOld(false)); });
/** The sketch being drawn in, if any. */
export const activeSketch = () => { const c = cad(); return c && c.active != null ? c.sketches.find(s => s.id === c.active) ?? null : null; };

// ---------------------------------------------------------------- the drawing tools

type P2 = [number, number];
/** A drawing tool: the points it takes, what to tell the user for each, and the engine command. */
type Tool = { label: string; icon: string; hint: string[]; cmd: (p: P2[]) => [string, object] | null };
const d = (a: P2, b: P2) => Math.hypot(b[0] - a[0], b[1] - a[1]);
const ang = (a: P2, b: P2) => Math.atan2(b[1] - a[1], b[0] - a[0]) * 180 / Math.PI;
export const [sides, setSides] = createSignal(6);
export const [slotW, setSlotW] = createSignal(10);
/** The corner radius of the next rounded rectangle (mm, 0.7.6). */
export const [roundR, setRoundR] = createSignal(5);
/** The ellipse through centre c, major-axis end m and a point q: its minor radius, q's distance from the major axis. */
const minorR = (c: P2, m: P2, q: P2) => { const L = d(c, m) || 1, u = [(m[0] - c[0]) / L, (m[1] - c[1]) / L]; return Math.abs(-u[1] * (q[0] - c[0]) + u[0] * (q[1] - c[1])); };
export const TOOLS: Record<string, Tool> = {
  line: { label: "Line", icon: "w-line", hint: ["Click where the line starts", "Click the next point (double-click or Esc to end the chain)"], cmd: p => ["sketch.line", { points: p }] },
  arc3: { label: "Arc · 3 points", icon: "w-arc3", hint: ["Click where the arc starts", "Click where it ends", "Click a point it passes through"], cmd: p => ["sketch.arc.three_point", { start: p[0], end: p[1], through: p[2] }] },
  arcc: { label: "Arc · centre and ends", icon: "w-arcc", hint: ["Click the centre", "Click where the arc starts", "Click where it ends (anticlockwise)"], cmd: p => ["sketch.arc.center_point", { center: p[0], start: p[1], end: p[2] }] },
  circle: { label: "Circle · centre and radius", icon: "w-circle", hint: ["Click the centre", "Click a point on the circle"], cmd: p => ["sketch.circle.center", { center: p[0], radius: d(p[0], p[1]) }] },
  circle2: { label: "Circle · 2 points", icon: "w-circle2", hint: ["Click one end of a diameter", "Click the other end"], cmd: p => ["sketch.circle.two_point", { p0: p[0], p1: p[1] }] },
  circle3: { label: "Circle · 3 points", icon: "w-circle3", hint: ["Click a point on the circle", "Click a second point", "Click a third point"], cmd: p => ["sketch.circle.three_point", { p0: p[0], p1: p[1], p2: p[2] }] },
  rect: { label: "Rectangle · 2 corners", icon: "w-rect", hint: ["Click one corner", "Click the opposite corner"], cmd: p => ["sketch.rectangle.two_point", { p0: p[0], p1: p[1] }] },
  rectc: { label: "Rectangle · centre and corner", icon: "w-rectc", hint: ["Click the centre", "Click a corner"], cmd: p => ["sketch.rectangle.center", { center: p[0], corner: p[1] }] },
  rect3: { label: "Rectangle · 3 points", icon: "w-rect3", hint: ["Click where the first side starts", "Click where it ends", "Click a point on the opposite side"], cmd: p => ["sketch.rectangle.three_point", { p0: p[0], p1: p[1], p2: p[2] }] },
  polyin: { label: "Polygon · inside a circle", icon: "w-polyin", hint: ["Click the centre", "Click a corner"], cmd: p => ["sketch.polygon.inscribed", { center: p[0], radius: d(p[0], p[1]), sides: sides(), angle: ang(p[0], p[1]) }] },
  polyout: { label: "Polygon · around a circle", icon: "w-polyout", hint: ["Click the centre", "Click the middle of a side"], cmd: p => ["sketch.polygon.circumscribed", { center: p[0], radius: d(p[0], p[1]), sides: sides(), angle: ang(p[0], p[1]) }] },
  polyedge: { label: "Polygon · by one side", icon: "w-polyedge", hint: ["Click where a side starts", "Click where it ends"], cmd: p => ["sketch.polygon.edge", { p0: p[0], p1: p[1], sides: sides() }] },
  slotc: { label: "Slot · centre to centre", icon: "w-slotc", hint: ["Click the centre of one end", "Click the centre of the other end"], cmd: p => ["sketch.slot.center_to_center", { p0: p[0], p1: p[1], width: slotW() }] },
  slot: { label: "Slot · overall", icon: "w-slot", hint: ["Click one end", "Click the other end"], cmd: p => ["sketch.slot.overall", { p0: p[0], p1: p[1], width: slotW() }] },
  // 0.7.6: more shapes the engine draws.
  rectr: { label: "Rectangle · rounded corners", icon: "w-rectr", hint: ["Click one corner", "Click the opposite corner (the corner radius is set beside the button)"], cmd: p => ["sketch.rectangle.rounded", { p0: p[0], p1: p[1], radius: roundR() }] },
  ellipse: { label: "Ellipse", icon: "w-ellipse", hint: ["Click the centre", "Click the end of the long axis", "Click a point the ellipse passes through across it"], cmd: p => ["sketch.ellipse", { center: p[0], major: p[1], minor_radius: Math.max(minorR(p[0], p[1], p[2]), 1e-3) }] },
  spline: { label: "Spline", icon: "w-spline", hint: ["Click where the curve starts", "Click the points it passes through: double-click or Enter on the last one"], cmd: p => ["sketch.spline.fit_point", { points: p }] },
};
const NEED: Record<string, number> = { rectr: 2, ellipse: 3, spline: 1e9, line: 2, arc3: 3, arcc: 3, circle: 2, circle2: 2, circle3: 3, rect: 2, rectc: 2, rect3: 3, polyin: 2, polyout: 2, polyedge: 2, slotc: 2, slot: 2 };
/** Each shape's ways of drawing, in the order its ▾ lists them. */
export const FAMILIES: [string, string, string[]][] = [
  ["Line", "beam", ["line"]], ["Arc", "curve", ["arc3", "arcc"]], ["Circle", "circle", ["circle", "circle2", "circle3"]],
  ["Rectangle", "rect", ["rect", "rectc", "rect3", "rectr"]], ["Polygon", "ngon", ["polyin", "polyout", "polyedge"]], ["Slot", "slot", ["slotc", "slot"]],
  ["Ellipse", "circle", ["ellipse"]], ["Spline", "curve", ["spline"]],
];
export const [cadTool, setCadTool] = createSignal<string | null>(null);
/** The way last used for each shape (what its button draws). */
export const [ways, setWays] = createSignal<Record<string, string>>({ Line: "line", Arc: "arc3", Circle: "circle", Rectangle: "rect", Polygon: "polyin", Slot: "slotc", Ellipse: "ellipse", Spline: "spline" });
const [draft, setDraft] = createSignal<P2[]>([]);
export const pickTool = (t: string | null) => { setCadTool(t); setDraft([]); };

/** Starts a sketch on one of the origin planes (the floor XY, front XZ, side YZ). */
/** The three planes at the origin, as they are looked at (0.7.5): the floor from above, the front
 * from the front and the side from the right, each with Z (the height) up the screen. (The
 * engine's own XZ plane has its y axis down, so what was drawn upward went below the floor.) */
const PLANES = { XY: "XY", XZ: { origin: [0, 0, 0], x_dir: [1, 0, 0], y_dir: [0, 0, 1] }, YZ: { origin: [0, 0, 0], x_dir: [0, 1, 0], y_dir: [0, 0, 1] } } as const;
export const newSketch = async (plane: "XY" | "XZ" | "YZ" | { named: string }) => {
  if (!(await cadRun("sketch.create", { plane: typeof plane === "string" ? PLANES[plane] : plane.named }))) return;
  setTab("geometry"); pickTool(ways().Line);
  // A 3D frame is metres across: the sheet starts showing about 10 m (a part: about 200 mm).
  if (model()?.analysis === "space") setSheetView({ s: 0.08, cx: 3000, cy: plane === "XY" ? 2000 : 2500 });
};
/** 3D frames drawn with the sketch tools (0.7.5): whether lines become truss bars (else beams). */
export const [memberTruss, setMemberTruss] = createSignal(false);
/** 3D frames (0.7.6): whether closed outlines on their own become plates (else members round them). */
export const [memberPlates, setMemberPlates] = createSignal(false);
export const finishSketch = async () => {
  pickTool(null);
  const sk = activeSketch();
  if (!(await cadRun("sketch.finish"))) return;
  setTurnBack(turnBack() + 1);
  // A 3D frame: the sketch's lines become members, joined where they meet (0.7.5).
  if (model()?.analysis === "space") {
    try {
      const o = await invoke<Out>("cad_members", { truss: memberTruss(), plates: memberPlates() });
      setOut(o); await cadLoad();
      // Frame the frame now that it is there (the turn back started before it was made).
      setTurnBack(turnBack() + 1);
      const n = o.created.filter(id => o.m.members.some(b => b.id === id)).length, np = o.created.filter(id => o.m.shapes.some(s => s.id === id && s.plate)).length;
      const what = [n && `${n} ${memberTruss() ? (n === 1 ? "truss bar" : "truss bars") : (n === 1 ? "beam" : "beams")}`, np && `${np} ${np === 1 ? "plate" : "plates"}`].filter(Boolean).join(" and ");
      toast(`${what} made from the sketch, joined where they meet: give them supports and loads, then solve.${np ? " Plates start 0.2 m thick: change it in the Inspector." : ""}`, true);
    } catch (e) { toast(String(e)); }
    return;
  }
  // A closed shape: Extrude opens at once, with the solid made and its size to change (0.7.2).
  const done = cad()?.sketches.find(x => x.id === sk?.id);
  if (done && done.profiles > 0 && !(cad()?.bodies.length)) setTimeout(() => import("./extrude").then(k => k.openExtrude()), 450);
  else if (done && !done.curves.length) toast(`${done.name} is empty: draw a closed shape in it (Edit Sketch), or delete it.`);
};
export const cancelSketch = async () => { pickTool(null); if (await cadRun("sketch.cancel")) setTurnBack(turnBack() + 1); };

// ---------------------------------------------------------------- editing a sketch

/** What a click on the sheet picked: a point (an end or a centre) or a curve, and where. */
export type Pick = { kind: "point" | "curve"; id: string; at: P2; line?: boolean } | { kind: "spot"; id: ""; at: P2 };
type Need = "curve" | "line" | "any" | "spot";
/** An editing tool: what it asks to be clicked, in order, and the engine command it then runs
 * (with the curves selected beforehand, for the tools that work on a selection). */
type Edit = { label: string; icon: string; hint: string[]; picks: Need[]; sel?: boolean; run: (p: Pick[], sel: string[]) => [string, object] };
export const [consMode, setConsMode] = createSignal(false);
export const [filletR, setFilletR] = createSignal(5);
export const [chamferD, setChamferD] = createSignal(5);
export const [offsetD, setOffsetD] = createSignal(5);
export const [patN, setPatN] = createSignal(6);
export const [patA, setPatA] = createSignal(360);
export const [rowN, setRowN] = createSignal(3);
export const [rowS, setRowS] = createSignal(20);
export const [colN, setColN] = createSignal(1);
export const [colS, setColS] = createSignal(20);
const two = (name: string, cmd: string, hint: string): Edit => ({ label: name, icon: "c_" + name.toLowerCase(), hint: [hint, "Click the second one"], picks: ["any", "any"], run: p => [cmd, { a: p[0].id, b: p[1].id }] });
export const EDITS: Record<string, Edit> = {
  trim: { label: "Trim", icon: "cut", hint: ["Click the piece of a curve to cut away (up to where it crosses others)"], picks: ["curve"], run: p => ["sketch.trim", { curve: p[0].id, at: p[0].at }] },
  extend: { label: "Extend", icon: "repeat", hint: ["Click near the end of a line or arc to run it on to the next curve"], picks: ["curve"], run: p => ["sketch.extend", { curve: p[0].id, at: p[0].at }] },
  offset: { label: "Offset", icon: "offset", hint: ["Click a curve (its connected chain is offset)", "Click the side to offset it to"], picks: ["curve", "spot"], run: p => ["sketch.offset", { curves: [p[0].id], distance: offsetD(), side: p[1].at }] },
  fillet: { label: "Fillet", icon: "fillet", hint: ["Click the first of two curves meeting at a corner", "Click the second"], picks: ["curve", "curve"], run: p => ["sketch.fillet", { a: p[0].id, b: p[1].id, radius: filletR() }] },
  chamfer: { label: "Chamfer", icon: "chamfer", hint: ["Click the first of two lines meeting at a corner", "Click the second"], picks: ["curve", "curve"], run: p => ["sketch.chamfer.equal_distance", { a: p[0].id, b: p[1].id, distance: chamferD() }] },
  mirror: { label: "Mirror", icon: "symmetry", hint: ["Click the line to mirror the selected curves in"], picks: ["line"], sel: true, run: (p, sel) => ["sketch.mirror", { entities: sel, line: p[0].id }] },
  cpattern: { label: "Circular", icon: "cpattern", hint: ["Click the centre to copy the selected curves around"], picks: ["spot"], sel: true, run: (p, sel) => ["sketch.pattern.circular", { entities: sel, center: p[0].at, count: patN(), angle: patA() }] },
  coincident: { ...two("Coincident", "sketch.constraint.coincident", "Click a point"), hint: ["Click a point", "Click another point, or a curve it should lie on"] },
  tangent: two("Tangent", "sketch.constraint.tangent", "Click a curve"),
  parallel: two("Parallel", "sketch.constraint.parallel", "Click a line"),
  perpendicular: two("Perpendicular", "sketch.constraint.perpendicular", "Click a line"),
  equal: two("Equal", "sketch.constraint.equal", "Click a line, circle or arc"),
  concentric: two("Concentric", "sketch.constraint.concentric", "Click a circle or arc"),
  collinear: two("Collinear", "sketch.constraint.collinear", "Click a line"),
  midpoint: { label: "Midpoint", icon: "c_midpoint", hint: ["Click a point", "Click the line whose middle it goes to"], picks: ["any", "line"], run: p => ["sketch.constraint.midpoint", { a: p[0].id, b: p[1].id }] },
  symmetry: { label: "Symmetry", icon: "symmetry", hint: ["Click a point or curve", "Click its partner", "Click the line they are symmetric about"], picks: ["any", "any", "line"], run: p => ["sketch.constraint.symmetry", { a: p[0].id, b: p[1].id, line: p[2].id }] },
  hv: { label: "Horizontal / Vertical", icon: "c_hv", hint: ["Click a line: it turns level or upright, whichever it is nearer"], picks: ["line"], run: p => ["sketch.constraint.horizontal_vertical", { line: p[0].id }] },
  fix: { label: "Fix", icon: "pin", hint: ["Click a point or a curve to hold it where it is"], picks: ["any"], run: p => ["sketch.constraint.fix", { entity: p[0].id }] },
};
/** The constraints, in the order the Constrain ▾ lists them. */
export const CONSTRAINTS = ["coincident", "tangent", "parallel", "perpendicular", "equal", "concentric", "collinear", "midpoint", "symmetry", "hv", "fix"];
/** A constraint's mark on the sheet. */
const BADGE: Record<string, string> = { Coincident: "●", PointOnCurve: "◦", Horizontal: "H", Vertical: "V", HorizontalPoints: "H", VerticalPoints: "V", Parallel: "∥", Perpendicular: "⊥", Collinear: "⋯", Tangent: "T", Equal: "=", Concentric: "◎", Midpoint: "M", Symmetric: "⇋", Fix: "⚓", Smooth: "S" };
export const [sel, setSel] = createSignal<string[]>([]);
const [picks, setPicks] = createSignal<Pick[]>([]);
/** A dimension being typed: what it measures, where its box is, and the value shown. */
const [dimEdit, setDimEdit] = createSignal<{ entities: string[]; x: number; y: number; value: string } | null>(null);
export const pickEdit = (t: string | null) => { pickTool(t); setPicks([]); setDimEdit(null); };
/** Turns the selected curves into construction (or back). */
export const toggleConstruction = () => { const s = sel(); if (!s.length) return toast("Select curves first (Select, then click them)."); cadRun("sketch.construction", { curves: s }); };
export const deleteSelected = () => { const s = sel(); if (!s.length) return; cadRun("sketch.delete", { entities: s }).then(ok => ok && setSel([])); };
export const rectPattern = () => { const s = sel(); if (!s.length) return toast("Select the curves to copy first."); cadRun("sketch.pattern.rectangular", { entities: s, count: rowN(), spacing: rowS(), ...(colN() > 1 ? { count2: colN(), spacing2: colS() } : {}) }); };

/** The sketch sheet's scale and pan (px per mm, centre in mm): the 3D view's flat camera follows
 * it, so what is drawn on the sheet lies on the plane, over the model (0.7.2). */
export const [sheetView, setSheetView] = createSignal({ s: 4, cx: 0, cy: 0 });
/** Picking the plane to sketch on: the origin planes show, and a click on one (or on a flat face
 * of a solid) starts the sketch there (0.7.2). */
export const [planePick, setPlanePick] = createSignal(false);
/** A Finish Sketch just happened: the 3D view turns back to its usual view (0.7.2). */
export const [turnBack, setTurnBack] = createSignal(0);

// ---------------------------------------------------------------- the sketch canvas

/** The sheet square to the sketch's plane, over the 3D view: grid, the sketch's curves, the
 * tool's rubber band, and the point under the pointer (snapped to the grid and to the ends and
 * centres already drawn). */
export const SketchCanvas = () => {
  let svg!: SVGSVGElement;
  const [box, setBox] = createSignal({ w: 800, h: 600 });
  const [view, setView] = [sheetView, setSheetView];
  const [hover, setHover] = createSignal<P2 | null>(null);
  const sk = () => activeSketch();
  const toPx = (p: P2): P2 => { const v = view(), b = box(); return [b.w / 2 + (p[0] - v.cx) * v.s, b.h / 2 - (p[1] - v.cy) * v.s]; };
  const toMm = (x: number, y: number): P2 => { const v = view(), b = box(); return [v.cx + (x - b.w / 2) / v.s, v.cy - (y - b.h / 2) / v.s]; };
  const nice = (v: number) => { const p = 10 ** Math.floor(Math.log10(v)), k = v / p; return (k <= 1 ? 1 : k <= 2 ? 2 : k <= 5 ? 5 : 10) * p; };
  const step = () => nice(12 / view().s);
  /** Ends, middles of straight pieces and centres of what is drawn: the points a click snaps to. */
  const keys = (): P2[] => (sk()?.curves ?? []).flatMap(c => { const a = c.pts[0], b = c.pts[c.pts.length - 1]; return a && b ? [a, b] : []; });
  const snap = (x: number, y: number): P2 => {
    for (const k of keys()) { const q = toPx(k); if (Math.hypot(q[0] - x, q[1] - y) < 8) return k; }
    const g = step(), p = toMm(x, y); return [Math.round(p[0] / g) * g, Math.round(p[1] / g) * g];
  };
  onMount(() => {
    const measure = () => { const r = svg.getBoundingClientRect(); setBox({ w: r.width || 800, h: r.height || 600 }); };
    const ro = new ResizeObserver(measure); measure();
    ro.observe(svg); onCleanup(() => ro.disconnect());
    // A new sketch opens about 200 mm across, or fitted to what it already has.
    const pts = (sk()?.curves ?? []).flatMap(c => c.pts);
    if (pts.length) { const xs = pts.map(p => p[0]), ys = pts.map(p => p[1]); const w = Math.max(...xs) - Math.min(...xs), h = Math.max(...ys) - Math.min(...ys); setView({ s: Math.min(8, 0.6 * Math.min(box().w / (w || 1), box().h / (h || 1))), cx: (Math.max(...xs) + Math.min(...xs)) / 2, cy: (Math.max(...ys) + Math.min(...ys)) / 2 }); }
    else setView({ s: Math.max(1, box().w / 220), cx: 0, cy: 0 });
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.tagName === "INPUT") return;
      if (e.key === "Enter" && cadTool() === "spline") { endSpline(); return; }
      if (e.key === "Escape") { if (dimEdit()) setDimEdit(null); else if (draft().length || picks().length) { setDraft([]); setPicks([]); } else if (sel().length) setSel([]); else pickTool(null); }
      if ((e.key === "Delete" || e.key === "Backspace") && sel().length) deleteSelected();
    };
    window.addEventListener("keydown", key); onCleanup(() => window.removeEventListener("keydown", key));
  });
  /** The point or curve under (x, y) px: points first (within 8 px), then curves (within 6 px). */
  const pickAt = (x: number, y: number): Pick => {
    const s = sk(), at = toMm(x, y);
    for (const q of s?.points ?? []) { const r = toPx(q.at); if (Math.hypot(r[0] - x, r[1] - y) < 8) return { kind: "point", id: q.id, at: q.at }; }
    let best: { c: CadCurve; d: number } | null = null;
    for (const c of s?.curves ?? []) for (let i = 0; i + 1 < c.pts.length; i++) {
      const a = toPx(c.pts[i]), b = toPx(c.pts[i + 1]), ab = [b[0] - a[0], b[1] - a[1]], L = ab[0] ** 2 + ab[1] ** 2 || 1;
      const t = Math.max(0, Math.min(1, ((x - a[0]) * ab[0] + (y - a[1]) * ab[1]) / L)), dd = Math.hypot(a[0] + t * ab[0] - x, a[1] + t * ab[1] - y);
      if (dd < 6 && (!best || dd < best.d)) best = { c, d: dd };
    }
    return best ? { kind: "curve", id: best.c.id, at, line: best.c.kind === "line" } : { kind: "spot", id: "", at };
  };
  const fits = (n: Need, p: Pick) => n === "spot" ? true : n === "any" ? p.kind !== "spot" : n === "line" ? p.kind === "curve" && !!p.line : p.kind === "curve";
  const edit = async (x: number, y: number, e: PointerEvent) => {
    const t = cadTool()!, p = pickAt(x, y);
    if (t === "select") {
      if (p.kind === "spot") { setSel([]); return; }
      const on = sel().includes(p.id);
      setSel(e.shiftKey || e.ctrlKey ? (on ? sel().filter(i => i !== p.id) : [...sel(), p.id]) : on ? [] : [p.id]);
      return;
    }
    if (t === "dimension") {
      if (p.kind === "spot") return;
      const de = dimEdit();
      const ents = de ? [...de.entities, p.id].slice(0, 2) : [p.id];
      const c = sk()?.curves.find(k => k.id === p.id);
      const size = ents.length === 1 && c ? (c.kind === "line" ? d(c.pts[0], c.pts[c.pts.length - 1]) : c.kind === "circle" ? 2 * d(c.pts[0], (sk()?.points.find(q => q.id === c.center)?.at ?? c.pts[0])) : c.kind === "arc" ? d(c.pts[0], (sk()?.points.find(q => q.id === c.center)?.at ?? c.pts[0])) : null) : null;
      setDimEdit({ entities: ents, x, y, value: size === null ? "" : String(+size.toPrecision(6)) });
      return;
    }
    const ed = EDITS[t]; if (!ed) return;
    if (ed.sel && !sel().length) return toast("Select the curves first (Select, then click them; Shift adds), then pick " + ed.label + ".");
    const need = ed.picks[picks().length];
    if (!fits(need, p)) return toast(ed.hint[picks().length]);
    const ps = [...picks(), p];
    if (ps.length < ed.picks.length) { setPicks(ps); return; }
    setPicks([]);
    const [cmd, params] = ed.run(ps, sel());
    if (await cadRun(cmd, params) && ed.sel) setSel([]);
  };
  const applyDim = async () => {
    const de = dimEdit(); if (!de) return;
    const v = de.value.trim();
    if (await cadRun("sketch.dimension", v ? { entities: de.entities, value: isNaN(+v) ? v : +v } : { entities: de.entities })) setDimEdit(null);
  };
  const place = async (p: P2) => {
    const t = cadTool(); if (!t) return;
    const dr = [...draft(), p];
    if (dr.length >= 2 && d(dr[dr.length - 2], p) < 1e-9) return;
    if (dr.length < NEED[t]) { setDraft(dr); return; }
    const c = TOOLS[t].cmd(dr); if (!c) return;
    const ok = await cadRun(c[0], consMode() ? { ...c[1], construction: true } : c[1]);
    // A line goes on from where it ended; every other shape starts again.
    setDraft(ok && t === "line" ? [p] : []);
  };
  /** A spline's last point is in: draw it through all of them. */
  const endSpline = async () => { const dr = draft(); if (dr.length >= 2 && await cadRun("sketch.spline.fit_point", { points: dr, ...(consMode() ? { construction: true } : {}) })) setDraft([]); };
  let pan: { x: number; y: number; cx: number; cy: number } | null = null;
  const onDown = (e: PointerEvent) => {
    if (e.button === 1 || e.button === 2) { pan = { x: e.clientX, y: e.clientY, cx: view().cx, cy: view().cy }; svg.setPointerCapture(e.pointerId); return; }
    if (e.button === 0) {
      const r = svg.getBoundingClientRect(), x = e.clientX - r.left, y = e.clientY - r.top, t = cadTool();
      if (!t) { setCadTool("select"); edit(x, y, e); } else if (t === "select" || t === "dimension" || EDITS[t]) edit(x, y, e); else place(snap(x, y));
    }
  };
  const onMove = (e: PointerEvent) => {
    const r = svg.getBoundingClientRect();
    if (pan) { const v = view(); setView({ ...v, cx: pan.cx - (e.clientX - pan.x) / v.s, cy: pan.cy + (e.clientY - pan.y) / v.s }); return; }
    setHover(snap(e.clientX - r.left, e.clientY - r.top));
  };
  const onWheel = (e: WheelEvent) => {
    e.preventDefault(); const r = svg.getBoundingClientRect(), x = e.clientX - r.left, y = e.clientY - r.top, before = toMm(x, y);
    const v = view(), s = Math.min(400, Math.max(0.02, v.s * (e.deltaY < 0 ? 1.2 : 1 / 1.2))), b = box();
    setView({ s, cx: before[0] - (x - b.w / 2) / s, cy: before[1] + (y - b.h / 2) / s });
  };
  const poly = (pts: P2[]) => pts.map(p => toPx(p).map(v => v.toFixed(1)).join(",")).join(" ");
  /** The tool's rubber band: the shape it would draw with the pointer as its next point. */
  const band = (): P2[][] => {
    const t = cadTool(), h = hover(); if (!t || !h || !draft().length) return [];
    const p = [...draft(), h], a = p[0], b = p[1];
    const circ = (o: P2, r: number, n = 64, a0 = 0, sw = 360): P2[] => Array.from({ length: n + 1 }, (_, i) => { const t2 = (a0 + sw * i / n) * Math.PI / 180; return [o[0] + r * Math.cos(t2), o[1] + r * Math.sin(t2)]; });
    switch (t) {
      case "circle": return [circ(a, d(a, b))];
      case "circle2": return [circ([(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], d(a, b) / 2)];
      case "rect": case "rectr": return [[a, [b[0], a[1]], b, [a[0], b[1]], a]];
      case "ellipse": { const L = d(a, b) || 1, u = [(b[0] - a[0]) / L, (b[1] - a[1]) / L], r2 = p.length > 2 ? minorR(a, b, p[2]) : L / 2; return [Array.from({ length: 65 }, (_, i) => { const t2 = 2 * Math.PI * i / 64; return [a[0] + u[0] * L * Math.cos(t2) - u[1] * r2 * Math.sin(t2), a[1] + u[1] * L * Math.cos(t2) + u[0] * r2 * Math.sin(t2)] as P2; })]; }
      case "rectc": { const q: P2 = [2 * a[0] - b[0], 2 * a[1] - b[1]]; return [[q, [b[0], q[1]], b, [q[0], b[1]], q]]; }
      case "polyin": case "polyout": { const n = sides(), r = d(a, b) / (t === "polyout" ? Math.cos(Math.PI / n) : 1), a0 = ang(a, b) + (t === "polyout" ? 180 / n : 0); return [circ(a, r, n, a0)]; }
      case "polyedge": { const n = sides(), L = d(a, b), R = L / (2 * Math.sin(Math.PI / n)), mid: P2 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], h2 = L / (2 * Math.tan(Math.PI / n)), u = [(b[0] - a[0]) / L, (b[1] - a[1]) / L], o: P2 = [mid[0] - u[1] * h2, mid[1] + u[0] * h2]; return [circ(o, R, n, ang(o, a))]; }
      case "slotc": case "slot": { const L = d(a, b) || 1, u = [(b[0] - a[0]) / L, (b[1] - a[1]) / L], w = slotW() / 2, n: P2 = [-u[1] * w, u[0] * w]; const [p0, p1] = t === "slot" ? [[a[0] + u[0] * w, a[1] + u[1] * w] as P2, [b[0] - u[0] * w, b[1] - u[1] * w] as P2] : [a, b]; return [[[p0[0] + n[0], p0[1] + n[1]], [p1[0] + n[0], p1[1] + n[1]]], [[p0[0] - n[0], p0[1] - n[1]], [p1[0] - n[0], p1[1] - n[1]]], circ(p0, w, 24, ang(p0, p1) + 90, 180), circ(p1, w, 24, ang(p0, p1) - 90, 180)]; }
      default: return [p];
    }
  };
  const gridLines = () => {
    // Big squares with small squares inside (0.7.2): 1 → 5, 2 → 10, 5 → 50, as on the 3D floor.
    const g = step(), mant = Math.round(g / 10 ** Math.floor(Math.log10(g) + 1e-9)), G = g * (mant === 5 ? 10 : 5);
    const isMaj = (v: number) => Math.abs(v / G - Math.round(v / G)) < 1e-6;
    const b = box(), lo = toMm(0, b.h), hi = toMm(b.w, 0), out: { x1: number; y1: number; x2: number; y2: number; major: boolean }[] = [];
    for (let i = Math.ceil(lo[0] / g); i * g <= hi[0]; i++) { const x = i * g, px = toPx([x, 0])[0]; out.push({ x1: px, y1: 0, x2: px, y2: b.h, major: isMaj(x) }); }
    for (let i = Math.ceil(lo[1] / g); i * g <= hi[1]; i++) { const y = i * g, py = toPx([0, y])[1]; out.push({ x1: 0, y1: py, x2: b.w, y2: py, major: isMaj(y) }); }
    return out;
  };
  const hint = () => {
    const t = cadTool();
    if (!t || t === "select") return sel().length ? `${sel().length} selected: Delete removes them; Construction, Mirror and the patterns use them` : "Click curves to select them (Shift adds), or pick a tool on the ribbon";
    if (t === "dimension") return dimEdit() ? "Type the size and press Enter (or click a second item to measure between the two)" : "Click a line, circle or arc to size it, or a point and then another item";
    if (EDITS[t]) return EDITS[t].hint[Math.min(picks().length, EDITS[t].hint.length - 1)];
    return TOOLS[t].hint[Math.min(draft().length, TOOLS[t].hint.length - 1)];
  };
  const label = (m: CadMark) => m.value === null ? BADGE[m.name] ?? m.name.slice(0, 1) : m.name === "Diameter" ? `Ø${+m.value.toPrecision(5)}` : m.name === "Radius" ? `R${+m.value.toPrecision(5)}` : m.name.includes("Angle") || m.name.includes("Sweep") ? `${+m.value.toPrecision(4)}°` : `${+m.value.toPrecision(5)}`;
  const picked = (id: string) => sel().includes(id) || picks().some(p => p.id === id) || (dimEdit()?.entities.includes(id) ?? false);
  return (
    <div class="cadsheet">
      <svg ref={svg} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={() => { pan = null; }} onWheel={onWheel} onContextMenu={e => e.preventDefault()}
        onDblClick={() => { if (cadTool() === "line") setDraft([]); if (cadTool() === "spline") endSpline(); }}>
        <For each={gridLines()}>{l => <line x1={l.x1} y1={l.y1} x2={l.x2} y2={l.y2} class={l.major ? "gmaj" : "gmin"} />}</For>
        <line class="axx" x1={0} y1={toPx([0, 0])[1]} x2={box().w} y2={toPx([0, 0])[1]} />
        <line class="axy" x1={toPx([0, 0])[0]} y1={0} x2={toPx([0, 0])[0]} y2={box().h} />
        <For each={sk()?.curves ?? []}>{c => <polyline class="cv" classList={{ cons: c.construction, free: !c.fixed, on: picked(c.id) }} points={poly(c.pts)} />}</For>
        <For each={(sk()?.points ?? []).filter(q => picked(q.id) || cadTool() === "dimension" || EDITS[cadTool() ?? ""])}>{q => <circle class="spt" classList={{ on: picked(q.id) }} cx={toPx(q.at)[0]} cy={toPx(q.at)[1]} r={3} />}</For>
        <For each={sk()?.marks ?? []}>{m => <g class="mark" classList={{ dim: m.value !== null }}><text x={toPx(m.at)[0] + (m.value === null ? 6 : 0)} y={toPx(m.at)[1] - 6}>{label(m)}</text></g>}</For>
        <For each={band()}>{b => <polyline class="band" points={poly(b)} />}</For>
        <For each={draft()}>{p => <circle class="dpt" cx={toPx(p)[0]} cy={toPx(p)[1]} r={3.5} />}</For>
        <Show when={hover()}>{h => <circle class="hpt" cx={toPx(h())[0]} cy={toPx(h())[1]} r={4} />}</Show>
      </svg>
      <Show when={dimEdit()}>{de => <div class="dimbox" style={{ left: `${de().x + 10}px`, top: `${de().y - 34}px` }}>
        <input class="mono" value={de().value} ref={el => setTimeout(() => { el.focus(); el.select(); })} onInput={e => setDimEdit({ ...de(), value: e.currentTarget.value })}
          onKeyDown={e => { if (e.key === "Enter") applyDim(); if (e.key === "Escape") setDimEdit(null); }} placeholder="size (mm), or W / 2" /><span>mm</span>
        <button class="btn sm pri" onClick={applyDim}>OK</button></div>}</Show>
      <div class="cadhint"><b>{sk()?.name ?? "Sketch"}</b><span>{hint()}</span>
        <Show when={hover()}>{h => <span class="mono">{fmt("coord", h()[0] * 1e-3)}, {fmt("coord", h()[1] * 1e-3)}</span>}</Show>
        <span class="muted">{sk() ? `${sk()!.profiles} closed ${sk()!.profiles === 1 ? "region" : "regions"} · ${sk()!.status.replace("Solved, ", "").replace("Solved", "fully defined")}` : ""}</span></div>
    </div>
  );
};

// ---------------------------------------------------------------- solids from a sketch

export const [extrudeD, setExtrudeD] = createSignal(20);
export const [extrudeDir, setExtrudeDir] = createSignal<"positive" | "negative" | "symmetric">("positive");
export const [revolveA, setRevolveA] = createSignal(360);
export const [revolveAxis, setRevolveAxis] = createSignal<"x" | "y">("y");
/** The last sketch: what Extrude and Revolve use. */
const lastSketch = () => { const s = cad()?.sketches ?? []; return s[s.length - 1]; };
export const extrude = () => {
  const s = lastSketch(); if (!s) return toast("Draw a sketch first: New Sketch, then a closed shape.");
  if (!s.profiles) return toast(`${s.name} has no closed region to extrude: close the shape.`);
  return cadRun("solid.extrude", { distance: extrudeD(), direction: extrudeDir(), sketch: s.id });
};
export const revolve = () => {
  const s = lastSketch(); if (!s) return toast("Draw a sketch first: New Sketch, then a closed shape beside an axis.");
  if (!s.profiles) return toast(`${s.name} has no closed region to revolve: close the shape.`);
  return cadRun("solid.revolve", { axis: revolveAxis(), angle: revolveA(), sketch: s.id });
};

// ---------------------------------------------------------------- 3D tools (picked in the 3D view)

/** What a click in the 3D view found on the solids (engine mm, Z up). */
export type CadHit = { face: { body: string; index: number; at: [number, number, number] } | null; edge: { body: string; at: [number, number, number] } | null };
export const [cad3Tool, setCad3Tool] = createSignal<string | null>(null);
export const [holeD3, setHoleD3] = createSignal(10);
export const [holeDepth, setHoleDepth] = createSignal(0);
export const [fillet3R, setFillet3R] = createSignal(3);
export const [chamfer3D, setChamfer3D] = createSignal(2);
export const [shellT, setShellT] = createSignal(2);
/** The draft angle of the next Draft (°, 0.7.6), measured from the floor's normal. */
export const [draftA, setDraftA] = createSignal(3);
export const [pressD, setPressD] = createSignal(5);
/** A 3D tool: what it asks for and the engine command a click makes. */
export const TOOLS3: Record<string, { label: string; icon: string; hint: string; needs: "face" | "edge"; run: (h: CadHit) => [string, object] | null }> = {
  hole3: { label: "Hole", icon: "hcircle", hint: "Click a face where the hole goes (it is drilled square into it)", needs: "face", run: h => ["solid.hole", { position: h.face!.at, diameter: holeD3(), ...(holeDepth() > 0 ? { depth: holeDepth() } : {}) }] },
  fillet3: { label: "Fillet", icon: "fillet", hint: "Click an edge to round it (its smooth chain goes with it)", needs: "edge", run: h => ["solid.fillet", { edges: [h.edge!.at], radius: fillet3R() }] },
  chamfer3: { label: "Chamfer", icon: "chamfer", hint: "Click an edge to bevel it", needs: "edge", run: h => ["solid.chamfer", { edges: [h.edge!.at], distance: chamfer3D() }] },
  shell3: { label: "Shell", icon: "solid", hint: "Click the face to open: the solid is hollowed out to the wall thickness", needs: "face", run: h => ["solid.shell", { faces: [h.face!.at], thickness: shellT(), body: h.face!.body }] },
  press3: { label: "Press Pull", icon: "up", hint: "Click a face to push it out (or in, with a negative distance)", needs: "face", run: h => ["solid.press_pull", { face: h.face!.at, distance: pressD() }] },
  draft3: { label: "Draft", icon: "chamfer", hint: "Click a side face to slope it by the draft angle (from the floor plane up), as a moulded or cast part needs", needs: "face", run: h => ["solid.draft", { faces: [h.face!.at], angle: draftA(), neutral: "XY", body: h.face!.body }] },
  measure3: { label: "Measure", icon: "dist", hint: "Click a face or an edge to measure it; click a second for the distance and angle between them", needs: "face", run: () => null },
  sketchface: { label: "Sketch on a face", icon: "new", hint: "Click a flat face of a solid to sketch on it", needs: "face", run: h => ["sketch.create", { plane: { face: h.face!.at, body: h.face!.body, index: h.face!.index } }] },
};
export const pick3 = (t: string | null) => { setCad3Tool(t); pickTool(null); };
export const cad3Click = async (h: CadHit, tool?: string) => {
  const t = tool ?? cad3Tool(); if (!t) return;
  const T = TOOLS3[t];
  if (t === "measure3") return import("./solidtools").then(k => k.measureClick(h));
  if ((T.needs === "face" && !h.face) || (T.needs === "edge" && !h.edge)) return toast(T.hint);
  const c = T.run(h); if (!c) return;
  if (await cadRun(c[0], c[1]) && t === "sketchface") { setCad3Tool(null); setTab("geometry"); pickTool(ways().Line); }
};
/** The solids' names: what Move, Mirror and Pattern act on. */
export const bodyNames = () => (cad()?.bodies ?? []).map(b => b.name);
