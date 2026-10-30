// Parts: the mesh and the results of plane stress, plane strain and heat conduction. Contours are
// filled bands (12 by default, blue to red or viridis) drawn on a canvas under the drawing, from
// paths built once in model coordinates and redrawn with one transform on pan and zoom. Also the
// deformed shape, mesh lines, min/max markers, a probe that reads any point, the legend, the
// results tables and the mesh statistics. The engine computes every value; this only draws them.
import { For, Show, createMemo, createEffect } from "solid-js";
import { Pt, Mesh, PartResults, PlaneResults, HeatResults, Field, Model, num, clean } from "./api";
import { model, partRes, show, setShow, stale, cursor, opts, meshed, meshStale, view, setView, themeTick, setSel, modes, modeSel, modeOn, phase, modesStale, solvedFor } from "./store";
import { modeAmp } from "./modes";
import { toS, scale, pan } from "./canvas";
import { Q, fmt, unit } from "./units";
import { itemName } from "./panels";
import { Help } from "./help";
import { Icon } from "./ui";

// ---------- fields ----------
export type FieldDef = { key: string; name: string; short: string; q: Q; heat?: boolean; strain?: boolean; reverse?: boolean; nodal?: boolean };
export const FIELDS: FieldDef[] = [
  { key: "vm", name: "Von Mises stress", short: "σvm", q: "stress" },
  { key: "s1", name: "Largest principal stress σ1", short: "σ1", q: "stress" },
  { key: "s2", name: "Smallest principal stress σ2", short: "σ2", q: "stress" },
  { key: "sx", name: "Stress σx", short: "σx", q: "stress" },
  { key: "sy", name: "Stress σy", short: "σy", q: "stress" },
  { key: "txy", name: "Shear stress τxy", short: "τxy", q: "stress" },
  { key: "sz", name: "Stress σz (across the plane)", short: "σz", q: "stress", strain: true },
  { key: "umag", name: "Displacement", short: "|u|", q: "len", nodal: true },
  { key: "ux", name: "Displacement in x", short: "ux", q: "len", nodal: true },
  { key: "uy", name: "Displacement in y", short: "uy", q: "len", nodal: true },
  { key: "sf", name: "Safety factor against yield", short: "SF", q: "ratio", reverse: true },
  { key: "t", name: "Temperature", short: "T", q: "temp", heat: true, nodal: true },
  { key: "qmag", name: "Heat flux", short: "|q|", q: "flux", heat: true },
  { key: "qx", name: "Heat flux in x", short: "qx", q: "flux", heat: true },
  { key: "qy", name: "Heat flux in y", short: "qy", q: "flux", heat: true },
  // 3D solids (0.3.0).
  { key: "s3", name: "Smallest principal stress σ3", short: "σ3", q: "stress" },
  { key: "tyz", name: "Shear stress τyz", short: "τyz", q: "stress" },
  { key: "tzx", name: "Shear stress τzx", short: "τzx", q: "stress" },
  { key: "uz", name: "Displacement in z", short: "uz", q: "len", nodal: true },
  // 3D heat (0.3.4).
  { key: "qz", name: "Heat flux in z", short: "qz", q: "flux", heat: true },
];
const SOLID = ["vm", "s1", "s3", "sx", "sy", "sz", "txy", "tyz", "tzx", "umag", "ux", "uy", "uz", "sf"];
/** In a turned (axisymmetric) solid x is the radius and y the axis: the results are named so. */
const AXI: Record<string, [string, string]> = { sx: ["Radial stress σr", "σr"], sy: ["Axial stress σz", "σz"], sz: ["Hoop stress σθ", "σθ"], txy: ["Shear stress τrz", "τrz"], ux: ["Radial displacement", "ur"], uy: ["Axial displacement", "uz"] };
const named = (f: FieldDef, a: string): FieldDef => (a === "axi" && AXI[f.key] ? { ...f, name: AXI[f.key][0], short: AXI[f.key][1] } : f);
/** A result's description, named for the open model's analysis. */
export const fieldDef = (key: string): FieldDef => named(FIELDS.find(x => x.key === key)!, model()?.analysis ?? "stress");
/** The results an analysis has; a part solved with thermal stress also has its temperatures. */
export const fieldsFor = (a: string) => {
  if (a === "solidheat") return ["t", "qmag", "qx", "qy", "qz"].map(k => FIELDS.find(f => f.key === k)!);
  if (a === "solid") return SOLID.map(k => FIELDS.find(f => f.key === k)!).map(f => f.key === "sz" ? { ...f, name: "Stress σz" } : f);
  const r = partRes(), warm = a !== "heat" && r?.kind === "plane" && r.fields.some(f => f.key === "t");
  return FIELDS.filter(f => !["s3", "tyz", "tzx", "uz", "qz"].includes(f.key) && (a === "heat" ? f.heat : (!f.heat || (warm && f.key === "t")) && (!f.strain || a === "strain" || a === "axi"))).map(f => named(f, a));
};
/** The field being shown: the one picked, if the analysis has it, else the first. */
export const curField = (): FieldDef => { const list = fieldsFor(partRes()?.kind === "heat" ? "heat" : model()?.analysis ?? "stress"); return list.find(f => f.key === show().field) ?? list[0]; };
const fieldOf = (r: PartResults, key: string, elem: boolean): Field | undefined => (elem ? r.elem.find(f => f.key === key) : undefined) ?? r.fields.find(f => f.key === key);
/** Whether a field is drawn per element now (displacements and temperatures are always smooth). */
const isElem = (d: FieldDef) => show().elem && !d.nodal;

