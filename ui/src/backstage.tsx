// File (backstage): home with examples and recent models, export, print, options, roadmap, updates.
import { For, Show, createSignal, createResource, onMount, JSX, lazy, Suspense } from "solid-js";
// Printing loads on demand (0.5.5: the startup budget).
const PrintPage = lazy(() => import("./print").then(m => ({ default: m.PrintPage })));
const PAPER_NAMES = ["A4", "A3", "A2", "Letter", "Legal", "Tabloid"];
import { RoadmapPage } from "./roadmap";
import { Options, Out, Example, listen, invoke } from "./api";
import { backstage, setBackstage, call, out, setOut, model, opts, setOpts, toast, setThemeRaw, applyTheme, results } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import { Icon, Text, Pick, Check } from "./ui";
// File › New loads only when opened (0.5.3: it put the startup code over its budget).
const NewStudy = lazy(() => import("./newstudy").then(m => ({ default: m.NewStudy })));

type Info = { current: string; latest: string | null; beta: boolean; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string };
export const [update, setUpdate] = createSignal<Info | null>(null);
export const [checking, setChecking] = createSignal(false), [checkedAt, setCheckedAt] = createSignal("");
export const checkUpdate = async (quiet: boolean) => {
  setChecking(true);
  try { setUpdate(await invoke<Info>("check_update")); setCheckedAt(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })); if (!quiet) toast(update()?.available ? `Version ${update()!.latest} is available.` : "You have the latest version.", true); }
  catch (e) { if (!quiet) toast(`Could not check for updates: ${e}`); }
  finally { setChecking(false); }
};
const GH = "https://github.com/Edgar-Mendonca/Datum-Mesh";
export const GitHubLinks = () => (
  <div class="ghlinks">
    <For each={[["", "GitHub", "The project and its code"], ["/releases", "Releases", "Every version and its change log"], ["/issues/new", "Report an issue", "Tell us about a bug or an idea"]]}>{([path, l, t]) =>
      <button class="btn" title={t} onClick={() => call("open_url", { url: GH + path })}><svg class="ic"><use href={path === "/issues/new" ? "#i-bug" : path ? "#i-tag" : "#i-github"} /></svg>{l}</button>}</For>
  </div>
);
const md = (s: string) => s.split("\n").map(l => {
  const e = l.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`(.+?)`/g, "<code>$1</code>");
  return l.startsWith("## ") ? `<h3>${e.slice(3)}</h3>` : l.startsWith("- ") ? `<li>${e.slice(2)}</li>` : e.trim() ? `<p>${e}</p>` : "";
}).join("");

