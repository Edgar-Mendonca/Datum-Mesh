// Sketching on a plane (0.6.2), loaded when a sketch starts. New Sketch asks for a plane (the
// three squares at the origin, in the 3D view); the view turns to face it and the sketch canvas lies
// over the 3D view, lined up with it, so the rest of the model stays in sight. For a 3D solid each
// sketch is a part's (a new part, placed on the plane); for a 3D frame, lines on the plane are
// members. Finish keeps the sketch, Cancel undoes it. The view can be turned away at any time; Look
// At turns it back square to the sketch.
import { Show, createSignal } from "solid-js";
import { Part, num, parseNum } from "./api";
import { model, sketch, setSketch, setSketchPick, setActivePart, setSel, setTool, act, toast, setTab, tool } from "./store";
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

/** New Sketch: pick a plane in the 3D view. */
export const newSketch = () => { setPlane("3d"); setSketch(null); setSketchPick(true); setTool("select"); toast("Pick a plane to sketch on: click one of the three squares at the origin.", true); };

/** Change an existing part's sketch, on the part's own plane. */
export const editSketch = (part: number) => {
  setSketchPick(false); setActivePart(part); setPlane("xy");
  setSketch({ kind: "part", part, isNew: false, plane: "xy", depth: 0, facing: true, edits: 0 });
  setTool("select"); setTab("sketch");
  import("./canvas").then(c => setTimeout(() => c.fit(), 30));
};

/** Keep the sketch. An empty new part is not kept. */
export const finishSketch = async () => {
  const sk = sketch(); if (!sk) return;
  setTool("select");
  if (sk.kind === "part") {
    const m = model(), any = m?.shapes.some(s => s.part === sk.part && !s.hole);
    if (!any && sk.isNew) { await act({ Delete: { ids: [sk.part] } }); setActivePart(0); setSketch(null); setPlane("3d"); setTab("geometry"); toast("The sketch was empty, so no part was made."); return; }
  }
  setSketch(null); setPlane("3d"); setTab("geometry");
  if (sk.kind === "part") {
    setSel([sk.part]);
    toast("Sketch done. Set how far it is extruded (or revolved) in the Inspector.", true);
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
    const n = Math.hypot(...f.N), N = f.N.map(c => c / n) as V3;
    act({ Set: { item: { Part: { ...p, at: [N[0] * d, N[1] * d, N[2] * d] } } } });
  };
  /** The offset as shown: along the shown axis of the plane's normal. */
  const shownDepth = () => { const s = sk(); if (s.kind === "frame") return s.plane === "xy" ? -s.depth || 0 : s.depth; const p = part(); return p ? (s.plane === "xy" ? -p.at[2] : s.plane === "zy" ? p.at[0] : p.at[1]) || 0 : 0; };
  const fromShown = (v: number) => sk().plane === "xy" ? -v : v;
  const place = async () => {
    const t = typed().trim(), rel = t.startsWith("@"), v = t.replace("@", "").split(/[\s,;]+/).filter(Boolean).map(parseNum);
    if (v.length !== 2 || v.some(x => x === null)) { toast(`Type a point on the sketch as across, up (or @d-across, d-up from the last point), in ${unit("coord")}.`); return; }
    const q = v.map(x => fromU("coord", x!)) as [number, number], last = lastPlaced() ?? [0, 0];
    await canvasPlace.run?.(rel ? [last[0] + q[0], last[1] + q[1]] : q);
    setTyped("");
  };
  return (
    <div class="sketchbar" onPointerDown={e => e.stopPropagation()}>
      <b>Sketch</b><span class="muted">{sk().kind === "part" ? part()?.name ?? "Part" : "3D frame"} · {sk().isNew || sk().kind === "frame" ? `${PLANE_NAME[sk().plane][0]} plane (${PLANE_NAME[sk().plane][1]})` : "its own plane"}</span>
      <Show when={sk().isNew || sk().kind === "frame"}>
        <label>{PLANE_NAME[sk().plane][2]} =</label>
        <input class="mono num" value={num(toU("coord", shownDepth()), 6)} title={`Move the sketch plane along ${PLANE_NAME[sk().plane][2]} (${unit("coord")})`} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null) setOffset(fromShown(fromU("coord", v))); }} />
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
      <button class="btn sm pri" title="Keep the sketch" onClick={finishSketch}>Finish Sketch</button>
    </div>
  );
};
