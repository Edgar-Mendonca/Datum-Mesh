// The drawing canvas: frames (nodes and members) and parts (shapes with arcs and holes), with their
// supports and loads. Pan, zoom, grid with snap, rulers, select and move, draw, and place supports
// and loads. World coordinates are metres with y up; the screen has y down. The engine owns the
// model: the canvas only draws it and sends edits.
import { For, Show, JSX, createSignal, createMemo, onMount, onCleanup, batch } from "solid-js";
import { Pt, Geom, Model, Target, Support, Load, Shape, Member } from "./api";
import { model, sel, setSel, edgeSel, setEdgeSel, tool, setTool, act, opts, setCursor, openMenu, Item, toast, partRes, view, meshed } from "./store";
import { C } from "./cmds";
import { svgPath, edgePts, edgeCount, distTo, inside, outline, keyPts, orient, area } from "./geom";
import { fmt } from "./units";
import { defaultLoad, defaultSupport } from "./panels";
import { ResultsLayer, ResultsProbe } from "./results";
import { PartLayer, PartMarkers, PartProbe } from "./parts";

export const [scale, setScale] = createSignal(60); // px per metre
export const [pan, setPan] = createSignal<Pt>([80, 500]); // screen position of the world origin
let box: HTMLDivElement | undefined;
const RULER = 22;

export const toS = (p: Pt): Pt => [pan()[0] + p[0] * scale(), pan()[1] - p[1] * scale()];
const toW = (cx: number, cy: number): Pt => { const r = box!.getBoundingClientRect(); return [(cx - r.left - pan()[0]) / scale(), -(cy - r.top - pan()[1]) / scale()]; };
const grid = () => model()?.grid ?? 0.5;
const snapV = (v: number) => (opts().snap ? Math.round(v / grid()) * grid() : v);

export const fit = () => {
  const m = model(); if (!m || !box) return;
  const r = box.getBoundingClientRect();
  const b = boundsOf(m) ?? (m.analysis === "frame" ? [0, 0, 12, 6] : [0, 0, 0.2, 0.1]);
  const w = Math.max(b[2] - b[0], 1e-6), h = Math.max(b[3] - b[1], 1e-6);
  const s = Math.min((r.width - RULER - 120) / w, (r.height - RULER - 120) / h);
  batch(() => { setScale(s); setPan([RULER + (r.width - RULER - w * s) / 2 - b[0] * s, RULER + (r.height - RULER + h * s) / 2 + b[1] * s]); });
};
export const zoomBy = (f: number, at?: Pt) => {
  const r = box?.getBoundingClientRect(); if (!r) return;
  const [cx, cy] = at ?? [r.width / 2, r.height / 2];
  const s0 = scale(), s = Math.min(1e7, Math.max(1e-2, s0 * f));
  batch(() => { setPan([cx - (cx - pan()[0]) * s / s0, cy - (cy - pan()[1]) * s / s0]); setScale(s); });
};
export const zoomPct = () => scale();

/** The automatic element size, as the engine picks it (`mesh::auto_size`): a twenty-fifth of the
 * part's larger side, and no larger than gives about 800 triangles over its area. */
const autoSize = (m: Model) => {
  const b = boundsOf(m); if (!b) return 0.01;
  const a = Math.max(0, m.shapes.reduce((s, x) => s + (x.hole ? -1 : 1) * Math.abs(x.geom.kind === "circle" ? Math.PI * x.geom.r ** 2 : area(outline(x.geom, 1e-4))), 0));
  return Math.min(Math.max(b[2] - b[0], b[3] - b[1]) / 25, a > 0 ? Math.sqrt(a / (0.43 * 800)) : Infinity);
};

const boundsOf = (m: Model): [number, number, number, number] | null => {
  const ps: Pt[] = [...m.nodes.map(n => [n.x, n.y] as Pt), ...m.shapes.flatMap(s => s.geom.kind === "circle" ? [[s.geom.c[0] - s.geom.r, s.geom.c[1] - s.geom.r], [s.geom.c[0] + s.geom.r, s.geom.c[1] + s.geom.r]] as Pt[] : outline(s.geom, Infinity))];
  if (!ps.length) return null;
  return [Math.min(...ps.map(p => p[0])), Math.min(...ps.map(p => p[1])), Math.max(...ps.map(p => p[0])), Math.max(...ps.map(p => p[1]))];
};

/** A nice step for grid lines and rulers: at least `px` pixels apart, a multiple of the snap step. */
const niceStep = (px: number) => {
  let s = grid();
  for (let i = 0; s * scale() < px && i < 40; i++) s *= i % 3 === 1 ? 2.5 : 2;
  return s;
};

// ---------- what the pointer is over ----------
type Hit = { kind: "node"; id: number } | { kind: "member"; id: number } | { kind: "edge"; shape: number; edge: number } | { kind: "shape"; id: number } | { kind: "support"; id: number } | { kind: "load"; id: number } | null;

/** The point of an item that a drag puts on the grid: a node, a member's start, a shape's first corner or centre. */
const anchorOf = (h: Hit): Pt | null => {
  const m = model(); if (!m || !h) return null;
  if (h.kind === "node") { const n = m.nodes.find(x => x.id === h.id); return n ? [n.x, n.y] : null; }
  if (h.kind === "member") { const b = m.members.find(x => x.id === h.id), n = b && m.nodes.find(x => x.id === b.a); return n ? [n.x, n.y] : null; }
  if (h.kind === "shape") { const s = m.shapes.find(x => x.id === h.id); return s ? (s.geom.kind === "circle" ? s.geom.c : s.geom.pts[0]) : null; }
  return null;
};
const nodePos = (m: Model) => new Map(m.nodes.map(n => [n.id, [n.x, n.y] as Pt]));

