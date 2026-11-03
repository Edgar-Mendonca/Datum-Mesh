// Sketching on a plane (0.6.2), loaded when a sketch starts. New Sketch asks for a plane (the
// three squares at the origin, in the 3D view); the view turns to face it and the sketch canvas lies
// over the 3D view, lined up with it, so the rest of the model stays in sight. For a 3D solid each
// sketch is a part's (a new part, placed on the plane); for a 3D frame, lines on the plane are
// members. Finish keeps the sketch, Cancel undoes it. The view can be turned away at any time; Look
// At turns it back square to the sketch.
import { Show, createSignal } from "solid-js";
import { Part, num, parseNum } from "./api";
import { model, sketch, setSketch, setSketchPick, setActivePart, setSel, setTool, act, toast, setTab, tool, setSweepOn } from "./store";
import type { PartHit } from "./modelling";
import { setPlane, setDepth, depths } from "./plane3";
import { canvasPlace, lastPlaced } from "./canvas";
import { fmt, unit, fromU, toU } from "./units";
import { C } from "./cmds";

export type WP = "xy" | "zy" | "xz";
type V3 = [number, number, number];
/** The planes as shown (Z up): name, axes, the shown depth axis. */
export const PLANE_NAME: Record<WP, [string, string, string]> = { xy: ["Front", "X–Z", "Y"], zy: ["Side", "Y–Z", "X"], xz: ["Top", "X–Y", "Z"] };
/** A new part's turn so its sketch lies on the plane, extruded out of it (towards you). */
const TURN: Record<WP, V3> = { xy: [0, 0, 0], zy: [0, Math.PI / 2, 0], xz: [-Math.PI / 2, 0, 0] };

/** A point of a part's sketch space placed in the assembly (as the engine's `Part::place`). */
export const placeOn = (p: Pick<Part, "at" | "turn">, q: V3): V3 => {
  const [sx, cx, sy, cy, sz, cz] = [Math.sin(p.turn[0]), Math.cos(p.turn[0]), Math.sin(p.turn[1]), Math.cos(p.turn[1]), Math.sin(p.turn[2]), Math.cos(p.turn[2])];
  const a: V3 = [q[0], q[1] * cx - q[2] * sx, q[1] * sx + q[2] * cx];
  const b: V3 = [a[0] * cy + a[2] * sy, a[1], -a[0] * sy + a[2] * cy];
  const c: V3 = [b[0] * cz - b[1] * sz, b[0] * sz + b[1] * cz, b[2]];
  return [c[0] + p.at[0], c[1] + p.at[1], c[2] + p.at[2]];
};

/** The sketch's plane in the engine's axes: origin, across (u), up (v), and its normal (towards
 * whoever looks square at the sketch canvas). */
export const sketchFrame = (): { O: V3; U: V3; V: V3; N: V3 } | null => {
  const sk = sketch(), m = model(); if (!sk || !m) return null;
  const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  let O: V3, U: V3, V: V3;
  if (sk.kind === "part") {
    const p = m.parts?.find(x => x.id === sk.part); if (!p) return null;
    O = placeOn(p, [0, 0, 0]); U = sub(placeOn(p, [1, 0, 0]), O); V = sub(placeOn(p, [0, 1, 0]), O);
  } else {
    // The canvas's (u, v) on a working plane at a depth d (plane3.ts `unproj`).
    const d = sk.depth, at = (u: number, v: number): V3 => sk.plane === "zy" ? [d, v, u] : sk.plane === "xz" ? [u, d, -v] : [u, v, d];
    O = at(0, 0); U = sub(at(1, 0), O); V = sub(at(0, 1), O);
  }
  return { O, U, V, N: cross(U, V) };
};

/** Start a sketch on plane k (picked in the 3D view). */
export const beginSketch = async (k: WP) => {
  const m = model(); if (!m) return;
  setSketchPick(false);
  if (m.analysis === "space") {
    const d = 0;
    setPlane(k); setDepth(d);
    setSketch({ kind: "frame", part: 0, isNew: false, plane: k, depth: d, facing: true, edits: 0 });
    if (!["beam", "truss", "node"].includes(tool())) setTool("beam");
  } else {
    const depth = Math.max((m.grid > 0 ? m.grid : 0.005) * 10, 0.01);
    const o = await act({ Add: { item: { Part: { id: 0, name: "", depth, angle: 0, material: m.materials[0]?.id ?? 0, at: [0, 0, 0], turn: TURN[k], size: 0 } } } });
    const id = o?.created[0]; if (!id) return;
    setActivePart(id); setPlane("xy");
    setSketch({ kind: "part", part: id, isNew: true, plane: k, depth: 0, facing: true, edits: 0 });
    setTool("rect");
  }
  setTab("sketch");
  import("./canvas").then(c => setTimeout(() => c.fit(), 30));
};

/** Start a sketch on a flat face of a part (0.6.3), clicked while picking a plane: a new part on
 * that face, its sketch square to it and its extrusion going out of it. On the front or back face
 * the new sketch keeps the part's own axes, so Finish can cut it through the part instead. */
