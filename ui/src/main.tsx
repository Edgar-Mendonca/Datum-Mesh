// Datum Mesh window: layout, sidebar, status bar, keyboard shortcuts, startup and recovery.
import { partsOpen } from "./panels";
import { historyKey } from "./history";
import { cadLoad } from "./cad";
import { render } from "solid-js/web";
import { For, Show, Switch, Match, onMount, createEffect, createSignal, on, lazy, Suspense } from "solid-js";
import "./fonts.css";
import { toD } from "./axes";
import "./styles.css";
import { invoke, listen, Out, Options, num, isFrame } from "./api";
import { sketch, out, setOut, view, setView, backstage, setBackstage, setOpts, opts, dialogs, toast, loadOut, setThemeRaw, applyTheme, sel, cursor, tool, model, results, rawResults, stale, solving, partRes, meshed, modes, modeOn, closeDialog } from "./store";
import { C } from "./cmds";
import { TitleBar, Ribbon, palette } from "./ribbon";
import { Backstage, update, checkUpdate } from "./backstage";
import { Canvas, fit, scale } from "./canvas";
import { Tree, Inspector, ANALYSES, Flow, popShown, setInspOpen } from "./panels";
import { Console, runLine } from "./console";
import { CtxBar, ToolStrip } from "./shell";
import { ResultsLegend } from "./results";
const ResultsPage = lazy(() => import("./resultspage").then(m => ({ default: m.ResultsPage })));
import { PartLegend, MeshBar } from "./parts";
import { ModesBar, ModeAnimator } from "./modes";
// The report and printing load on demand (0.5.5).
const ReportPage = lazy(() => import("./print").then(m => ({ default: m.ReportPage })));
import { PlaneBar, SpaceLegend, SolidLegend } from "./space3";
// The 3D view brings three.js: loaded only when a 3D frame is shown.
const View3D = lazy(() => import("./view3d").then(m => ({ default: m.View3D })));
const SketchBar = lazy(() => import("./sketchmode").then(m => ({ default: m.SketchBar })));
const Player = lazy(() => import("./player").then(m => ({ default: m.Player })));
import { in3d, plane, onPlane, unproj } from "./plane3";
import { Menu, Toast } from "./ui";
import { fmt } from "./units";
import * as D from "./dialogs";
/** The standard parts panel (0.10.3), loaded when first opened. */
const StdParts = lazy(() => import("./stdparts").then(k => ({ default: k.StdParts })));


const TOOL_HINT: Record<string, string> = {
  node: "Click to place a node.", beam: "Click the start, then the end of each beam; Esc stops.", truss: "Click the start, then the end of each bar; Esc stops.",
  rect: "Click two opposite corners.", hrect: "Click two opposite corners of the hole.", circle: "Click the centre, then a point on the circle.", hcircle: "Click the centre, then a point on the hole's edge.",
  poly: "Click each corner; click the first one, double-click or press Enter to close.",
  refine: "Click an edge or a point (on a 3D solid, a face): the elements there become a quarter of the size; click it again to take it off.",
};
const toolHint = () => { const t = tool(); if (t === "select" && solving()) return runLine() || (isFrame(model()?.analysis) ? "Solving…" : "Meshing and solving…"); if (t === "select" && results() && !stale()) return `Solved · ${results()!.stats.equations} equations · ${num(results()!.stats.ms, 2)} ms`; if (TOOL_HINT[t]) return TOOL_HINT[t]; if (t.startsWith("sup:")) return "Click where the support goes."; if (t.startsWith("load:")) return "Click where the load acts."; return "Ready"; };

const FrameNoMesh = () => <div class="legend"><div class="lrow"><span>Frames and trusses are not meshed: each member is one exact element.</span><button class="btn sm" onClick={() => setView("model")}>Back to the model</button></div></div>;