/** A small drawing for each example tile. */
const ExampleArt = (p: { id: string }) => {
  const s = { fill: "none", stroke: "var(--blue)", "stroke-width": 2, "stroke-linecap": "round" as const, "stroke-linejoin": "round" as const };
  const pics: Record<string, string> = {
    "blank-frame": "M20 50V22H80V50M20 22L50 10L80 22",
    "blank-stress": "M18 48V14H82V48z",
    "m-bracket": "M14 44L30 52H86L70 44zM14 44V36L30 44M30 52V44H86M70 44V36H14M62 40a6 3 0 1 0 0.1 0",
    "m-shaft": "M12 22H44V42H12zM44 26H88V38H44M12 22a3 10 0 0 0 0 20M88 26a2 6 0 0 1 0 12",
    "m-tubecrush": "M30 14a20 5 0 1 0 40 0a20 5 0 1 0 -40 0M30 14v12l6 6-6 6v12M70 14v12l-6 6 6 6v12M30 50a20 5 0 0 0 40 0M50 2v8M46 6l4 4 4-4",
    "m-tuning": "M10 18V46M10 28H86V34H10M86 22c4 0 4 4 0 4M86 36c4 0 4 4 0 4",
    portal: "M18 52V24L50 12L82 24V52M12 52h12M76 52h12",
    braced: "M14 54V10M50 54V10M86 54V10M14 32H86M14 10H86M14 54L50 32M8 54h12M44 54h12M80 54h12",
    quake: "M30 56V8M70 56V8M30 24H70M30 40H70M30 8H70M24 56h12M64 56h12M8 30q4-6 8 0t8 0",
    xbrace: "M30 56V8M70 56V8M30 32H70M30 8H70M30 56L70 32M70 56L30 32M30 32L70 8M70 32L30 8",
    building: "M20 54V14M50 58V18M80 54V14M20 14L50 18L80 14M20 28L50 32L80 28M20 41L50 45L80 41M20 14L40 8L70 6L80 14M40 8V48M70 6V46",
    slab: "M14 28L46 18L88 26L56 38zM18 29V54M46 19V40M84 27V50M56 38V58",
    lee: "M28 54V14H82M24 58L28 54L32 58zM82 14L86 18H78zM40 2V11M37 8L40 11L43 8",
    truss: "M10 46H90L74 30H26zM26 30V46M42 30V46M58 30V46M74 30V46M26 30L42 46M74 30L58 46M42 30H58",
    lug: "M30 54V30A20 20 0 0 1 70 30V54zM42 30a8 8 0 1 0 16 0a8 8 0 1 0 -16 0",
    bracket: "M24 8H38V40Q38 46 44 46H84V56H24z",
    flange: "M28 30a22 22 0 1 0 44 0a22 22 0 1 0 -44 0M42 30a8 8 0 1 0 16 0a8 8 0 1 0 -16 0M47 14a3 3 0 1 0 6 0a3 3 0 1 0 -6 0M47 46a3 3 0 1 0 6 0a3 3 0 1 0 -6 0",
    fin: "M14 52H86V44H80V10H76V44H62V10H58V44H44V10H40V44H26V10H22V44H14z",
    vessel: "M50 54V14M58 54V24A20 20 0 0 0 38 4V8A16 16 0 0 1 54 24V54zM44 54H66",
    hotplate: "M14 18H86V44H14zM44 31a6 6 0 1 0 12 0a6 6 0 1 0 -12 0M8 14V48M92 22v4M92 30v4M92 38v4",
    "blank-space": "M30 48V22L50 12L70 22V48M30 22L50 32L70 22M50 32V58M30 48L50 58L70 48",
    platform: "M18 22L50 10L82 22L50 34zM18 22V46M50 34V58M82 22V46M50 10V26M18 46L50 34",
    "blank-solid": "M14 30H66L84 18H32zM14 30V50H66V30M66 50L84 38V18",
    yield3d: "M14 18L22 12H86L78 18zM14 18H78V48H14zM78 48L86 42V12M38 33a8 8 0 1 0 16 0a8 8 0 1 0 -16 0",
    rubber: "M10 14V52M10 22H40Q70 22 78 46L70 50Q62 30 40 30H10M74 48v8M71 53l3 3l3-3",
    elbowheat: "M14 50V30A22 22 0 0 1 36 8H58M28 50V30A8 8 0 0 1 36 22H58M14 50h14M58 8v14M40 40c-2-3 2-5 0-8M48 44c-2-3 2-5 0-8",
    elbow: "M14 50V30A22 22 0 0 1 36 8H58M28 50V30A8 8 0 0 1 36 22H58M14 50h14M58 8v14M8 54h26",
    bracket3d: "M20 10L28 6V44Q28 48 34 48H82L74 52H26Q20 52 20 46zM28 44L36 40M82 48V56L74 60H26Q20 60 20 54V46",
  };
  return <svg viewBox="0 0 100 60" class="exart"><path d={pics[p.id] ?? pics["blank-frame"]} {...s} /></svg>;
};

