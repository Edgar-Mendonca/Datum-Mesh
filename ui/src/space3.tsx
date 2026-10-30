// 3D frames without three.js (0.2.6): what the members are coloured by, the deflection scale, the
// legend under the 3D view and the plane bar over it. The 3D view itself (view3d.tsx, with
// three.js) is loaded only when a 3D frame is open, so the window starts light.
import { Show, createMemo, createSignal } from "solid-js";
import { Model, SpaceMember, SpaceResults, num, parseNum } from "./api";
import { model, spaceRes, solidRes, stale, show, opts, setView, setTool, tool, act, setSel } from "./store";
import { defaultLoad, defaultSupport } from "./panels";
import { palette, BANDS, curField, fieldsFor, rangeOf, ColourBar } from "./parts";
import { fmt, unit, fromU, Q } from "./units";
import { setPlane, plane, PLANES, space, solid, depth, setDepth, depths, depthAxis } from "./plane3";
import { C, imbalanceOf } from "./cmds";
import { Icon } from "./ui";

export type P3 = [number, number, number];
export const at3 = (m: Model) => new Map(m.nodes.map(n => [n.id, [n.x, n.y, n.z ?? 0] as P3]));

/** The result the members are coloured by. */
export type Q3 = "util" | "n" | "mz" | "my" | "t" | "vy" | "vz" | "d" | "none";
export const Q3S: [Q3, string, Q][] = [["util", "Utilisation", "ratio"], ["n", "Axial force N", "force"], ["mz", "Bending moment Mz (about the strong axis)", "moment"], ["my", "Bending moment My (about the weak axis)", "moment"], ["t", "Torque T", "moment"], ["vy", "Shear Vy", "force"], ["vz", "Shear Vz", "force"], ["d", "Deflection", "len"], ["none", "None", "ratio"]];
export const [q3, setQ3] = createSignal<Q3>("util");

export const span3 = (m: Model) => {
  if (!m.nodes.length) return 1;
  const lo = [0, 1, 2].map(k => Math.min(...m.nodes.map(n => [n.x, n.y, n.z ?? 0][k]))), hi = [0, 1, 2].map(k => Math.max(...m.nodes.map(n => [n.x, n.y, n.z ?? 0][k])));
  return Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], 1e-6);
};
export const dlen = (d: P3) => Math.hypot(d[0], d[1], d[2]);
/** How many times the deflections are drawn larger: a round number, as for plane frames. */
export const deform3 = (m: Model, r: SpaceResults) => {
  const dmax = Math.max(0, ...r.members.flatMap(mr => mr.d.map(dlen)));
  if (!(dmax > 1e-12 * span3(m))) return 0;
  const f = 0.08 * span3(m) / dmax, p = 10 ** Math.floor(Math.log10(f)), k = f / p;
  return (k >= 5 ? 5 : k >= 2 ? 2 : 1) * p * show().scale;
};
/** A member's values for the chosen result, one per station. */
export const vals = (mr: SpaceMember, q: Q3): number[] => q === "util" ? mr.x.map(() => mr.util) : q === "d" ? mr.d.map(dlen) : q === "none" ? mr.x.map(() => 0) : mr[q];
export const range3 = createMemo((): [number, number] | null => {
  const r = spaceRes(), q = q3(); if (!r || q === "none") return null;
  const v = r.members.flatMap(mr => vals(mr, q)); if (!v.length) return null;
  let lo = Math.min(...v), hi = Math.max(...v);
  if (q === "util") { lo = 0; hi = Math.max(hi, 1); }
  if (hi - lo < 1e-12 * Math.max(Math.abs(hi), Math.abs(lo), 1e-300)) { hi = lo + 1; }
  return [lo, hi];
});

