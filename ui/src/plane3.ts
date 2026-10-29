// 3D frames (0.2.4): the canvas draws on a working plane, each at a depth. Front is seen from +z
// (x right, y up); Side from −x (z right, y up); Plan from above (x right, z down the screen, so the
// screen's up is −z). The
// canvas sees a projected copy of the model (the members on the plane, in plane coordinates u, v)
// and its edits are turned back into 3D here before they go to the engine. Members off the plane
// are drawn faint and cannot be picked; the 3D view shows everything.
import { createMemo, createSignal } from "solid-js";
import { Model, Node, Load, Pt, isFrame } from "./api";
import { model } from "./store";

export type Plane = "3d" | "xy" | "zy" | "xz";
/** What the model view shows for a 3D frame: the 3D view or a working plane. */
export const [plane, setPlane] = createSignal<Plane>("3d");
/** The working plane's depth (m): z for Front, x for Side, y for Plan. */
export const [depth, setDepth] = createSignal(0);
export const PLANES: [Plane, string, string][] = [["3d", "3D", "The whole frame in 3D: turn it with the left button, pan with the right, zoom with the wheel"], ["xy", "Front", "Front (x across, y up), at a depth z"], ["zy", "Side", "Side (z across, y up), at a depth x"], ["xz", "Plan", "Plan, seen from above (x across, z towards you down the screen), at a height y"]];
export const depthAxis = () => ({ xy: "z", zy: "x", xz: "y", "3d": "" })[plane()];

export const space = () => model()?.analysis === "space";
/** Drawing on a plane of a 3D frame (not the 3D view, not a plane frame). */
export const onPlane = () => space() && plane() !== "3d";

const SAME = 1e-7;
const xyz = (n: Node): [number, number, number] => [n.x, n.y, n.z ?? 0];
/** A 3D point on the working plane: (u, v) and its depth. */
export const proj = (p: [number, number, number]): [number, number, number] => {
  const [x, y, z] = p;
  switch (plane()) {
    case "zy": return [z, y, x];
    case "xz": return [x, -z, y];
    default: return [x, y, z];
  }
};
/** Plane coordinates back to 3D at the plane's depth. */
export const unproj = (u: number, v: number, d = depth()): [number, number, number] => {
  switch (plane()) {
    case "zy": return [d, v, u];
    case "xz": return [u, d, -v];
    default: return [u, v, d];
  }
};
const onIt = (n: Node) => Math.abs(proj(xyz(n))[2] - depth()) < SAME;

/** In-plane parts of a 3D vector (fx, fy, fz) on the working plane, and what is left across it. */
const inPlane = (v: [number, number, number]): [number, number, number] => proj(v);

/**
 * The model as the canvas sees it. For a 3D frame on a plane: the nodes on the plane in plane
 * coordinates, the members between them, and the supports and loads on those; force arrows show
 * their in-plane parts. Otherwise the model itself.
 */
export const planeModel = createMemo((): Model | undefined => {
  const m = model(); if (!m || !onPlane()) return m;
  const nodes = m.nodes.filter(onIt).map(n => { const [u, v] = proj(xyz(n)); return { ...n, x: u, y: v, z: 0 }; });
  const ids = new Set(nodes.map(n => n.id));
  const members = m.members.filter(b => ids.has(b.a) && ids.has(b.b));
  const mids = new Set(members.map(b => b.id));
  const keep = (t: { at: any }) => t.at.on === "node" ? ids.has(t.at.id) : t.at.on === "member" ? mids.has(t.at.id) : t.at.on === "body";
  const loads: Load[] = m.loads.filter(keep).map(l => {
    const k = l.kind;
    if (k.kind === "force") { const [a, b] = inPlane([k.fx, k.fy, k.fz ?? 0]); return { ...l, kind: { kind: "force", fx: a, fy: b } }; }
    // Counter-clockwise on the screen turns about the axis towards the viewer: +z, −x, +y.
    if (k.kind === "moment") { const mm = inPlane([k.mx ?? 0, k.my ?? 0, k.m])[2]; return { ...l, kind: { kind: "moment", m: plane() === "zy" ? -mm : mm } }; }
    if (k.kind === "dist") {
      // A global direction becomes the plane's u or v, or none when it points across the plane.
      const g: Record<string, [number, number, number]> = { x: [1, 0, 0], y: [0, 1, 0], z: [0, 0, 1] };
      if (k.dir === "local") return plane() === "xy" ? l : { ...l, kind: { ...k, dir: "z" } };
      const [a, b] = inPlane(g[k.dir]), sg = a || b || 1;
      return { ...l, kind: { ...k, w1: k.w1 * sg, w2: k.w2 * sg, dir: a ? "x" : b ? "y" : "z" } };
    }
    return l;
  });
  return { ...m, nodes, members, supports: m.supports.filter(keep), loads };
});

/** Members not on the plane, flattened onto it: drawn faint for orientation. */
export const ghosts = createMemo((): [Pt, Pt][] => {
  const m = model(); if (!m || !onPlane()) return [];
  const at = new Map(m.nodes.map(n => [n.id, n]));
  return m.members.flatMap(b => {
    const a = at.get(b.a), c = at.get(b.b); if (!a || !c || (onIt(a) && onIt(c))) return [];
    const p = proj(xyz(a)), q = proj(xyz(c));
    return [[[p[0], p[1]], [q[0], q[1]]] as [Pt, Pt]];
  });
});

/** The depths that have nodes on them, for the depth picker. */
export const depths = createMemo(() => {
  const m = model(); if (!m || !space() || plane() === "3d") return [] as number[];
  const ds = [...new Set(m.nodes.map(n => +proj(xyz(n))[2].toPrecision(12)))].sort((a, b) => a - b);
  return ds.length ? ds : [0];
});

/** An edit made on the plane, in 3D. Ops without coordinates pass through unchanged. */
export const lift = (op: any): any => {
  if (!onPlane() || !op || typeof op !== "object") return op;
  if (op.Batch) return { Batch: op.Batch.map(lift) };
  if (op.AddMember) {
    const { from, to } = op.AddMember, a = unproj(from[0], from[1]), b = unproj(to[0], to[1]);
    return { AddMember: { ...op.AddMember, from: [a[0], a[1]], to: [b[0], b[1]], z: [a[2], b[2]] } };
  }
  if (op.Move) { const [dx, dy, dz] = unproj(op.Move.dx, op.Move.dy, 0); return { Move: { ...op.Move, dx, dy, dz } }; }
  for (const k of ["Add", "Set"]) {
    const n = op[k]?.item?.Node;
    if (n) {
      // A node set from the canvas keeps its depth; a new one goes on the plane.
      const old = model()?.nodes.find(x => x.id === n.id);
      const d = old && k === "Set" ? proj(xyz(old))[2] : depth();
      const [x, y, z] = unproj(n.x, n.y, d);
      return { [k]: { item: { Node: { ...n, x, y, z } } } };
    }
  }
  return op;
};

/** Is the model one the canvas edits directly (a plane frame or a part), or a 3D frame on a plane? */
export const editable = () => { const m = model(); return !!m && (!isFrame(m.analysis) || m.analysis === "frame" || onPlane()); };

/** The 3D view registers its fit here, so Fit (F) works on either view. */
export const fit3d: { run: (() => void) | null } = { run: null };
