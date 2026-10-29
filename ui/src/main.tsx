// Datum Mesh window: layout, sidebar, status bar, keyboard shortcuts, startup and recovery.
import { render } from "solid-js/web";
import { For, Show, Switch, Match, onMount, createEffect, on } from "solid-js";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "@fontsource/ibm-plex-sans/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
import "./styles.css";
import { invoke, listen, Out, Options, num } from "./api";
import { out, setOut, view, setView, View, backstage, setBackstage, setOpts, opts, dialogs, toast, loadOut, setThemeRaw, applyTheme, sel, cursor, tool, model, results, stale, solving, partRes, meshed } from "./store";
import { C } from "./cmds";
import { TitleBar, Ribbon, palette } from "./ribbon";
import { Backstage, update, checkUpdate } from "./backstage";
import { Canvas, fit, scale } from "./canvas";
import { Tree, Inspector, ANALYSES } from "./panels";
import { ResultsPage, ResultsLegend } from "./results";
import { PartLegend, MeshBar } from "./parts";
import { ReportPage } from "./print";
import { Menu, Toast, Icon } from "./ui";
import { fmt } from "./units";
import * as D from "./dialogs";

const SIDE: ([View, string, string] | "-")[] = [["model", "model", "Model"], ["mesh", "mesh", "Mesh"], "-", ["results", "contour", "Results"], ["report", "report", "Report"]];

const TOOL_HINT: Record<string, string> = {
  node: "Click to place a node.", beam: "Click the start, then the end of each beam; Esc stops.", truss: "Click the start, then the end of each bar; Esc stops.",
  rect: "Click two opposite corners.", hrect: "Click two opposite corners of the hole.", circle: "Click the centre, then a point on the circle.", hcircle: "Click the centre, then a point on the hole's edge.",
  poly: "Click each corner; click the first one, double-click or press Enter to close.",
  refine: "Click an edge or a point: the elements there become a quarter of the size.",
};
const toolHint = () => { const t = tool(); if (t === "select" && solving()) return model()?.analysis === "frame" ? "Solving…" : "Meshing and solving…"; if (t === "select" && results() && !stale()) return `Solved · ${results()!.stats.equations} equations · ${num(results()!.stats.ms, 2)} ms`; if (TOOL_HINT[t]) return TOOL_HINT[t]; if (t.startsWith("sup:")) return "Click where the support goes."; if (t.startsWith("load:")) return "Click where the load acts."; return "Ready"; };

const FrameNoMesh = () => <div class="legend"><div class="lrow"><span>Frames and trusses are not meshed: each member is one exact element.</span><button class="btn sm" onClick={() => setView("model")}>Back to the model</button></div></div>;

const Status = () => {
  const ratio = () => { const r = 3779.5 / scale(); return r >= 1 ? `1:${num(r, 3)}` : `${num(1 / r, 3)}:1`; };
  return (
    <footer class="status">
      <span>{toolHint()}</span>
      <Show when={cursor()}>{c => <span class="mono muted">x {fmt("coord", c()[0])} · y {fmt("coord", c()[1])}</span>}</Show>
      <span class="muted">{model() ? `${ANALYSES.find(a => a[0] === model()!.analysis)?.[1]} · ${model()!.analysis === "frame" ? `${model()!.nodes.length} nodes · ${model()!.members.length} members` : `${model()!.shapes.length} shapes${partRes() ? ` · ${partRes()!.mesh.tris.length.toLocaleString()} elements` : ""}`}` : ""}{sel().length ? ` · ${sel().length} selected` : ""}</span>
      <span class="grow" />
      <span class="vsc"><For each={SIDE.filter(x => x !== "-") as [View, string, string][]}>{x => <button title={x[2]} onClick={() => setView(x[0])}><Icon n={x[1]} /></button>}</For></span>
      <button onClick={C.zoomOut}>−</button><span class="muted zoomv" title="Drawing scale on a 96 dpi screen">{ratio()}</span><button onClick={C.zoomIn}>+</button>
    </footer>
  );
};

const App = () => {
  // Recovery saves every few minutes; a new interval in Options applies at once (0.1.1 review: it needed a restart).
  let timer = 0;
  createEffect(on(() => opts().autosave, every => { clearInterval(timer); if (every > 0) timer = window.setInterval(() => invoke("autosave").catch(() => {}), every * 60000); }));
  // Opening the Mesh view of a part that has no mesh yet meshes it.
  createEffect(on(view, v => { const m = model(); if (v === "mesh" && m && m.analysis !== "frame" && !meshed() && !partRes()) C.mesh(); }));
  onMount(async () => {
    try {
      const o = await invoke<Options>("options"); setOpts(o); setThemeRaw(o.theme || "auto"); applyTheme();
      setOut(await invoke<Out>("state"));
      const start = await invoke<string | null>("startup_file");
      if (start) { await loadOut("open", { path: start }); setTimeout(fit, 80); }
      const rec = await invoke<{ name: string; saved: string } | null>("recovery");
      if (rec) D.recoverDialog(rec.name, rec.saved, () => { setBackstage(null); setTimeout(fit, 60); });
      checkUpdate(true);
      listen("close-requested", () => D.confirmClose(() => invoke("quit")));
    } catch (e) { toast(String(e)); }
  });
  const keys = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase();
    if (dialogs().length) return;
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
    if ((ctrl && k === "k") || (e.altKey && k === "q")) { e.preventDefault(); palette(); }
    else if (ctrl && k === "s") { e.preventDefault(); C.save(e.shiftKey); }
    else if (ctrl && k === "o") { e.preventDefault(); C.open(); }
    else if (ctrl && k === "n") { e.preventDefault(); setBackstage("home"); }
    else if (ctrl && k === "p") { e.preventDefault(); C.print(); }
    else if (ctrl && k === "z" && !typing) { e.preventDefault(); C.undo(); }
    else if (ctrl && k === "y" && !typing) { e.preventDefault(); C.redo(); }
    else if (e.key === "F1") { e.preventDefault(); D.shortcuts(); }
    else if (e.key === "F5") { e.preventDefault(); C.solve(); }
    else if (k === "f" && !ctrl && !typing && !backstage()) fit();
    else if (e.key === "Escape" && backstage() && out()) setBackstage(null);
  };
  return (
    <div class="app" ref={() => document.addEventListener("keydown", keys)}>
      <TitleBar update={update()?.available ? update()!.latest : null} />
      <Ribbon />
      <main class="work">
        <nav class="side"><For each={SIDE}>{x => x === "-" ? <hr /> : <button classList={{ on: view() === x[0] }} onClick={() => setView(x[0])} title={x[2]}><Icon n={x[1]} /><span>{x[2]}</span></button>}</For></nav>
        <div class="content">
          <Show when={out()}>
            <Switch fallback={<div class="modelview"><Tree /><div class="center"><Canvas /><Show when={view() === "mesh"} fallback={<><ResultsLegend /><PartLegend /></>}><Show when={model()?.analysis !== "frame"} fallback={<FrameNoMesh />}><MeshBar remesh={C.mesh} /></Show></Show></div><Inspector /></div>}>
              <Match when={view() === "results"}><ResultsPage /></Match>
              <Match when={view() === "report"}><ReportPage /></Match>
            </Switch>
          </Show>
        </div>
      </main>
      <Status />
      <Show when={backstage()}><Backstage /></Show>
      <For each={dialogs()}>{d => d()}</For>
      <Menu /><Toast />
    </div>
  );
};

render(() => <App />, document.getElementById("app")!);