/** Under the 3D view once solved: what colours the members, the deflection scale, equilibrium. */
export const SpaceLegend = () => {
  const r = () => spaceRes(), m = () => model();
  const qd = () => Q3S.find(x => x[0] === q3())!;
  const dmax = () => Math.max(0, ...(r()?.members.flatMap(mr => mr.d.map(dlen)) ?? [0]));
  const cols = () => palette(opts().contours);
  return (
    <Show when={r() && m()}>
      <div class="legend">
        <Show when={stale()}><div class="stalebar"><Icon n="info" /> The model changed since it was solved. <button class="btn sm pri" onClick={C.solve}>Solve again (F5)</button></div></Show>
        <div><b>Colour by</b><select class="fpick" value={q3()} onChange={e => setQ3(e.currentTarget.value as Q3)}>{Q3S.map(([k, l]) => <option value={k} selected={k === q3()}>{l}</option>)}</select></div>
        <Show when={range3()}>{rg => <span class="cbar"><span class="cb">{cols().map((c, k) => <i style={{ background: c }}><Show when={k % 3 === 0}><small>{q3() === "util" ? `${num((rg()[0] + (rg()[1] - rg()[0]) * k / BANDS) * 100, 2)}%` : num(+fmt(qd()[2], rg()[0] + (rg()[1] - rg()[0]) * k / BANDS, 3).split(" ")[0].replace("−", "-"), 3)}</small></Show></i>)}<b><small>{q3() === "util" ? `${num(rg()[1] * 100, 2)}%` : num(+fmt(qd()[2], rg()[1], 3).split(" ")[0].replace("−", "-"), 3)}</small></b></span><em>{q3() === "util" ? "" : unit(qd()[2])}</em></span>}</Show>
        <div><b>Deflected shape</b><span>{deform3(m()!, r()!) > 0 ? <>largest {fmt("len", dmax())} · drawn × {num(deform3(m()!, r()!), 3)}</> : "nothing moves: there are no loads"}</span>
          <input type="range" min="0.1" max="5" step="0.1" value={show().scale} onInput={e => C.show({ scale: +e.currentTarget.value })} title="Exaggeration" /></div>
        <div class="eqline" classList={{ bad: imbalanceOf(r()!) > 1e-6 }}><Icon n={imbalanceOf(r()!) > 1e-6 ? "info" : "check"} /><span>{imbalanceOf(r()!) > 1e-6 ? "Reactions do not balance the loads" : "Equilibrium: reactions balance the loads"}</span><button class="lnk" onClick={() => setView("results")}>Details</button></div>
      </div>
    </Show>
  );
};

/** Over the model view of a 3D frame: the 3D view or a working plane, and the plane's depth. */
export const PlaneBar = () => {
  const [typed, setTyped] = createSignal("");
  /** Put the support or load being placed on the back or front face of a solid. */
  const onEnd = async (front: boolean) => {
    const t = tool(), [kind, val] = t.startsWith("sup:") ? ["Support", defaultSupport(t.slice(4) === "pinned" ? "fixed" : t.slice(4))] : ["Load", defaultLoad(t.slice(5), false)];
    const o = await act({ Add: { item: { [kind]: { id: 0, at: { on: "end", front }, kind: val } } } });
    if (o?.created.length) setSel(o.created);
  };
  const full = () => (model()?.revolve ?? 0) >= Math.PI * 2 - 1e-9;
  const placing = () => (tool().startsWith("sup:") && !["sup:temperature", "sup:rollerx", "sup:rollery", "sup:spring"].includes(tool())) || ["load:pressure", "load:traction"].includes(tool());
  return (<>
    <Show when={solid()}>
      <div class="planebar">
        <span class="seg"><button classList={{ on: plane() === "3d" }} title="The solid in 3D: turn it with the left button, pan with the right, zoom with the wheel" onClick={() => { setPlane("3d"); }}>3D</button><button classList={{ on: plane() !== "3d" }} title="The part's section (x-y), drawn and edited as a plane part; it is extruded along z by the thickness" onClick={() => setPlane("xy")}>Section</button></span>
        <span class="muted">{model()?.revolve ? `revolved ${num(model()!.revolve! * 180 / Math.PI, 4)}° about the y axis` : `extruded ${fmt("len", model()?.thickness ?? 0)} along z`}</span>
        <Show when={placing()}><span class="grow" /><span class="muted">Click a face in 3D, or put it on</span><Show when={!full()}><button class="btn sm" onClick={() => onEnd(false)} title={model()?.revolve ? "The cut face where the turn starts" : "The face at z = 0"}>the back face</button><button class="btn sm" onClick={() => onEnd(true)} title={model()?.revolve ? "The cut face where the turn ends" : "The face at z = thickness"}>the front face</button></Show><span class="muted">or an edge in Section</span></Show>
      </div>
    </Show>
    <Show when={space()}>
      <div class="planebar">
        <span class="seg">{PLANES.map(([k, l, t]) => <button classList={{ on: plane() === k }} title={t} onClick={() => { setPlane(k); if (k === "3d") setTool("select"); }}>{l}</button>)}</span>
        <Show when={plane() !== "3d"}>
          <label>{depthAxis()} =</label>
          <select value={String(depth())} onChange={e => setDepth(+e.currentTarget.value)}>{[...new Set([...depths(), depth()])].sort((a, b) => a - b).map(d => <option value={String(d)} selected={d === depth()}>{fmt("coord", d)}</option>)}</select>
          <input class="mono" placeholder="other" value={typed()} title={`Another depth (${unit("coord")})`} onChange={e => { const v = parseNum(e.currentTarget.value); if (v !== null) { setDepth(fromU("coord", v)); setTyped(""); e.currentTarget.value = ""; } }} />
          <span class="muted">{plane() === "xy" ? "x across, y up" : plane() === "zy" ? "z across, y up (from the left)" : "x across, z towards you (from above)"} · members off this plane are faint</span>
        </Show>
      </div>
    </Show>
  </>);
};