const hitTest = (p: Pt): Hit => {
  const m = model(); if (!m) return null;
  const tol = 7 / scale(), np = nodePos(m);
  for (const n of m.nodes) if (Math.hypot(n.x - p[0], n.y - p[1]) < tol) return { kind: "node", id: n.id };
  for (const mb of m.members) { const a = np.get(mb.a), b = np.get(mb.b); if (a && b && distTo([a, b], p) < tol) return { kind: "member", id: mb.id }; }
  // Edges of holes first (they sit on top of solids), then solids.
  const shapes = [...m.shapes].sort((a, b) => Number(b.hole) - Number(a.hole));
  for (const s of shapes) for (let e = 0; e < edgeCount(s.geom); e++) if (distTo(edgePts(s.geom, e, 4 / scale()), p) < tol) return { kind: "edge", shape: s.id, edge: e };
  for (const s of shapes) if (inside(outline(s.geom, 4 / scale()), p)) return { kind: "shape", id: s.id };
  return null;
};

/** Snap a point: to a node or a shape's corner nearby, else to the grid. */
const snapPt = (p: Pt): Pt => {
  const m = model(); if (!m) return p;
  const tol = 8 / scale();
  for (const n of m.nodes) if (Math.hypot(n.x - p[0], n.y - p[1]) < tol) return [n.x, n.y];
  for (const s of m.shapes) for (const k of keyPts(s.geom)) if (Math.hypot(k[0] - p[0], k[1] - p[1]) < tol) return k;
  return [snapV(p[0]), snapV(p[1])];
};

// ---------- drawing state ----------
const [hover, setHover] = createSignal<Hit>(null);
/** Points placed so far by a drawing tool, and where the pointer is. */
const [draft, setDraft] = createSignal<Pt[]>([]);
const [ptr, setPtr] = createSignal<Pt | null>(null);
/** Moving the selection: the offset so far (world). */
const [drag, setDrag] = createSignal<Pt | null>(null);
const [marquee, setMarquee] = createSignal<[Pt, Pt] | null>(null);
const [panning, setPanning] = createSignal(false);
export const cancelDraft = () => batch(() => { setDraft([]); setMarquee(null); setDrag(null); });

const selSet = createMemo(() => new Set(sel()));

/** Where to put an item for a support or load tool, from what was clicked. */
const placeOn = (h: Hit, p: Pt, frame: boolean): Target | null => {
  if (h?.kind === "node") return { on: "node", id: h.id };
  if (h?.kind === "member") return { on: "member", id: h.id };
  if (h?.kind === "edge") return { on: "edge", shape: h.shape, edge: h.edge };
  if (!frame && (h?.kind === "shape" || h === null)) return { on: "point", x: p[0], y: p[1] };
  return null;
};

const finishTool = async (t: string, pts: Pt[]) => {
  const geom = (): Geom | null => {
    if (t === "rect" || t === "hrect") { const [a, b] = pts; if (a[0] === b[0] || a[1] === b[1]) return null; const [x0, x1, y0, y1] = [Math.min(a[0], b[0]), Math.max(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[1], b[1])]; return { kind: "poly", pts: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]], bulge: [0, 0, 0, 0] }; }
    if (t === "circle" || t === "hcircle") { const r = Math.hypot(pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]); return r > 0 ? { kind: "circle", c: pts[0], r } : null; }
    return { kind: "poly", pts, bulge: pts.map(() => 0) };
  };
  const g = geom(); if (!g) return;
  const o = await act({ Add: { item: { Shape: { id: 0, name: "", hole: t.startsWith("h"), material: model()!.materials[0].id, geom: g } } } });
  if (o?.created.length) setSel(o.created);
};

// ---------- pictures of supports and loads ----------
const ARROW = 46;
const arrowPath = (tip: Pt, dir: Pt, len = ARROW) => {
  const l = Math.hypot(dir[0], dir[1]) || 1, ux = dir[0] / l, uy = dir[1] / l, tail: Pt = [tip[0] - ux * len, tip[1] - uy * len];
  const h = 7, w = 3.5;
  return `M${tail[0]} ${tail[1]}L${tip[0]} ${tip[1]}M${tip[0] - ux * h - uy * w} ${tip[1] - uy * h + ux * w}L${tip[0]} ${tip[1]}L${tip[0] - ux * h + uy * w} ${tip[1] - uy * h - ux * w}`;
};

/** A support symbol at a node, in screen space. */
const SupportGlyph = (p: { s: Support; at: Pt; on: boolean }) => <>{supportBody(p)}</>;
/** Read inside the JSX above, so the symbol follows pan, zoom and edits. */
const supportBody = (p: { s: Support; at: Pt; on: boolean }) => {
  const [x, y] = p.at, k = p.s.kind.kind;
  const hatch = (y0: number, w: number) => <path d={`M${x - w} ${y0}h${2 * w}` + Array.from({ length: 5 }, (_, i) => `M${x - w + 4 + i * (2 * w - 4) / 5} ${y0}l-5 6`).join("")} class="supl" />;
  if (k === "fixed") return <g class="sup" classList={{ sel: p.on }} data-id={p.s.id}><rect x={x - 12} y={y - 2} width={24} height={4} class="supf" />{hatch(y + 2, 12)}</g>;
  if (k === "pinned" || k === "rollerx") return <g class="sup" classList={{ sel: p.on }} data-id={p.s.id}><path d={`M${x} ${y}l-10 15h20z`} class="supf" />{k === "rollerx" ? <><circle cx={x - 5} cy={y + 18.5} r={3} class="supl" /><circle cx={x + 5} cy={y + 18.5} r={3} class="supl" />{hatch(y + 22, 12)}</> : hatch(y + 15, 12)}</g>;
  if (k === "rollery") return <g class="sup" classList={{ sel: p.on }} data-id={p.s.id}><path d={`M${x} ${y}l-15 -10v20z`} class="supf" /><circle cx={x - 18.5} cy={y - 5} r={3} class="supl" /><circle cx={x - 18.5} cy={y + 5} r={3} class="supl" /><path d={`M${x - 22} ${y - 12}v24`} class="supl" /></g>;
  if (k === "spring") return <g class="sup" classList={{ sel: p.on }} data-id={p.s.id}><path d={`M${x} ${y}v4l-6 3l12 4l-12 4l12 4l-6 3v4M${x - 10} ${y + 26}h20`} class="supl" /></g>;
  if (k === "temperature") return <g class="sup heat" classList={{ sel: p.on }} data-id={p.s.id}><circle cx={x} cy={y} r={6} class="supl" /><text x={x + 9} y={y - 7} class="ltxt">{fmt("temp", (p.s.kind as any).t, 3)}</text></g>;
  return <circle cx={x} cy={y} r={5} class="supl" />;
};

