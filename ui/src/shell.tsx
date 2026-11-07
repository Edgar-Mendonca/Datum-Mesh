// The window shell of 0.10.0 (docs/PLAN-1.0.md, the mock-ups): the context bar under the ribbon
// (load case, what is picked, how the model is drawn, the view, units, planned features) and the
// tool strip beside the tree (the current step's most used tools). Nothing here calculates.
import { For, Show, JSX } from "solid-js";
import { model, tab, tool, show, results, opts, setOpts } from "./store";
import * as D from "./dialogs";
import { stepsDialog } from "./steps";
import { C } from "./cmds";
import { Icon } from "./ui";
import { isFrame, isSolidA } from "./api";
import { CasePicker } from "./cases";
import { showPlanned, setShowPlanned } from "./planned";
import { hasSolids } from "./study";

/** A dimmed control for a feature still to come (its release in the tooltip). */
const Soon = (p: { children: JSX.Element; v: string; what: string }) => <span class="soonc" title={`${p.what}: planned for ${p.v}`}>{p.children}</span>;

export const CtxBar = () => {
  const m = () => model();
  const frame = () => isFrame(m()?.analysis);
  return <div class="ctxbar">
    <Show when={frame()}><label>Case</label><CasePicker /><span class="sep" /></Show>
    {/* 0.10.1: what the bar holds follows the tab (the mock-ups): the materials, the steps, what is shown of a frame, the snaps. */}
    <Show when={tab() === "properties"}><span class="ctxtab" data-ctx="materials"><label>Materials</label><b>{m()?.materials.length ?? 0}</b>
      <button class="btn sm" onClick={() => D.materials()} title="The materials of the model: pick from the library or type your own">Materials…</button>
      <Soon v="0.10.2" what="Colour the model by material or by section"><label>Colour</label><span class="seg"><span class="on">By material</span><span>By section</span></span></Soon></span><span class="sep" /></Show>
    <Show when={tab() === "solve"}><span class="ctxtab" data-ctx="steps"><label>Steps</label><b>{(m()?.steps ?? []).length || 1}</b><small>{(m()?.steps ?? []).some(s => s.kind !== "linear") ? "nonlinear" : "linear"}</small>
      <button class="btn sm" onClick={stepsDialog} title="Analysis steps: linear, or large deflection, yielding, dynamics">Steps…</button>
      <Soon v="0.10.2" what="Show only the loads active in a step"><label>Show</label><span class="seg"><span class="on">Loads in the step</span><span>All</span></span></Soon></span><span class="sep" /></Show>
    <Show when={frame() && tab() === "geometry"}><span class="ctxtab" data-ctx="frame">
      <Soon v="0.10.2" what="Member and node numbers, sections, end releases and local axes drawn on the frame"><label>Show</label><span class="seg"><span>Numbers</span><span>Sections</span><span>Releases</span><span>Local axes</span></span></Soon>
      <label>Snap</label><span class="seg"><button classList={{ on: opts().snap }} onClick={() => setOpts({ ...opts(), snap: !opts().snap })} title="Points land on the grid">Grid</button></span>
      <Soon v="0.11" what="Snap to nodes and to the middle of members"><span class="seg"><span>Nodes</span><span>Midpoints</span></span></Soon></span><span class="sep" /></Show>
    <Show when={m()?.analysis === "space" && tab() === "results"}><Soon v="0.11" what="Plate results on the top or bottom face"><label>Face</label><span class="seg"><span>Top</span><span class="on">Bottom</span></span></Soon><span class="sep" /></Show>
    <label>Pick</label>
    <Soon v="0.11" what="A selection filter: only faces, edges, nodes, members or parts"><span class="seg"><span class="on">Anything</span>{(frame() ? ["Nodes", "Members", "Loads"] : ["Parts", "Faces", "Edges", "Nodes"]).map(x => <span>{x}</span>)}</span></Soon>
    <span class="sep" />
    <span class="seg" role="group" aria-label="How the model is drawn">
      <button classList={{ on: !show().mesh }} title="Shaded" onClick={() => C.show({ mesh: false })}><Icon n="solid" /></button>
      <button classList={{ on: show().mesh }} title="Shaded with the elements' edges" onClick={() => C.show({ mesh: true })}><Icon n="meshlines" /></button>
      <Soon v="0.11" what="Wireframe and hidden lines"><button disabled title="Wireframe"><Icon n="mesh" /></button><button disabled title="Hidden lines"><Icon n="model" /></button></Soon>
    </span>
    <span class="sep" />
    <button class="ib" title="Fit the model in the view (F)" onClick={C.fit}><Icon n="fit" /></button>
    <button class="ib" title="Zoom in" onClick={C.zoomIn}><Icon n="zin" /></button>
    <button class="ib" title="Zoom out" onClick={C.zoomOut}><Icon n="zout" /></button>
    <Soon v="0.11" what="Saved views: keep up to four views and go back with one key"><span class="seg"><span>1</span><span>2</span><span>3</span><span>4</span></span></Soon>
    <span class="sep" />
    <Soon v="0.11" what="Display groups: show only some parts, faces or members"><label>Show</label><span class="fakesel">Everything</span></Soon>
    <span class="grow" />
    <label class="chk" title="The features still to come, dimmed, with the release that brings each"><input type="checkbox" checked={showPlanned()} onChange={e => setShowPlanned(e.currentTarget.checked)} />Planned features</label>
    <span class="sep" />
    <label>Units</label>
    <select value={m()?.units ?? "SI"} onChange={e => C.units(e.currentTarget.value as "SI" | "US")} title="Units shown and typed (the file keeps SI)">
      <option value="SI">{frame() ? "SI · m, kN" : "SI · mm, N, MPa"}</option><option value="US">{frame() ? "US · ft, kip" : "US · in, lbf, ksi"}</option>
    </select>
  </div>;
};

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