const Home = () => {
  const [ex] = createResource(() => call<Example[]>("examples_list"));
  const [recent] = createResource(() => call<{ path: string; name: string; modified: string }[]>("recent"));
  return (
    <div class="home">
      <div>
        <h1>Good {new Date().getHours() < 12 ? "morning" : new Date().getHours() < 18 ? "afternoon" : "evening"}</h1>
        <button class="startnew" onClick={() => setBackstage("new")}><Icon n="new" size={28} /><span><b>Start a new study</b><small>Choose what you are analysing (a truss, a frame, a part, a 3D solid or heat) and what you want to know: the model is set up for it.</small></span></button>
        <h2>Or start from a blank model</h2>
        <div class="tiles">
          <For each={(ex() ?? []).filter(t => (t.group ?? "Blank") === "Blank")}>{t => <button class="tile" onClick={() => C.newModel(t.id)}><div class="pic"><ExampleArt id={t.id} /></div><b>{t.name}</b><span>{t.what}</span></button>}</For>
          <button class="tile" onClick={() => setBackstage("examples")}><div class="pic"><svg class="ic"><use href="#i-folder" /></svg></div><b>Examples</b><span>{(ex() ?? []).filter(t => t.group && t.group !== "Blank").length} worked examples, by kind: frames, 3D frames, parts, 3D solids and assemblies, nonlinear, dynamics.</span></button>
        </div>
        <h2>Recent</h2>
        <table class="recent"><tbody>
          <For each={recent() ?? []} fallback={<tr><td class="muted">Models you open or save appear here.</td></tr>}>{r => <tr class="f" onClick={() => C.openPath(r.path)}><td><b>{r.name}</b><div class="p">{r.path}</div></td><td class="p">{r.modified}</td></tr>}</For>
        </tbody></table>
      </div>
      <UpdateCard />
    </div>
  );
};

/** File › Examples (0.6.1): the worked examples under headings, each group from simple to harder. */
const ExamplesPage = () => {
  const [ex] = createResource(() => call<Example[]>("examples_list"));
  const groups = () => { const out: [string, Example[]][] = []; for (const t of ex() ?? []) { const g = t.group ?? "Blank"; if (g === "Blank") continue; let e = out.find(x => x[0] === g); if (!e) { e = [g, []]; out.push(e); } e[1].push(t); } return out; };
  return (
    <div class="home exhome">
      <div>
        <h1>Examples</h1>
        <p class="muted">Each opens as a new model, ready to solve (F5). Its notes (Inspector, nothing selected) say what it shows.</p>
        <For each={groups()}>{([g, list]) => <>
          <h2>{g}</h2>
          <div class="tiles"><For each={list}>{t => <button class="tile" onClick={() => C.newModel(t.id)}><div class="pic"><ExampleArt id={t.id} /></div><b>{t.name}</b><span>{t.what}</span></button>}</For></div>
        </>}</For>
      </div>
    </div>
  );
};

const UpdateCard = () => {
  const [pct, setPct] = createSignal<number | null>(null);
  const [ver] = createResource(() => call<string>("version"));
  onMount(() => { listen("update-progress", (p: number) => setPct(p)); if (!update()) checkUpdate(true); });
  const install = async () => { const u = update(); if (!u?.url) return; setPct(0); await call("install_update", { url: u.url, sums: u.sums }); setPct(null); };
  return (
    <div class="card">
      <div class="brand"><img src="logo.svg" alt="" /><div><b>Datum Mesh</b><span class="muted">Version {ver()}{opts().beta ? " · beta channel" : ""}</span></div></div>
      <Show when={update()} fallback={<><span class="pill">{checking() ? "Checking for updates…" : "Not checked yet"}</span><button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check for updates"}</button><GitHubLinks /></>}>{u => <>
        <Show when={u().available} fallback={<span class="pill ok">You have the latest version</span>}><span class="pill warn">Version {u().latest}{u().beta ? " (beta)" : ""} is ready</span></Show>
        <Show when={u().available && u().url}><button class="btn pri wide" disabled={pct() !== null} onClick={install}>{pct() === null ? `Update now (${Math.round(u().size / 1048576)} MB)` : `Downloading… ${pct()}%`}</button></Show>
        <Show when={pct() !== null}><div class="prog"><i style={{ width: `${pct()}%` }} /></div></Show>
        <button class="btn wide" disabled={checking()} onClick={() => checkUpdate(false)}>{checking() ? <><i class="spin" /> Checking…</> : "Check again"}</button>
        <Show when={checkedAt() && !checking()}><span class="muted small">Checked at {checkedAt()}</span></Show>
        <GitHubLinks />
        <div class="notes"><h4>What's new{u().latest ? ` in ${u().latest}` : ""}</h4><div innerHTML={md(u().notes || "Release notes will appear here.")} /></div>
      </>}</Show>
    </div>
  );
};