/** Ticks along an edge on the side away from the body: supports on edges. */
const EdgeSupport = (p: { s: Support; sh: Shape; on: boolean }) => {
  const at = p.s.at as Extract<Target, { on: "edge" }>;
  const pts = createMemo(() => edgePts(p.sh.geom, at.edge, 12 / scale()).map(toS));
  const sign = () => orient(p.sh.geom) * (p.sh.hole ? -1 : 1);
  const d = () => {
    const ps = pts(), k = p.s.kind.kind, out: string[] = [];
    let acc = 0;
    for (let i = 0; i + 1 < ps.length; i++) {
      const [a, b] = [ps[i], ps[i + 1]], L = Math.hypot(b[0] - a[0], b[1] - a[1]);
      if (!L) continue;
      // Outward normal on screen (y down flips the turn).
      const nx = -(b[1] - a[1]) / L * sign(), ny = (b[0] - a[0]) / L * sign();
      for (let s = (12 - acc % 12) % 12; s <= L; s += 12) {
        const x = a[0] + (b[0] - a[0]) * s / L, y = a[1] + (b[1] - a[1]) * s / L;
        if (k === "fixed" || k === "pinned") out.push(`M${x} ${y}l${nx * 8 + ny * 4} ${ny * 8 - nx * 4}`);
        else out.push(`M${x + nx * 5} ${y + ny * 5}m-2.5 0a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0 -5 0`);
      }
      acc += L;
    }
    return out.join("");
  };
  const line = () => pts().map((q, i) => `${i ? "L" : "M"}${q[0]} ${q[1]}`).join("");
  return <g class="sup" classList={{ sel: p.on, heat: p.s.kind.kind === "temperature" }} data-id={p.s.id}><path d={line()} class="esup" /><Show when={p.s.kind.kind !== "temperature"}><path d={d()} class="supl" /></Show>
    <Show when={p.s.kind.kind === "temperature"}><text x={pts()[Math.floor(pts().length / 2)][0] + 6} y={pts()[Math.floor(pts().length / 2)][1] - 6} class="ltxt">{fmt("temp", (p.s.kind as any).t, 3)}</text></Show></g>;
};

/** A load: arrows in screen space with the value. */
/** Loads of the same kind and value on many edges (convection all round a fin) are labelled once. */
const labelled = (m: Model) => {
  const seen = new Set<string>(), out = new Set<number>();
  for (const l of m.loads) { const k = JSON.stringify(l.kind) + (l.at.on === "edge" ? `@${l.at.shape}` : `#${l.id}`); if (!seen.has(k)) { seen.add(k); out.add(l.id); } }
  return out;
};

