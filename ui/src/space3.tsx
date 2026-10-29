// 3D frames without three.js (0.2.6): what the members are coloured by, the deflection scale, the
// legend under the 3D view and the plane bar over it. The 3D view itself (view3d.tsx, with
// three.js) is loaded only when a 3D frame is open, so the window starts light.
import { Show, createMemo, createSignal } from "solid-js";
import { Model, SpaceMember, SpaceResults, num, parseNum } from "./api";
import { model, spaceRes, stale, show, opts, setView, setTool } from "./store";
import { palette, BANDS } from "./parts";
import { fmt, unit, fromU, Q } from "./units";
import { setPlane, plane, PLANES, space, depth, setDepth, depths, depthAxis } from "./plane3";
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
  return (
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
  );
};