// ---------- colours ----------
const VIR = ["#440154", "#482475", "#414487", "#355f8d", "#2a788e", "#21918c", "#22a884", "#44bf70", "#7ad151", "#bddf26", "#fde725"];
const hex = (h: string) => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
export const BANDS = 12;
/** The band colours, lowest first. */
export const palette = (kind: string, reverse = false): string[] => {
  const out = Array.from({ length: BANDS }, (_, i) => {
    const t = (i + 0.5) / BANDS;
    if (kind === "viridis") {
      const x = t * (VIR.length - 1), k = Math.min(VIR.length - 2, Math.floor(x)), f = x - k, a = hex(VIR[k]), b = hex(VIR[k + 1]);
      return `rgb(${a.map((v, j) => Math.round(v + (b[j] - v) * f)).join(",")})`;
    }
    return `hsl(${Math.round(240 - 240 * t)}, 82%, ${t > 0.35 && t < 0.65 ? 42 : 48}%)`;
  });
  return reverse ? out.reverse() : out;
};

/** The values the colours span. Safety factors span the smallest to 10 (over 10 is all safe). */
export const rangeOf = (d: FieldDef, v: number[]): [number, number] | null => {
  let lo = Infinity, hi = -Infinity;
  for (const x of v) { if (d.key === "sf" && !(x > 0)) continue; if (x < lo) lo = x; if (x > hi) hi = x; }
  if (!(lo <= hi)) return null;
  if (d.key === "sf" && lo < 10) hi = Math.min(hi, 10);
  return [lo, hi];
};

// ---------- pictures ----------
/** Part of a polygon (flat x, y list with a value per point) on one side of a level. */
const clip = (P: number[], V: number[], t: number, above: boolean): [number[], number[]] => {
  const oP: number[] = [], oV: number[] = [], n = V.length;
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n, vi = V[i], vj = V[j], a = above ? vi >= t : vi <= t, b = above ? vj >= t : vj <= t;
    if (a) { oP.push(P[2 * i], P[2 * i + 1]); oV.push(vi); }
    if (a !== b) { const s = (t - vi) / (vj - vi); oP.push(P[2 * i] + (P[2 * j] - P[2 * i]) * s, P[2 * i + 1] + (P[2 * j + 1] - P[2 * i + 1]) * s); oV.push(t); }
  }
  return [oP, oV];
};
// Closed by a line back to the start: Path2D.closePath slows down with every sub-path in Chromium
// (10 s for 100 000 triangles).
const poly = (path: Path2D, P: number[]) => { if (P.length < 6) return; path.moveTo(P[0], P[1]); for (let i = 2; i < P.length; i += 2) path.lineTo(P[i], P[i + 1]); path.lineTo(P[0], P[1]); };

export type Picture = {
  /** Node positions as drawn (deformed if shown), flat x, y. */
  pos: Float64Array;
  bands: Path2D[]; colours: string[]; unknown: Path2D | null;
  lines: Path2D; outline: Path2D;
  range: [number, number] | null;
  /** Buckets of drawing triangles, for the probe. */
  grid: { x0: number; y0: number; cell: number; nx: number; ny: number; cells: Map<number, number[]> };
};

/** How many times the movements are drawn larger than they are (0 when nothing moves or for heat). */
export const deformOf = (r: PartResults | null): number => {
  if (!r || r.kind !== "plane" || !show().deformed) return 0;
  const u = r.fields.find(f => f.key === "umag")!.max, b = boundsOf(r.mesh), size = Math.max(b[2] - b[0], b[3] - b[1]);
  if (!(u > 1e-12 * size)) return 0;
  const f = 0.06 * size / u, p = 10 ** Math.floor(Math.log10(f)), k = f / p;
  return (k >= 5 ? 5 : k >= 2 ? 2 : 1) * p * show().scale;
};
export const boundsOf = (ms: Mesh): [number, number, number, number] => {
  let b: [number, number, number, number] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of ms.nodes) b = [Math.min(b[0], p[0]), Math.min(b[1], p[1]), Math.max(b[2], p[0]), Math.max(b[3], p[1])];
  return b;
};

const meshPaths = (ms: Mesh, pos: Float64Array) => {
  const lines = new Path2D(), outline = new Path2D();
  for (const t of ms.tris) {
    const ring = t.length === 6 ? [t[0], t[3], t[1], t[4], t[2], t[5]] : t;
    lines.moveTo(pos[2 * ring[0]], pos[2 * ring[0] + 1]);
    for (let i = 1; i < ring.length; i++) lines.lineTo(pos[2 * ring[i]], pos[2 * ring[i] + 1]);
    lines.lineTo(pos[2 * ring[0]], pos[2 * ring[0] + 1]);
  }
  for (const e of ms.edges) {
    const ns = e.n[1] >= 0 && e.n[1] < 4294967295 ? e.n : [e.n[0], e.n[2]];
    outline.moveTo(pos[2 * ns[0]], pos[2 * ns[0] + 1]);
    for (let i = 1; i < ns.length; i++) outline.lineTo(pos[2 * ns[i]], pos[2 * ns[i] + 1]);
  }
  return { lines, outline };
};