/** The side panels' widths (0.9.1): dragged, and kept for next time. */
const keep = (k: string, d: number) => { try { return +(localStorage.getItem(k) ?? d) || d; } catch { return d; } };
const [treeW, setTreeW] = createSignal(Math.min(480, Math.max(180, keep("dm.tree.w", 290))));
const [inspW, setInspW] = createSignal(Math.min(520, Math.max(220, keep("dm.insp.w", 270))));
const grip = (e: PointerEvent, which: "tree" | "insp") => {
  const x0 = e.clientX, w0 = which === "tree" ? treeW() : inspW(), el = e.currentTarget as HTMLElement; el.setPointerCapture(e.pointerId);
  const mv = (m: PointerEvent) => { const d = m.clientX - x0; if (which === "tree") setTreeW(Math.min(480, Math.max(180, w0 + d))); else setInspW(Math.min(520, Math.max(220, w0 - d))); };
  const up = () => { el.removeEventListener("pointermove", mv); el.removeEventListener("pointerup", up); try { localStorage.setItem(`dm.${which}.w`, String(which === "tree" ? treeW() : inspW())); } catch { /* private window */ } window.dispatchEvent(new Event("resize")); };
  el.addEventListener("pointermove", mv); el.addEventListener("pointerup", up);
};
/** Above the results tables and the report: the way back to the model (0.9.1: the side bar is gone). */
const PageBar = (p: { t: string }) => <div class="pagebar"><button class="btn sm" onClick={() => setView("model")}>← Back to the model</button><b>{p.t}</b><span class="grow" /><Show when={view() === "results"}><button class="btn sm" onClick={() => setView("report")}>Report</button></Show><Show when={view() === "report"}><button class="btn sm" onClick={() => setView("results")}>Tables</button></Show></div>;

const Status = () => {
  const ratio = () => { const r = 3779.5 / scale(); return r >= 1 ? `1:${num(r, 3)}` : `${num(1 / r, 3)}:1`; };
  return (
    <footer class="status">
      <span>{toolHint()}</span>
      <Show when={cursor()}>{c => <span class="mono muted">{onPlane() ? (() => { const p = toD(unproj(c()[0], c()[1])); return `X ${fmt("coord", p[0])} · Y ${fmt("coord", p[1])} · Z ${fmt("coord", p[2])}`; })() : `x ${fmt("coord", c()[0])} · y ${fmt("coord", c()[1])}`}</span>}</Show>
      <span class="muted">{model() ? `${ANALYSES.find(a => a[0] === model()!.analysis)?.[1]} · ${isFrame(model()!.analysis) ? `${model()!.nodes.length} nodes · ${model()!.members.length} members` : model()!.volume ? `${model()!.volume!.elements.toLocaleString()} tetrahedra` : model()!.parts?.length ? `${model()!.parts!.length} parts` : `${model()!.shapes.length} shapes${partRes() ? ` · ${partRes()!.mesh.tris.length.toLocaleString()} elements` : ""}`}` : ""}{sel().length ? ` · ${sel().length} selected` : ""}</span>
      <span class="grow" />
      <button onClick={C.zoomOut}>−</button><span class="muted zoomv" title="Drawing scale on a 96 dpi screen">{ratio()}</span><button onClick={C.zoomIn}>+</button>
    </footer>
  );
};

