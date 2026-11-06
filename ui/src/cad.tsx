// The geometry engine in the window (0.7.1): sketches on a plane and solids made from them, by
// the engine behind crates/cad (the shell's cad_run). This file keeps what the engine last
// returned (the scene), the drawing tools with their ways of drawing, and the sketch canvas: a
// sheet over the 3D view, square to the sketch's plane, where clicks place the points.
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { invoke } from "./api";
import { toast } from "./store";
import { fmt } from "./units";

export type CadBody = { name: string; pos: number[]; nrm: number[]; tris: number[]; face: number[]; edges: number[][]; closed: boolean };
export type CadCurve = { id: string; pts: [number, number][]; construction: boolean };
export type CadSketch = { id: number; name: string; origin: number[]; x: number[]; y: number[]; curves: CadCurve[]; profiles: number; status: string };
export type CadScene = { bodies: CadBody[]; sketches: CadSketch[]; active: number | null; undo: boolean; redo: boolean };

export const [cad, setCad] = createSignal<CadScene | null>(null);
/** Runs one engine command; the scene comes back. False (and a message) when it is refused. */
export const cadRun = async (id: string, params: object = {}) => {
  try { setCad(await invoke<CadScene>("cad_run", { id, params })); return true; } catch (e) { toast(String(e)); return false; }
};
export const cadLoad = async () => { try { setCad(await invoke<CadScene>("cad_scene")); } catch { /* no engine yet */ } };
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
export const TOOLS: Record<string, Tool> = {
  line: { label: "Line", icon: "beam", hint: ["Click where the line starts", "Click the next point (double-click or Esc to end the chain)"], cmd: p => ["sketch.line", { points: p }] },
  arc3: { label: "Arc · 3 points", icon: "curve", hint: ["Click where the arc starts", "Click where it ends", "Click a point it passes through"], cmd: p => ["sketch.arc.three_point", { start: p[0], end: p[1], through: p[2] }] },
  arcc: { label: "Arc · centre and ends", icon: "curve", hint: ["Click the centre", "Click where the arc starts", "Click where it ends (anticlockwise)"], cmd: p => ["sketch.arc.center_point", { center: p[0], start: p[1], end: p[2] }] },
  circle: { label: "Circle · centre and radius", icon: "circle", hint: ["Click the centre", "Click a point on the circle"], cmd: p => ["sketch.circle.center", { center: p[0], radius: d(p[0], p[1]) }] },
  circle2: { label: "Circle · 2 points", icon: "circle", hint: ["Click one end of a diameter", "Click the other end"], cmd: p => ["sketch.circle.two_point", { p0: p[0], p1: p[1] }] },
  circle3: { label: "Circle · 3 points", icon: "circle", hint: ["Click a point on the circle", "Click a second point", "Click a third point"], cmd: p => ["sketch.circle.three_point", { p0: p[0], p1: p[1], p2: p[2] }] },
  rect: { label: "Rectangle · 2 corners", icon: "rect", hint: ["Click one corner", "Click the opposite corner"], cmd: p => ["sketch.rectangle.two_point", { p0: p[0], p1: p[1] }] },
  rectc: { label: "Rectangle · centre and corner", icon: "rect", hint: ["Click the centre", "Click a corner"], cmd: p => ["sketch.rectangle.center", { center: p[0], corner: p[1] }] },
  rect3: { label: "Rectangle · 3 points", icon: "rect", hint: ["Click where the first side starts", "Click where it ends", "Click a point on the opposite side"], cmd: p => ["sketch.rectangle.three_point", { p0: p[0], p1: p[1], p2: p[2] }] },
  polyin: { label: "Polygon · inside a circle", icon: "ngon", hint: ["Click the centre", "Click a corner"], cmd: p => ["sketch.polygon.inscribed", { center: p[0], radius: d(p[0], p[1]), sides: sides(), angle: ang(p[0], p[1]) }] },
  polyout: { label: "Polygon · around a circle", icon: "ngon", hint: ["Click the centre", "Click the middle of a side"], cmd: p => ["sketch.polygon.circumscribed", { center: p[0], radius: d(p[0], p[1]), sides: sides(), angle: ang(p[0], p[1]) }] },
  polyedge: { label: "Polygon · by one side", icon: "ngon", hint: ["Click where a side starts", "Click where it ends"], cmd: p => ["sketch.polygon.edge", { p0: p[0], p1: p[1], sides: sides() }] },
  slotc: { label: "Slot · centre to centre", icon: "slot", hint: ["Click the centre of one end", "Click the centre of the other end"], cmd: p => ["sketch.slot.center_to_center", { p0: p[0], p1: p[1], width: slotW() }] },
  slot: { label: "Slot · overall", icon: "slot", hint: ["Click one end", "Click the other end"], cmd: p => ["sketch.slot.overall", { p0: p[0], p1: p[1], width: slotW() }] },
};
const NEED: Record<string, number> = { line: 2, arc3: 3, arcc: 3, circle: 2, circle2: 2, circle3: 3, rect: 2, rectc: 2, rect3: 3, polyin: 2, polyout: 2, polyedge: 2, slotc: 2, slot: 2 };
/** Each shape's ways of drawing, in the order its ▾ lists them. */
export const FAMILIES: [string, string, string[]][] = [
  ["Line", "beam", ["line"]], ["Arc", "curve", ["arc3", "arcc"]], ["Circle", "circle", ["circle", "circle2", "circle3"]],
  ["Rectangle", "rect", ["rect", "rectc", "rect3"]], ["Polygon", "ngon", ["polyin", "polyout", "polyedge"]], ["Slot", "slot", ["slotc", "slot"]],
];
export const [cadTool, setCadTool] = createSignal<string | null>(null);
/** The way last used for each shape (what its button draws). */
export const [ways, setWays] = createSignal<Record<string, string>>({ Line: "line", Arc: "arc3", Circle: "circle", Rectangle: "rect", Polygon: "polyin", Slot: "slotc" });
const [draft, setDraft] = createSignal<P2[]>([]);
export const pickTool = (t: string | null) => { setCadTool(t); setDraft([]); };