const gridOf = (pos: Float64Array, tris: [number, number, number][]) => {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (let i = 0; i < pos.length; i += 2) { x0 = Math.min(x0, pos[i]); x1 = Math.max(x1, pos[i]); y0 = Math.min(y0, pos[i + 1]); y1 = Math.max(y1, pos[i + 1]); }
  const n = Math.max(1, Math.ceil(Math.sqrt(tris.length / 2))), cell = Math.max(x1 - x0, y1 - y0, 1e-12) / n;
  const nx = Math.ceil((x1 - x0) / cell) + 1, ny = Math.ceil((y1 - y0) / cell) + 1, cells = new Map<number, number[]>();
  tris.forEach((t, k) => {
    const xs = t.map(i => pos[2 * i]), ys = t.map(i => pos[2 * i + 1]);
    for (let i = Math.floor((Math.min(...xs) - x0) / cell); i <= Math.floor((Math.max(...xs) - x0) / cell); i++)
      for (let j = Math.floor((Math.min(...ys) - y0) / cell); j <= Math.floor((Math.max(...ys) - y0) / cell); j++) { const c = i * ny + j; const l = cells.get(c); if (l) l.push(k); else cells.set(c, [k]); }
  });
  return { x0, y0, cell, nx, ny, cells };
};

/** The contour picture of a result: band paths, mesh lines and outline, in model coordinates. */
export const buildPicture = (r: PartResults, d: FieldDef, elem: boolean, factor: number, colours: string[]): Picture => {
  const ms = r.mesh, nn = ms.nodes.length, pos = new Float64Array(2 * nn);
  const disp = r.kind === "plane" ? r.disp : null;
  for (let i = 0; i < nn; i++) { pos[2 * i] = ms.nodes[i][0] + (disp ? disp[i][0] * factor : 0); pos[2 * i + 1] = ms.nodes[i][1] + (disp ? disp[i][1] * factor : 0); }
  const tris = r.draw, f = fieldOf(r, d.key, elem)!, perEl = elem && f !== r.fields.find(x => x.key === d.key);
  const range = rangeOf(d, f.v), bands = Array.from({ length: BANDS }, () => new Path2D());
  let unknown: Path2D | null = null;
  const per = ms.order === 2 ? 4 : 1;
  if (range) {
    const [lo, hi] = range, span = hi - lo, lv = (k: number) => lo + span * k / BANDS;
    const band = (v: number) => (span > 0 ? Math.max(0, Math.min(BANDS - 1, Math.floor((v - lo) / span * BANDS))) : BANDS >> 1);
    tris.forEach((t, k) => {
      const P = [pos[2 * t[0]], pos[2 * t[0] + 1], pos[2 * t[1]], pos[2 * t[1] + 1], pos[2 * t[2]], pos[2 * t[2] + 1]];
      if (perEl) {
        const v = f.v[Math.floor(k / per)];
        if (d.key === "sf" && !(v > 0)) { (unknown ??= new Path2D()); poly(unknown, P); return; }
        poly(bands[band(v)], P); return;
      }
      const raw = t.map(i => f.v[i]);
      if (d.key === "sf" && raw.some(v => !(v > 0))) { (unknown ??= new Path2D()); poly(unknown, P); return; }
      const V = raw.map(v => Math.max(lo, Math.min(hi, v)));
      const b0 = band(Math.min(...V)), b1 = band(Math.max(...V));
      if (b0 === b1 || span <= 0) { poly(bands[b0], P); return; }
      for (let b = b0; b <= b1; b++) {
        let [p, v] = [P, V];
        if (b > b0) [p, v] = clip(p, v, lv(b), true);
        if (b < b1) [p, v] = clip(p, v, lv(b + 1), false);
        poly(bands[b], p);
      }
    });
  }
  const { lines, outline } = meshPaths(ms, pos);
  return { pos, bands, colours, unknown, lines, outline, range, grid: gridOf(pos, tris) };
};

/** The mesh coloured by element quality (smallest angle): fine over 30°, fair 20–30°, poor under 20°. */
export const buildMeshPicture = (ms: Mesh) => {
  const pos = new Float64Array(ms.nodes.flatMap(p => p));
  const q = [new Path2D(), new Path2D(), new Path2D()];
  const count = [0, 0, 0];
  for (const t of ms.tris) {
    const p = [0, 1, 2].map(i => ms.nodes[t[i]]);
    const l = [0, 1, 2].map(i => Math.hypot(p[(i + 1) % 3][0] - p[i][0], p[(i + 1) % 3][1] - p[i][1]));
    let a = 180;
    for (let i = 0; i < 3; i++) { const [x, y, z] = [l[i], l[(i + 1) % 3], l[(i + 2) % 3]]; a = Math.min(a, Math.acos(Math.max(-1, Math.min(1, (x * x + y * y - z * z) / (2 * x * y)))) * 180 / Math.PI); }
    const k = a >= 30 ? 0 : a >= 20 ? 1 : 2;
    count[k]++;
    const ring = t.length === 6 ? [t[0], t[3], t[1], t[4], t[2], t[5]] : t;
    poly(q[k], ring.flatMap(i => [pos[2 * i], pos[2 * i + 1]]));
  }
  return { pos, quality: q, count, ...meshPaths(ms, pos) };
};