const App = () => {
  // Recovery saves every few minutes; a new interval in Options applies at once (0.1.1 review: it needed a restart).
  let timer = 0;
  createEffect(on(() => opts().autosave, every => { clearInterval(timer); if (every > 0) timer = window.setInterval(() => invoke("autosave").catch(() => {}), every * 60000); }));
  // Opening the Mesh view of a part that has no mesh yet meshes it.
  // A 3D mesh is shown in the 3D view instead (C.mesh, 0.9.1).
  createEffect(on(view, v => { const m = model(); if (v === "mesh" && m && !isFrame(m.analysis) && (m.volume || (!meshed() && !partRes()))) C.mesh(); }));
  // 0.7.2: the browser's own menu (Back, Refresh, Print…) never shows; text boxes keep theirs.
  document.addEventListener("contextmenu", e => { if (!(e.target as HTMLElement)?.closest?.("input, textarea")) e.preventDefault(); });
  document.addEventListener("keydown", e => { if (historyKey(e)) e.preventDefault(); });
  onMount(async () => {
    try {
      const o = await invoke<Options>("options"); setOpts(o); setThemeRaw(o.theme || "auto"); applyTheme();
      setOut(await invoke<Out>("state"));
      cadLoad();
      const start = await invoke<string | null>("startup_file");
      if (start?.toLowerCase().endsWith(".dlink")) import("./link").then(k => k.readLink(start));
      else if (start) { await loadOut("open", { path: start }); setTimeout(fit, 80); }
      const rec = await invoke<{ name: string; saved: string } | null>("recovery");
      if (rec) D.recoverDialog(rec.name, rec.saved, () => { setBackstage(null); setTimeout(fit, 60); });
      checkUpdate(true);
      listen("close-requested", () => D.confirmClose(() => invoke("quit")));
    } catch (e) { toast(String(e)); }
  });
  const keys = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    // A dialog left open under one that closed has lost the focus: Escape still closes it.
    if (dialogs().length) { if (e.key === "Escape") { e.preventDefault(); closeDialog(); } return; }
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
    if ((ctrl && k === "k") || (e.altKey && k === "q")) { e.preventDefault(); palette(); }
    else if (ctrl && k === "s") { e.preventDefault(); C.save(e.shiftKey); }
    else if (ctrl && k === "o") { e.preventDefault(); C.open(); }
    else if (ctrl && k === "n") { e.preventDefault(); setBackstage("new"); }
    else if (ctrl && k === "p") { e.preventDefault(); C.print(); }
    else if (ctrl && k === "z" && !typing) { e.preventDefault(); C.undo(); }
    else if (ctrl && k === "y" && !typing) { e.preventDefault(); C.redo(); }
    else if (e.key === "F1") { e.preventDefault(); D.shortcuts(); }
    else if (e.key === "F5") { e.preventDefault(); C.solve(); }
    else if (k === "f" && !ctrl && !typing && !backstage()) fit();
    else if (e.key === "Escape" && backstage() && out()) setBackstage(null);
    else if (e.key === "Escape" && !typing) setInspOpen(false);
  };
  return (
    <div class="app" ref={() => document.addEventListener("keydown", keys)}>
      <TitleBar update={update()?.available ? update()!.latest : null} />
      <Ribbon />
      <Show when={out()}><CtxBar /></Show>
      <main class="work">
        <div class="workrow">
        <Show when={out()}><Flow /></Show>
        <div class="content" style={{ "--tree-w": `${treeW()}px`, "--insp-w": `${inspW()}px` }}>
          <Show when={out()}>
            <Switch fallback={<div class="modelview"><Tree /><div class="vgrip" onPointerDown={e => grip(e, "tree")} title="Drag to widen or narrow the steps" /><ToolStrip /><Show when={partsOpen()}><Suspense><StdParts /></Suspense></Show><div class="center" onDragOver={e => { if (e.dataTransfer?.types.includes("application/x-datum-part")) { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; } }} onDrop={e => import("./stdparts").then(k => k.dropPart(e))}><Show when={!sketch()}><PlaneBar /></Show><Show when={!(sketch() && view() !== "mesh")} fallback={
                // Sketching (0.6.2): the 3D view behind, square to the plane, the sketch canvas over it.
                <div class="sketchstage"><Suspense fallback={<div class="canvas" />}><View3D /></Suspense><Show when={sketch()?.facing}><div class="sketchover"><Canvas /></div></Show><Suspense><SketchBar /></Suspense></div>}><Show when={in3d() && plane() === "3d" && view() !== "mesh"} fallback={<Canvas />}><Suspense fallback={<div class="canvas" />}><View3D /></Suspense></Show></Show><Show when={(rawResults() || (modeOn() && modes())) && !sketch() && view() !== "mesh"}><div class="playwrap"><Suspense><Player /></Suspense></div></Show><Show when={view() === "mesh"} fallback={<Show when={modeOn() && modes()} fallback={<><ResultsLegend /><PartLegend /><SpaceLegend /><SolidLegend /></>}><ModesBar /></Show>}><Show when={!isFrame(model()?.analysis)} fallback={<FrameNoMesh />}><MeshBar remesh={C.mesh} /></Show></Show></div><Show when={popShown()}><div class="ipop" classList={{ low: !!sketch() }}><div class="vgrip" onPointerDown={e => grip(e, "insp")} title="Drag to widen or narrow this panel" /><Inspector /></div></Show></div>}>
              <Match when={view() === "results"}><div class="pagewrap"><PageBar t="Results: tables and equilibrium" /><Suspense><ResultsPage /></Suspense></div></Match>
              <Match when={view() === "report"}><div class="pagewrap"><PageBar t="Report" /><Suspense><ReportPage /></Suspense></div></Match>
            </Switch>
          </Show>
        </div>
        </div>
        <Show when={out()}><Console /></Show>
      </main>
      <Status />
      <Show when={backstage()}><Backstage /></Show>
      <For each={dialogs()}>{d => d()}</For>
      <Menu /><Toast /><ModeAnimator />
    </div>
  );
};

// For the window checks: open a view directly where no button leads (a view with nothing to show).
(window as any).__setView = setView;
render(() => <App />, document.getElementById("app")!);
