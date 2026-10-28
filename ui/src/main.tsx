// Datum Mesh window: layout, sidebar, status bar, keyboard shortcuts, startup and recovery.
import { render } from "solid-js/web";
import { For, Show, Switch, Match, onMount } from "solid-js";
import "@fontsource/ibm-plex-sans/latin-400.css";
import "@fontsource/ibm-plex-sans/latin-500.css";
import "@fontsource/ibm-plex-sans/latin-600.css";
import "@fontsource/ibm-plex-sans/latin-700.css";
import "@fontsource/ibm-plex-mono/latin-400.css";
import "@fontsource/ibm-plex-mono/latin-500.css";
import "@fontsource/ibm-plex-mono/latin-600.css";
import "./styles.css";
import { invoke, listen, Out, Options, num } from "./api";
import { out, setOut, view, setView, View, backstage, setBackstage, setOpts, opts, dialogs, toast, loadOut, setThemeRaw, applyTheme, sel, cursor, tool, model } from "./store";
import { C } from "./cmds";
import { TitleBar, Ribbon, palette } from "./ribbon";
import { Backstage, update, checkUpdate } from "./backstage";
import { Canvas, fit, scale } from "./canvas";
import { Tree, Inspector, ANALYSES } from "./panels";
import { Menu, Toast, Icon } from "./ui";
import { fmt } from "./units";
import * as D from "./dialogs";

const SIDE: ([View, string, string] | "-")[] = [["model", "model", "Model"], ["mesh", "mesh", "Mesh"], "-", ["results", "contour", "Results"], ["report", "report", "Report"]];

const TOOL_HINT: Record<string, string> = {
  node: "Click to place a node.", beam: "Click the start, then the end of each beam; Esc stops.", truss: "Click the start, then the end of each bar; Esc stops.",
  rect: "Click two opposite corners.", hrect: "Click two opposite corners of the hole.", circle: "Click the centre, then a point on the circle.", hcircle: "Click the centre, then a point on the hole's edge.",
  poly: "Click each corner; click the first one, double-click or press Enter to close.",
};
const toolHint = () => { const t = tool(); if (TOOL_HINT[t]) return TOOL_HINT[t]; if (t.startsWith("sup:")) return "Click where the support goes."; if (t.startsWith("load:")) return "Click where the load acts."; return "Ready"; };

const Planned = (p: { t: string; v: string; what: string }) => (
  <div class="page"><h1>{p.t}</h1><div class="emptypage"><Icon n="list" size={34} /><b>{p.t} arrives in {p.v}</b><p class="muted">{p.what}</p>
    <p><button class="btn" onClick={() => setBackstage("roadmap")}>See the roadmap</button> <button class="btn" onClick={() => setView("model")}>Back to the model</button></p></div></div>
);

const Status = () => {
  const ratio = () => { const r = 3779.5 / scale(); return r >= 1 ? `1:${num(r, 3)}` : `${num(1 / r, 3)}:1`; };
  return (
    <footer class="status">
      <span>{toolHint()}</span>
      <Show when={cursor()}>{c => <span class="mono muted">x {fmt("coord", c()[0])} · y {fmt("coord", c()[1])}</span>}</Show>
      <span class="muted">{model() ? `${ANALYSES.find(a => a[0] === model()!.analysis)?.[1]} · ${model()!.analysis === "frame" ? `${model()!.nodes.length} nodes · ${model()!.members.length} members` : `${model()!.shapes.length} shapes`}` : ""}{sel().length ? ` · ${sel().length} selected` : ""}</span>
      <span class="grow" />
      <span class="vsc"><For each={SIDE.filter(x => x !== "-") as [View, string, string][]}>{x => <button title={x[2]} onClick={() => setView(x[0])}><Icon n={x[1]} /></button>}</For></span>
      <button onClick={C.zoomOut}>−</button><span class="muted zoomv" title="Drawing scale on a 96 dpi screen">{ratio()}</span><button onClick={C.zoomIn}>+</button>
    </footer>
  );
};

const App = () => {
  onMount(async () => {
    try {
      const o = await invoke<Options>("options"); setOpts(o); setThemeRaw(o.theme || "auto"); applyTheme();
      setOut(await invoke<Out>("state"));
      const start = await invoke<string | null>("startup_file");
      if (start) { await loadOut("open", { path: start }); setTimeout(fit, 80); }
      const rec = await invoke<{ name: string; saved: string } | null>("recovery");
      if (rec) D.recoverDialog(rec.name, rec.saved, () => { setBackstage(null); setTimeout(fit, 60); });
      checkUpdate(true);
      setInterval(() => { if (opts().autosave > 0) invoke("autosave").catch(() => {}); }, Math.max(1, opts().autosave) * 60000);
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
            <Switch fallback={<div class="modelview"><Tree /><div class="center"><Canvas /></div><Inspector /></div>}>
              <Match when={view() === "mesh"}><Planned t="Mesh" v="0.1.2" what="The part split into triangles, with the number of elements and nodes and the quality of every triangle (smallest angle, aspect ratio)." /></Match>
              <Match when={view() === "results"}><Planned t="Results" v={model()?.analysis === "frame" ? "0.1.1" : "0.1.2"} what="Deflected shapes, moment, shear and axial diagrams, stress and temperature contours with a legend, a probe, reactions and the equilibrium check." /></Match>
              <Match when={view() === "report"}><Planned t="Report" v="0.1.3" what="A printable report: the model, the mesh, materials, loads and supports, result pictures with legends, tables of maxima and reactions, and the equilibrium check." /></Match>
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
