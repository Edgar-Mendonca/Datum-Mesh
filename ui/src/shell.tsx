// The window shell of 0.10.0 (docs/PLAN-1.0.md, the mock-ups): the context bar under the ribbon
// (load case, what is picked, how the model is drawn, the view, units, planned features) and the
// tool strip beside the tree (the current step's most used tools). Nothing here calculates.
import { For, Show } from "solid-js";
import { model, tab, tool, show, results, opts, setOpts } from "./store";
import * as D from "./dialogs";
import { stepsDialog } from "./steps";
import { C } from "./cmds";
import { Icon } from "./ui";
import { isFrame, isSolidA, DynInfo, num } from "./api";
import { CasePicker } from "./cases";
import { showPlanned, setShowPlanned } from "./planned";
import { hasSolids } from "./study";


export const CtxBar = () => {
  const m = () => model();
  const frame = () => isFrame(m()?.analysis);
  const dyn = () => (results() as { dynamic?: DynInfo } | null)?.dynamic;
  // 0.10.2 (the mock-ups): only what works is on the bar; what is still to come is on each tab's
  // Coming menu, not here. Its middle follows the tab: the case, the materials, the steps, a time
  // history's time, record and damping.
  return <div class="ctxbar">
    <Show when={frame() && !(tab() === "results" && dyn()?.kind === "history")}><label>Case</label><CasePicker /><span class="sep" /></Show>
    <Show when={tab() === "properties"}><span class="ctxtab" data-ctx="materials"><label>Materials</label><b>{m()?.materials.length ?? 0}</b>
      <button class="btn sm" onClick={() => D.materials()} title="The materials of the model: pick from the library or type your own">Materials…</button></span><span class="sep" /></Show>
    <Show when={tab() === "solve"}><span class="ctxtab" data-ctx="steps"><label>Steps</label><b>{(m()?.steps ?? []).length || 1}</b><small>{(m()?.steps ?? []).some(s => s.kind !== "linear") ? "nonlinear" : "linear"}</small>
      <button class="btn sm" onClick={stepsDialog} title="Analysis steps: linear, or large deflection, yielding, dynamics">Steps…</button></span><span class="sep" /></Show>
    <Show when={tab() === "results" && dyn()?.kind === "history" ? dyn() : null}>{d => <span class="ctxtab" data-ctx="history">
      <label>Time</label><b>{num(d().peak_t, 4)} s</b><small>the peak, of {num(d().t[d().t.length - 1] ?? 0, 4)} s</small>
      <label>Record</label><span class="ctxv">{dynRecord(d())}</span>
      <label>Damping</label><span class="ctxv">{dynDamping()}</span></span>}</Show>
    <Show when={frame() && tab() === "geometry"}><span class="ctxtab" data-ctx="frame"><label>Snap</label><span class="seg"><button classList={{ on: opts().snap }} onClick={() => setOpts({ ...opts(), snap: !opts().snap })} title="Points land on the grid">Grid</button><button classList={{ on: opts().grid }} onClick={() => setOpts({ ...opts(), grid: !opts().grid })} title="Show the grid">Show grid</button></span></span><span class="sep" /></Show>
    <span class="seg" role="group" aria-label="How the model is drawn">
      <button classList={{ on: !show().mesh }} title="Shaded" onClick={() => C.show({ mesh: false })}><Icon n="solid" /></button>
      <button classList={{ on: show().mesh }} title="Shaded with the elements' edges" onClick={() => C.show({ mesh: true })}><Icon n="meshlines" /></button>
    </span>
    <span class="sep" />
    <button class="ib" title="Fit the model in the view (F)" onClick={C.fit}><Icon n="fit" /></button>
    <button class="ib" title="Zoom in" onClick={C.zoomIn}><Icon n="zin" /></button>
    <button class="ib" title="Zoom out" onClick={C.zoomOut}><Icon n="zout" /></button>
    <span class="grow" />
    <label class="chk" title="The features still to come, dimmed, with the release that brings each"><input type="checkbox" checked={showPlanned()} onChange={e => setShowPlanned(e.currentTarget.checked)} />Planned features</label>
    <span class="sep" />
    <label>Units</label>
    <select value={m()?.units ?? "SI"} onChange={e => C.units(e.currentTarget.value as "SI" | "US")} title="Units shown and typed (the file keeps SI)">
      <option value="SI">{frame() ? "SI · m, kN" : "SI · mm, N, MPa"}</option><option value="US">{frame() ? "US · ft, kip" : "US · in, lbf, ksi"}</option>
    </select>
  </div>;
};
/** A time history's ground record, as its step names it. */
const dynRecord = (d: DynInfo) => { const st = (model()?.steps ?? []).find(s => s.kind === "history"); return st ? `${st.ground ? "ground shaking" : "load in time"} · ${st.time.length} points` : `${d.steps.toLocaleString()} steps`; };
const dynDamping = () => { const st = (model()?.steps ?? []).find(s => s.kind === "history"); return st ? `${num(st.damping * 100, 3)} %` : "—"; };

