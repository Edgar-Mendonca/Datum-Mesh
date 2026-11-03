// Basic modelling of parts (0.6.3), loaded with the 3D view. A part is a sketch swept along its z
// (or turned about its y) and placed; the engine does the modelling (`PlaceParts`, `Cut`, `Hole`,
// `PartEdge` in doc.rs, solids/modelling.rs). Here: the Extrude panel with its preview, the dialogs
// to move, copy, turn, mirror, pattern and cut parts, and the 3D tools that act where a part is
// clicked. Coordinates are shown and typed with Z up (axes.ts) and sent in the engine's axes.
import { Show, createSignal, untrack } from "solid-js";
import { Part, Target, num } from "./api";
import { model, act, toast, sel, setSel, openDialog, sweepOn, setSweepOn, holeD, edgeSize } from "./store";
import { Dialog } from "./ui";
import { toU, fromU, unit } from "./units";
import { fromD } from "./axes";
import { numIn } from "./panels";

type V3 = [number, number, number];

/** The turn of a part (about x, then y, then z) as a matrix, R = Rz · Ry · Rx (as `rot` in the engine). */
export const rotOf = (t: V3): number[][] => {
  const [sx, cx, sy, cy, sz, cz] = [Math.sin(t[0]), Math.cos(t[0]), Math.sin(t[1]), Math.cos(t[1]), Math.sin(t[2]), Math.cos(t[2])];
  return [[cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx], [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx], [-sy, cy * sx, cy * cx]];
};
/** The turn of a rotation matrix (as `euler` in the engine). */
export const turnOf = (r: number[][]): V3 => {
  const sy = Math.max(-1, Math.min(1, -r[2][0])), y = Math.asin(sy), z0 = (v: number) => (Math.abs(v) < 1e-15 ? 0 : v);
  if (Math.abs(sy) > 1 - 1e-12) return [z0(Math.atan2(r[0][1] * sy, r[1][1])), y, 0];
  return [z0(Math.atan2(r[2][1], r[2][2])), z0(y), z0(Math.atan2(r[1][0], r[0][0]))];
};

/** A part under the pointer in the 3D view: the part, its face, the point and the face's normal
 * (towards the viewer). */
export type PartHit = { part: number; target: Target | null; point: V3; normal: V3 };

/** The 3D tools (Geometry › Modify): a hole through the face clicked, or the edge along the
 * depth nearest the click rounded or bevelled. */
export const toolOnPart = async (t: string, h: PartHit) => {
  const p = model()?.parts?.find(x => x.id === h.part); if (!p) return;
  if (t === "hole3") {
    if (h.target?.on !== "partend" || p.angle > 0) { toast("Click the front or back face of an extruded part: a hole goes right through it, along its depth."); return; }
    const o = await act({ Hole: { part: h.part, at: h.point, d: holeD() } });
    if (o) toast(`A hole ${num(toU("len", holeD()), 4)} ${unit("len")} across through ${p.name}.`);
    return;
  }
  const o = await act({ PartEdge: { part: h.part, near: h.point, size: edgeSize(), round: t === "fillet3" } });
  if (o) toast(`${p.name}: the edge ${t === "fillet3" ? "rounded" : "bevelled"} (its sketch corner, along the whole depth).`);
};

/** The parts selected (in the tree or in 3D), or a hint when none is. */
const chosen = (): Part[] => {
  const s = new Set(sel()), ps = (model()?.parts ?? []).filter(p => s.has(p.id));
  if (!ps.length) toast("Select the parts first: click one in 3D (Select), or in the tree.");
  return ps;
};
const names = (ps: Part[]) => (ps.length === 1 ? ps[0].name : `${ps.length} parts`);

/** A typed length or coordinate in a dialog: shown units in, metres out. */
const Len = (p: { l: string; v: number; set: (v: number) => void; q?: "len" | "coord" }) => (
  <><label>{p.l} ({unit(p.q ?? "coord")})</label><input class="mono" value={num(toU(p.q ?? "coord", p.v), 6).replace("−", "-")} onChange={numIn(v => p.set(fromU(p.q ?? "coord", v)), () => num(toU(p.q ?? "coord", p.v), 6).replace("−", "-"))} /></>
);
const Count = (p: { l: string; v: number; set: (v: number) => void; min: number }) => (
  <><label>{p.l}</label><input class="mono" value={p.v} onChange={numIn(v => (v >= p.min && v <= 1000 && Number.isInteger(v) ? p.set(v) : false), () => p.v)} /></>
);
const AXES: [string, V3][] = [["X", [1, 0, 0]], ["Y", [0, 1, 0]], ["Z", [0, 0, 1]]];