export const beginSketchOnFace = async (h: PartHit) => {
  const m = model(), p = m?.parts?.find(x => x.id === h.part); if (!m || !p) return;
  if (p.angle > 0) { toast("Sketch on a flat face of an extruded part; a turned part's faces are curved (sketch on a plane at the origin instead)."); return; }
  const { rotOf, turnOf } = await import("./modelling");
  const R = rotOf(p.turn), col = (j: number): V3 => [R[0][j], R[1][j], R[2][j]];
  const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  let U: V3, V: V3, N: V3, O: V3;
  const end = h.target?.on === "partend";
  if (h.target?.on === "partend") {
    const front = h.target.front, f = p.from ?? 0;
    U = col(0); V = front ? col(1) : col(1).map(c => -c) as V3; N = front ? col(2) : col(2).map(c => -c) as V3;
    O = placeOn(p, [0, 0, front ? f + p.depth : f]);
  } else {
    // A side face: across along the face, up along the part's depth, out of the face.
    N = h.normal; V = col(2);
    const along = dot(V, N); V = V.map((c, i) => c - along * N[i]) as V3;
    const nv = Math.hypot(...V); if (nv < 1e-9) { toast("Sketch on a flat face of the part."); return; }
    V = V.map(c => c / nv) as V3; U = cross(V, N);
    const P0 = placeOn(p, [0, 0, p.from ?? 0]), s = dot([h.point[0] - P0[0], h.point[1] - P0[1], h.point[2] - P0[2]], N);
    O = [P0[0] + s * N[0], P0[1] + s * N[1], P0[2] + s * N[2]];
  }
  const turn = turnOf([[U[0], V[0], N[0]], [U[1], V[1], N[1]], [U[2], V[2], N[2]]]);
  setSketchPick(false);
  const depth = Math.max((m.grid > 0 ? m.grid : 0.005) * 10, 0.01);
  const o = await act({ Add: { item: { Part: { id: 0, name: "", depth, angle: 0, material: p.material, at: O, turn, size: 0 } } } });
  const id = o?.created[0]; if (!id) return;
  setActivePart(id); setPlane("xy");
  setSketch({ kind: "part", part: id, isNew: true, plane: "xy", depth: 0, facing: true, edits: 0, face: { of: p.id, end }, base: O });
  setTool(end ? "circle" : "rect"); setTab("sketch");
  import("./canvas").then(c => setTimeout(() => c.fit(), 30));
};

/** New Sketch: pick a plane in the 3D view. */
export const newSketch = () => { setPlane("3d"); setSketch(null); setSketchPick(true); setTool("select"); toast(model()?.parts?.length ? "Pick a plane to sketch on: a square at the origin, or a flat face of a part." : "Pick a plane to sketch on: click one of the three squares at the origin.", true); };

/** Change an existing part's sketch, on the part's own plane. */
export const editSketch = (part: number) => {
  setSketchPick(false); setActivePart(part); setPlane("xy");
  setSketch({ kind: "part", part, isNew: false, plane: "xy", depth: 0, facing: true, edits: 0 });
  setTool("select"); setTab("sketch");
  import("./canvas").then(c => setTimeout(() => c.fit(), 30));
};

/** Keep the sketch: a new part opens the Extrude panel. An empty new part is not kept. With
 * `cut`, a sketch on a part's front or back face is cut right through that part instead. */
export const finishSketch = async (cut = false) => {
  const sk = sketch(); if (!sk) return;
  setTool("select");
  if (sk.kind === "part") {
    const m = model(), any = m?.shapes.some(s => s.part === sk.part && !s.hole);
    if (!any && sk.isNew) { await act({ Delete: { ids: [sk.part] } }); setActivePart(0); setSketch(null); setPlane("3d"); setTab("geometry"); toast("The sketch was empty, so no part was made."); return; }
    if (cut && sk.face) {
      const o = await act({ Cut: { tool: sk.part, from: sk.face.of } });
      if (!o) return;
      setActivePart(sk.face.of); setSketch(null); setPlane("3d"); setTab("geometry"); setSel([sk.face.of]);
      toast("Cut: the sketch's outlines go right through the part as holes.", true);
      return;
    }
  }
  setSketch(null); setPlane("3d"); setTab("geometry");
  if (sk.kind === "part") {
    setSel([sk.part]);
    if (sk.isNew) setSweepOn(sk.part);
    else toast("Sketch done.", true);
  }
};

/** Throw the sketch away: every edit made while sketching is undone (a new part goes with them). */
export const cancelSketch = async () => {
  const sk = sketch(); if (!sk) return;
  const n = sk.edits + (sk.isNew ? 1 : 0);
  // Undo first, then close: the 3D view that comes back is built from the model as it was.
  setTool("select");
  for (let i = 0; i < n; i++) await C.undo();
  if (sk.isNew) setActivePart(0);
  setSketch(null); setPlane("3d"); setTab("geometry");
};