/** Starts a sketch on one of the origin planes (the floor XY, front XZ, side YZ). */
export const newSketch = async (plane: "XY" | "XZ" | "YZ") => { if (await cadRun("sketch.create", { plane })) pickTool(ways().Line); };
export const finishSketch = async () => { pickTool(null); await cadRun("sketch.finish"); };
export const cancelSketch = async () => { pickTool(null); await cadRun("sketch.cancel"); };

// ---------------------------------------------------------------- the sketch canvas

/** The sheet square to the sketch's plane, over the 3D view: grid, the sketch's curves, the
 * tool's rubber band, and the point under the pointer (snapped to the grid and to the ends and
 * centres already drawn). */
export const SketchCanvas = () => {
  let svg!: SVGSVGElement;
  const [box, setBox] = createSignal({ w: 800, h: 600 });
  const [view, setView] = createSignal({ s: 4, cx: 0, cy: 0 }); // px per mm, centre in mm
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
    const ro = new ResizeObserver(() => setBox({ w: svg.clientWidth || 800, h: svg.clientHeight || 600 }));
    ro.observe(svg); onCleanup(() => ro.disconnect());
    // A new sketch opens about 200 mm across, or fitted to what it already has.
    const pts = (sk()?.curves ?? []).flatMap(c => c.pts);
    if (pts.length) { const xs = pts.map(p => p[0]), ys = pts.map(p => p[1]); const w = Math.max(...xs) - Math.min(...xs), h = Math.max(...ys) - Math.min(...ys); setView({ s: Math.min(8, 0.6 * Math.min((svg.clientWidth || 800) / (w || 1), (svg.clientHeight || 600) / (h || 1))), cx: (Math.max(...xs) + Math.min(...xs)) / 2, cy: (Math.max(...ys) + Math.min(...ys)) / 2 }); }
    else setView({ s: Math.max(1, (svg.clientWidth || 800) / 220), cx: 0, cy: 0 });
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { if (draft().length) setDraft([]); else pickTool(null); } };
    window.addEventListener("keydown", key); onCleanup(() => window.removeEventListener("keydown", key));
  });
  const place = async (p: P2) => {
    const t = cadTool(); if (!t) return;
    const dr = [...draft(), p];
    if (dr.length >= 2 && d(dr[dr.length - 2], p) < 1e-9) return;
    if (dr.length < NEED[t]) { setDraft(dr); return; }
    const c = TOOLS[t].cmd(dr); if (!c) return;
    const ok = await cadRun(c[0], c[1]);
    // A line goes on from where it ended; every other shape starts again.
    setDraft(ok && t === "line" ? [p] : []);
  };
  let pan: { x: number; y: number; cx: number; cy: number } | null = null;
  const onDown = (e: PointerEvent) => {
    if (e.button === 1 || e.button === 2) { pan = { x: e.clientX, y: e.clientY, cx: view().cx, cy: view().cy }; svg.setPointerCapture(e.pointerId); return; }
    if (e.button === 0) { const r = svg.getBoundingClientRect(); place(snap(e.clientX - r.left, e.clientY - r.top)); }
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
      case "rect": return [[a, [b[0], a[1]], b, [a[0], b[1]], a]];
      case "rectc": { const q: P2 = [2 * a[0] - b[0], 2 * a[1] - b[1]]; return [[q, [b[0], q[1]], b, [q[0], b[1]], q]]; }
      case "polyin": case "polyout": { const n = sides(), r = d(a, b) / (t === "polyout" ? Math.cos(Math.PI / n) : 1), a0 = ang(a, b) + (t === "polyout" ? 180 / n : 0); return [circ(a, r, n, a0)]; }
      case "polyedge": { const n = sides(), L = d(a, b), R = L / (2 * Math.sin(Math.PI / n)), mid: P2 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2], h2 = L / (2 * Math.tan(Math.PI / n)), u = [(b[0] - a[0]) / L, (b[1] - a[1]) / L], o: P2 = [mid[0] - u[1] * h2, mid[1] + u[0] * h2]; return [circ(o, R, n, ang(o, a))]; }
      case "slotc": case "slot": { const L = d(a, b) || 1, u = [(b[0] - a[0]) / L, (b[1] - a[1]) / L], w = slotW() / 2, n: P2 = [-u[1] * w, u[0] * w]; const [p0, p1] = t === "slot" ? [[a[0] + u[0] * w, a[1] + u[1] * w] as P2, [b[0] - u[0] * w, b[1] - u[1] * w] as P2] : [a, b]; return [[[p0[0] + n[0], p0[1] + n[1]], [p1[0] + n[0], p1[1] + n[1]]], [[p0[0] - n[0], p0[1] - n[1]], [p1[0] - n[0], p1[1] - n[1]]], circ(p0, w, 24, ang(p0, p1) + 90, 180), circ(p1, w, 24, ang(p0, p1) - 90, 180)]; }
      default: return [p];
    }
  };
  const gridLines = () => {
    const g = step(), b = box(), lo = toMm(0, b.h), hi = toMm(b.w, 0), out: { x1: number; y1: number; x2: number; y2: number; major: boolean }[] = [];
    for (let x = Math.ceil(lo[0] / g) * g; x <= hi[0]; x += g) { const px = toPx([x, 0])[0]; out.push({ x1: px, y1: 0, x2: px, y2: b.h, major: Math.abs(x / (g * 5) - Math.round(x / (g * 5))) < 1e-6 }); }
    for (let y = Math.ceil(lo[1] / g) * g; y <= hi[1]; y += g) { const py = toPx([0, y])[1]; out.push({ x1: 0, y1: py, x2: b.w, y2: py, major: Math.abs(y / (g * 5) - Math.round(y / (g * 5))) < 1e-6 }); }
    return out;
  };
  const hint = () => { const t = cadTool(); return t ? TOOLS[t].hint[Math.min(draft().length, TOOLS[t].hint.length - 1)] : "Pick a drawing tool on the ribbon"; };
  return (
    <div class="cadsheet">
      <svg ref={svg} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={() => { pan = null; }} onWheel={onWheel} onContextMenu={e => e.preventDefault()}
        onDblClick={() => { if (cadTool() === "line") setDraft([]); }}>
        <For each={gridLines()}>{l => <line x1={l.x1} y1={l.y1} x2={l.x2} y2={l.y2} class={l.major ? "gmaj" : "gmin"} />}</For>
        <line class="axx" x1={0} y1={toPx([0, 0])[1]} x2={box().w} y2={toPx([0, 0])[1]} />
        <line class="axy" x1={toPx([0, 0])[0]} y1={0} x2={toPx([0, 0])[0]} y2={box().h} />
        <For each={sk()?.curves ?? []}>{c => <polyline class="cv" classList={{ cons: c.construction }} points={poly(c.pts)} />}</For>
        <For each={band()}>{b => <polyline class="band" points={poly(b)} />}</For>
        <For each={draft()}>{p => <circle class="dpt" cx={toPx(p)[0]} cy={toPx(p)[1]} r={3.5} />}</For>
        <Show when={hover()}>{h => <circle class="hpt" cx={toPx(h())[0]} cy={toPx(h())[1]} r={4} />}</Show>
      </svg>
      <div class="cadhint"><b>{sk()?.name ?? "Sketch"}</b><span>{hint()}</span>
        <Show when={hover()}>{h => <span class="mono">{fmt("coord", h()[0] * 1e-3)}, {fmt("coord", h()[1] * 1e-3)}</span>}</Show>
        <span class="muted">{sk() ? `${sk()!.profiles} closed ${sk()!.profiles === 1 ? "region" : "regions"}` : ""}</span></div>
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