/** Move or copy parts by a step (copies 0: move them). */
export const moveParts = () => {
  const ps = chosen(); if (!ps.length) return;
  openDialog(() => {
    const g = model()?.grid || 0.01;
    const [d, setD] = createSignal<V3>([g * 10, 0, 0]), [n, setN] = createSignal(0);
    return <Dialog title="Move or Copy Parts" okText={n() ? "Copy" : "Move"} help="modelling" ok={async () => {
      const o = await act({ PlaceParts: { ids: ps.map(p => p.id), how: { kind: "move", by: fromD(d()) }, copies: n() } });
      if (!o) return false;
      toast(n() ? `${n()} ${n() === 1 ? "copy" : "copies"} of ${names(ps)}, each a step further on.` : `${names(ps)} moved.`);
    }}>
      <p class="muted small">{names(ps)}: moved by the step, or copied (each copy a step further on: a linear pattern). Copies take their sketches; supports and loads stay on the original.</p>
      <div class="form">{AXES.map(([a], k) => <Len l={`Step along ${a}`} v={d()[k]} set={v => setD(d().map((q, i) => (i === k ? v : q)) as V3)} />)}
        <Count l="Copies (0: move)" v={n()} set={setN} min={0} /></div>
    </Dialog>;
  });
};

/** Turn parts about an axis, or copy them round it (a circular pattern). */
export const turnParts = () => {
  const ps = chosen(); if (!ps.length) return;
  openDialog(() => {
    const [ax, setAx] = createSignal(2), [at, setAt] = createSignal<V3>([0, 0, 0]), [deg, setDeg] = createSignal(90), [n, setN] = createSignal(3);
    return <Dialog title="Turn or Pattern Parts" okText={n() ? "Copy" : "Turn"} help="modelling" ok={async () => {
      const o = await act({ PlaceParts: { ids: ps.map(p => p.id), how: { kind: "turn", about: fromD(at()), axis: fromD(AXES[ax()][1]), angle: deg() * Math.PI / 180 }, copies: n() } });
      if (!o) return false;
      toast(n() ? `${n()} ${n() === 1 ? "copy" : "copies"} of ${names(ps)} round the ${AXES[ax()][0]} axis, ${deg()}° apart.` : `${names(ps)} turned ${deg()}° about ${AXES[ax()][0]}.`);
    }}>
      <p class="muted small">{names(ps)}: turned about an axis through a point, or copied round it, each copy turned the angle further (a circular pattern: 4 copies at 72° make a ring of five).</p>
      <div class="form"><label>About the axis along</label><select onChange={e => setAx(+e.currentTarget.value)}>{AXES.map(([a], k) => <option value={k} selected={k === ax()}>{a}</option>)}</select>
        {AXES.map(([a], k) => <Len l={`Through ${a}`} v={at()[k]} set={v => setAt(at().map((q, i) => (i === k ? v : q)) as V3)} />)}
        <label>Angle (°)</label><input class="mono" value={deg()} onChange={numIn(v => (Math.abs(v) > 1e-9 ? setDeg(v) : false), deg)} />
        <Count l="Copies (0: turn)" v={n()} set={setN} min={0} /></div>
    </Dialog>;
  });
};

/** Mirror parts in a plane, as a copy or in place. */
export const mirrorParts = () => {
  const ps = chosen(); if (!ps.length) return;
  openDialog(() => {
    const [ax, setAx] = createSignal(0), [off, setOff] = createSignal(0), [copy, setCopy] = createSignal(true);
    const plane = ["Y-Z (X = offset)", "X-Z (Y = offset)", "X-Y (Z = offset)"];
    return <Dialog title="Mirror Parts" okText="Mirror" help="modelling" ok={async () => {
      const nrm = AXES[ax()][1], pt = nrm.map(c => c * off()) as V3;
      const o = await act({ PlaceParts: { ids: ps.map(p => p.id), how: { kind: "mirror", point: fromD(pt), normal: fromD(nrm) }, copies: copy() ? 1 : 0 } });
      if (!o) return false;
      toast(`${names(ps)} mirrored${copy() ? " as a copy" : ""}.`);
    }}>
      <p class="muted small">{names(ps)}: the mirror image in a plane. The copy's sketch is the same; it is swept the other way.</p>
      <div class="form"><label>Mirror plane</label><select onChange={e => setAx(+e.currentTarget.value)}>{plane.map((a, k) => <option value={k} selected={k === ax()}>{a}</option>)}</select>
        <Len l="Offset" v={off()} set={setOff} />
        <label>Keep the original</label><input type="checkbox" checked={copy()} onChange={e => setCopy(e.currentTarget.checked)} /></div>
    </Dialog>;
  });
};