const Export = () => (
  <div><h1>Export</h1>
    <div class="tiles">
      <button class="tile" onClick={() => setBackstage("print")}><div class="pic"><svg class="ic"><use href="#i-print" /></svg></div><b>Print or save as PDF</b><span>The model drawn to scale, with its materials, sections, supports and loads.</span></button>
      <button class="tile" disabled={!model()?.shapes.length} onClick={() => import("./sketch").then(k => k.exportDxf())}><div class="pic"><svg class="ic"><use href="#i-export" /></svg></div><b>Section as DXF</b><span>The part's outline and holes as an ASCII DXF in millimetres, for CAD.</span></button>
      <button class="tile" disabled={!results()} onClick={() => import("./link").then(k => { const t = k.plotTables(); if (t.length && t[0] !== "-") t[0].run?.(); else toast("Solve with a nonlinear step, a time history or natural frequencies first: those give curves and lists to plot."); })}><div class="pic"><svg class="ic"><use href="#i-csv" /></svg></div><b>Send to Datum Plot</b><span>A load–deflection curve, a time history or the natural frequencies, as a table for graphs.</span></button>
      <button class="tile" disabled={!results()} onClick={() => import("./link").then(k => k.sendCalc())}><div class="pic"><svg class="ic"><use href="#i-calc" /></svg></div><b>Send to Datum Calc</b><span>The largest stress, movement, safety factor or frequency, as values with units for a hand check.</span></button>
    </div>
  </div>
);

const Opt = (p: { l: string; hint?: string; children: JSX.Element }) => <div class="opt"><div><b>{p.l}</b>{p.hint && <small>{p.hint}</small>}</div><div class="ctl">{p.children}</div></div>;
const Sec = (p: { t: string; about: string; children: JSX.Element }) => <section class="osec"><h3>{p.t}</h3><p class="muted">{p.about}</p>{p.children}</section>;