/** The cutting plane through a solved solid (0.3.2): across x, y or z, a fraction of the way
 * across the part; "off" shows the whole part. */
export type CutAxis = "off" | "x" | "y" | "z";
export const [cut, setCut] = createSignal<{ axis: CutAxis; at: number }>({ axis: "off", at: 0.5 });

/** Under the 3D view of a solved 3D solid: the result shown and its colours, the deformed shape,
 * the cutting plane, the equilibrium check. */
export const SolidLegend = () => {
  const r = () => solidRes(), fd = () => curField();
  const fld = () => r()?.fields.find(f => f.key === fd().key);
  const cols = () => palette(opts().contours, fd().reverse);
  const dmax = () => Math.max(0, ...(r()?.disp.map(d => Math.hypot(d[0], d[1], d[2])) ?? [0]));
  return (
    <Show when={r() && model()?.analysis === "solid"}>
      <div class="legend">
        <Show when={stale()}><div class="stalebar"><Icon n="info" /> The model changed since it was solved. <button class="btn sm pri" onClick={C.solve}>Solve again (F5)</button></div></Show>
        <div><b>Result</b><select class="fpick" value={fd().key} onChange={e => C.show({ field: e.currentTarget.value })}>{fieldsFor("solid").map(f => <option value={f.key} selected={f.key === fd().key}>{f.name}</option>)}</select>
          <span>max {fmt(fd().q, fld()?.max ?? 0)} · min {fmt(fd().q, fld()?.min ?? 0)}</span></div>
        <ColourBar d={fd()} range={fld() ? rangeOf(fd(), fld()!.v) : null} colours={cols()} />
        <div><b>Deformed shape</b><span>{show().deformed && dmax() > 0 ? <>largest {fmt("len", r()!.fields.find(f => f.key === "umag")!.max)}</> : "off"}</span>
          <input type="range" min="0.1" max="5" step="0.1" value={show().scale} onInput={e => C.show({ scale: +e.currentTarget.value })} title="Exaggeration" /></div>
        <Show when={r()!.volume}><div><b>Cut</b><span class="seg">{(["off", "x", "y", "z"] as CutAxis[]).map(a => <button classList={{ on: cut().axis === a }} title={a === "off" ? "The whole part" : `Cut across ${a}: the part beyond the plane is hidden and the inside shown`} onClick={() => setCut({ ...cut(), axis: a })}>{a === "off" ? "Off" : a}</button>)}</span>
          <Show when={cut().axis !== "off"}><input type="range" min="0" max="1" step="0.01" value={cut().at} onInput={e => setCut({ ...cut(), at: +e.currentTarget.value })} title="Where the plane cuts, across the part" /></Show></div></Show>
        <div><b>Mesh</b><span>{r()!.elements.toLocaleString()} {r()!.order === 2 ? "18-node" : "6-node"} wedges, {r()!.layers} {r()!.layers === 1 ? "layer" : "layers"}</span></div>
        <div class="eqline" classList={{ bad: imbalanceOf(r()!) > 1e-6 }}><Icon n={imbalanceOf(r()!) > 1e-6 ? "info" : "check"} /><span>{imbalanceOf(r()!) > 1e-6 ? "Reactions do not balance the loads" : "Equilibrium: reactions balance the loads"}</span><button class="lnk" onClick={() => setView("results")}>Details</button></div>
      </div>
    </Show>
  );
};