/** Cut the selected part right through another: its outlines become holes there. */
export const cutParts = () => {
  const ps = chosen(); if (!ps.length) return;
  if (ps.length !== 1) { toast("Select the one part to cut with (its sketch's outlines go through the other part)."); return; }
  const others = (model()?.parts ?? []).filter(p => p.id !== ps[0].id);
  if (!others.length) { toast("There is no other part to cut."); return; }
  openDialog(() => {
    const [from, setFrom] = createSignal(others[0].id);
    return <Dialog title="Cut" okText="Cut" help="modelling" ok={async () => {
      const o = await act({ Cut: { tool: ps[0].id, from: from() } });
      if (!o) return false;
      setSel([from()]);
      toast(`${ps[0].name} cut through ${others.find(p => p.id === from())?.name}: its outlines are holes there now.`);
    }}>
      <p class="muted small">The outlines of {ps[0].name} go right through the part picked, along its depth, as holes; {ps[0].name} goes. Both are extruded, their sketches parallel (sketch the cutter on the part's front or back face).</p>
      <div class="form"><label>Cut through</label><select onChange={e => setFrom(+e.currentTarget.value)}>{others.map(p => <option value={p.id}>{p.name}</option>)}</select></div>
    </Dialog>;
  });
};

/** The sweep being set in the Extrude panel, drawn as a preview in 3D. */
export const [draft, setDraft] = createSignal<{ part: number; depth: number; angle: number; from: number } | null>(null);
type Dir = "one" | "other" | "both";
const startOf = (len: number, d: Dir) => (d === "one" ? 0 : d === "other" ? -len : -len / 2);
export const dirOf = (p: Part): Dir => { const l = p.angle > 0 ? p.angle : p.depth, f = p.from ?? 0; return Math.abs(f) < 1e-12 ? "one" : Math.abs(f + l) < 1e-9 * Math.max(1, l) ? "other" : "both"; };

/** Open the Extrude panel for a part (after Finish Sketch, or Geometry › Extrude). */
export const editSweep = (id?: number) => {
  const p = id ? model()?.parts?.find(x => x.id === id) : chosen()[0];
  if (p) setSweepOn(p.id);
};

/** The Extrude panel: extrude or revolve, how far, which way; the preview follows. */
export const SweepPanel = () => (
  <Show when={sweepOn() || null} keyed>{id => {
    const p0 = untrack(() => model()?.parts?.find(x => x.id === id));
    if (!p0) { setSweepOn(0); return null; }
    const [rev, setRev] = createSignal(p0.angle > 0), [len, setLen] = createSignal(p0.depth > 0 ? p0.depth : 0.01), [ang, setAng] = createSignal(p0.angle > 0 ? p0.angle : Math.PI * 2), [dir, setDir] = createSignal<Dir>(dirOf(p0));
    const now = () => ({ part: id, depth: len(), angle: rev() ? ang() : 0, from: startOf(rev() ? ang() : len(), dir()) });
    const show = () => setDraft(now());
    show();
    const close = () => { setDraft(null); setSweepOn(0); };
    const ok = async () => {
      const p = model()?.parts?.find(x => x.id === id); if (!p) return close();
      const d = now();
      const o = await act({ Set: { item: { Part: { ...p, depth: d.depth, angle: d.angle, from: d.from } } } });
      if (o) { close(); setSel([id]); }
    };
    return (
      <div class="sweeppanel" onPointerDown={e => e.stopPropagation()} onPointerUp={e => e.stopPropagation()} onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") ok(); if (e.key === "Escape") close(); }}>
        <b>{rev() ? "Revolve" : "Extrude"}</b><span class="muted">{p0.name}</span>
        <span class="seg"><button classList={{ on: !rev() }} onClick={() => { setRev(false); show(); }}>Extrude</button><button classList={{ on: rev() }} onClick={() => { setRev(true); show(); }}>Revolve</button></span>
        <Show when={rev()} fallback={<label>Distance<input class="mono num" value={num(toU("len", len()), 6)} onChange={numIn(v => (v > 0 ? (setLen(fromU("len", v)), show()) : false), () => num(toU("len", len()), 6))} /><span class="unit">{unit("len")}</span></label>}>
          <label>Angle<input class="mono num" value={num(ang() * 180 / Math.PI, 6)} onChange={numIn(v => (v > 0 && v <= 360 ? (setAng(v * Math.PI / 180), show()) : false), () => num(ang() * 180 / Math.PI, 6))} /><span class="unit">°</span></label>
        </Show>
        <select title="Which way from the sketch" onChange={e => { setDir(e.currentTarget.value as Dir); show(); }}>
          <option value="one" selected={dir() === "one"}>{rev() ? "One way" : "Out of the sketch"}</option><option value="other" selected={dir() === "other"}>{rev() ? "The other way" : "Into the sketch"}</option><option value="both" selected={dir() === "both"}>Both ways, evenly</option>
        </select>
        <button class="btn sm" onClick={close}>Cancel</button><button class="btn sm pri" onClick={ok}>OK</button>
      </div>
    );
  }}</Show>
);