const OptionsPage = () => {
  const [o, setO] = createSignal<Options>({ ...opts() });
  const save = async () => { const r = await call<Out>("set_options", { opts: o() }); if (r) { setOpts(o()); setOut(r); toast("Options saved.", true); } };
  const put = <K extends keyof Options>(k: K, v: Options[K]) => setO({ ...o(), [k]: v });
  return (
    <div class="options"><h1>Options</h1>
      <p class="muted">Settings for Datum Mesh on this computer. Defaults for new models apply when a model is created; the settings of an open model are in the Inspector when nothing is selected.</p>
      <Sec t="General" about="How the window looks.">
        <Opt l="Theme" hint="Auto follows the Windows light or dark setting."><Pick value={o().theme || "auto"} set={v => { put("theme", v); setThemeRaw(v); applyTheme(); }} options={[["auto", "Auto (follow Windows)"], ["light", "Light"], ["dark", "Dark"]]} /></Opt>
        <Opt l="Canvas" hint="The grid, snapping to it, and the rulers along the edges."><span class="inline"><Check label="Show the grid" value={o().grid} set={v => put("grid", v)} /><Check label="Snap to the grid" value={o().snap} set={v => put("snap", v)} /><Check label="Rulers" value={o().rulers} set={v => put("rulers", v)} /></span></Opt>
        <Opt l="Contour colours" hint="Blue (low) to red (high), or viridis, which people with colour-blindness read as well as anyone."><Pick value={o().contours} set={v => put("contours", v as any)} options={[["rainbow", "Blue to red"], ["viridis", "Viridis (colour-blind safe)"]]} /></Opt>
      </Sec>
      <Sec t="Start-up and saving" about="What happens when Datum Mesh starts, and how your work is protected.">
        <Opt l="When Datum Mesh starts" hint="Show the File page, or reopen the model you used last."><Pick value={o().openLast ? "last" : "home"} set={v => put("openLast", v === "last")} options={[["home", "Show the File page"], ["last", "Reopen the last model"]]} /></Opt>
        <Opt l="Recovery copy every" hint="A copy is kept while you work, so a crash or power cut loses little. 0 turns it off."><span class="inline"><input type="number" min="0" style={{ width: "70px" }} value={o().autosave} onChange={e => put("autosave", +e.currentTarget.value)} /> minutes</span></Opt>
      </Sec>
      <Sec t="New models" about="Defaults given to every new model. Each model keeps its own settings afterwards.">
        <Opt l="Units" hint="How values are shown and typed. Files always keep SI units."><Pick value={o().units} set={v => put("units", v as any)} options={[["SI", "SI: mm, kN, MPa, °C"], ["US", "US: in, kip, ksi, °F"]]} /></Opt>
      </Sec>
      <Sec t="Printing" about="Defaults for File › Print.">
        <Opt l="Paper" hint="Default paper for printouts."><span class="inline"><Pick value={o().paper || "A4"} set={v => put("paper", v)} options={PAPER_NAMES} /><Pick value={o().landscape === false ? "0" : "1"} set={v => put("landscape", v === "1")} options={[["1", "Landscape"], ["0", "Portrait"]]} /></span></Opt>
        <Opt l="Company" hint="Shown in the header of printouts."><Text value={o().company} set={v => put("company", v)} /></Opt>
      </Sec>
      <Sec t="Updates" about="Datum Mesh checks GitHub for new versions when it starts. Updates install from File › Updates.">
        <Opt l="Beta versions" hint="Get new features early. They may have bugs."><Check label="Get beta versions" value={o().beta} set={v => put("beta", v)} /></Opt>
      </Sec>
      <p class="osave"><button class="btn pri" onClick={save}>Save options</button><span class="muted">Datum Mesh keeps everything on this computer. Its only connection is the update check to GitHub.</span></p>
    </div>
  );
};

export const Backstage = () => (
  <div class="bs">
    <aside>
      <button class="back" onClick={() => out() && setBackstage(null)} title="Back">←</button>
      <For each={[["home", "home", "Home"], ["new", "new", "New"], ["examples", "folder", "Examples"], ["open", "open", "Open"], ["save", "save", "Save"], ["saveas", "save", "Save As"], ["export", "export", "Export"], ["print", "print", "Print"], ["roadmap", "roadmap", "Roadmap"]]}>{([k, i, l]) =>
        <button classList={{ on: backstage() === k }} onClick={() => {
          if (k === "open") C.open(); else if (k === "save") C.save(); else if (k === "saveas") C.save(true);
          else setBackstage(k);
        }}><Icon n={i} size={22} />{l}</button>}</For>
      <span class="grow" />
      <button classList={{ on: backstage() === "options" }} onClick={() => setBackstage("options")}><Icon n="opts" size={22} />Options</button>
      <button classList={{ on: backstage() === "about" }} onClick={() => setBackstage("about")}><Icon n="upd" size={22} />Updates</button>
    </aside>
    <section class="bsmain">
      <Show when={backstage() === "home"}><Home /></Show>
      <Show when={backstage() === "new"}><Suspense><NewStudy /></Suspense></Show>
      <Show when={backstage() === "examples"}><ExamplesPage /></Show>
      <Show when={backstage() === "export"}><Export /></Show>
      <Show when={backstage() === "options"}><OptionsPage /></Show>
      <Show when={backstage() === "print"}><Suspense><PrintPage /></Suspense></Show>
      <Show when={backstage() === "roadmap"}><RoadmapPage /></Show>
      <Show when={backstage() === "about"}><div class="about"><h1>Updates</h1><UpdateCard /><p><button class="btn" onClick={() => D.shortcuts()}>Keyboard shortcuts</button></p></div></Show>
    </section>
  </div>
);