/** A load on the drawing. `np` and `shape` give positions as drawn, so arrows follow a drag. */
const LoadGlyph = (p: { l: Load; m: Model; on: boolean; label: boolean; np: Map<number, Pt>; shape: (id: number) => Shape | undefined }) => {
  const l = () => p.l, np = () => p.np;
  const body = (): JSX.Element => {
    const k = l().kind, at = l().at;
    if ((k.kind === "force") && (at.on === "node" || at.on === "point")) {
      const w = at.on === "node" ? np().get(at.id) : [at.x, at.y] as Pt; if (!w) return null;
      const s = toS(w), dir: Pt = [k.fx, -k.fy];
      if (!k.fx && !k.fy) return <circle cx={s[0]} cy={s[1]} r={4} class="lf" />;
      const l2 = Math.hypot(dir[0], dir[1]);
      return <><path d={arrowPath(s, dir)} class="la" /><text x={s[0] - dir[0] / l2 * (ARROW + 6)} y={s[1] - dir[1] / l2 * (ARROW + 6)} class="ltxt" text-anchor="middle">{fmt("force", Math.hypot(k.fx, k.fy), 3)}</text></>;
    }
    if (k.kind === "moment" && at.on === "node") {
      const w = np().get(at.id); if (!w) return null;
      const [x, y] = toS(w), r = 18, ccw = k.m >= 0;
      return <><path d={`M${x + r} ${y}A${r} ${r} 0 1 ${ccw ? 0 : 1} ${x} ${y + (ccw ? r : -r)}`} class="la" /><path d={ccw ? `M${x - 5} ${y + r - 5}L${x} ${y + r}L${x - 5} ${y + r + 5}` : `M${x - 5} ${y - r - 5}L${x} ${y - r}L${x - 5} ${y - r + 5}`} class="la" /><text x={x + r + 4} y={y - r} class="ltxt">{fmt("moment", Math.abs(k.m), 3)}</text></>;
    }
    if (k.kind === "dist" && at.on === "member") {
      const mb = p.m.members.find(x => x.id === at.id); const a = mb && np().get(mb.a), b = mb && np().get(mb.b); if (!a || !b) return null;
      const [sa, sb] = [toS(a), toS(b)], L = Math.hypot(sb[0] - sa[0], sb[1] - sa[1]); if (!L) return null;
      const wmax = Math.max(Math.abs(k.w1), Math.abs(k.w2)) || 1, ux = (sb[0] - sa[0]) / L, uy = (sb[1] - sa[1]) / L;
      // Direction of the load on screen: global x, global y, or the member's local y (left of a→b in the world).
      const d = (w: number): Pt => k.dir === "x" ? [w, 0] : k.dir === "y" ? [0, -w] : [uy * w, -ux * w];
      const n = Math.max(2, Math.min(14, Math.floor(L / 28))), out: string[] = [], tops: Pt[] = [];
      for (let i = 0; i <= n; i++) {
        const t = i / n, w = k.w1 + (k.w2 - k.w1) * t, tip: Pt = [sa[0] + (sb[0] - sa[0]) * t, sa[1] + (sb[1] - sa[1]) * t], v = d(w / wmax * 30);
        if (Math.abs(w) > 1e-12) out.push(arrowPath(tip, v, Math.hypot(v[0], v[1])));
        tops.push([tip[0] - v[0], tip[1] - v[1]]);
      }
      const mid = tops[Math.floor(tops.length / 2)];
      return <><path d={out.join("") + tops.map((q, i) => `${i ? "L" : "M"}${q[0]} ${q[1]}`).join("")} class="la thin" /><text x={mid[0]} y={mid[1] - 6} class="ltxt" text-anchor="middle">{k.w1 === k.w2 ? fmt("dist", Math.abs(k.w1), 3) : `${fmt("dist", k.w1, 3)} → ${fmt("dist", k.w2, 3)}`}</text></>;
    }
    if (at.on === "edge") {
      const sh = p.shape(at.shape); if (!sh) return null;
      const pts = edgePts(sh.geom, at.edge, 4 / scale()).map(toS), sign = orient(sh.geom) * (sh.hole ? -1 : 1);
      const out: string[] = [];
      let acc = 0, mid: Pt = pts[0], anchor: "start" | "middle" | "end" = "middle";
      const total = pts.slice(1).reduce((s, q, i) => s + Math.hypot(q[0] - pts[i][0], q[1] - pts[i][1]), 0);
      const step = Math.max(16, Math.min(40, total / 6));
      for (let i = 0; i + 1 < pts.length; i++) {
        const [a, b] = [pts[i], pts[i + 1]], L = Math.hypot(b[0] - a[0], b[1] - a[1]); if (!L) continue;
        const nx = -(b[1] - a[1]) / L * sign, ny = (b[0] - a[0]) / L * sign; // outward
        for (let s = (step - acc % step) % step; s <= L; s += step) {
          const x = a[0] + (b[0] - a[0]) * s / L, y = a[1] + (b[1] - a[1]) * s / L;
          if (k.kind === "pressure") out.push(k.p >= 0 ? arrowPath([x, y], [-nx, -ny], 22) : arrowPath([x + nx * 22, y + ny * 22], [nx, ny], 22));
          else if (k.kind === "traction") out.push(arrowPath([x + nx * 10, y + ny * 10], [k.fx, -k.fy], 22));
          else if (k.kind === "bearing") { const c = -(nx * k.fx - ny * k.fy) / (Math.hypot(k.fx, k.fy) || 1); if (c > 0.05) out.push(arrowPath([x, y], [-nx, -ny], 6 + 18 * c)); }
          else if (k.kind === "flux") out.push(arrowPath([x, y], [-nx * Math.sign(k.q || 1), -ny * Math.sign(k.q || 1)], 18));
          else if (k.kind === "convection") out.push(`M${x + nx * 6 - ny * 5} ${y + ny * 6 + nx * 5}q${nx * 4 + ny * 2.5} ${ny * 4 - nx * 2.5} ${ny * 5} ${-nx * 5}t${ny * 5} ${-nx * 5}`);
        }
        if (acc <= total / 2 && acc + L >= total / 2) { const s = (total / 2 - acc) / L; mid = [a[0] + (b[0] - a[0]) * s + nx * 34, a[1] + (b[1] - a[1]) * s + ny * 34 + 4]; anchor = nx > 0.5 ? "start" : nx < -0.5 ? "end" : "middle"; }
        acc += L;
      }
      const label = k.kind === "bearing" ? `pin ${fmt("force", Math.hypot(k.fx, k.fy), 3)}` : k.kind === "pressure" ? fmt("stress", k.p, 3) : k.kind === "traction" ? `${fmt(k.total ? "force" : "dist", Math.hypot(k.fx, k.fy), 3)}${k.total ? " total" : ""}` : k.kind === "flux" ? fmt("flux", k.q, 3) : k.kind === "convection" ? `h ${fmt("film", k.h, 3)}, ${fmt("temp", k.t_inf, 3)}` : "";
      return <><path d={out.join("")} class="la thin" classList={{ heat: k.kind === "flux" || k.kind === "convection" }} /><Show when={p.label || p.on}><text x={mid[0]} y={mid[1]} class="ltxt" text-anchor={anchor}>{label}</text></Show></>;
    }
    return null;
  };
  return <g class="load" classList={{ sel: p.on }} data-id={l().id}>{body()}</g>;
};