type Tl = [string, string, string]; // icon, label, tool id ("cmd:…" runs a command)
const TOOLS: Record<string, (a: string) => Tl[]> = {
  geometry: a => a === "frame" || a === "space" ? [["select", "Select", "select"], ["beam", "Beam", "beam"], ["truss", "Truss bar", "truss"], ["node", "Node", "node"]]
    : isSolidA(a) ? [["select", "Select", "select"], ["hcircle", "Hole", "hole3"], ["fillet", "Fillet", "fillet3"], ["chamfer", "Chamfer", "chamfer3"]]
      : [["select", "Select", "select"], ["rect", "Rectangle", "rect"], ["circle", "Circle", "circle"], ["poly", "Polygon", "poly"], ["hrect", "Rectangular hole", "hrect"], ["hcircle", "Round hole", "hcircle"]],
  loads: a => a === "heat" || a === "solidheat" ? [["temp", "Temperature", "sup:temperature"], ["flux", "Heat flux", "load:flux"], ["convection", "Convection", "load:convection"]]
    : a === "frame" || a === "space" ? [["fixed", "Fixed", "sup:fixed"], ["pinned", "Pinned", "sup:pinned"], ["roller", "Roller", "sup:roller"], ["spring", "Spring", "sup:spring"], ["-", "", ""], ["force", "Force", "load:force"], ["dist", "Distributed", "load:dist"], ["moment", "Moment", "load:moment"], ["settle", "Settlement", "load:settlement"], ["mass", "Mass", "load:mass"]]
      : [["fixed", "Fixed", "sup:fixed"], ["symmetry", "Symmetry", "sup:symmetry"], ["roller", "Hold directions", "sup:hold"], ["-", "", ""], ["force", "Force", "load:force"], ["pressure", "Pressure", "load:pressure"], ["settle", "Move a face", "load:settlement"]],
  mesh: a => a === "frame" || a === "space" ? [] : [["mesh", "Mesh now", "cmd:mesh"], ["refine", "Finer near", "refine"]],
  solve: a => [["solve", "Solve (F5)", "cmd:solve"], ["modes", "Natural frequencies", "cmd:modes"], ...(a === "frame" || a === "space" ? [["buckle", "Buckling", "cmd:buckling"] as Tl] : [])],
  results: () => [["contour", "Contours", "cmd:contour"], ["deform", "Deformed shape", "cmd:deform"], ["-", "", ""], ["table", "Tables", "cmd:tables"], ["report", "Report", "cmd:report"]],
};
const run = (t: string) => {
  if (!t.startsWith("cmd:")) return C.tool(t);
  const c = t.slice(4);
  if (c === "mesh") return hasSolids() ? import("./study").then(k => k.meshSolids()) : C.mesh();
  if (c === "solve") return C.solve();
  if (c === "modes") return C.modes();
  if (c === "buckling") return C.buckling();
  if (c === "contour") return C.view("model");
  if (c === "deform") return C.show({ deformed: !show().deformed });
  if (c === "tables") return C.view("results");
  if (c === "report") return C.view("report");
};

/** Beside the tree: the current step's most used tools, one click each (0.10.0). */
export const ToolStrip = () => {
  const list = () => (TOOLS[tab()] ?? (() => [["select", "Select", "select"]] as Tl[]))(model()?.analysis ?? "frame");
  return <Show when={list().length}><div class="toolstrip" role="toolbar" aria-label="Tools of this step">
    <For each={list()}>{([i, l, t]) => i === "-" ? <hr /> : <button classList={{ on: tool() === t || (t === "cmd:deform" && show().deformed && !!results()) }} title={l} onClick={() => run(t)}><Icon n={i} /></button>}</For>
  </div></Show>;
};