/** Turn the view away from the sketch (to look round), or back square to it. */
export const lookAt = (facing: boolean) => { const sk = sketch(); if (sk) setSketch({ ...sk, facing }); };

/** The bar above the view while sketching: the plane, its offset, a typed point, Look At, Finish, Cancel. */
export const SketchBar = () => {
  const sk = () => sketch()!;
  const [typed, setTyped] = createSignal("");
  const part = () => model()?.parts?.find(p => p.id === sk().part);
  /** Move the sketch plane along its normal: a frame's working plane, or a new part's place. */
  const setOffset = (d: number) => {
    const s = sk();
    if (s.kind === "frame") { setDepth(d); setSketch({ ...s, depth: d }); return; }
    const p = part(), f = sketchFrame(); if (!p || !f) return;
    const n = Math.hypot(...f.N), N = f.N.map(c => c / n) as V3, b = s.base ?? [0, 0, 0];
    act({ Set: { item: { Part: { ...p, at: [b[0] + N[0] * d, b[1] + N[1] * d, b[2] + N[2] * d] } } } });
  };
  /** The offset as shown: along the shown axis of the plane's normal (out of the face, on a face). */
  const shownDepth = () => {
    const s = sk(); if (s.kind === "frame") return s.plane === "xy" ? -s.depth || 0 : s.depth;
    const p = part(); if (!p) return 0;
    if (s.face) { const f = sketchFrame(), b = s.base ?? [0, 0, 0]; if (!f) return 0; const n = Math.hypot(...f.N); return ((p.at[0] - b[0]) * f.N[0] + (p.at[1] - b[1]) * f.N[1] + (p.at[2] - b[2]) * f.N[2]) / n || 0; }
    return (s.plane === "xy" ? -p.at[2] : s.plane === "zy" ? p.at[0] : p.at[1]) || 0;
  };
  const fromShown = (v: number) => (sk().plane === "xy" && !sk().face ? -v : v);
  const faceOf = () => model()?.parts?.find(p => p.id === sk().face?.of)?.name ?? "a part";
  const place = async () => {
    const t = typed().trim(), rel = t.startsWith("@"), v = t.replace("@", "").split(/[\s,;]+/).filter(Boolean).map(parseNum);
    if (v.length !== 2 || v.some(x => x === null)) { toast(`Type a point on the sketch as across, up (or @d-across, d-up from the last point), in ${unit("coord")}.`); return; }
    const q = v.map(x => fromU("coord", x!)) as [number, number], last = lastPlaced() ?? [0, 0];
    await canvasPlace.run?.(rel ? [last[0] + q[0], last[1] + q[1]] : q);
    setTyped("");
  };
  return (
    <div class="sketchbar" onPointerDown={e => e.stopPropagation()}>
      <b>Sketch</b><span class="muted">{sk().kind === "part" ? part()?.name ?? "Part" : "3D frame"} · {sk().face ? `on a face of ${faceOf()}` : sk().isNew || sk().kind === "frame" ? `${PLANE_NAME[sk().plane][0]} plane (${PLANE_NAME[sk().plane][1]})` : "its own plane"}</span>
      <Show when={sk().isNew || sk().kind === "frame"}>
        <label>{sk().face ? "Offset" : `${PLANE_NAME[sk().plane][2]} =`}</label>
        <input class="mono num" value={num(toU("coord", shownDepth()), 6)} title={sk().face ? `Move the sketch plane out of the face (${unit("coord")})` : `Move the sketch plane along ${PLANE_NAME[sk().plane][2]} (${unit("coord")})`} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null) setOffset(fromShown(fromU("coord", v))); }} />
        <Show when={sk().kind === "frame" && depths().length > 1}>
          <select title="Planes that have nodes on them" onChange={e => setOffset(+e.currentTarget.value)}><option value="">planes with nodes…</option>{depths().map(d => <option value={String(d)}>{fmt("coord", sk().plane === "xy" ? -d || 0 : d)}</option>)}</select>
        </Show>
      </Show>
      <input class="mono pt2" placeholder="across, up  or  @d, d" value={typed()} title={`Type a point on the sketch (${unit("coord")}) and press Enter: across, up, or @d-across, d-up from the last point`} onInput={e => setTyped(e.currentTarget.value)} onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") place(); }} />
      <span class="grow" />
      <Show when={sk().facing} fallback={<button class="btn sm" title="Turn the view square to the sketch again, to draw" onClick={() => lookAt(true)}>Look At</button>}>
        <button class="btn sm" title="Turn the view freely to look round the model; Look At comes back to the sketch" onClick={() => lookAt(false)}>Turn the View</button>
      </Show>
      <button class="btn sm" title="Throw this sketch away" onClick={cancelSketch}>Cancel</button>
      <Show when={sk().face?.end}><button class="btn sm" title={`Cut the sketch's outlines right through ${faceOf()}, as holes`} onClick={() => finishSketch(true)}>Cut Through</button></Show>
      <button class="btn sm pri" title="Keep the sketch: a new part, then set its extrusion" onClick={() => finishSketch()}>Finish Sketch</button>
    </div>
  );
};