// ---------- the canvas ----------
export const Canvas = () => {
  let svg!: SVGSVGElement;
  const [size, setSize] = createSignal<[number, number]>([800, 600]);
  onMount(() => {
    const ro = new ResizeObserver(() => { const r = box!.getBoundingClientRect(); setSize([r.width, r.height]); });
    ro.observe(box!);
    setTimeout(fit, 30);
    onCleanup(() => ro.disconnect());
  });
  const m = () => model();
  const frame = () => (m()?.analysis ?? "frame") === "frame";
  const S = (p: Pt) => toS(p);
  const moved = (id: number, p: Pt): Pt => {
    const d = drag(); if (!d) return p;
    const mm = m()!;
    const nodeMoves = selSet().has(id) || mm.members.some(mb => selSet().has(mb.id) && (mb.a === id || mb.b === id));
    return nodeMoves ? [p[0] + d[0], p[1] + d[1]] : p;
  };
  const np = createMemo(() => { const mm = m(); return mm ? new Map(mm.nodes.map(n => [n.id, moved(n.id, [n.x, n.y])])) : new Map<number, Pt>(); });
  /** A shape as drawn: moved with the drag when it is selected (edge supports and loads follow it). */
  const shapeAt = (id: number): Shape | undefined => {
    const sh = m()?.shapes.find(x => x.id === id), d = drag(); if (!sh || !d || !selSet().has(id)) return sh;
    const g = sh.geom, mv = (q: Pt): Pt => [q[0] + d[0], q[1] + d[1]];
    return { ...sh, geom: g.kind === "circle" ? { ...g, c: mv(g.c) } : { ...g, pts: g.pts.map(mv) } };
  };

  // Grid lines and rulers.
  const gridLines = createMemo(() => {
    if (!opts().grid) return { minor: "", major: "" };
    const [w, h] = size(), st = niceStep(12), maj = st * 5;
    const x0 = -pan()[0] / scale(), x1 = (w - pan()[0]) / scale(), y1 = pan()[1] / scale(), y0 = (pan()[1] - h) / scale();
    let minor = "", major = "";
    for (let x = Math.ceil(x0 / st) * st; x <= x1; x += st) { const sx = Math.round(toS([x, 0])[0]) + 0.5, s = `M${sx} 0V${h}`; if (Math.abs(x / maj - Math.round(x / maj)) < 1e-6) major += s; else minor += s; }
    for (let y = Math.ceil(y0 / st) * st; y <= y1; y += st) { const sy = Math.round(toS([0, y])[1]) + 0.5, s = `M0 ${sy}H${w}`; if (Math.abs(y / maj - Math.round(y / maj)) < 1e-6) major += s; else minor += s; }
    return { minor, major };
  });
  const rulerTicks = createMemo(() => {
    const [w, h] = size(), st = niceStep(70);
    const xs: [number, number][] = [], ys: [number, number][] = [];
    // Ticks are whole multiples of the step (k · st), so no rounding noise shows as −6.9e−15.
    for (let k = Math.ceil(-pan()[0] / scale() / st); k * st <= (w - pan()[0]) / scale(); k++) xs.push([toS([k * st, 0])[0], +(k * st).toPrecision(12)]);
    for (let k = Math.ceil((pan()[1] - h) / scale() / st); k * st <= pan()[1] / scale(); k++) ys.push([toS([0, k * st])[1], +(k * st).toPrecision(12)]);
    return { xs, ys };
  });

  // ---------- pointer ----------
  let downAt: { c: Pt; w: Pt; hit: Hit; moved: boolean; button: number } | null = null;
  let spaceDown = false;
  const onDown = (e: MouseEvent) => {
    box!.focus();
    const w = toW(e.clientX, e.clientY);
    if (e.button === 1 || (e.button === 0 && spaceDown)) { e.preventDefault(); setPanning(true); downAt = { c: [e.clientX, e.clientY], w, hit: null, moved: false, button: 1 }; return; }
    if (e.button !== 0) return;
    const t = tool(), g = (e.target as Element).closest?.(".load[data-id], .sup[data-id]") as SVGElement | null;
    // Nodes win over the arrows and symbols drawn on them, which win over members and shapes.
    const h0 = hitTest(w);
    const h: Hit = g && t === "select" && h0?.kind !== "node" ? { kind: g.classList.contains("load") ? "load" : "support", id: +g.dataset.id! } : h0;
    downAt = { c: [e.clientX, e.clientY], w, hit: h, moved: false, button: 0 };
    if (t === "select") {
      if (!h) { if (!e.shiftKey && !e.ctrlKey) { setSel([]); setEdgeSel(null); } setMarquee([w, w]); return; }
      if (h.kind === "edge") { batch(() => { setEdgeSel({ shape: h.shape, edge: h.edge }); if (!e.shiftKey && !e.ctrlKey) setSel([]); }); return; }
      const id = (h as any).id as number;
      if (e.shiftKey || e.ctrlKey) setSel(selSet().has(id) ? sel().filter(i => i !== id) : [...sel(), id]);
      else if (!selSet().has(id)) batch(() => { setSel([id]); setEdgeSel(null); });
      if (h.kind !== "support" && h.kind !== "load") setDrag([0, 0]);
    }
  };
  const onMove = (e: MouseEvent) => {
    const w = toW(e.clientX, e.clientY);
    setCursor(w);
    if (downAt && downAt.button === 1) { setPan([pan()[0] + e.movementX, pan()[1] + e.movementY]); return; }
    const t = tool();
    if (downAt) {
      if (Math.hypot(e.clientX - downAt.c[0], e.clientY - downAt.c[1]) > 3) downAt.moved = true;
      if (marquee()) { setMarquee([marquee()![0], w]); return; }
      if (drag() && downAt.moved) {
        // The grabbed point lands on the grid (0.1.1 review: the offset was snapped, so items
        // off the grid stayed off it).
        const a = anchorOf(downAt.hit) ?? downAt.w;
        setDrag([snapV(a[0] + w[0] - downAt.w[0]) - a[0], snapV(a[1] + w[1] - downAt.w[1]) - a[1]]);
        return;
      }
    }
    setHover(t === "select" || t.startsWith("sup:") || t.startsWith("load:") ? hitTest(w) : null);
    setPtr(t === "select" ? null : snapPt(w));
  };
  const onUp = async (e: MouseEvent) => {
    const d = downAt; downAt = null;
    if (!d) return;
    if (d.button === 1) { setPanning(false); return; }
    const w = toW(e.clientX, e.clientY), t = tool(), mm = m(); if (!mm) return;
    if (t === "select") {
      const mq = marquee();
      if (mq) {
        setMarquee(null);
        if (!d.moved) return;
        const [x0, x1, y0, y1] = [Math.min(mq[0][0], mq[1][0]), Math.max(mq[0][0], mq[1][0]), Math.min(mq[0][1], mq[1][1]), Math.max(mq[0][1], mq[1][1])];
        const inR = (p: Pt) => p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1;
        const ids = [...mm.nodes.filter(n => inR([n.x, n.y])).map(n => n.id), ...mm.members.filter(mb => inR(np().get(mb.a)!) && inR(np().get(mb.b)!)).map(mb => mb.id), ...mm.shapes.filter(s => outline(s.geom, Infinity).every(inR) && (s.geom.kind !== "circle" || (inR([s.geom.c[0] - s.geom.r, s.geom.c[1] - s.geom.r]) && inR([s.geom.c[0] + s.geom.r, s.geom.c[1] + s.geom.r])))).map(s => s.id)];
        setSel(e.shiftKey || e.ctrlKey ? [...new Set([...sel(), ...ids])] : ids);
        return;
      }
      const dd = drag(); setDrag(null);
      if (dd && d.moved && (dd[0] || dd[1])) await act({ Move: { ids: sel(), dx: dd[0], dy: dd[1] } });
      return;
    }
    const p = snapPt(w);
    if (t === "node") { const o = await act({ Add: { item: { Node: { id: 0, x: p[0], y: p[1] } } } }); if (o?.created.length) setSel(o.created); return; }
    if (t === "beam" || t === "truss") {
      const dr = draft();
      if (!dr.length) { setDraft([p]); return; }
      if (p[0] === dr[0][0] && p[1] === dr[0][1]) return;
      const o = await act({ AddMember: { from: dr[0], to: p, kind: t, section: null, material: null } });
      setDraft(o ? [p] : dr);
      return;
    }
    if (t === "rect" || t === "hrect" || t === "circle" || t === "hcircle") {
      const dr = draft();
      if (!dr.length) { setDraft([p]); return; }
      if (p[0] === dr[0][0] && p[1] === dr[0][1]) return;
      setDraft([]); await finishTool(t, [dr[0], p]); return;
    }
    if (t === "poly") {
      const dr = draft();
      if (dr.length >= 3 && Math.hypot((p[0] - dr[0][0]) * scale(), (p[1] - dr[0][1]) * scale()) < 8) { setDraft([]); await finishTool(t, dr); return; }
      if (dr.length && p[0] === dr[dr.length - 1][0] && p[1] === dr[dr.length - 1][1]) return;
      setDraft([...dr, p]); return;
    }
    if (t === "refine") {
      const h = hitTest(w), at = placeOn(h, p, false);
      if (!at || at.on === "node" || at.on === "member") return toast("Click an edge or a point of the part to make the elements finer there.");
      const ms = mm.mesh, base = ms.size > 0 ? ms.size : (meshed()?.size ?? partRes()?.mesh.size ?? autoSize(mm));
      const same = (x: Target) => JSON.stringify(x) === JSON.stringify(at);
      const refine = [...ms.refine.filter(r => !same(r.at)), { at, size: base / 4 }];
      await act({ SetSettings: { s: { analysis: mm.analysis, units: mm.units, grid: mm.grid, thickness: mm.thickness, mesh: { ...ms, refine } } } });
      return toast(`Elements a quarter of the size (${fmt("len", base / 4)}) near there. Change or remove it in the tree, under Mesh.`, true);
    }
    if (t.startsWith("sup:") || t.startsWith("load:")) {
      const h = hitTest(w), at = placeOn(h, frame() ? w : p, frame());
      if (!at) return toast(t.startsWith("sup:") ? (frame() ? "Click a node to put the support on." : "Click an edge or a point of the part.") : "Click where the load goes: a node, a member or an edge.");
      const [kind, val] = t.startsWith("sup:") ? ["Support", defaultSupport(t.slice(4))] : ["Load", defaultLoad(t.slice(5), frame())];
      const o = await act({ Add: { item: { [kind]: { id: 0, at, kind: val } } } });
      if (o?.created.length) batch(() => { setSel(o.created); setEdgeSel(null); });
    }
  };
  const onDbl = async () => {
    if (tool() === "poly" && draft().length >= 3) { const dr = draft(); setDraft([]); await finishTool("poly", dr); }
  };
  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    const r = box!.getBoundingClientRect();
    if (e.shiftKey) setPan([pan()[0] - e.deltaY, pan()[1]]);
    else zoomBy(Math.exp(-e.deltaY * 0.0015), [e.clientX - r.left, e.clientY - r.top]);
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement) return;
    if (e.key === " ") { spaceDown = true; e.preventDefault(); }
    else if (e.key === "Escape") { if (draft().length) setDraft([]); else if (tool() !== "select") setTool("select"); else batch(() => { setSel([]); setEdgeSel(null); }); }
    else if (e.key === "Enter" && tool() === "poly" && draft().length >= 3) { const dr = draft(); setDraft([]); finishTool("poly", dr); }
    else if (e.key === "Delete" || e.key === "Backspace") { e.preventDefault(); C.del(); }
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") { e.preventDefault(); const mm = m(); if (mm) setSel([...mm.nodes, ...mm.members, ...mm.shapes].map(x => x.id)); }
    else if (e.key.startsWith("Arrow") && sel().length) {
      e.preventDefault();
      const g = grid(), [dx, dy] = { ArrowLeft: [-g, 0], ArrowRight: [g, 0], ArrowUp: [0, g], ArrowDown: [0, -g] }[e.key] as Pt;
      act({ Move: { ids: sel(), dx, dy } });
    }
  };
  const onKeyUp = (e: KeyboardEvent) => { if (e.key === " ") spaceDown = false; };
  const onCtx = (e: MouseEvent) => {
    e.preventDefault();
    if (draft().length) { setDraft([]); return; }
    const h = hitTest(toW(e.clientX, e.clientY));
    if (h && h.kind !== "edge" && !selSet().has((h as any).id)) setSel([(h as any).id]);
    if (h?.kind === "edge") setEdgeSel({ shape: h.shape, edge: h.edge });
    const items: Item[] = [
      { label: "Select", run: () => setTool("select"), icon: "select" },
      "-",
      { label: "Delete", run: C.del, keys: "Del", off: !sel().length, icon: "del" },
      { label: "Fit to window", run: fit, icon: "fit" },
    ];
    openMenu(e, items);
  };

  // ---------- what to draw ----------
  const shapes = () => { const mm = m(); return mm ? [...mm.shapes.filter(s => !s.hole), ...mm.shapes.filter(s => s.hole)] : []; };
  const shapePath = (s: Shape) => {
    const d = drag(), off: Pt = d && selSet().has(s.id) ? d : [0, 0];
    return svgPath(s.geom, p => S([p[0] + off[0], p[1] + off[1]]), scale());
  };
  const draftPath = () => {
    const dr = draft(), p = ptr(), t = tool(); if (!dr.length || !p) return "";
    if (t === "beam" || t === "truss") return `M${S(dr[0]).join(" ")}L${S(p).join(" ")}`;
    if (t === "rect" || t === "hrect") { const a = S(dr[0]), b = S(p); return `M${a[0]} ${a[1]}H${b[0]}V${b[1]}H${a[0]}z`; }
    if (t === "circle" || t === "hcircle") { const c = S(dr[0]), r = Math.hypot(p[0] - dr[0][0], p[1] - dr[0][1]) * scale(); return `M${c[0] - r} ${c[1]}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0z`; }
    return [...dr, p].map((q, i) => `${i ? "L" : "M"}${S(q).join(" ")}`).join("");
  };
  const draftLabel = () => {
    const dr = draft(), p = ptr(), t = tool(); if (!p) return "";
    if (!dr.length) return `${fmt("coord", p[0])}, ${fmt("coord", p[1])}`;
    const d = [p[0] - dr[dr.length - 1][0], p[1] - dr[dr.length - 1][1]];
    if (t === "rect" || t === "hrect") return `${fmt("coord", Math.abs(p[0] - dr[0][0]))} × ${fmt("coord", Math.abs(p[1] - dr[0][1]))}`;
    if (t === "circle" || t === "hcircle") return `r ${fmt("coord", Math.hypot(d[0], d[1]))}`;
    return `L ${fmt("coord", Math.hypot(d[0], d[1]))} · ${Math.round(Math.atan2(d[1], d[0]) * 180 / Math.PI)}°`;
  };
  const hoverIs = (k: string, id: number) => { const h = hover(); return !!h && h.kind === k && (h as any).id === id; };
  const edgeHover = () => { const h = hover(); return h?.kind === "edge" ? h : null; };
  const edgePath = (e: { shape: number; edge: number }) => { const s = m()?.shapes.find(x => x.id === e.shape); if (!s) return ""; return edgePts(s.geom, e.edge, 3 / scale()).map((p, i) => `${i ? "L" : "M"}${S(p).join(" ")}`).join(""); };
  const memberEnds = (mb: Member): [Pt, Pt] | null => { const a = np().get(mb.a), b = np().get(mb.b); return a && b ? [S(a), S(b)] : null; };
  const supportsAtNodes = () => (m()?.supports ?? []).filter(s => s.at.on === "node" || s.at.on === "point");
  const supportsOnEdges = () => (m()?.supports ?? []).filter(s => s.at.on === "edge");
  const supAt = (s: Support): Pt | null => s.at.on === "node" ? (np().get(s.at.id) ?? null) : s.at.on === "point" ? [s.at.x, s.at.y] : null;
  const gravity = () => (m()?.loads ?? []).filter(l => l.kind.kind === "gravity" || l.kind.kind === "heat" || l.kind.kind === "warming");
  const labels = createMemo(() => (m() ? labelled(m()!) : new Set<number>()));
  const empty = () => { const mm = m(); return !!mm && !mm.nodes.length && !mm.shapes.length; };
  /** Contours or the mesh are drawn under the outlines: shapes are not filled then. */
  const under = () => !frame() && (view() === "mesh" ? true : !!partRes());
  const refines = () => (m()?.mesh.refine ?? []);

  return (
    <div class="canvas" classList={{ panning: panning(), [`t-${tool().split(":")[0]}`]: true }} ref={box} tabIndex={0}
      onMouseDown={onDown} onMouseMove={onMove} onMouseUp={onUp} onDblClick={onDbl} onWheel={onWheel} onKeyDown={onKey} onKeyUp={onKeyUp} onContextMenu={onCtx}
      onMouseLeave={() => { setCursor(null); setHover(null); }}>
      <PartLayer size={size()} />
      <svg class="dsvg" ref={svg} classList={{ under: under() }}>
        <path d={gridLines().minor} class="gminor" /><path d={gridLines().major} class="gmajor" />
        <path d={`M${pan()[0]} 0V${size()[1]}M0 ${pan()[1]}H${size()[0]}`} class="gaxis" />
        <Show when={m()?.analysis === "axi"}><path d={`M${pan()[0]} 0V${size()[1]}`} class="axi" /><text x={pan()[0] + 6} y={RULER + 16} class="axilbl">axis of turning · draw right of it</text></Show>
        {/* plane geometry */}
        <For each={shapes()}>{s => <path d={shapePath(s)} class="shape" classList={{ hole: s.hole, sel: selSet().has(s.id), hover: hoverIs("shape", s.id) }} data-id={s.id} />}</For>
        <Show when={edgeHover()}>{e => <path d={edgePath(e())} class="edgehl" />}</Show>
        <Show when={edgeSel()}>{e => <path d={edgePath(e())} class="edgesel" />}</Show>
        <For each={refines()}>{r => <Show when={r.at.on === "edge" || r.at.on === "point"}>{r.at.on === "edge" ? <path d={edgePath(r.at as any)} class="refl" /> : <circle cx={S([(r.at as any).x, (r.at as any).y])[0]} cy={S([(r.at as any).x, (r.at as any).y])[1]} r={9} class="refl" />}</Show>}</For>
        <For each={supportsOnEdges()}>{s => { const sh = () => shapeAt((s.at as any).shape); return <Show when={sh()}><EdgeSupport s={s} sh={sh()!} on={selSet().has(s.id)} /></Show>; }}</For>
        {/* frames */}
        <For each={m()?.members ?? []}>{mb => <Show when={memberEnds(mb)}>{e => <g class="mem" classList={{ truss: mb.kind === "truss", sel: selSet().has(mb.id), hover: hoverIs("member", mb.id) }} data-id={mb.id}>
          <line x1={e()[0][0]} y1={e()[0][1]} x2={e()[1][0]} y2={e()[1][1]} />
          <Show when={mb.kind === "truss"}><circle cx={e()[0][0] + (e()[1][0] - e()[0][0]) * 0.06} cy={e()[0][1] + (e()[1][1] - e()[0][1]) * 0.06} r={2.5} class="pin" /><circle cx={e()[1][0] - (e()[1][0] - e()[0][0]) * 0.06} cy={e()[1][1] - (e()[1][1] - e()[0][1]) * 0.06} r={2.5} class="pin" /></Show>
        </g>}</Show>}</For>
        <ResultsLayer />
        <PartMarkers />
        <For each={supportsAtNodes()}>{s => <Show when={supAt(s)}>{p => <SupportGlyph s={s} at={S(p())} on={selSet().has(s.id)} />}</Show>}</For>
        <For each={m()?.nodes ?? []}>{n => { const p = () => S(np().get(n.id) ?? [n.x, n.y]); return <rect x={p()[0] - 3.5} y={p()[1] - 3.5} width={7} height={7} class="node" classList={{ sel: selSet().has(n.id), hover: hoverIs("node", n.id) }} data-id={n.id} />; }}</For>
        <For each={(m()?.loads ?? []).filter(l => l.at.on !== "body")}>{l => <LoadGlyph l={l} m={m()!} on={selSet().has(l.id)} label={labels().has(l.id)} np={np()} shape={shapeAt} />}</For>
        {/* drawing in progress */}
        <path d={draftPath()} class="draft" />
        <For each={draft()}>{p => <circle cx={S(p)[0]} cy={S(p)[1]} r={3} class="draftpt" />}</For>
        <Show when={ptr() && tool() !== "select"}>{_ => <><path d={`M${S(ptr()!)[0] - 8} ${S(ptr()!)[1]}h16M${S(ptr()!)[0]} ${S(ptr()!)[1] - 8}v16`} class="xhair" /><text x={S(ptr()!)[0] + 10} y={S(ptr()!)[1] - 10} class="dlabel">{draftLabel()}</text></>}</Show>
        <Show when={marquee()}>{q => { const a = S(q()[0]), b = S(q()[1]); return <rect x={Math.min(a[0], b[0])} y={Math.min(a[1], b[1])} width={Math.abs(a[0] - b[0])} height={Math.abs(a[1] - b[1])} class="marquee" />; }}</Show>
        {/* rulers */}
        <Show when={opts().rulers}>
          <rect x={0} y={0} width={size()[0]} height={RULER} class="ruler" /><rect x={0} y={0} width={RULER} height={size()[1]} class="ruler" />
          <For each={rulerTicks().xs}>{([sx, x]) => <><path d={`M${sx} ${RULER - 6}V${RULER}`} class="rtick" /><text x={sx + 3} y={RULER - 9} class="rtxt">{fmt("coord", x, 5).replace(/ .*/, "")}</text></>}</For>
          <For each={rulerTicks().ys}>{([sy, y]) => <><path d={`M${RULER - 6} ${sy}H${RULER}`} class="rtick" /><text x={RULER - 9} y={sy - 3} class="rtxt" transform={`rotate(-90 ${RULER - 9} ${sy - 3})`}>{fmt("coord", y, 5).replace(/ .*/, "")}</text></>}</For>
          <rect x={0} y={0} width={RULER} height={RULER} class="ruler corner" /><text x={3} y={14} class="rtxt unitlbl">{fmt("coord", 0).split(" ")[1]}</text>
        </Show>
      </svg>
      <Show when={gravity().length}><div class="gbadge"><For each={gravity()}>{l => <button classList={{ on: selSet().has(l.id) }} onClick={() => setSel([l.id])}>{l.kind.kind === "gravity" ? `g = ${fmt("accel", Math.hypot((l.kind as any).gx, (l.kind as any).gy), 3)}` : l.kind.kind === "warming" ? `ΔT = ${fmt("dtemp", (l.kind as any).dt, 3)}` : `q = ${fmt("heatgen", (l.kind as any).q, 3)}`}</button>}</For></div></Show>
      <Show when={empty() && tool() === "select"}>
        <div class="empty"><b>{frame() ? "Draw a frame or truss" : "Draw a part"}</b>
          <span>{frame() ? "Pick Beam or Truss in Geometry, then click two points for each member. Supports and loads are on the Loads tab." : "Pick Rectangle, Circle or Polygon in Geometry and draw the outline; holes cut out of it. Supports and loads go on its edges."}</span>
          <button class="btn pri" onClick={() => setTool(frame() ? "beam" : "rect")}>{frame() ? "Draw members" : "Draw a rectangle"}</button></div>
      </Show>
      <ResultsProbe />
      <PartProbe />
      <div class="zoomctl"><button title="Zoom out" onClick={() => zoomBy(1 / 1.25)}>−</button><button title="Fit (F)" onClick={fit}>Fit</button><button title="Zoom in" onClick={() => zoomBy(1.25)}>+</button></div>
    </div>
  );
};