const css = (v: string) => getComputedStyle(document.documentElement).getPropertyValue(v).trim() || "#333";

/** Paint a picture with model → pixel transform (scale s, origin at px, py; y up). */
export const paint = (ctx: CanvasRenderingContext2D, pic: Picture, s: number, px: number, py: number, dpr: number, lines: boolean) => {
  ctx.setTransform(dpr * s, 0, 0, -dpr * s, dpr * px, dpr * py);
  ctx.lineJoin = "round";
  // Each band is filled and stroked thinly in its own colour, so no hairline seams show between triangles.
  pic.bands.forEach((b, k) => { ctx.fillStyle = pic.colours[k]; ctx.strokeStyle = pic.colours[k]; ctx.lineWidth = 0.7 / s; ctx.fill(b); ctx.stroke(b); });
  if (pic.unknown) { ctx.fillStyle = "#9aa3ad"; ctx.fill(pic.unknown); }
  if (lines) { ctx.strokeStyle = "rgba(15,20,30,0.38)"; ctx.lineWidth = 0.6 / s; ctx.stroke(pic.lines); }
  ctx.strokeStyle = "#1B2733"; ctx.lineWidth = 1.2 / s; ctx.stroke(pic.outline);
};

// ---------- on the canvas ----------
const picture = createMemo(() => {
  const r = partRes(); if (!r) return null;
  const d = curField();
  return buildPicture(r, d, isElem(d), deformOf(r), palette(opts().contours, d.reverse));
});
const meshPicture = createMemo(() => { const ms = meshed() ?? partRes()?.mesh; return ms ? buildMeshPicture(ms) : null; });
/** Results drawn now: parts solved, in the model or results view (the Mesh view shows the mesh). */
const showResults = () => !!partRes() && view() !== "mesh" && !modeOn();
/** A part's mode drawn at a step of its swing (16 steps, built when first needed). */
const modeCache = new Map<string, Picture>();
createEffect(() => { modes(); opts().contours; show().scale; modeCache.clear(); });
const modePic = () => {
  const r = modes(); if (!r || r.kind !== "plane" || !modeOn() || view() === "mesh") return null;
  const k = modeSel(), step = Math.round(phase() * 8), key = `${k}:${step}`;
  let pic = modeCache.get(key);
  if (!pic) {
    const md = r.modes[k], v = md.disp.map(d => Math.hypot(d[0], d[1]));
    const pseudo = { kind: "plane", mesh: r.mesh, draw: r.draw, disp: md.disp, fields: [{ key: "umag", v, min: 0, max: Math.max(...v), at_min: [0, 0], at_max: [0, 0] }], elem: [] } as unknown as PartResults;
    pic = buildPicture(pseudo, fieldDef("umag"), false, modeAmp() * step / 8, palette(opts().contours));
    modeCache.set(key, pic);
  }
  return pic;
};

/** The canvas under the drawing: contours, or the mesh in the Mesh view. */
export const PartLayer = (p: { size: [number, number] }) => {
  let cv!: HTMLCanvasElement;
  createEffect(() => {
    const [w, h] = p.size, dpr = window.devicePixelRatio || 1, s = scale(), [px, py] = pan();
    themeTick();
    if (!cv) return;
    cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr);
    const ctx = cv.getContext("2d"); if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, cv.width, cv.height);
    if (view() === "mesh") {
      const mp = meshPicture(); if (!mp) return;
      ctx.setTransform(dpr * s, 0, 0, -dpr * s, dpr * px, dpr * py);
      if (show().quality) ["rgba(31,122,77,0.10)", "rgba(196,138,22,0.45)", "rgba(200,40,40,0.55)"].forEach((c, k) => { ctx.fillStyle = c; ctx.fill(mp.quality[k]); });
      ctx.strokeStyle = css("--ink2"); ctx.lineWidth = 0.7 / s; ctx.stroke(mp.lines);
      ctx.strokeStyle = css("--ink"); ctx.lineWidth = 1.4 / s; ctx.stroke(mp.outline);
      return;
    }
    const mp = modePic();
    if (mp) { paint(ctx, mp, s, px, py, dpr, show().mesh); return; }
    const pic = picture(); if (!pic || !showResults()) return;
    paint(ctx, pic, s, px, py, dpr, show().mesh);
  });
  return <canvas ref={cv} class="contours" classList={{ stale: modeOn() ? modesStale() : stale() && view() !== "mesh", on: view() === "mesh" ? !!meshPicture() : showResults() || !!modePic() }} style={{ width: `${p.size[0]}px`, height: `${p.size[1]}px` }} />;
};

