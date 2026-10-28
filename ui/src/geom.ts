// Plane geometry for drawing: arcs from bulges, edges as points, normals and SVG paths. The same
// rules as crates/core/src/model.rs (Geom): segment i runs from pts[i] to pts[i+1], bulging into an
// arc when bulge[i] ≠ 0 (bulge = tan(θ/4), θ > 0 counter-clockwise).
import { Geom, Pt } from "./api";

/** Centre, radius, start angle and swept angle (signed) of a bulged segment. */
export const arc = (p: Pt, q: Pt, b: number) => {
  const th = 4 * Math.atan(b), dx = q[0] - p[0], dy = q[1] - p[1], c = Math.hypot(dx, dy);
  const r = c / Math.abs(2 * Math.sin(th / 2)), d = c / 2 / Math.tan(th / 2);
  const cen: Pt = [(p[0] + q[0]) / 2 - dy / c * d, (p[1] + q[1]) / 2 + dx / c * d];
  return { cen, r, a0: Math.atan2(p[1] - cen[1], p[0] - cen[0]), th };
};

export const edgeCount = (g: Geom) => (g.kind === "circle" ? 1 : g.pts.length);

/** Points along edge i, at most `seg` apart (world units). */
export const edgePts = (g: Geom, i: number, seg: number): Pt[] => {
  if (g.kind === "circle") {
    const n = Math.min(720, Math.max(24, Math.ceil(2 * Math.PI * g.r / seg)));
    return Array.from({ length: n + 1 }, (_, k) => { const t = k / n * 2 * Math.PI; return [g.c[0] + g.r * Math.cos(t), g.c[1] + g.r * Math.sin(t)] as Pt; });
  }
  const p = g.pts[i], q = g.pts[(i + 1) % g.pts.length], b = g.bulge[i] ?? 0;
  if (!b) return [p, q];
  const { cen, r, a0, th } = arc(p, q, b);
  const n = Math.min(360, Math.max(4, Math.ceil(r * Math.abs(th) / seg)));
  return Array.from({ length: n + 1 }, (_, k) => { const t = a0 + th * k / n; return [cen[0] + r * Math.cos(t), cen[1] + r * Math.sin(t)] as Pt; });
};

/** The whole outline as points. */
export const outline = (g: Geom, seg: number): Pt[] => g.kind === "circle" ? edgePts(g, 0, seg).slice(0, -1) : g.pts.flatMap((_, i) => edgePts(g, i, seg).slice(0, -1));

export const area = (pts: Pt[]) => pts.reduce((a, p, i) => { const q = pts[(i + 1) % pts.length]; return a + (p[0] * q[1] - q[0] * p[1]) / 2; }, 0);

/** 1 if the shape runs counter-clockwise, −1 if clockwise. */
export const orient = (g: Geom) => (g.kind === "circle" ? 1 : area(outline(g, Infinity)) >= 0 ? 1 : -1);

/** SVG path in screen coordinates (map turns world into screen; y points down there). */
export const svgPath = (g: Geom, map: (p: Pt) => Pt, scale: number) => {
  if (g.kind === "circle") {
    const [x, y] = map(g.c), r = g.r * scale;
    return `M${x - r} ${y}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0z`;
  }
  return g.pts.map((p, i) => {
    const q = g.pts[(i + 1) % g.pts.length], b = g.bulge[i] ?? 0, s = map(q);
    const head = i === 0 ? `M${map(p)[0]} ${map(p)[1]}` : "";
    if (!b) return `${head}L${s[0]} ${s[1]}`;
    const { r, th } = arc(p, q, b);
    // Flipping y keeps the picture's turn: a counter-clockwise arc still looks counter-clockwise,
    // which is SVG's negative angle direction (its y points down): sweep flag 0.
    return `${head}A${r * scale} ${r * scale} 0 ${Math.abs(th) > Math.PI ? 1 : 0} ${th > 0 ? 0 : 1} ${s[0]} ${s[1]}`;
  }).join("") + "z";
};

/** Distance from point p to the polyline. */
export const distTo = (pts: Pt[], p: Pt) => {
  let best = Infinity;
  for (let i = 0; i + 1 < pts.length; i++) {
    const [a, b] = [pts[i], pts[i + 1]], dx = b[0] - a[0], dy = b[1] - a[1], l = dx * dx + dy * dy;
    const t = l ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l)) : 0;
    best = Math.min(best, Math.hypot(a[0] + t * dx - p[0], a[1] + t * dy - p[1]));
  }
  return best;
};

/** Is p inside the closed outline (even-odd)? */
export const inside = (pts: Pt[], p: Pt) => {
  let c = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [a, b] = [pts[i], pts[j]];
    if ((a[1] > p[1]) !== (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]) c = !c;
  }
  return c;
};

/** The points that can be snapped to on a shape: corners and circle centres. */
export const keyPts = (g: Geom): Pt[] => (g.kind === "circle" ? [g.c] : g.pts);