/** Where the largest and smallest values are, marked on the drawing. */
export const PartMarkers = () => {
  const marks = createMemo(() => {
    const r = partRes(), pic = picture(); if (!r || !pic || !showResults()) return [];
    const d = curField(), el = isElem(d), f = fieldOf(r, d.key, el)!;
    let imin = -1, imax = -1;
    f.v.forEach((v, i) => { if (d.key === "sf" && !(v > 0)) return; if (imin < 0 || v < f.v[imin]) imin = i; if (imax < 0 || v > f.v[imax]) imax = i; });
    if (imin < 0) return [];
    const at = (i: number): Pt => {
      if (!el || f === r.fields.find(x => x.key === d.key)) return [pic.pos[2 * i], pic.pos[2 * i + 1]];
      const t = r.mesh.tris[i]; return [(pic.pos[2 * t[0]] + pic.pos[2 * t[1]] + pic.pos[2 * t[2]]) / 3, (pic.pos[2 * t[0] + 1] + pic.pos[2 * t[1] + 1] + pic.pos[2 * t[2] + 1]) / 3];
    };
    return [["max", at(imax), f.v[imax]], ["min", at(imin), f.v[imin]]] as [string, Pt, number][];
  });
  // When the two marks are close, the smallest's label goes to the left so they do not overlap.
  const close = () => { const ms = marks(); if (ms.length < 2) return false; const a = toS(ms[0][1]), b = toS(ms[1][1]); return Math.abs(a[0] - b[0]) < 170 && Math.abs(a[1] - b[1]) < 18; };
  return <For each={marks()}>{([k, p, v]) => { const s = () => toS(p), left = () => (k === "min" && close()) || s()[0] > (document.querySelector(".canvas")?.clientWidth ?? 1e9) - 170; return <g class={`mm ${k}`}><path d={`M${s()[0]} ${s()[1]}l-5 -9h10z`} /><text x={s()[0] + (left() ? -8 : 8)} y={s()[1] - 8} text-anchor={left() ? "end" : "start"}>{k} {fmt(curField().q, v, 4)}</text></g>; }}</For>;
};

/** Values under the pointer, from the drawing triangle it is over. */
export const partProbe = createMemo(() => {
  const r = partRes(), pic = picture(), c = cursor(); if (!r || !pic || !c || !showResults()) return null;
  const g = pic.grid, i = Math.floor((c[0] - g.x0) / g.cell), j = Math.floor((c[1] - g.y0) / g.cell);
  if (i < 0 || j < 0 || i >= g.nx || j >= g.ny) return null;
  for (const k of g.cells.get(i * g.ny + j) ?? []) {
    const t = r.draw[k], P = t.map(n => [pic.pos[2 * n], pic.pos[2 * n + 1]]);
    const det = (P[1][0] - P[0][0]) * (P[2][1] - P[0][1]) - (P[2][0] - P[0][0]) * (P[1][1] - P[0][1]);
    const l1 = ((P[1][0] - c[0]) * (P[2][1] - c[1]) - (P[2][0] - c[0]) * (P[1][1] - c[1])) / det;
    const l2 = ((P[2][0] - c[0]) * (P[0][1] - c[1]) - (P[0][0] - c[0]) * (P[2][1] - c[1])) / det;
    const l3 = 1 - l1 - l2, eps = -1e-9;
    if (l1 < eps || l2 < eps || l3 < eps) continue;
    const el = Math.floor(k / (r.mesh.order === 2 ? 4 : 1));
    const val = (key: string) => {
      const d = fieldDef(key);
      if (isElem(d)) { const ef = r.elem.find(x => x.key === key); if (ef) return ef.v[el]; }
      const f = r.fields.find(x => x.key === key); return f ? l1 * f.v[t[0]] + l2 * f.v[t[1]] + l3 * f.v[t[2]] : NaN;
    };
    // The point in the undeformed part (what a person measures on the drawing).
    const ms = r.mesh.nodes, at: Pt = [l1 * ms[t[0]][0] + l2 * ms[t[1]][0] + l3 * ms[t[2]][0], l1 * ms[t[0]][1] + l2 * ms[t[1]][1] + l3 * ms[t[2]][1]];
    const keys = r.kind === "heat" ? ["t", "qmag"] : ["vm", "s1", "s2", "umag"];
    const cur = curField().key;
    return { at, el, rows: [cur, ...keys.filter(k => k !== cur)].map(k => [k, val(k)] as [string, number]) };
  }
  return null;
});

export const PartProbe = () => (
  <Show when={partProbe()}>{p => <div class="probe"><b>At {fmt("coord", p().at[0])}, {fmt("coord", p().at[1])}</b>
    <For each={p().rows}>{([k, v]) => { const d = fieldDef(k); return <span>{d.short} {k === "sf" && !(v > 0) ? "—" : fmt(d.q, v, 4)}</span>; }}</For></div>}</Show>
);

// ---------- the legend ----------
export const ColourBar = (p: { d: FieldDef; range: [number, number] | null; colours: string[] }) => (
  <Show when={p.range} fallback={<span class="muted">{p.d.key === "sf" ? "No yield strengths to compare with: set them in Home › Materials." : "No values."}</span>}>{rg => {
    const tick = (k: number) => rg()[0] + (rg()[1] - rg()[0]) * k / BANDS;
    const label = (k: number) => num(clean(+fmt(p.d.q, tick(k), 3).split(" ")[0].replace("−", "-"), 1), 3);
    return <span class="cbar">
      <span class="cb">{p.colours.map((c, k) => <i style={{ background: c }}><Show when={k % 2 === 0}><small>{label(k)}</small></Show></i>)}<b><small>{label(BANDS)}{p.d.key === "sf" && rg()[1] >= 10 ? "+" : ""}</small></b></span>
      <em>{unit(p.d.q)}</em>
    </span>;
  }}</Show>
);

export const PartLegend = () => {
  const r = () => partRes();
  const d = () => curField();
  return (
    <Show when={r() && view() !== "mesh" && !modeOn()}>
      <div class="legend">
        <Show when={stale()}><div class="stalebar"><Icon n="info" /> The model changed since it was solved. <button class="btn sm pri" onClick={() => import("./cmds").then(c => c.C.solve())}>Solve again (F5)</button></div></Show>
        <Show when={solvedFor()}><div><b>Load case</b><span>{solvedFor()}</span></div></Show>
        <div class="lrow">
          <select class="fpick" value={d().key} onChange={e => setShow({ ...show(), field: e.currentTarget.value })} title="The result to colour the part by">
            <For each={fieldsFor(r()!.kind === "heat" ? "heat" : model()!.analysis)}>{f => <option value={f.key} selected={f.key === d().key}>{f.name}</option>}</For>
          </select>
          <ColourBar d={d()} range={picture()?.range ?? null} colours={palette(opts().contours, d().reverse)} />
        </div>
        <div class="lrow small">
          <span class="seg"><button classList={{ on: !show().elem }} onClick={() => setShow({ ...show(), elem: false })} title="Values averaged where elements meet">Smoothed</button><button classList={{ on: show().elem }} disabled={!!d().nodal} onClick={() => setShow({ ...show(), elem: true })} title="Each element's own value: where these differ much from the smoothed picture, the mesh is too coarse">Per element</button></span>
          <label class="chk"><input type="checkbox" checked={show().mesh} onChange={e => setShow({ ...show(), mesh: e.currentTarget.checked })} />Mesh lines</label>
          <Show when={r()!.kind === "plane"}>
            <label class="chk"><input type="checkbox" checked={show().deformed} onChange={e => setShow({ ...show(), deformed: e.currentTarget.checked })} />Deformed shape</label>
            <Show when={show().deformed}><span class="muted">{deformOf(r()) > 0 ? `× ${num(deformOf(r()), 3)}` : "nothing moves"}</span><input type="range" min="0" max="5" step="0.1" value={show().scale} onInput={e => setShow({ ...show(), scale: +e.currentTarget.value })} title="Exaggeration" /></Show>
          </Show>
          <span class="muted">{r()!.mesh.tris.length.toLocaleString()} elements · {r()!.mesh.nodes.length.toLocaleString()} nodes</span>
        </div>
        <div class="eqline" classList={{ bad: r()!.imbalance > 1e-6 }}><Icon n={r()!.imbalance > 1e-6 ? "info" : "check"} /><span>{r()!.kind === "heat" ? (r()!.imbalance > 1e-6 ? "Heat in and out do not balance" : "Heat balance: what flows in flows out") : r()!.imbalance > 1e-6 ? "Reactions do not balance the loads" : "Equilibrium: reactions balance the loads"}</span><button class="lnk" onClick={() => setView("results")}>Details</button></div>
      </div>
    </Show>
  );
};

/** Under the canvas in the Mesh view: what the mesh is and how good its triangles are. */
export const MeshBar = (p: { remesh: () => void }) => {
  const ms = () => meshed() ?? partRes()?.mesh ?? null, mp = () => meshPicture();
  return (
    <div class="legend">
      <Show when={ms()} fallback={<div class="lrow"><span>The part has not been meshed yet.</span><button class="btn sm pri" onClick={p.remesh}>Mesh</button></div>}>{m => <>
        <Show when={meshStale() || (!meshed() && stale())}><div class="stalebar"><Icon n="info" /> The model changed since it was meshed. <button class="btn sm pri" onClick={p.remesh}>Mesh again</button></div></Show>
        <div class="lrow"><b>{m().tris.length.toLocaleString()} {m().order === 2 ? "6-node" : "3-node"} triangles · {m().nodes.length.toLocaleString()} nodes</b>
          <span class="muted">size {fmt("len", m().size)} · smallest angle {num(m().quality.min_angle, 3)}° · worst aspect {num(m().quality.max_aspect, 3)} · {num(m().quality.good * 100, 3)} % over 30°</span></div>
        <div class="lrow small"><label class="chk"><input type="checkbox" checked={show().quality} onChange={e => setShow({ ...show(), quality: e.currentTarget.checked })} />Colour by quality</label>
          <Show when={show().quality && mp()}><span class="ukey"><i style={{ background: "rgba(31,122,77,0.35)" }} />over 30° ({mp()!.count[0]})<i style={{ background: "rgba(196,138,22,0.75)" }} />20–30° ({mp()!.count[1]})<i style={{ background: "rgba(200,40,40,0.8)" }} />under 20° ({mp()!.count[2]})</span></Show>
          <span class="grow" /><button class="btn sm" onClick={p.remesh}>Mesh again</button></div>
      </>}</Show>
    </div>
  );
};

// ---------- the results page ----------
const supName = (m: Model, id: number) => itemName(m, id) || "A support";

export const PartResultsPage = (p: { r: PartResults }) => {
  const m = () => model()!;
  const fields = () => fieldsFor(p.r.kind === "heat" ? "heat" : m().analysis).filter(d => d.key !== "sf" || p.r.fields.find(f => f.key === "sf")!.max > 0);
  return <>
    <Show when={p.r.kind === "plane"}>{_ => { const r = p.r as PlaneResults; return <>
      <h2>Equilibrium check <Help k="equilibrium" /></h2>
      <p classList={{ okline: r.imbalance <= 1e-6, badline: r.imbalance > 1e-6 }}><Icon n={r.imbalance > 1e-6 ? "info" : "check"} /> {r.imbalance > 1e-6 ? `The reactions miss the loads by ${num(r.imbalance * 100, 2)} %.` : `The reactions balance the loads (difference ${num(r.imbalance * 100, 2)} % of the load).`}</p>
      <EqTable applied={r.applied} reacted={r.reacted} scale={r.scale} axi={m().analysis === "axi"} />
      <h2>Reactions</h2>
      <Show when={m().analysis !== "axi"} fallback={<table class="tbl"><thead><tr><th>Support</th><th class="r">Along the axis ({unit("force")})</th></tr></thead><tbody>
        <For each={r.reactions}>{x => <tr class="link" onClick={() => { setSel([x.support]); setView("model"); }}><td>{supName(m(), x.support)}</td><td class="r">{fmt("force", clean(x.fy, r.scale[0])).split(" ")[0]}</td></tr>}</For></tbody></table>}>
      <table class="tbl"><thead><tr><th>Support</th><th class="r">Fx ({unit("force")})</th><th class="r">Fy ({unit("force")})</th></tr></thead><tbody>
        <For each={r.reactions}>{x => <tr class="link" onClick={() => { setSel([x.support]); setView("model"); }}><td>{supName(m(), x.support)}</td><td class="r">{fmt("force", clean(x.fx, r.scale[0])).split(" ")[0]}</td><td class="r">{fmt("force", clean(x.fy, r.scale[0])).split(" ")[0]}</td></tr>}</For></tbody></table></Show>
      <Show when={m().analysis === "axi"}><p class="muted small">A turned solid: forces are totals for the full circle; the radial pushes round the circle cancel by themselves, so only the forces along the axis are summed.</p></Show>
      <Show when={m().analysis === "stress"}><p class="muted small">Forces are for the whole thickness ({fmt("len", m().thickness)}). Plane strain results are per metre of length.</p></Show>
      <Show when={m().analysis === "strain"}><p class="muted small">Plane strain: forces are per metre of the body's length.</p></Show>
    </>; }}</Show>
    <Show when={p.r.kind === "heat"}>{_ => { const r = p.r as HeatResults; const total = () => [...r.supports, ...r.loads].reduce((a, f) => a + f.q, 0), size = () => [...r.supports, ...r.loads].reduce((a, f) => a + Math.abs(f.q), 0); return <>
      <h2>Heat balance <Help k="equilibrium" /></h2>
      <p classList={{ okline: r.imbalance <= 1e-6, badline: r.imbalance > 1e-6 }}><Icon n={r.imbalance > 1e-6 ? "info" : "check"} /> {r.imbalance > 1e-6 ? `Heat in and out differ by ${num(r.imbalance * 100, 2)} %.` : `Heat flowing in equals heat flowing out (difference ${num(r.imbalance * 100, 2)} %).`}</p>
      <table class="tbl"><thead><tr><th>Where</th><th class="r">Heat into the part ({unit("power")})</th></tr></thead><tbody>
        <For each={[...r.supports, ...r.loads]}>{x => <tr class="link" onClick={() => { setSel([x.id]); setView("model"); }}><td>{supName(m(), x.id)}</td><td class="r">{fmt("power", clean(x.q, size())).split(" ")[0]}</td></tr>}</For>
        <tr><td><b>Sum</b></td><td class="r"><b>{fmt("power", clean(total(), size())).split(" ")[0]}</b></td></tr></tbody></table>
      <p class="muted small">For the thickness {fmt("len", m().thickness)}; negative values leave the part.</p>
    </>; }}</Show>
    <h2>Largest and smallest values</h2>
    <table class="tbl"><thead><tr><th>Result</th><th class="r">Smallest</th><th>at (x, y)</th><th class="r">Largest</th><th>at (x, y)</th></tr></thead><tbody>
      <For each={fields()}>{d => { const f = p.r.fields.find(x => x.key === d.key)!; const at = (q: Pt) => `${fmt("coord", q[0]).split(" ")[0]}, ${fmt("coord", q[1])}`; return <tr class="link" onClick={() => { setShow({ ...show(), field: d.key }); setView("model"); }}>
        <td>{d.name}</td><td class="r">{d.key === "sf" ? num(Math.min(...f.v.filter(v => v > 0)), 4) : fmt(d.q, clean(f.min, Math.max(Math.abs(f.min), Math.abs(f.max))))}</td><td class="muted">{at(f.at_min)}</td><td class="r">{d.key === "sf" ? (f.max >= 1e6 ? "—" : num(f.max, 4)) : fmt(d.q, clean(f.max, Math.max(Math.abs(f.min), Math.abs(f.max))))}</td><td class="muted">{at(f.at_max)}</td></tr>; }}</For></tbody></table>
    <p class="muted small">Smoothed values: stresses averaged where elements meet. Click a row to see that result on the part.</p>
    <h2>Mesh and solver</h2>
    <table class="tbl"><tbody>
      <tr><td>Elements</td><td class="r">{p.r.mesh.tris.length.toLocaleString()} {p.r.mesh.order === 2 ? "6-node" : "3-node"} triangles</td></tr>
      <tr><td>Nodes</td><td class="r">{p.r.mesh.nodes.length.toLocaleString()}</td></tr>
      <tr><td>Element size</td><td class="r">{fmt("len", p.r.mesh.size)}</td></tr>
      <tr><td>Smallest angle · worst aspect</td><td class="r">{num(p.r.mesh.quality.min_angle, 3)}° · {num(p.r.mesh.quality.max_aspect, 3)}</td></tr>
      <tr><td>Equations</td><td class="r">{p.r.stats.equations.toLocaleString()}</td></tr><tr><td>Stored stiffness entries</td><td class="r">{p.r.stats.nonzeros.toLocaleString()}</td></tr>
      <tr><td>Residual ‖Ku − f‖ / ‖f‖</td><td class="r">{p.r.stats.residual.toExponential(1)}</td></tr><tr><td>Time</td><td class="r">{num(p.r.stats.ms, 3)} ms</td></tr>
      <tr><td>Method</td><td>{p.r.kind === "heat" ? "Steady heat conduction" : m().analysis === "strain" ? "Plane strain" : "Plane stress"}, {p.r.mesh.order === 2 ? "6-node (quadratic)" : "3-node (linear)"} triangles; sparse Cholesky with a fill-reducing ordering</td></tr></tbody></table>
  </>;
};

/** Loads, reactions and their sums: forces in x and y, moments about the origin. */
export const EqTable = (p: { applied: number[]; reacted: number[]; scale: number[]; axi?: boolean }) => {
  const all: [string, number, number, "force" | "moment", number][] = [["Σ Fx", p.applied[0], p.reacted[0], "force", p.scale[0]], ["Σ Fy", p.applied[1], p.reacted[1], "force", p.scale[0]], ["Σ M about the origin", p.applied[2], p.reacted[2], "moment", p.scale[1]]];
  const rows = p.axi ? [["Σ F along the axis", p.applied[1], p.reacted[1], "force", p.scale[0]] as [string, number, number, "force" | "moment", number]] : all;
  return <table class="tbl"><thead><tr><th /><th class="r">Loads</th><th class="r">Reactions</th><th class="r">Sum</th></tr></thead><tbody>
    <For each={rows}>{([l, a, b, q, sc]) => <tr><td>{l}</td><td class="r">{fmt(q, clean(a, sc))}</td><td class="r">{fmt(q, clean(b, sc))}</td><td class="r">{fmt(q, clean(a + b, sc))}</td></tr>}</For></tbody></table>;
};

/** A results picture for print: the contours drawn to fit a box, as an image. */
export const contourImage = (r: PartResults, d: FieldDef, w: number, h: number, colours: string[]): string => {
  const pic = buildPicture(r, d, false, deformOf(r), colours), b = boundsOf(r.mesh);
  for (let i = 0; i < pic.pos.length; i += 2) { b[0] = Math.min(b[0], pic.pos[i]); b[2] = Math.max(b[2], pic.pos[i]); b[1] = Math.min(b[1], pic.pos[i + 1]); b[3] = Math.max(b[3], pic.pos[i + 1]); }
  const pad = 16, s = Math.min((w - 2 * pad) / Math.max(b[2] - b[0], 1e-12), (h - 2 * pad) / Math.max(b[3] - b[1], 1e-12));
  const cv = document.createElement("canvas"), k = 2;
  cv.width = w * k; cv.height = h * k;
  const ctx = cv.getContext("2d"); if (!ctx) return "";
  paint(ctx, pic, s, (w - (b[2] - b[0]) * s) / 2 - b[0] * s, (h + (b[3] - b[1]) * s) / 2 + b[1] * s, k, false);
  return cv.toDataURL("image/png");
};

/** Field to show after solving: keep the person's pick when it fits the analysis. */
export const fieldAfterSolve = (a: string) => { const list = fieldsFor(a); if (!list.some(f => f.key === show().field)) setShow({ ...show(), field: list[0].key }); };
